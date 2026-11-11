# Verificação de segurança — Sonixx

**Data:** 2026-11-11  
**Escopo:** árvore de trabalho e histórico do repositório
`A:\Projetos\Sonixx-Audio-Routing-and-Soundbaord` (commit `9f94e8f`).

## Resultado

**Nenhum indício de malware foi encontrado na análise estática.** Não foi
possível concluir uma varredura antimalware sob demanda por causa de políticas
da máquina (detalhes em “Limitações”).

## O que foi verificado

1. **Inventário completo:** 61 arquivos, 4,87 MB. Nenhum `.exe`, `.dll`, `.sys`
   ou script suspeito fora do pacote oficial do VB-Cable.
2. **Análise estática dos fontes:** busca por downloads, execução remota,
   `base64`, `eval/exec`, persistência, keylogging, injeção e comandos
   ofuscados. Nenhum achado relevante. As únicas chamadas sensíveis são
   legítimas:
   - `ShellExecuteW("runas")` para elevar o app a administrador;
   - `winreg` para autostart e renomear o dispositivo VB-Cable;
   - `subprocess.run` para executar o instalador oficial do VB-Cable;
   - `webbrowser.open` para abrir o GitHub do autor.
3. **Histórico do Git:** os únicos binários/scripts já versionados são
   `run.bat` e `VBCABLE_Driver_Pack45.zip`. Não há executáveis antigos
   escondidos no histórico.
4. **Pacote do driver VB-Cable:**
   - SHA-256: `B950E39F01AF1D04EA623C8F6D8EB9B6EA5C477C637295FABF20631C85116BFB`
   - Todos os `.exe`, `.sys` e `.cat` verificados com assinatura **válida**:
     - `VBCABLE_Setup.exe`, `VBCABLE_Setup_x64.exe`, `VBCABLE_ControlPanel.exe`
       e os drivers: **BUREL VINCENT / VB-Audio**;
     - `vbaudio_cable64_win10.cat`: **Microsoft Windows Hardware Compatibility
       Publisher**.
   - O pacote contém apenas o instalador, drivers, INF, ícones e readme do
     VB-Cable. Nada foi executado durante a verificação.

## Hashes SHA-256 dos arquivos principais

| Arquivo | SHA-256 |
|---|---|
| `VBCABLE_Driver_Pack45.zip` | `B950E39F01AF1D04EA623C8F6D8EB9B6EA5C477C637295FABF20631C85116BFB` |
| `run.bat` | `02D63AFF009E05F2A8CC924DCDCFFB52A52F50B4BB9EA66DAAE84360DF7A251D` |
| `main.py` | `12FBE08D47F9B8D0747B059F7778B771C2B4CB79ADA711BC2B2F33D25E503182` |
| `app/audio_router.py` | `23F0D7CEB0E4C4E786B04E2B46C35D5101B8FA40207030A20E612BA969CFA458` |
| `app/driver.py` | `4C2C92CDCD147B3EA4AC1735A32EE0A3CD160F6DA6634F00D638F24736E70D8F` |
| `app/soundboard.py` | `D9BE2FF6F72C6C8B38F93963FA6AC4EA322E6CFC39124C8F61BD4664ECD79506` |
| `app/ui.py` | `3E2B53BA15B44B0115F757BD34B18B442000BE12AAE38FA2F9B081FB08531F3F` |

## Limitações

- **Kaspersky Endpoint Security** está ativo, mas a política corporativa bloqueia
  o controle local de tarefas:
  `Error: Local task control is denied by policy`. O scan via `avp.com` não pôde
  ser iniciado.
- **Microsoft Defender** está desativado por política
  (`HKLM\SOFTWARE\Microsoft\Windows Defender\DisableAntiSpyware = 1`) e o
  `MpCmdRun.exe` recusa o scan (“Product/Feature disabled”).
- Portanto, a conclusão acima é de **análise estática + verificação de
  assinaturas**, não de uma varredura antimalware completa.

## Recomendação

Para confirmação definitiva, execute uma varredura personalizada pela interface
do Kaspersky (pasta `A:\Projetos\Sonixx-Audio-Routing-and-Soundbaord`). Antes de
executar o instalador do VB-Cable, as assinaturas dos binários já foram
validadas — o pacote é o oficial da VB-Audio.
