# Plano de migração — Sonixx 100% Rust

> Documento vivo. Toda decisão de arquitetura da reescrita deve ser registrada
> aqui antes de virar código.

## 1. Objetivo

Substituir a implementação Python (`main.py` + `app/*.py`) por um aplicativo
nativo em Rust, com **interface também em Rust** (sem webview), mantendo o
comportamento e a experiência da versão 2.0.0.

Critérios de aceite:

- [ ] Nenhum arquivo Python no produto final.
- [ ] Nenhuma DLL de captura de terceiros (`process-audio-capture` eliminado).
- [ ] UI nativa em Rust (`egui`/`eframe`), tema escuro, pt-BR.
- [ ] Captura por aplicativo, microfones, loopback do sistema e soundboard
      mixados para o VB-Cable.
- [ ] Medidores de nível, volume por fonte, mute, master 0–200%, monitor mix.
- [ ] Tray, atalhos globais, autostart, configurações persistidas.
- [ ] Instalação/detecção/renomeação do VB-Cable.
- [ ] `.exe` único/portable, sem Python instalado.

## 2. Inventário funcional atual

| Recurso | Onde está hoje | Observação para o Rust |
|---|---|---|
| Captura por app | `AppAudioSource` + DLL `process-audio-capture` via named pipe | Implementar loopback por processo com WASAPI nativo |
| Microfones | `MicSource` (PyAudio/WASAPI) | `IAudioClient`/`cpal` |
| Loopback do sistema | `AudioRouter` (WASAPI loopback) | `AUDIOCLIENT_ACTIVATION_PARAMS` com `LOOPBACK` |
| Mixer | callback `_out_cb` (numpy) | callback Rust com buffers pré-alocados |
| Jitter buffer | 10 chunks (~210 ms) | ring buffer com histerese |
| Monitor mix | stream extra para fones | segundo `IAudioRenderClient` |
| Soundboard | `SoundboardManager` | `symphonia` + mixer próprio |
| Master/mute/medidores | UI + router | `AtomicU32`/`AtomicBool` + snapshots |
| VB-Cable | `app/driver.py` | detecção MMDevice + instalação via zip oficial |
| Renomear dispositivo | registro `MMDevices` | `windows-registry` (com aviso de admin) |
| Autostart | `HKCU\...\Run` | `windows-registry` |
| Hotkeys | `keyboard` | `global-hotkey` |
| Tray | `pystray` | `tray-icon` + `muda` |
| UI | CustomTkinter | `egui`/`eframe` |
| Empacotamento | PyInstaller | `cargo build --release` + instalador |

## 3. Workspace proposto

```
Cargo.toml                  # workspace
crates/
  sonixx-core/              # biblioteca de áudio e regras de negócio
    src/
      wasapi/               # COM: device, session, process loopback
      mixer.rs              # soma, volume, mute, clipping, medidores
      ring.rs               # ring buffer lock-free
      resample.rs           # rubato
      soundboard.rs         # decode + vozes simultâneas
      settings.rs           # %APPDATA%\Sonixx\settings.json
  sonixx-app/               # binário da UI
    src/
      main.rs
      ui/                   # telas, cards, medidores, tema
      tray.rs
      hotkeys.rs
      elevation.rs
tools/
  devtool/                  # sonixx-dev (já implementado)
```

Dependências principais (revisar versões na implementação):

- `eframe`/`egui` — UI nativa.
- `windows` — WASAPI, COM, MMDevice, registry, ShellExecute.
- `cpal` — opcional para dispositivos simples; não cobre process loopback.
- `rubato` — reamostragem.
- `symphonia` — decodificação de MP3/WAV/FLAC/OGG.
- `sysinfo` — enumeração de processos.
- `global-hotkey` — atalhos globais.
- `tray-icon` + `muda` — bandeja e menu.
- `serde` + `serde_json` — configuração.
- `anyhow` + `thiserror` — erros.
- `tracing` + `tracing-subscriber` — logs.
- `image` — ícones/logo.

## 4. Decisões de arquitetura

### 4.1 Captura por processo (substitui a DLL)

No Windows 10 2004+ é possível capturar apenas o áudio de um processo com
`ActivateAudioInterfaceAsync` + `AUDIOCLIENT_ACTIVATION_PARAMS`:

- `ActivationType = AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`;
- `ProcessLoopbackParams.TargetProcessId = pid`;
- `ProcessLoopbackParams.ProcessLoopbackMode = PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE`;
- ativar com `IID_IAudioClient` e montar `IAudioCaptureClient`.

Vantagens: sem DLL de terceiros, sem named pipe, sem cópia extra, e o pacote
fica 100% Rust. Requisito: Windows 10 2004+ (o app pode avisar em versões
antigas).

