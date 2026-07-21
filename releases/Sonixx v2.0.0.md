# 🔊 Sonixx v2.0.0 — Soundboard & Advanced Audio Control Release

**The major update bringing a 9-slot Soundboard, pre-decoded instant audio playback, independent Microphone & Application Mute hotkeys, and managed sound file storage.**

Sonixx v2.0.0 transforms the application from a live virtual audio router into a full-fledged, studio-grade audio control center and soundboard for gamers, streamers, and content creators.

---

## ✨ What's New in v2.0.0

### 📣 9-Slot Soundboard Panel
- **Integrated Tabbed Layout**: Access the new **"📣 Soundboard"** panel right next to your audio sources.
- **9 Independent Sound Slots**: Register up to 9 custom audio effects (`.mp3`, `.wav`, `.flac`, `.ogg`).
- **Instant 0ms Playback**: Audio files are pre-decoded into 48kHz Float32 stereo PCM buffers upon selection for zero-latency hotkey response.
- **Managed Local Storage**: Audio files are automatically copied to local application storage (`~/.sonixx/sounds/`), ensuring soundboard slots remain fully functional even if original files are moved or deleted.
- **Dedicated Global Hotkeys**: Trigger soundboard slots 1 through 9 instantly using `Ctrl+Alt+1` through `Ctrl+Alt+9` (fully rebindable).
- **Per-Slot Controls**: Custom label naming, native file picker integration, preview play/stop toggle, and slot clear buttons.

### 🎤 Independent Audio Mute Toggles & Hotkeys
- **Toggle Mute Microphones**: Mute all physical microphones instantly (`Ctrl+Alt+N`) while keeping application audio streaming.
- **Toggle Mute Applications**: Mute all application capture streams instantly (`Ctrl+Alt+A`) while keeping microphone audio active.
- **Header Mute Buttons**: One-click visual mute controls (`🔊` / `🔇`) added to Applications and Microphones headers.

### ⚙️ Enhanced Settings & Single-Combo Hotkeys
- **Streamlined Hotkey Recorder**: Capture 1 combination of up to 3 keys per action (e.g., `Ctrl+Alt+M`, `Ctrl+Alt+N`, `Ctrl+Alt+A`, `Ctrl+Alt+1..9`).
- **Persistent Settings**: Soundboard names, volumes, file paths, and customized hotkeys are automatically saved to `settings.json`.

### 🛠️ Stability & Exception Handling
- **Startup Crash Shield**: Integrated interactive Windows error alert and logging to capture and present any unhandled startup exceptions.
- **CTkTabview Fix**: Resolved CustomTkinter keyword argument initialization across Windows environments.

---

## 📦 Download & Build

| Asset | Description |
|---|---|
| `SonixxAudioRouter.exe` | Standalone executable — batteries included (VB-Cable driver bundled, pre-packaged miniaudio decoder) |

```bash
# Build standalone v2.0.0 executable
uv run pyinstaller SonixxAudioRouter.spec --noconfirm
```

---

Built by **Felix-au** (Harshit Soni)

- 🔗 GitHub: [github.com/Felix-au](https://github.com/Felix-au)
- 📧 Email: [felixaugum@gmail.com](mailto:felixaugum@gmail.com)
