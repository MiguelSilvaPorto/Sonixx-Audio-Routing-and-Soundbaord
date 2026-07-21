# 🔊 Sonixx v1.0.0 — Initial Release

**The first soundboard that routes live audio between applications.**

Every soundboard on the market — Voicemod, Soundpad, EXP Soundboard, Resanance — plays pre-downloaded MP3/WAV files through a virtual mic. Sonixx is different: it captures **live audio** from any running application (Spotify, Chrome, games, DAWs) and mixes it with your microphone into a single virtual mic output, in real-time.

No file downloads. No manual recording. Just pick an app and its audio goes straight into Discord, VALORANT, Zoom, or OBS.

---

## ✨ What's Included

### Core Audio Engine
- **Per-app audio capture** via Windows WASAPI per-process loopback + named pipes (`process-audio-capture` DLL)
- **Microphone mixing** — add multiple physical mics, each with independent controls
- **Hardware-synced mixing** — all audio processing runs inside WASAPI callbacks, synced to the hardware clock (zero drift)
- **210 ms jitter buffering** — absorbs OS scheduling jitter for glitch-free, studio-quality output
- **Mono → Stereo upmix** — mono mics are automatically upmixed for consistent output
- **Loopback monitor** — hear exactly what's being sent to the virtual mic through your headphones

### Per-Source Controls
- Individual **volume sliders** (0–200%) for every app and mic source
- **Enable/disable toggles** and **remove buttons** per source
- Real-time **peak meters** with green → orange → red level indicators

### Master Controls
- Master volume (0–200%) with peak metering
- Master mute button + global hotkey (`Ctrl+Alt+M`)

### System Integration
- **VB-Cable auto-install** — bundled `VBCABLE_Driver_Pack45.zip`, installed silently on first launch if the driver is missing
- **Device renaming** — rename VB-Cable to "Sonixx" in Windows audio registry so it appears branded in apps
- **System tray** — minimize to tray on close, restore with double-click
- **Auto-start with Windows** — optional registry-based startup
- **Self-exclusion** — Sonixx excludes its own process from audio scans to prevent feedback loops

### UI
- Dark-themed interface built with CustomTkinter, designed to feel like a native Windows 11 audio panel
- Animated source row insertion
- Footer with author profile links (GitHub + email)

---

## 📦 Download

| Asset | Description |
|---|---|
| `SonixxAudioRouter.exe` | Standalone single-file executable — batteries included (VB-Cable driver bundled, no Python required) |

> **Requires:** Windows 10/11 • Administrator privileges (for driver install + device renaming)

---

## 🚀 Quick Start

1. Download and run `SonixxAudioRouter.exe`
2. Accept the Administrator prompt
3. If VB-Cable isn't installed, click **⚡ Install VB-Cable Driver** (one-click, silent)
4. Play audio in any app (Spotify, YouTube, a game)
5. Click **⟳ Scan** to detect audio-producing apps
6. Toggle on the sources you want to route
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
- **No sample rate resampling** — devices with mismatched sample rates may produce artifacts
- **No auto-rescan** — new apps require a manual Scan click
- **Dark theme only** — light mode is not yet implemented

---

## 🙏 Acknowledgements

- [VB-Audio](https://vb-audio.com/Cable/) — for the free VB-Cable virtual audio driver
- [process-audio-capture](https://pypi.org/project/process-audio-capture/) — for per-process audio isolation on Windows
- [CustomTkinter](https://github.com/TomSchimansky/CustomTkinter) — for the modern UI framework
- [PyAudioWPatch](https://github.com/s0d3s/PyAudioWPatch) — for WASAPI loopback support

---

Built by **Felix-au** (Harshit Soni)

- 🔗 GitHub: [github.com/Felix-au](https://github.com/Felix-au)
- 📧 Email: [felixaugum@gmail.com](mailto:felixaugum@gmail.com)

---
