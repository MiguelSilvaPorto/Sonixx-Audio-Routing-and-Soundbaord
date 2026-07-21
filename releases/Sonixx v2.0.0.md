# 🔊 Sonixx v2.0.0 — Soundboard & Advanced Audio Control Release

**The major update bringing a 9-slot Soundboard, pre-decoded instant audio playback, independent Microphone & Application Mute hotkeys, and managed sound file storage.**

Sonixx v2.0.0 transforms the application from a live virtual audio router into a full-fledged, studio-grade audio control center and soundboard for gamers, streamers, and content creators.

---

## ✨ What's Included & New in v2.0.0

### 📣 9-Slot Soundboard Panel
- **Integrated Tabbed Layout** — access the new **"📣 Soundboard"** panel right next to your audio sources
- **9 Independent Sound Slots** — register up to 9 custom audio effects (`.mp3`, `.wav`, `.flac`, `.ogg`)
- **Instant 0ms Playback** — audio files are pre-decoded into 48kHz Float32 stereo PCM buffers upon selection for zero-latency hotkey response
- **Managed Local Storage** — audio files are automatically copied to local application root storage (`sounds/`), ensuring soundboard slots remain fully functional even if original files are moved or deleted
- **Unified Master Volume Slider** — smooth 0% to 200% volume control scaling across all soundboard playbacks with live percentage display
- **Toggle Mute Soundboard** — instant soundboard mute button (`🔊`/`🔇`) and dedicated global hotkey (`Ctrl+Alt+Q`)
- **Dedicated Global Hotkeys** — trigger soundboard slots 1 through 9 instantly using `Ctrl+Alt+1` through `Ctrl+Alt+9` (fully rebindable)
- **Per-Slot Controls** — custom label naming with explicit `✓` save confirmation, native file picker integration, preview play/stop toggle, and slot clear buttons

### 🎤 Independent Audio Mute Toggles & Hotkeys
- **Toggle Mute Microphones** — mute all physical microphones instantly (`Ctrl+Alt+N`) while keeping application audio streaming
- **Toggle Mute Applications** — mute all application capture streams instantly (`Ctrl+Alt+A`) while keeping microphone audio active
- **Header Mute Buttons** — one-click visual mute controls (`🔊` / `🔇`) added to Applications and Microphones headers

### ⚙️ Enhanced Settings & Single-Combo Hotkeys
- **Streamlined Hotkey Recorder** — capture 1 combination of up to 3 keys per action (e.g., `Ctrl+Alt+M`, `Ctrl+Alt+N`, `Ctrl+Alt+A`, `Ctrl+Alt+1..9`)
- **Persistent Settings** — soundboard names, volumes, file paths, and customized hotkeys are automatically saved to `settings.json`

### 🛠️ Core Engine & Stability Improvements
- **Startup Crash Shield** — integrated interactive Windows error alert and logging to capture and present any unhandled startup exceptions
- **CTkTabview Fix** — resolved CustomTkinter keyword argument initialization across Windows environments
- **Sample Rate Resampling** — real-time linear interpolation resampling in NumPy when headphone sample rates differ from the virtual cable sample rate (48kHz)

---

## 📦 Download

| Asset | Description |
|---|---|
| `SonixxAudioRouter.exe` | Standalone single-file executable — batteries included (VB-Cable driver bundled, pre-packaged miniaudio decoder, no Python required) |

> **Requires:** Windows 10/11 • Administrator privileges (for driver install + device renaming)

---

## 🚀 Quick Start

1. Download and run `SonixxAudioRouter.exe`
2. Accept the Administrator prompt
3. If VB-Cable isn't installed, click **⚡ Install VB-Cable Driver** (one-click, silent)
4. Play audio in any app (Spotify, YouTube, a game) or switch to **"📣 Soundboard"** tab to load your favorite MP3/WAV effects
5. Click **⟳ Scan** to detect audio-producing apps
6. Toggle on the sources you want to route or trigger soundboard slots using `Ctrl+Alt+1..9`
7. Click **▶ Start**
8. In Discord/VALORANT/Zoom, select **"Sonixx"** (or "CABLE Output") as your microphone

---

## 🔧 Building from Source

```bash
git clone https://github.com/Felix-au/Sonixx
cd Sonixx

# Requires Python ≥ 3.13 + uv
uv sync
uv run main.py

# Build standalone .exe
uv run pyinstaller SonixxAudioRouter.spec --noconfirm
```

---

## ⚠️ Known Limitations

- **Windows-only** — relies on WASAPI APIs and per-process audio capture
- **No auto-rescan** — new apps require a manual Scan click
- **Dark theme only** — light mode is not yet implemented

---

## 🙏 Acknowledgements

- [VB-Audio](https://vb-audio.com/Cable/) — for the free VB-Cable virtual audio driver
- [process-audio-capture](https://pypi.org/project/process-audio-capture/) — for per-process audio isolation on Windows
- [miniaudio](https://pypi.org/project/miniaudio/) — for ultra-fast C-based audio file decoding (MP3, WAV, FLAC, OGG)
- [CustomTkinter](https://github.com/TomSchimansky/CustomTkinter) — for the modern UI framework
- [PyAudioWPatch](https://github.com/s0d3s/PyAudioWPatch) — for WASAPI loopback support

---

Built by **Felix-au** (Harshit Soni)

- 🔗 GitHub: [github.com/Felix-au](https://github.com/Felix-au)
- 📧 Email: [felixaugum@gmail.com](mailto:felixaugum@gmail.com)

---
