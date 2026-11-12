# AGENTS.md — Sonixx (Rust)

> **Contrato operacional para qualquer IA ou desenvolvedor neste repositório.**
> Leia antes de qualquer ação. As regras valem para todas as IAs e para todas as
> worktrees do clone.

---

## 1. Missão

Transformar o **Sonixx** em um aplicativo **100% Rust** para Windows 10/11 —
engine de áudio e interface — sem Python em runtime, sem webview e sem DLL de
captura de terceiros.

O Python foi removido do repositório: não há mais `main.py`, `app/`,
`requirements.txt`, `uv.lock` nem PyInstaller. Toda implementação nova nasce em
Rust, dentro de uma worktree. A paridade funcional é acompanhada em
[`docs/RUST-PLAN.md`](docs/RUST-PLAN.md).

---

## 2. Fluxo de worktrees (obrigatório)

A cópia principal fica **permanentemente na `master`**, limpa. Toda tarefa —
por menor que seja — nasce numa **worktree isolada** em `.worktrees/`, com a sua
própria branch local. É isso que permite várias IAs no mesmo repositório sem uma
atropelar a outra.

O hook `pre-commit` instalado por `sonixx-dev init` **bloqueia commit direto na
`master` (e na `main`)** em todo o clone, inclusive dentro das worktrees.

### O ciclo

1. **Abrir** (na cópia principal, na `master` limpa):
   ```powershell
   cargo run -p sonixx-dev -- worktree open feat/minha-tarefa
   ```
   Cria `.worktrees/feat-minha-tarefa` na branch `feat/minha-tarefa`, baseada em
   `origin/master` recém-baixada.

2. **Trabalhar** dentro da worktree: edições, builds e commits acontecem lá,
   nunca na cópia principal.

3. **Validar** (obrigatório antes de fechar):
   ```powershell
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all-targets
   ```

4. **Fechar** (de dentro da worktree):
   ```powershell
   cargo run -p sonixx-dev -- worktree close -m "feat(escopo): descrição"
   ```
   O comando commita o que faltar, roda a validação, integra `origin/master`,
   publica os commits em `origin/master` e agenda a limpeza automática da
   worktree e da branch local. Se houver conflito, ele **para**, lista os
   arquivos e deixa o merge em andamento para você resolver e rodar de novo.

5. **PR — ação humana.** A IA **nunca** abre, aprova ou mergeia Pull Request.
   Para imprimir a URL de comparação:
   ```powershell
   cargo run -p sonixx-dev -- worktree pr
   ```

6. **Limpeza.** O `close` agenda sozinho. Manual:
   ```powershell
   cargo run -p sonixx-dev -- worktree clean feat/minha-tarefa --delete-branch
   ```

### Comandos úteis

```powershell
cargo run -p sonixx-dev -- init                 # instala/atualiza o hook de proteção
cargo run -p sonixx-dev -- worktree status      # worktrees, origin vs upstream, limpeza pendente
cargo run -p sonixx-dev -- worktree list
cargo run -p sonixx-dev -- worktree open fix/outro-bug
cargo run -p sonixx-dev -- worktree close -m "fix(escopo): correção"
```

### Limites

- Proibido commitar na **cópia principal** (o hook bloqueia).
- Proibido push direto no **upstream** (`Felix-au/...`); o push vai para
  `origin` (o seu fork).
- Proibido deixar worktree órfã: use `close` ou `clean`.
- Proibido abrir/aprovar/mergear PR pela IA.
- Cada solicitação nova — ou correção seguinte — nasce numa **worktree nova**;
  não se reaproveita a anterior.

---

## 3. Arquitetura Rust (destino)

```
Sonixx/
├── Cargo.toml                 # workspace
├── crates/
│   ├── sonixx-core/           # engine: WASAPI, captura por processo, mixer, soundboard
│   └── sonixx-app/            # UI egui/eframe, tray, hotkeys, settings
├── tools/
│   └── devtool/               # CLI sonixx-dev (worktrees e proteção de branch)
├── assets/                    # logo, ícones
├── VBCABLE_Driver_Pack45.zip  # driver VB-Cable (assinado)
└── docs/
```

Regras de arquitetura:

- **UI 100% Rust** com `egui`/`eframe` (sem Tauri, sem webview).
- **Áudio** com o crate `windows` (WASAPI/COM). `cpal` pode ser usado para
  dispositivos simples; a captura por processo é WASAPI nativa
  (`ActivateAudioInterfaceAsync` + `AUDIOCLIENT_ACTIVATION_PARAMS`).
- **Nada de `process-audio-capture`**: o loopback por processo é implementado
  em Rust, sem DLL externa.
- **Extensão opcional do Chrome** (`chrome-extension/`): só detecta abas
  audíveis via `chrome.tabs`; o app continua Rust. Não captura áudio.
- Reamostragem com `rubato`; decodificação de áudio do soundboard com
  `symphonia`.
- Processos com `sysinfo`; hotkeys com `global-hotkey`; tray com `tray-icon`.
- Configuração em `%APPDATA%\Sonixx\settings.json` (`serde` + `serde_json`).
- Erros com `anyhow`/`thiserror`; logging com `tracing`.
- **Sem downloads em runtime** e **sem telemetria**.

O plano completo, com fases e critérios de aceite, está em
[`docs/RUST-PLAN.md`](docs/RUST-PLAN.md).

---

## 4. Regras de código

- Rust stable (MSVC). `cargo fmt` e `clippy -D warnings` sempre limpos.
- Sem `unwrap()`/`expect()` em caminho de produção (use `?` + contexto).
- Callback de áudio em tempo real: **sem alocação, lock ou I/O**; use ring
  buffers e canais sem bloqueio.
- UI e mensagens visíveis em **português do Brasil (pt-BR)**.
- Nomes de código, campos de settings e APIs em inglês.
- UTF-8 **sem BOM** em todos os arquivos.
- Commits no padrão **Conventional Commits**:
  `feat(escopo): ...`, `fix(escopo): ...`, `docs: ...`, `refactor(escopo): ...`.

---

## 5. Segurança

- Nunca commitar `.env`, tokens, senhas ou credenciais.
- Elevação de administrador só quando indispensável (instalar driver, renomear
  dispositivo). O app deve funcionar sem admin para o uso normal.
- O instalador do VB-Cable é um binário de terceiro: manter o pacote oficial,
  conferir hash/assinatura e nunca baixar executável em runtime.
- Não adicionar dependência sem necessidade; revisar crates novos.
- Não executar código remoto, `curl | sh`, `iex` ou similares.

---

## 6. Validação e release

Antes de fechar qualquer tarefa:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

O release final será um `.exe` nativo (portable primeiro; instalador depois),
sem Python e sem runtime externo.
