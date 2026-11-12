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