### 4.2 Callback de áudio

O callback de render do VB-Cable é o relógio do mixer:

- nenhuma alocação/lock/IO no callback;
- cada fonte escreve num ring buffer SPSC;
- o callback consome, soma, aplica volume/mute, faz clipping e publica picos em
  atomics;
- fontes que atrasam são silenciadas por alguns blocos (jitter buffer);
- reamostragem fora do callback (thread por fonte) quando a taxa difere.

### 4.3 UI

`egui`/`eframe` com tema escuro customizado, cards por fonte, sliders 0–200%,
switch on/off, medidor vertical e botão de remover. A UI lê snapshots atômicos
do engine e envia comandos por canal; nunca bloqueia o áudio.

### 4.4 Elevação e driver

- O app inicia sem admin; só pede elevação ao instalar/renomear o VB-Cable.
- Detecção por MMDevice (nome contém `CABLE Input`/`CABLE Output`).
- Instalação: extrair o zip oficial para temp e executar
  `VBCABLE_Setup_x64.exe -i -h`, com timeout e checagem de assinatura.
- Renomear: propriedade `{a45c254e-df1c-4efd-8020-67d146a850e0},2` no registro,
  somente com confirmação do usuário.

## 5. Fases

### Fase 0 — Ferramentas (concluída)

- `sonixx-dev` em Rust: worktrees, hook de proteção, status, limpeza.
- `AGENTS.md` e este plano.
- Toolchain Rust MSVC instalado.

### Fase 1 — Fundação (concluída)

- Workspace + `sonixx-core` + `sonixx-app`.
- Janela egui com tema escuro e layout do app.
- Settings (`%APPDATA%\Sonixx\settings.json`).
- Enumeração real de dispositivos WASAPI e seleção na UI.
- **Python removido do repositório.**

### Fase 2 — Engine base (em andamento)

- [x] Estrutura da engine: threads, ring buffers, mixer, render WASAPI.
- [x] Volume máster, mute, medidores e settings.
- [x] Captura de microfone e loopback do sistema (código Rust).
- [ ] Validar captura em máquina com permissão de microfone liberada —
      em alguns ambientes corporativos o `IAudioClient::Initialize` de captura
      retorna `E_INVALIDARG` até para bibliotecas de referência; testar em
      máquina limpa e registrar o resultado.
- [ ] Monitor mix.
- [ ] Testes unitários do mixer/ring buffer.

### Fase 3 — Captura por processo (implementada)

- [x] Process loopback WASAPI (`ActivateAudioInterfaceAsync` +
      `AUDIOCLIENT_ACTIVATION_PARAMS`).
- [x] Listagem de sessões de áudio (PID + nome) na UI.
- [x] Mixagem da captura por aplicativo junto com sistema e microfones.
- [ ] Validação audível em máquina com captura WASAPI liberada.

### Fase 4 — Soundboard, tray e hotkeys (parcial)

- [x] Decodificação WAV/MP3/FLAC/OGG/AAC com `symphonia`.
- [x] Mixagem de múltiplas vozes no render, volume e mute do soundboard.
- [x] Lista de arquivos de `sounds/` e botões reais na UI.
- [ ] Hotkeys globais (mute master + soundboard).
- [ ] Tray com restaurar/sair; fechar para a bandeja.
- [ ] Autostart.

### Fase 5 — Driver e paridade

- Detecção/instalação/renomeação do VB-Cable.
- Migração de settings da versão Python.
- Paridade visual e funcional; remover Python do repositório.

### Fase 6 — Release

- `cargo build --release`.
- Portable `.exe` + instalador.
- README, changelog e licenças de terceiros.
- Teste em máquina limpa (sem Python/Rust).

## 6. Riscos e mitigação

| Risco | Mitigação |
|---|---|
| Process loopback exige Win10 2004+ | Detectar versão e avisar; fallback para loopback do sistema |
| Taxas de amostra diferentes | Reamostrar por fonte com `rubato`; medir latência |
| Callback com lock/alocação | Revisão + testes de tempo real; ring buffers |
| VB-Cable exige admin/reboot | Detectar antes; instalar só sob ação do usuário |
| UI egui “parecer ferramenta” | Tema customizado, fontes, espaçamento e assets |
| Regressão de recursos | Checklist de paridade por fase; testes manuais guiados |
| Licença do VB-Cable | Manter o pacote oficial; não modificar o driver |

## 7. Definition of Done (por tarefa)

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets`
- `cargo build --release`
- Teste manual do recurso no Windows.
- `AGENTS.md`/este plano atualizados se a arquitetura mudar.
