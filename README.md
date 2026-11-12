# Sonixx — Rust

Roteador de áudio virtual para Windows, escrito em **Rust puro** (engine e
interface). Captura o áudio do sistema e microfones, mixa e envia para um
dispositivo de saída — idealmente o **VB-Cable**, para aparecer como microfone
virtual no Discord, OBS, jogos e chamadas.

> O projeto foi migrado de Python para Rust. Não há mais `main.py`, `app/`,
> PyInstaller nem dependências Python no repositório.

## Estado atual

Implementado:

- Janela nativa com `egui`/`eframe` (sem webview), tema escuro.
- Enumeração real de dispositivos WASAPI (saída e entrada).
- Seleção de dispositivo de saída, fonte de loopback do sistema e microfones.
- **Detector em tempo real** de áudio saindo do PC: endpoint e cada sessão
  (`IAudioMeterInformation`), sem captura e sem driver.
- **Ícones reais** extraídos do executável de cada aplicativo (Shell do Windows),
  com filtro de sessões de microfone e de processos do sistema.
- **Sessões agrupadas por aplicativo**: duas sessões do Teams viram um cartão
  “2 sessões”; WebView2/crashpad são mapeados ao app pai em vez de aparecerem
  como aplicativos separados.
- **Volume e mute individuais por app** direto nos cards, via
  `ISimpleAudioVolume` do Windows — funciona mesmo com a engine parada.
  O botão “+ Mixagem” é separado e adiciona a fonte ao mixer do Sonixx.
- **UI visual e interativa**: sliders com trilha/preenchimento, gráfico de
  atividade em tempo real, ações rápidas “+ Mixagem”, botão do instalador
  VB-Cable e tiles do soundboard.
- **Abas do Chrome** via extensão opcional (`chrome-extension/`): títulos e
  detecção das abas audíveis, sem depender de driver.
- Captura por aplicativo via **process loopback WASAPI** nativo, sem DLL externa.
- Soundboard tocando arquivos **WAV, MP3, FLAC, OGG e AAC** pela mixagem.
- Engine Rust com captura WASAPI, mixagem, volume máster, mute e medidores.
- Saída WASAPI para o dispositivo escolhido (VB-Cable quando instalado).
- Persistência de configurações em `%APPDATA%\Sonixx\settings.json`.
- CLI de desenvolvimento `sonixx-dev` (worktrees + proteção da `master`).

Em desenvolvimento (próximas fases):

- Monitor mix, bandeja do sistema, atalhos globais e autostart.
- Instalação/renomeação do VB-Cable pela interface.
- Instalador e release portable.

> Em alguns ambientes corporativos o Windows recusa a inicialização de
> captura WASAPI (`E_INVALIDARG`) por política de segurança/driver. A captura
> por aplicativo abre normalmente; a validação de áudio depende dessa liberação.
> Detalhes e a matriz completa de testes estão em
> [`docs/AUDIO-CAPTURE.md`](docs/AUDIO-CAPTURE.md).
>
> **Chrome:** o Windows expõe o navegador como **uma única sessão de áudio**;
> abas individuais não são separadas pelo WASAPI. O Sonixx inclui uma extensão
> opcional que **detecta** as abas audíveis (`chrome-extension/`). Capturar o
> áudio de cada aba separadamente exigiria `chrome.tabCapture` + streaming PCM
> para o app — próxima fase.

## Abas do Chrome (opcional)

1. Abra `chrome://extensions`.
2. Ative o **Modo do desenvolvedor**.
3. Clique em **Carregar sem compactação** e escolha a pasta `chrome-extension/`.
4. Deixe o Sonixx aberto e use a aba **Chrome** do app.

A extensão só lê `chrome.tabs.query({ audible: true })` e envia título/URL para
`127.0.0.1:37645`. Ela não captura áudio nem envia dados para fora da máquina.

## Requisitos

- Windows 10/11 (x64).
- Toolchain Rust stable MSVC (`rustup`).
- **VB-Cable** para usar o Sonixx como microfone virtual. O pacote oficial
  `VBCABLE_Driver_Pack45.zip` está na raiz do repositório.
- Permissão de microfone habilitada em
  **Configurações → Privacidade → Microfone**.

## Rodar

```powershell
cargo run -p sonixx-app
```

Build release:

```powershell
cargo build -p sonixx-app --release
```

O executável fica em `target\release\sonixx.exe`.

## Estrutura

```
Cargo.toml                 # workspace
crates/
  sonixx-core/             # settings + engine WASAPI (devices, captura, mixer, render)
  sonixx-app/              # UI egui/eframe
tools/
  devtool/                 # CLI sonixx-dev (worktrees e proteção de branch)
assets/                    # logo e ícones
sounds/                    # arquivos do soundboard (usuário)
VBCABLE_Driver_Pack45.zip  # driver VB-Cable oficial
docs/
  RUST-PLAN.md             # plano e fases da migração
  SECURITY-SCAN.md         # verificação estática do projeto original
```

## Desenvolvimento

O fluxo obrigatório (worktrees, branches, validação, commits) está em
[`AGENTS.md`](AGENTS.md). Resumo:

```powershell
cargo run -p sonixx-dev -- worktree open feat/minha-tarefa
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo run -p sonixx-dev -- worktree close -m "feat(escopo): descrição"
```

## Licença

MIT. O VB-Cable é um driver de terceiros da VB-Audio; consulte os termos do
fabricante antes de redistribuir.
