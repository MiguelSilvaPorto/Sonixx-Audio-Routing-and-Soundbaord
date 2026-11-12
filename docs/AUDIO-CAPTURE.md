# Captura e detecção de áudio no Windows — o que dá para fazer sem driver

**Data dos testes:** 2026-11-12  
**Máquina:** Windows local, sessão de console, Realtek + USB headset + HDMI,
VB-Cable **não instalado**.

## Resumo

| Capacidade | Sem driver? | Resultado nesta máquina |
|---|---|---|
| Detectar **se** há áudio saindo (endpoint) | Sim | **Funciona** (`IAudioMeterInformation`) |
| Detectar **qual app** está tocando (sessão) | Sim | **Funciona** (PID + nome + pico) |
| Capturar o áudio do sistema (WASAPI loopback) | Sim (API do Windows) | **Bloqueado**: `E_INVALIDARG` em todos os endpoints |
| Capturar áudio de um app (process loopback) | Sim (API do Windows) | Abre em 48 kHz/2 ch, mas sem pacotes/peaks nesta máquina |
| Capturar “Stereo Mix” | Depende do driver do áudio | Não exposto como endpoint ativo aqui |
| Criar um microfone virtual para outros apps | **Não** | Exige driver/dispositivo virtual (VB-Cable etc.) |
| Tocar/decodificar soundboard e mixar na saída | Sim | **Funciona** |

## O que foi testado

### 1. Detecção por medidor de endpoint — funciona

`IAudioMeterInformation` no dispositivo de reprodução padrão:

```
endpoint peak: 0.129  (com áudio tocando)
```

Não abre stream de captura, não consome amostras e não precisa de driver. É a
forma mais barata de saber **se** o PC está emitindo som.

### 2. Detecção por sessão de áudio — funciona

`IAudioSessionManager2` → `IAudioSessionEnumerator` →
`IAudioSessionControl2` e `QueryInterface` para `IAudioMeterInformation`:

```
 pid=15904  peak=0.108  chrome.exe
 pid=12340  peak=0.000  ms-teams.exe
 pid=19836  peak=0.000  Mozilla Thunderbird
 ...
```

A UI usa isso na aba **Aplicativos**: cada sessão mostra `tocando`/`silencioso`
e um medidor em tempo real, sem iniciar a engine e sem capturar áudio.

### 3. WASAPI loopback de endpoint — bloqueado nesta máquina

Testado em todos os dispositivos de reprodução ativos, com:

- `AUDCLNT_STREAMFLAGS_LOOPBACK`;
- formato mix do dispositivo e formato fixo 48 kHz/2 canais/float;
- `WAVEFORMATEX` e `WAVEFORMATEXTENSIBLE`;
- modo polling e event-driven;
- `AUTOCONVERTPCM` + `SRC_DEFAULT_QUALITY`;
- durações de buffer 0, 100 ms, 1 s e 10 s.

Todos retornam `E_INVALIDARG (0x80070057)` no `IAudioClient::Initialize`.
O crate de referência `wasapi` (0.27) também falha para microfone e loopback
nesta máquina, o que indica bloqueio do ambiente (política/antivírus/driver), não
um erro da nossa implementação.

### 4. Process loopback — abre, mas não entrega amostras aqui

`ActivateAudioInterfaceAsync` + `AUDIOCLIENT_ACTIVATION_PARAMS` com
`PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE`:

```
abrindo process loopback para PID 15904...
OK: 48000 Hz, 2 canais, 32 bits
```

A ativação e o `Initialize` funcionam, mas os pacotes lidos ficam vazios e o
peak permanece 0.000, mesmo com o Chrome em 0.108 no medidor de sessão. É a
mesma família de bloqueio de captura; a API em si está implementada em Rust.

### 5. “Stereo Mix” / Mixagem estéreo

Aparece no registro de dispositivos, mas não está ativo como endpoint WASAPI de
captura nesta máquina. Quando o driver oferece esse endpoint, ele é uma
alternativa **sem cabo virtual** para capturar o mix do sistema — mas depende do
driver de áudio (Realtek), não é universal e não cria um microfone virtual.

### 6. Saída/render e soundboard — funcionam

`IAudioRenderClient` abre e escreve no dispositivo escolhido. O soundboard
decodifica WAV/MP3/FLAC/OGG/AAC e mixa no render; o medidor máster subiu para
~0.209 com o tom de teste.

## Limite fundamental do Windows

Capturar áudio é diferente de **publicar um microfone**:

- WASAPI loopback e process loopback entregam amostras para o **seu** processo.
  Eles não criam um dispositivo de captura que Discord, OBS, jogos ou chamadas
  enxerguem.
- Um endpoint de captura (“microfone”) só existe se um **driver de áudio** o
  registrar. VB-Cable, VoiceMeeter e similares são drivers/dispositivos virtuais.
- APIs WinRT (`Windows.Media.Audio`, `AudioGraph`) capturam de um dispositivo;
  não transformam o áudio do sistema em microfone. O `AudioPlaybackCapture` é
  opt-in de apps UWP e não se aplica a apps desktop arbitrários.
- ETW de áudio informa atividade/eventos, mas não entrega amostras.
- APO/kernel streaming também exigem driver.

**Conclusão:** sem driver, o máximo é **detectar** (feito) e, quando o ambiente
permitir, **capturar/monitorar** via WASAPI loopback/process loopback. Para
rotear o áudio para dentro de outro app como microfone, o VB-Cable (ou outro
dispositivo virtual) continua sendo necessário.

## O que foi entregue no Sonixx

- Detector de atividade do endpoint e de cada sessão, em tempo real, sem captura
  e sem driver (`IAudioMeterInformation`).
- Aba **Aplicativos** mostrando quem está tocando, com medidor.
- Captura por aplicativo implementada com process loopback.
- Engine de captura/render e soundboard em Rust.

## Ferramentas de diagnóstico

```powershell
# Endpoint + sessões com peak em tempo real
cargo run -p sonixx-core --example audio_probe

# Tenta loopback em todos os dispositivos de saída
cargo run -p sonixx-core --example loopback_probe

# Abre process loopback para um PID
cargo run -p sonixx-core --example process_probe -- 1234

# Abre render em todos os dispositivos
cargo run -p sonixx-core --example render_probe

# Mede o peak do process loopback de um PID
cargo run -p sonixx-core --example process_smoke -- 1234
```

## Próximos testes recomendados

1. Rodar o Sonixx em uma máquina Windows limpa (sem política de antivírus) e
   repetir `cargo run -p sonixx-core --example loopback_probe`.
2. Verificar a proteção de microfone/áudio do Kaspersky Endpoint Security.
3. Reinstalar/atualizar o driver de áudio (Realtek/Intel/USB) e reiniciar o
   serviço de áudio.
4. Se o objetivo for microfone virtual, instalar o VB-Cable (já incluído no
   repositório) — não existe alternativa 100% user-mode/ Rust para criar o
   endpoint.
