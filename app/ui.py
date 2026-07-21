"""Sonixx UI - Shows individual apps like Windows Sound Settings."""
import customtkinter as ctk
import threading, time, webbrowser, os
import pyaudiowpatch as pyaudio
from PIL import Image
from app.driver import (is_cable_installed, get_cable_output_device, silent_install_vbcable,
                         rename_to_sonixx, find_wasapi, SONIXX, SONIXX_FULL)
from app.audio_router import AudioRouter
from process_audio_capture import ProcessAudioCapture
import sys, json
import keyboard
from pystray import Icon, Menu, MenuItem

def resource_path(relative_path):
    """ Get absolute path to resource, works for dev and for PyInstaller """
    try:
        base_path = sys._MEIPASS
    except Exception:
        base_path = os.path.abspath(".")
    return os.path.join(base_path, relative_path)


P = {"bg":"#080810","card":"#101018","card2":"#181824","hover":"#222236",
     "sel":"#1c1c3c","accent":"#7c6cf0","accent2":"#a99dff",
     "green":"#00d9a3","green_d":"#0a4a3a","red":"#f04040","red_d":"#401818",
     "orange":"#f0a030","txt":"#eaeaf4","dim":"#8080a0","muted":"#505068","border":"#282840"}

SETTINGS_FILE = os.path.join(os.getenv("APPDATA", "."), "Sonixx", "settings.json")

FRIENDLY={"chrome.exe":"Google Chrome","msedge.exe":"Microsoft Edge","firefox.exe":"Firefox",
"spotify.exe":"Spotify","discord.exe":"Discord","brave.exe":"Brave","vlc.exe":"VLC",
"obs64.exe":"OBS Studio","steam.exe":"Steam","teams.exe":"MS Teams",
"steamwebhelper.exe":"Steam Web","VALORANT-Win64-Shipping.exe":"VALORANT",
"audiodg.exe":"Windows Audio Device Graph"}

def friendly(n):
    return FRIENDLY.get(n, n.replace(".exe","").replace("_"," ").replace("-"," ").title())

class SourceRow(ctk.CTkFrame):
    def __init__(self, master, label, stype, on_toggle, on_vol, on_rm, **kw):
        super().__init__(master, fg_color=P["card"], corner_radius=10, border_width=1, border_color=P["border"], **kw)
        self.source_id = None; self.pid = None
        self.grid_columnconfigure(2, weight=1)
        icon = "🎤" if stype=="mic" else "🎵"
        
        ctk.CTkButton(self, text="✕", width=26, height=26, font=("Segoe UI",11),
                       fg_color="transparent", hover_color=P["red_d"], text_color=P["muted"],
                       corner_radius=6, command=lambda: on_rm(self)).grid(row=0,column=0,padx=(8,4))
        
        ctk.CTkLabel(self, text=f"{icon} {label}", font=("Segoe UI Semibold",12),
                     text_color=P["txt"], anchor="w").grid(row=0,column=1,sticky="w",padx=(0,8))
                     
        vf = ctk.CTkFrame(self, fg_color="transparent"); vf.grid(row=0,column=2,sticky="ew",padx=4)
        vf.grid_columnconfigure(0, weight=1)
        self.slider = ctk.CTkSlider(vf, from_=0, to=200, number_of_steps=200, width=120,
                                     fg_color=P["card2"], progress_color=P["accent"],
                                     button_color=P["accent2"], button_hover_color="#fff",
                                     command=lambda v: on_vol(self, v))
        self.slider.set(100); self.slider.grid(row=0,column=0,sticky="ew")
        self.vlbl = ctk.CTkLabel(vf, text="100%", width=40, font=("Segoe UI",10), text_color=P["dim"])
        self.vlbl.grid(row=0,column=1,padx=(4,0))
        
        self.peak_cv = ctk.CTkCanvas(self, width=50, height=8, bg=P["card"], highlightthickness=0)
        self.peak_cv.grid(row=0,column=3,padx=6)
        
        self.tog = ctk.CTkSwitch(self, text="", width=36, fg_color=P["card2"], progress_color=P["green"],
                                  button_color="#ccc", command=lambda: on_toggle(self))
        self.tog.grid(row=0,column=4,padx=(6,10),pady=10)
    def set_peak(self, level):
        w=50; self.peak_cv.delete("all")
        self.peak_cv.create_rectangle(0,0,w,8,fill=P["card2"],outline="")
        bw=int(w*min(level,1.0))
        if bw>0:
            c=P["green"] if level<0.7 else (P["orange"] if level<0.9 else P["red"])
            self.peak_cv.create_rectangle(0,0,bw,8,fill=c,outline="")

    def animate_in(self):
        self.configure(height=0)
        def step(h):
            if h < 46:
                self.configure(height=h+4)
                self.after(10, lambda: step(h+4))
        step(0)

DEFAULT_HOTKEYS = {
    "mute_master": {"label": "Toggle Master Mute", "default": "ctrl+alt+m"},
    "mute_mics": {"label": "Toggle Mute Microphones", "default": "ctrl+alt+n"},
    "mute_apps": {"label": "Toggle Mute Applications", "default": "ctrl+alt+a"},
    "toggle_router": {"label": "Toggle Audio Router", "default": "ctrl+alt+s"},
    "toggle_monitor": {"label": "Toggle Monitor Mix", "default": "ctrl+alt+o"},
    "scan_apps": {"label": "Scan Audio Apps", "default": "ctrl+alt+r"},
    "soundboard_1": {"label": "Soundboard Slot #1", "default": "ctrl+alt+1"},
    "soundboard_2": {"label": "Soundboard Slot #2", "default": "ctrl+alt+2"},
    "soundboard_3": {"label": "Soundboard Slot #3", "default": "ctrl+alt+3"},
    "soundboard_4": {"label": "Soundboard Slot #4", "default": "ctrl+alt+4"},
    "soundboard_5": {"label": "Soundboard Slot #5", "default": "ctrl+alt+5"},
    "soundboard_6": {"label": "Soundboard Slot #6", "default": "ctrl+alt+6"},
    "soundboard_7": {"label": "Soundboard Slot #7", "default": "ctrl+alt+7"},
    "soundboard_8": {"label": "Soundboard Slot #8", "default": "ctrl+alt+8"},
    "soundboard_9": {"label": "Soundboard Slot #9", "default": "ctrl+alt+9"},
}


class SoundboardTileCard(ctk.CTkFrame):
    def __init__(self, master, slot_id, slot_obj, on_file_pick, on_play_toggle, on_clear, on_name_change, get_hotkey, **kw):
        super().__init__(master, fg_color=P["card2"], corner_radius=10, border_width=1, border_color=P["border"], **kw)
        self.slot_id = slot_id
        self.slot_obj = slot_obj
        self.on_file_pick = on_file_pick
        self.on_play_toggle = on_play_toggle
        self.on_clear = on_clear
        self.on_name_change = on_name_change
        self.get_hotkey = get_hotkey

        self.grid_columnconfigure(1, weight=1)

        # Header Row: Badge (#1), Name Entry, Save Button (✓), Clear Button (✕)
        ctk.CTkLabel(self, text=f"#{slot_id}", font=("Segoe UI Black", 12), text_color=P["accent2"]).grid(row=0, column=0, padx=(8, 2), pady=4)
        
        self.name_entry = ctk.CTkEntry(self, height=24, font=("Segoe UI Semibold", 11),
                                       fg_color=P["card"], border_color=P["border"], text_color=P["txt"], corner_radius=6)
        self.name_entry.insert(0, slot_obj.name)
        self.name_entry.grid(row=0, column=1, sticky="ew", padx=2, pady=4)
        self.name_entry.bind("<Return>", lambda e: self.save_name())

        self.save_btn = ctk.CTkButton(self, text="✓", width=22, height=22, font=("Segoe UI Bold", 11),
                                      fg_color="transparent", hover_color=P["hover"], text_color=P["dim"],
                                      corner_radius=4, command=self.save_name)
        self.save_btn.grid(row=0, column=2, padx=1)

        ctk.CTkButton(self, text="✕", width=22, height=22, font=("Segoe UI", 10),
                       fg_color="transparent", hover_color=P["red_d"], text_color=P["muted"],
                       corner_radius=4, command=lambda: on_clear(slot_id, self)).grid(row=0, column=3, padx=(1, 4))

        # File Status Label
        fname = os.path.basename(slot_obj.file_path) if slot_obj.file_path else "No file selected"
        self.file_lbl = ctk.CTkLabel(self, text=fname, font=("Segoe UI", 9), text_color=P["dim"] if slot_obj.file_path else P["muted"], anchor="w")
        self.file_lbl.grid(row=1, column=0, columnspan=4, sticky="ew", padx=8, pady=(0, 2))

        # Bottom Row: Play Button, Choose File Button, Hotkey Badge
        btn_f = ctk.CTkFrame(self, fg_color="transparent")
        btn_f.grid(row=2, column=0, columnspan=4, sticky="ew", padx=6, pady=(2, 6))
        btn_f.grid_columnconfigure(1, weight=1)

        self.play_btn = ctk.CTkButton(btn_f, text="▶ Play", width=55, height=26, font=("Segoe UI Semibold", 10),
                                       fg_color=P["accent"], hover_color=P["accent2"], corner_radius=6,
                                       command=lambda: on_play_toggle(slot_id, self))
        self.play_btn.grid(row=0, column=0, padx=(0, 2))

        self.pick_btn = ctk.CTkButton(btn_f, text="📁 File", width=50, height=26, font=("Segoe UI", 10),
                                       fg_color=P["card"], hover_color=P["hover"], text_color=P["dim"], corner_radius=6,
                                       command=lambda: on_file_pick(slot_id, self))
        self.pick_btn.grid(row=0, column=1, sticky="w", padx=2)

        hk_str = get_hotkey(f"soundboard_{slot_id}") or f"ctrl+alt+{slot_id}"
        self.hk_lbl = ctk.CTkLabel(btn_f, text=f"[{hk_str.upper()}]", font=("Consolas", 9), text_color=P["muted"])
        self.hk_lbl.grid(row=0, column=2, sticky="e", padx=(2, 0))

    def save_name(self):
        new_name = self.name_entry.get()
        self.on_name_change(self.slot_id, new_name)
        self.save_btn.configure(text="✓", fg_color=P["green_d"], text_color=P["green"])
        self.after(1200, lambda: self.save_btn.configure(text="✓", fg_color="transparent", text_color=P["dim"]))

    def update_ui(self):
        fname = os.path.basename(self.slot_obj.file_path) if self.slot_obj.file_path else "No file selected"
        self.file_lbl.configure(text=fname, text_color=P["dim"] if self.slot_obj.file_path else P["muted"])
        hk_str = self.get_hotkey(f"soundboard_{self.slot_id}") or f"ctrl+alt+{self.slot_id}"
        self.hk_lbl.configure(text=f"[{hk_str.upper()}]")


class SettingsWindow(ctk.CTkToplevel):
    def __init__(self, parent):
        super().__init__(parent)
        self.parent = parent
        self.title("Sonixx - Settings & Hotkey Configuration")
        self.transient(parent)
        self.grab_set()

        w, h = 600, 520
        px = parent.winfo_x() + (parent.winfo_width() - w) // 2
        py = parent.winfo_y() + (parent.winfo_height() - h) // 2
        self.geometry(f"{w}x{h}+{px}+{py}")
        self.minsize(w, h)
        self.configure(fg_color=P["bg"])

        self.hotkeys = {}
        saved_hk = parent.settings.get("hotkeys", {})
        for act_id, default_info in DEFAULT_HOTKEYS.items():
            val = saved_hk.get(act_id, default_info["default"])
            if isinstance(val, list):
                val = val[0] if val else ""
            self.hotkeys[act_id] = str(val)

        self.inputs = {}
        self._active_hook = None
        self._build_ui()

    def _build_ui(self):
        self.grid_rowconfigure(1, weight=1)
        self.grid_columnconfigure(0, weight=1)

        # Header
        hdr = ctk.CTkFrame(self, fg_color=P["card"], corner_radius=0, height=45)
        hdr.grid(row=0, column=0, sticky="ew")
        hdr.pack_propagate(False)
        ctk.CTkLabel(hdr, text="⚙ Settings & Hotkey Configuration", font=("Segoe UI Semibold", 15), text_color=P["accent2"]).pack(side="left", padx=16)

        # Scrollable Body
        sf = ctk.CTkScrollableFrame(self, fg_color="transparent", scrollbar_button_color=P["border"])
        sf.grid(row=1, column=0, sticky="nsew", padx=16, pady=12)
        sf.grid_columnconfigure(0, weight=1)

        # Section 1: General Preferences
        ctk.CTkLabel(sf, text="General Preferences", font=("Segoe UI Semibold", 13), text_color=P["txt"]).pack(anchor="w", pady=(0, 6))
        
        gf = ctk.CTkFrame(sf, fg_color=P["card"], corner_radius=10, border_width=1, border_color=P["border"])
        gf.pack(fill="x", pady=(0, 16), ipady=4)

        self.tray_sw = ctk.CTkSwitch(gf, text="Minimize to System Tray on Close", font=("Segoe UI", 11), text_color=P["dim"],
                                     progress_color=P["accent"], button_color="#fff")
        self.tray_sw.pack(anchor="w", padx=14, pady=8)
        if self.parent.settings.get("tray", True):
            self.tray_sw.select()

        self.start_sw = ctk.CTkSwitch(gf, text="Start Sonixx automatically with Windows", font=("Segoe UI", 11), text_color=P["dim"],
                                      progress_color=P["accent"], button_color="#fff")
        self.start_sw.pack(anchor="w", padx=14, pady=(0, 8))
        if self.parent.settings.get("startup", False):
            self.start_sw.select()

        # Section 2: Hotkeys
        ctk.CTkLabel(sf, text="Hotkey Configuration (1 Combination of Up to 3 Keys per Action)", font=("Segoe UI Semibold", 13), text_color=P["txt"]).pack(anchor="w", pady=(0, 4))
        ctk.CTkLabel(sf, text="Click ⌨ to record a key combination (e.g. Ctrl+Alt+M or F9), or type directly.",
                      font=("Segoe UI", 10), text_color=P["dim"]).pack(anchor="w", pady=(0, 8))

        hk_frame = ctk.CTkFrame(sf, fg_color=P["card"], corner_radius=10, border_width=1, border_color=P["border"])
        hk_frame.pack(fill="x", pady=(0, 10))
        hk_frame.grid_columnconfigure(0, weight=1)

        for row_idx, (act_id, act_info) in enumerate(DEFAULT_HOTKEYS.items()):
            if row_idx > 0:
                ctk.CTkFrame(hk_frame, fg_color=P["border"], height=1).pack(fill="x", padx=8)

            r_frame = ctk.CTkFrame(hk_frame, fg_color="transparent")
            r_frame.pack(fill="x", padx=12, pady=8)
            r_frame.grid_columnconfigure(0, weight=1)

            ctk.CTkLabel(r_frame, text=act_info["label"], font=("Segoe UI Semibold", 11), text_color=P["txt"]).grid(row=0, column=0, sticky="w")

            slot_frame = ctk.CTkFrame(r_frame, fg_color="transparent")
            slot_frame.grid(row=0, column=1, sticky="e")

            val = self.hotkeys[act_id]
            entry = ctk.CTkEntry(slot_frame, width=150, height=28, font=("Consolas", 11),
                                 fg_color=P["card2"], border_color=P["border"], text_color=P["txt"],
                                 corner_radius=6)
            entry.insert(0, val)
            entry.grid(row=0, column=0, padx=(0, 4))
            self.inputs[act_id] = entry

            rec_btn = ctk.CTkButton(slot_frame, text="⌨ Record", width=75, height=28, font=("Segoe UI", 11),
                                     fg_color=P["card2"], hover_color=P["hover"], text_color=P["dim"],
                                     corner_radius=6, command=lambda a=act_id, e=entry: self._record_hotkey(a, e))
            rec_btn.grid(row=0, column=1, padx=(0, 4))

            clr_btn = ctk.CTkButton(slot_frame, text="✕", width=26, height=28, font=("Segoe UI", 10),
                                     fg_color="transparent", hover_color=P["red_d"], text_color=P["muted"],
                                     corner_radius=6, command=lambda e=entry: self._clear_slot(e))
            clr_btn.grid(row=0, column=2)

        # Footer Actions
        foot = ctk.CTkFrame(self, fg_color=P["card"], corner_radius=0, height=50)
        foot.grid(row=2, column=0, sticky="ew")
        foot.pack_propagate(False)

        ctk.CTkButton(foot, text="⟲ Reset Defaults", width=120, height=32, font=("Segoe UI", 11),
                       fg_color=P["card2"], hover_color=P["hover"], text_color=P["dim"],
                       corner_radius=8, command=self._reset_defaults).pack(side="left", padx=16, pady=9)

        ctk.CTkButton(foot, text="Save & Apply", width=120, height=32, font=("Segoe UI Semibold", 12),
                       fg_color=P["accent"], hover_color=P["accent2"], text_color="#fff",
                       corner_radius=8, command=self._save_and_close).pack(side="right", padx=16, pady=9)

        ctk.CTkButton(foot, text="Cancel", width=80, height=32, font=("Segoe UI", 11),
                       fg_color="transparent", hover_color=P["hover"], text_color=P["dim"],
                       corner_radius=8, command=self._cancel).pack(side="right", padx=(0, 4), pady=9)

    def _clear_slot(self, entry):
        entry.delete(0, "end")

    def _reset_defaults(self):
        for act_id, info in DEFAULT_HOTKEYS.items():
            entry = self.inputs[act_id]
            entry.delete(0, "end")
            entry.insert(0, info["default"])
        self.tray_sw.select()
        self.start_sw.deselect()

    def _record_hotkey(self, act_id, entry):
        if self._active_hook:
            try: keyboard.unhook(self._active_hook)
            except: pass
            self._active_hook = None

        entry.delete(0, "end")
        entry.insert(0, "Press keys...")
        entry.configure(border_color=P["accent"], text_color=P["accent2"])

        keys_pressed = []

        def on_key_event(event):
            if event.event_type != "down":
                return
            k = event.name.lower()
            if k in ["esc", "escape"]:
                if self._active_hook:
                    try: keyboard.unhook(self._active_hook)
                    except: pass
                    self._active_hook = None
                self.after(0, lambda: self._finish_recording(entry, ""))
                return

            mod_map = {"ctrl": "ctrl", "control": "ctrl", "left ctrl": "ctrl", "right ctrl": "ctrl",
                       "alt": "alt", "left alt": "alt", "right alt": "alt", "menu": "alt",
                       "shift": "shift", "left shift": "shift", "right shift": "shift"}
            clean_k = mod_map.get(k, k)
            if clean_k not in keys_pressed and len(keys_pressed) < 3:
                keys_pressed.append(clean_k)

            non_mods = [x for x in keys_pressed if x not in ["ctrl", "alt", "shift"]]
            if non_mods or len(keys_pressed) >= 3:
                mods = [x for x in keys_pressed if x in ["ctrl", "alt", "shift"]]
                combo_str = "+".join(mods + non_mods[:1]) if non_mods else "+".join(keys_pressed[:3])
                if self._active_hook:
                    try: keyboard.unhook(self._active_hook)
                    except: pass
                    self._active_hook = None
                self.after(0, lambda: self._finish_recording(entry, combo_str))

        try:
            self._active_hook = keyboard.hook(on_key_event)
        except Exception as e:
            print(f"[Settings] Key record error: {e}")
            entry.delete(0, "end")
            entry.configure(border_color=P["border"], text_color=P["txt"])

    def _finish_recording(self, entry, combo_str):
        entry.delete(0, "end")
        entry.insert(0, combo_str)
        entry.configure(border_color=P["border"], text_color=P["txt"])

    def _cancel(self):
        if self._active_hook:
            try: keyboard.unhook(self._active_hook)
            except: pass
            self._active_hook = None
        self.destroy()

    def _save_and_close(self):
        if self._active_hook:
            try: keyboard.unhook(self._active_hook)
            except: pass
            self._active_hook = None

        new_hotkeys = {}
        for act_id, entry in self.inputs.items():
            new_hotkeys[act_id] = entry.get().strip().lower()

        self.parent.settings["hotkeys"] = new_hotkeys
        self.parent.settings["tray"] = bool(self.tray_sw.get())
        self.parent.settings["startup"] = bool(self.start_sw.get())

        self.parent._save_settings_ui()
        self.parent._apply_hotkeys()
        self.destroy()


class App(ctk.CTk):
    def __init__(self):
        super().__init__()
        ctk.set_appearance_mode("dark")
        self.title(SONIXX_FULL)
        
        # Calculate 80% of screen height and width
        sw = self.winfo_screenwidth()
        sh = self.winfo_screenheight()
        w = int(sw * 0.8)
        h = int(sh * 0.8)
        
        # Center on screen
        x = int((sw - w) / 2)
        y = int((sh - h) / 2)
        
        self.geometry(f"{w}x{h}+{x}+{y}")
        self.minsize(w, h)
        self.configure(fg_color=P["bg"])

        # Set window icon
        try:
            icon_p = resource_path("assets/sonixx_logo.ico")
            if os.path.exists(icon_p):
                self.iconbitmap(icon_p)
        except: pass

        # Load profile icons
        try:
            gh_img = Image.open(resource_path("assets/github.png"))
            self.gh_icon = ctk.CTkImage(gh_img, size=(18, 18))
            em_img = Image.open(resource_path("assets/email.png"))
            self.em_icon = ctk.CTkImage(em_img, size=(18, 18))
        except:
            self.gh_icon = self.em_icon = None
        
        # Disable resizing (dragging) but keep Maximize button
        def disable_resize():
            try:
                import ctypes
                hwnd = ctypes.windll.user32.GetParent(self.winfo_id())
                style = ctypes.windll.user32.GetWindowLongW(hwnd, -16)
                style &= ~0x00040000  # Remove WS_THICKFRAME
                style |= 0x00010000   # Ensure WS_MAXIMIZEBOX
                ctypes.windll.user32.SetWindowLongW(hwnd, -16, style)
            except: pass
        self.after(100, disable_resize)
        
        # Load logo for header
        try:
            self.logo_img = ctk.CTkImage(Image.open(resource_path("assets/sonixx_logo.png")), size=(24, 24))
        except Exception:
            self.logo_img = None
        self.pa = pyaudio.PyAudio(); self.wi = find_wasapi(self.pa)
        self.router = AudioRouter(self.pa)
        self._load_settings()
        self.mic_rows={}; self.app_rows={}
        self._running=False; self._peak_job=None
        self._settings_win = None
        
        # Hotkey setup
        self._apply_hotkeys()

        self.protocol("WM_DELETE_WINDOW", self._on_window_close)
        
        if is_cable_installed(self.pa): self._build_main()
        else: self._build_setup()

    def _load_settings(self):
        os.makedirs(os.path.dirname(SETTINGS_FILE), exist_ok=True)
        defaults = {
            "tray": True,
            "startup": False,
            "hotkeys": {k: v["default"] for k, v in DEFAULT_HOTKEYS.items()}
        }
        if os.path.exists(SETTINGS_FILE):
            try:
                with open(SETTINGS_FILE, "r") as f:
                    saved = json.load(f)
                defaults.update(saved)
                saved_hk = defaults.get("hotkeys", {})
                for k, v in DEFAULT_HOTKEYS.items():
                    val = saved_hk.get(k, v["default"])
                    if isinstance(val, list):
                        val = val[0] if val else ""
                    saved_hk[k] = str(val)
                defaults["hotkeys"] = saved_hk
            except Exception as e:
                print(f"[UI] Settings load error: {e}")
        self.settings = defaults

        # Load Soundboard slots from settings
        sb_saved = self.settings.get("soundboard", {})
        for sid_str, slot_data in sb_saved.items():
            try:
                sid = int(sid_str)
                if 1 <= sid <= 9:
                    name = slot_data.get("name", f"Sound #{sid}")
                    file_p = slot_data.get("file", "")
                    vol = slot_data.get("volume", 1.0)
                    self.router.soundboard.configure_slot(sid, name=name, file_path=file_p, volume=vol)
            except Exception as e:
                print(f"[UI] Soundboard load error for slot {sid_str}: {e}")

    def _apply_hotkeys(self):
        try:
            keyboard.unhook_all_hotkeys()
        except Exception:
            try:
                keyboard.clear_all_hotkeys()
            except Exception:
                pass

        action_callbacks = {
            "mute_master": lambda: self.after(0, self._mute),
            "mute_mics": lambda: self.after(0, self._toggle_mics_mute),
            "mute_apps": lambda: self.after(0, self._toggle_apps_mute),
            "toggle_router": lambda: self.after(0, self._toggle_router_hotkey),
            "toggle_monitor": lambda: self.after(0, self._toggle_monitor_hotkey_action),
            "scan_apps": lambda: self.after(0, self._scan_apps),
        }
        for i in range(1, 10):
            action_callbacks[f"soundboard_{i}"] = (lambda slot_id=i: self.after(0, lambda s=slot_id: self._trigger_soundboard_slot(s)))

        hotkeys = self.settings.get("hotkeys", {})
        for act_id, combo in hotkeys.items():
            cb = action_callbacks.get(act_id)
            if not cb:
                continue
            if isinstance(combo, list):
                combo = combo[0] if combo else ""
            c = str(combo).strip().lower()
            if c:
                try:
                    keyboard.add_hotkey(c, cb)
                except Exception as e:
                    print(f"[UI] Failed to bind hotkey '{c}' for '{act_id}': {e}")

        # Update Soundboard Tile hotkey badges if UI is built
        if hasattr(self, "sb_tiles") and self.sb_tiles:
            for tile in self.sb_tiles.values():
                tile.update_ui()

    def _toggle_mics_mute(self):
        self.router.mics_mute = not self.router.mics_mute
        state_str = "MUTED" if self.router.mics_mute else "ACTIVE"
        if hasattr(self, "mic_mute_btn"):
            if self.router.mics_mute:
                self.mic_mute_btn.configure(text="🔇", fg_color=P["red_d"], text_color=P["red"])
            else:
                self.mic_mute_btn.configure(text="🔊", fg_color=P["card2"], text_color=P["dim"])
        if hasattr(self, "foot"):
            self.foot.configure(text=f"Microphones {state_str}")

    def _toggle_apps_mute(self):
        self.router.apps_mute = not self.router.apps_mute
        state_str = "MUTED" if self.router.apps_mute else "ACTIVE"
        if hasattr(self, "app_mute_btn"):
            if self.router.apps_mute:
                self.app_mute_btn.configure(text="🔇", fg_color=P["red_d"], text_color=P["red"])
            else:
                self.app_mute_btn.configure(text="🔊", fg_color=P["card2"], text_color=P["dim"])
        if hasattr(self, "foot"):
            self.foot.configure(text=f"Applications {state_str}")

    def _toggle_router_hotkey(self):
        if self._running:
            self._stop()
        else:
            self._start()

    def _toggle_monitor_hotkey_action(self):
        if hasattr(self, "monitor_switch"):
            cur = bool(self.monitor_switch.get())
            if cur:
                self.monitor_switch.deselect()
            else:
                self.monitor_switch.select()
        self._toggle_monitor()

    def _open_settings(self):
        if hasattr(self, "_settings_win") and self._settings_win and self._settings_win.winfo_exists():
            self._settings_win.lift()
        else:
            self._settings_win = SettingsWindow(self)

    def _save_settings(self):
        with open(SETTINGS_FILE, "w") as f: json.dump(self.settings, f)

    def _mute_toggle_hotkey(self):
        self.after(0, self._mute)

    def _on_window_close(self):
        is_tray = self.settings.get("tray", True)
        if is_tray:
            self.withdraw()
            self._show_tray()
        else:
            self.on_close()

    def _show_tray(self):
        if hasattr(self, "_tray_icon") and self._tray_icon: return
        try:
            icon_p = resource_path("assets/sonixx_logo.ico")
            if not os.path.exists(icon_p):
                icon_p = resource_path("assets/sonixx_logo.png")
            
            if not os.path.exists(icon_p):
                img = Image.new('RGB', (64, 64), color = (124, 108, 240))
            else:
                img = Image.open(icon_p)

            menu = Menu(MenuItem("Restore", self._restore_from_tray, default=True), MenuItem("Exit", self.on_close))
            self._tray_icon = Icon("Sonixx", img, "Sonixx", menu)
            
            t = threading.Thread(target=self._tray_icon.run, daemon=True)
            t.start()
        except Exception as e:
            print(f"[UI] Tray icon failed: {e}")

    def _restore_from_tray(self):
        if self._tray_icon:
            self._tray_icon.stop()
            self._tray_icon = None
        self.after(0, self.deiconify)

    def _build_setup(self):
        for w in self.winfo_children(): w.destroy()
        f=ctk.CTkFrame(self,fg_color=P["card"],corner_radius=20,border_width=1,border_color=P["border"])
        f.place(relx=0.5,rely=0.5,anchor="center",relwidth=0.6,relheight=0.55)
        ctk.CTkLabel(f,text=f"◉ {SONIXX} Setup",font=("Segoe UI Black",24),text_color=P["accent2"]).pack(pady=(30,10))
        ctk.CTkLabel(f,text="A lightweight audio driver (VB-Cable) is required\nto create the virtual microphone device.\nIt will be installed automatically.",
                     font=("Segoe UI",13),text_color=P["dim"],justify="center").pack(pady=(0,20))
        self.setup_st=ctk.CTkLabel(f,text="",font=("Segoe UI",12),text_color=P["orange"]); self.setup_st.pack(pady=(0,10))
        self.install_btn=ctk.CTkButton(f,text="⚡ Install VB-Cable Driver",height=44,font=("Segoe UI Semibold",14),
                       fg_color=P["accent"],hover_color=P["accent2"],corner_radius=12,command=self._do_install)
        self.install_btn.pack(padx=40,fill="x")
        ctk.CTkButton(f,text="⟳ Already installed — Refresh",height=32,font=("Segoe UI",11),
                       fg_color=P["card2"],hover_color=P["hover"],text_color=P["dim"],corner_radius=8,
                       command=self._chk_install).pack(padx=40,pady=(8,20),fill="x")

    def _do_install(self):
        self.install_btn.configure(state="disabled",fg_color=P["card2"])
        self.setup_st.configure(text="Installing driver... please wait.",text_color=P["orange"]); self.update()
        def w():
            ok, msg = silent_install_vbcable()
            if ok:
                self.after(0, lambda: self.setup_st.configure(text=f"✓ {msg}",text_color=P["green"]))
                # Auto-refresh after a short delay for the driver to register
                self.after(2000, self._chk_install)
            else:
                self.after(0, lambda: self.setup_st.configure(text=f"✕ {msg}",text_color=P["red"]))
                self.after(0, lambda: self.install_btn.configure(state="normal",fg_color=P["accent"]))
        threading.Thread(target=w,daemon=True).start()

    def _chk_install(self):
        self.pa.terminate(); self.pa=pyaudio.PyAudio(); self.wi=find_wasapi(self.pa); self.router.pa=self.pa
        if is_cable_installed(self.pa): self._build_main()
        else: self.setup_st.configure(text="Not detected yet. Install and retry.",text_color=P["red"])

    def _build_main(self):
        for w in self.winfo_children(): w.destroy()
        self.grid_rowconfigure(0, weight=0)
        self.grid_rowconfigure(1, weight=1)
        self.grid_rowconfigure(2, weight=0)
        self.grid_columnconfigure(0, weight=1)
        # Header
        h=ctk.CTkFrame(self,fg_color=P["card"],corner_radius=0,height=50); h.grid(row=0,column=0,sticky="ew")
        h.grid_columnconfigure(2,weight=1)
        
        if self.logo_img:
            ctk.CTkLabel(h, text="", image=self.logo_img).grid(row=0, column=0, padx=(20, 0), pady=11)
            ctk.CTkLabel(h, text=f"{SONIXX}", font=("Segoe UI Black", 20), text_color=P["accent2"]).grid(row=0, column=1, padx=(10, 20), pady=11, sticky="w")
        else:
            ctk.CTkLabel(h,text=f"◉ {SONIXX}",font=("Segoe UI Black",20),text_color=P["accent2"]).grid(row=0,column=0,padx=20,pady=11)
            
        hf = ctk.CTkFrame(h, fg_color="transparent")
        hf.grid(row=0, column=2, sticky="e", padx=20)
        self.status_lbl = ctk.CTkLabel(hf, text="● Stopped", font=("Segoe UI Semibold", 12), text_color=P["red"])
        self.status_lbl.pack(side="left", padx=(0, 12))
        self.settings_btn = ctk.CTkButton(hf, text="⚙ Settings", width=90, height=28, font=("Segoe UI Semibold", 11),
                                          fg_color=P["card2"], hover_color=P["hover"], text_color=P["txt"],
                                          corner_radius=8, command=self._open_settings)
        self.settings_btn.pack(side="left")

        # Body
        body=ctk.CTkFrame(self,fg_color="transparent"); body.grid(row=1,column=0,sticky="nsew",padx=14,pady=(8,0))
        body.grid_columnconfigure(0,weight=1); body.grid_columnconfigure(1,weight=1); body.grid_rowconfigure(0,weight=1)
        self._left(body); self._right(body)
        # Footer
        ft=ctk.CTkFrame(self,fg_color=P["card"],corner_radius=0,height=32); ft.grid(row=2,column=0,sticky="ew")
        ft.pack_propagate(False)

        self.foot=ctk.CTkLabel(ft,text="Add sources and press Start",font=("Segoe UI",10),text_color=P["muted"])
        self.foot.pack(side="left", padx=14, pady=6)
        
        # Profile section
        prof = ctk.CTkFrame(ft, fg_color="transparent")
        prof.pack(side="right", padx=14, pady=4)
        
        if self.gh_icon:
            ctk.CTkButton(prof, text=" Felix-au", image=self.gh_icon, height=24, fg_color="transparent", 
                          hover_color=P["hover"], corner_radius=6, font=("Segoe UI", 10), text_color=P["dim"],
                          command=lambda: webbrowser.open("https://github.com/Felix-au")).pack(side="left", padx=4)
        if self.em_icon:
            ctk.CTkButton(prof, text=" harshit.soni.23cse@bmu.edu.in", image=self.em_icon, height=24, fg_color="transparent", 
                          hover_color=P["hover"], corner_radius=6, font=("Segoe UI", 10), text_color=P["dim"],
                          command=lambda: webbrowser.open("mailto:harshit.soni.23cse@bmu.edu.in")).pack(side="left", padx=4)

        self._load_devices()

    def _left(self, parent):
        tv = ctk.CTkTabview(parent, fg_color=P["card"], corner_radius=14, border_width=1, border_color=P["border"],
                            segmented_button_fg_color=P["card2"], segmented_button_selected_color=P["accent"],
                            segmented_button_selected_hover_color=P["accent2"], text_color=P["txt"])
        tv.grid(row=0, column=0, sticky="nsew", padx=(0, 6))

        tab_sources = tv.add("🎵 Sources")
        tab_sb = tv.add("📣 Soundboard")

        # ── Tab 1: Sources (Apps + Mics) ──
        tab_sources.grid_columnconfigure(0, weight=1)
        tab_sources.grid_rowconfigure(1, weight=3)
        tab_sources.grid_rowconfigure(3, weight=1)

        # Apps section
        ab = ctk.CTkFrame(tab_sources, fg_color="transparent")
        ab.grid(row=0, column=0, sticky="ew", padx=14, pady=(10, 4))
        ab.grid_columnconfigure(0, weight=1)
        ctk.CTkLabel(ab, text="🎵 Applications", font=("Segoe UI Semibold", 14), text_color=P["txt"]).grid(row=0, column=0, sticky="w")
        self.app_mute_btn = ctk.CTkButton(ab, text="🔊", width=32, height=28, font=("Segoe UI", 12),
                                          fg_color=P["card2"], hover_color=P["hover"], text_color=P["dim"],
                                          corner_radius=8, command=self._toggle_apps_mute)
        self.app_mute_btn.grid(row=0, column=1, padx=(4, 4))
        ctk.CTkButton(ab, text="⟳ Scan", width=70, height=28, font=("Segoe UI", 11), fg_color=P["card2"],
                       hover_color=P["hover"], text_color=P["dim"], corner_radius=8,
                       command=self._scan_apps).grid(row=0, column=2)

        self.app_scroll = ctk.CTkScrollableFrame(tab_sources, fg_color="transparent", scrollbar_button_color=P["border"])
        self.app_scroll.grid(row=1, column=0, sticky="nsew", padx=6, pady=(4, 4))
        self.app_scroll.grid_columnconfigure(0, weight=1)
        self.app_empty = ctk.CTkLabel(self.app_scroll, text="Click Scan to detect apps with audio", font=("Segoe UI", 11), text_color=P["muted"])
        self.app_empty.grid(row=0, column=0, pady=20)

        # Mic section
        mb = ctk.CTkFrame(tab_sources, fg_color="transparent")
        mb.grid(row=2, column=0, sticky="ew", padx=14, pady=(8, 4))
        mb.grid_columnconfigure(1, weight=1)
        ctk.CTkLabel(mb, text="🎤 Microphones", font=("Segoe UI Semibold", 14), text_color=P["txt"]).grid(row=0, column=0, sticky="w")
        self.mic_var = ctk.StringVar()
        self.mic_cb = ctk.CTkComboBox(mb, variable=self.mic_var, values=["..."], height=28, font=("Segoe UI", 11),
                                      fg_color=P["card2"], border_color=P["border"], button_color=P["accent"],
                                      dropdown_fg_color=P["card2"], text_color=P["txt"], corner_radius=8)
        self.mic_cb.grid(row=0, column=1, padx=(8, 4), sticky="ew")
        self.mic_mute_btn = ctk.CTkButton(mb, text="🔊", width=32, height=28, font=("Segoe UI", 12),
                                          fg_color=P["card2"], hover_color=P["hover"], text_color=P["dim"],
                                          corner_radius=8, command=self._toggle_mics_mute)
        self.mic_mute_btn.grid(row=0, column=2, padx=(0, 4))
        ctk.CTkButton(mb, text="+ Add", width=55, height=28, font=("Segoe UI Semibold", 11), fg_color=P["accent"],
                       hover_color=P["accent2"], corner_radius=8, command=self._add_mic).grid(row=0, column=3)

        self.mic_scroll = ctk.CTkScrollableFrame(tab_sources, fg_color="transparent", scrollbar_button_color=P["border"])
        self.mic_scroll.grid(row=3, column=0, sticky="nsew", padx=6, pady=(4, 8))
        self.mic_scroll.grid_columnconfigure(0, weight=1)

        # ── Tab 2: Soundboard ──
        tab_sb.grid_columnconfigure(0, weight=1)
        tab_sb.grid_rowconfigure(0, weight=1)

        sb_scroll = ctk.CTkScrollableFrame(tab_sb, fg_color="transparent", scrollbar_button_color=P["border"])
        sb_scroll.grid(row=0, column=0, sticky="nsew", padx=4, pady=4)
        sb_scroll.grid_columnconfigure((0, 1), weight=1)

        self.sb_tiles = {}
        for sid in range(1, 10):
            slot_obj = self.router.soundboard.get_slot(sid)
            r = (sid - 1) // 2
            c = (sid - 1) % 2
            tile = SoundboardTileCard(
                sb_scroll, slot_id=sid, slot_obj=slot_obj,
                on_file_pick=self._sb_file_pick,
                on_play_toggle=self._sb_play_toggle,
                on_clear=self._sb_clear,
                on_name_change=self._sb_name_change,
                get_hotkey=lambda h_id: self.settings.get("hotkeys", {}).get(h_id, "")
            )
            tile.grid(row=r, column=c, sticky="nsew", padx=4, pady=4)
            self.sb_tiles[sid] = tile

    def _sb_file_pick(self, slot_id, tile):
        from tkinter import filedialog
        path = filedialog.askopenfilename(
            title=f"Select Audio File for Soundboard #{slot_id}",
            filetypes=[("Audio Files", "*.mp3 *.wav *.flac *.ogg"), ("All Files", "*.*")]
        )
        if path:
            ok = self.router.soundboard.configure_slot(slot_id, name="", file_path=path)
            if ok:
                tile.name_entry.delete(0, "end")
                tile.name_entry.insert(0, self.router.soundboard.get_slot(slot_id).name)
                tile.update_ui()
                self._save_soundboard_settings()

    def _sb_play_toggle(self, slot_id, tile):
        if self.router.soundboard.is_playing(slot_id):
            self.router.soundboard.stop_slot(slot_id)
            tile.play_btn.configure(text="▶ Play", fg_color=P["accent"])
        else:
            self._trigger_soundboard_slot(slot_id)

    def _sb_clear(self, slot_id, tile):
        self.router.soundboard.configure_slot(slot_id, name=f"Sound #{slot_id}", file_path="")
        tile.name_entry.delete(0, "end")
        tile.name_entry.insert(0, f"Sound #{slot_id}")
        tile.update_ui()
        self._save_soundboard_settings()

    def _sb_name_change(self, slot_id, new_name):
        slot = self.router.soundboard.get_slot(slot_id)
        if slot:
            slot.name = new_name or f"Sound #{slot_id}"
            self._save_soundboard_settings()

    def _trigger_soundboard_slot(self, slot_id):
        ok = self.router.soundboard.trigger_slot(slot_id)
        if ok:
            slot = self.router.soundboard.get_slot(slot_id)
            if hasattr(self, "foot"):
                self.foot.configure(text=f"🔊 Playing Soundboard #{slot_id}: {slot.name}")
            if hasattr(self, "sb_tiles") and slot_id in self.sb_tiles:
                tile = self.sb_tiles[slot_id]
                tile.play_btn.configure(text="■ Stop", fg_color=P["red_d"])
                self.after(200, lambda: self._check_sb_playback(slot_id))

    def _check_sb_playback(self, slot_id):
        if self.router.soundboard.is_playing(slot_id):
            self.after(200, lambda: self._check_sb_playback(slot_id))
        else:
            if hasattr(self, "sb_tiles") and slot_id in self.sb_tiles:
                tile = self.sb_tiles[slot_id]
                tile.play_btn.configure(text="▶ Play", fg_color=P["accent"])

    def _save_soundboard_settings(self):
        sb_data = {}
        for sid in range(1, 10):
            slot = self.router.soundboard.get_slot(sid)
            if slot:
                sb_data[str(sid)] = {
                    "name": slot.name,
                    "file": slot.file_path,
                    "volume": slot.volume
                }
        self.settings["soundboard"] = sb_data
        self._save_settings()

    def _right(self, parent):
        r=ctk.CTkFrame(parent,fg_color=P["card"],corner_radius=14,border_width=1,border_color=P["border"])
        r.grid(row=0,column=1,sticky="nsew",padx=(6,0)); r.grid_columnconfigure(0,weight=1)
        ctk.CTkLabel(r,text=f"Output → {SONIXX} Virtual Mic",font=("Segoe UI Semibold",14),
                     text_color=P["txt"]).grid(row=0,column=0,sticky="w",padx=16,pady=(16,4))
        cable=get_cable_output_device(self.pa); self._cable_dev=cable
        cn=cable["name"] if cable else "Not found"
        ctk.CTkLabel(r,text=f"Target: {cn}",font=("Segoe UI",11),
                     text_color=P["green"] if cable else P["red"]).grid(row=1,column=0,sticky="w",padx=20,pady=(0,2))
        ctk.CTkLabel(r,text=f'Select "{SONIXX}" or "CABLE Output" as mic in Valorant/Discord',
                     font=("Segoe UI",10),text_color=P["dim"],wraplength=280,anchor="w").grid(row=2,column=0,sticky="w",padx=20,pady=(0,8))
        self.rename_btn=ctk.CTkButton(r,text=f'Rename to "{SONIXX}" (Admin)',height=28,font=("Segoe UI",11),
                       fg_color=P["card2"],hover_color=P["hover"],text_color=P["dim"],corner_radius=8,command=self._rename)
        self.rename_btn.grid(row=3,column=0,sticky="ew",padx=16,pady=(0,8))
        # Loopback device selector
        ctk.CTkFrame(r,fg_color=P["border"],height=1).grid(row=4,column=0,sticky="ew",padx=12,pady=6)
        ctk.CTkLabel(r,text="Loopback Source",font=("Segoe UI",11),text_color=P["dim"]).grid(row=5,column=0,sticky="w",padx=18,pady=(0,2))
        self.loop_var=ctk.StringVar()
        self.loop_cb=ctk.CTkComboBox(r,variable=self.loop_var,values=["..."],height=30,font=("Segoe UI",11),
                                      fg_color=P["card2"],border_color=P["border"],button_color=P["accent"],
                                      dropdown_fg_color=P["card2"],text_color=P["txt"],corner_radius=8)
        self.loop_cb.grid(row=6,column=0,sticky="ew",padx=16,pady=(0,4))
        ctk.CTkLabel(r,text="ℹ Captures all audio from this output device",font=("Segoe UI",9),
                     text_color=P["muted"]).grid(row=7,column=0,sticky="w",padx=20,pady=(0,6))
        # Loopback & Settings
        ctk.CTkFrame(r,fg_color=P["border"],height=1).grid(row=8,column=0,sticky="ew",padx=12,pady=4)
        lsf=ctk.CTkFrame(r,fg_color="transparent"); lsf.grid(row=9,column=0,sticky="ew",padx=18,pady=(6,2))
        
        self.monitor_switch = ctk.CTkSwitch(lsf, text="Monitor Mix (Hear output)", font=("Segoe UI", 11), text_color=P["dim"],
                                             progress_color=P["accent"], button_color="#fff", button_hover_color="#ddd",
                                             command=self._toggle_monitor)
        self.monitor_switch.pack(side="left")
        
        self.start_sw = ctk.CTkSwitch(lsf, text="Auto-start", font=("Segoe UI", 11), text_color=P["dim"],
                                      command=self._save_settings_ui)
        self.start_sw.pack(side="right")
        if self.settings.get("startup"): self.start_sw.select()

        self.tray_sw = ctk.CTkSwitch(lsf, text="Minimize to Tray", font=("Segoe UI", 11), text_color=P["dim"],
                                     command=self._save_settings_ui)
        self.tray_sw.pack(side="right", padx=(0, 4))
        if self.settings.get("tray"): self.tray_sw.select()

        # Master vol & Mute
        ctk.CTkFrame(r,fg_color=P["border"],height=1).grid(row=10,column=0,sticky="ew",padx=12,pady=4)
        ctk.CTkLabel(r,text="Master Control",font=("Segoe UI",11),text_color=P["dim"]).grid(row=11,column=0,sticky="w",padx=18,pady=(4,2))
        mvf=ctk.CTkFrame(r,fg_color="transparent"); mvf.grid(row=12,column=0,sticky="ew",padx=18); mvf.grid_columnconfigure(0,weight=1)
        
        self.m_slider=ctk.CTkSlider(mvf,from_=0,to=200,number_of_steps=200,fg_color=P["card2"],
                                     progress_color=P["accent"],button_color=P["accent2"],button_hover_color="#fff",command=self._mvol)
        self.m_slider.set(100); self.m_slider.grid(row=0,column=0,sticky="ew")
        self.m_lbl=ctk.CTkLabel(mvf,text="100%",width=40,font=("Segoe UI",11),text_color=P["dim"])
        self.m_lbl.grid(row=0,column=1,padx=(6,0))
        
        self.master_mute_btn = ctk.CTkButton(mvf, text="🔊", width=32, height=32, font=("Segoe UI", 14),
                                            fg_color=P["card2"], hover_color=P["hover"], corner_radius=8, command=self._mute)
        self.master_mute_btn.grid(row=0, column=2, padx=(8, 0))

        # Peak
        self.m_peak=ctk.CTkCanvas(r,height=10,bg=P["card"],highlightthickness=0)
        self.m_peak.grid(row=13,column=0,sticky="ew",padx=18,pady=(8,4))
        

        # Spacer to push buttons to the bottom
        r.grid_rowconfigure(16, weight=1)
        
        # Buttons
        ctk.CTkFrame(r,fg_color=P["border"],height=1).grid(row=16,column=0,sticky="ew",padx=12,pady=8)
        bf=ctk.CTkFrame(r,fg_color="transparent"); bf.grid(row=17,column=0,sticky="ew",padx=14)
        bf.grid_columnconfigure(0,weight=1); bf.grid_columnconfigure(1,weight=1)
        self.start_btn=ctk.CTkButton(bf,text="▶ Start",height=40,font=("Segoe UI Semibold",13),
                                      fg_color=P["accent"],hover_color=P["accent2"],corner_radius=10,command=self._start)
        self.start_btn.grid(row=0,column=0,sticky="ew",padx=(0,4))
        self.stop_btn=ctk.CTkButton(bf,text="■ Stop",height=40,font=("Segoe UI Semibold",13),
                                     fg_color=P["red_d"],hover_color=P["red"],corner_radius=10,command=self._stop,state="disabled")
        self.stop_btn.grid(row=0,column=1,sticky="ew",padx=(4,0))

    def _toggle_monitor(self):
        self.router.monitor_enabled = bool(self.monitor_switch.get())
        if self.router.running:
            if self.router.monitor_enabled:
                self.router.start_monitor()
            else:
                self.router.stop_monitor()

    def _load_devices(self):
        self._mics={}; self._loops={}
        for i in range(self.pa.get_device_count()):
            d=self.pa.get_device_info_by_index(i)
            if d["hostApi"]!=self.wi: continue
            nl=d["name"].lower()
            if d.get("isLoopbackDevice",False) and d["maxInputChannels"]>0:
                self._loops[d["name"]]={"index":i,"name":d["name"],"channels":d["maxInputChannels"],"rate":int(d["defaultSampleRate"])}
            elif not d.get("isLoopbackDevice",False) and d["maxInputChannels"]>0:
                if "cable output" in nl or SONIXX.lower() in nl: continue
                self._mics[d["name"]]={"index":i,"name":d["name"],"channels":d["maxInputChannels"],"rate":int(d["defaultSampleRate"])}
        mn=list(self._mics.keys()) or ["No mics"]; ln=list(self._loops.keys()) or ["No loopback"]
        self.mic_cb.configure(values=mn); self.mic_var.set(mn[0])
        self.loop_cb.configure(values=ln); self.loop_var.set(ln[0])
        self._scan_apps()

    def _scan_apps(self):
        if self._running:
            self._stop()
            
        for w in self.app_scroll.winfo_children(): w.destroy()
        self.app_rows.clear()
        def work():
            import os
            try: 
                my_pid = os.getpid()
                procs = ProcessAudioCapture.enumerate_audio_processes()
                procs = [p for p in procs if 'audiodg' not in p.name.lower() and p.pid != my_pid]
            except: 
                procs = []
            self.after(0,lambda:self._show_apps(procs))
        threading.Thread(target=work,daemon=True).start()

    def _show_apps(self, procs):
        for w in self.app_scroll.winfo_children(): w.destroy()
        if not procs:
            ctk.CTkLabel(self.app_scroll,text="No apps producing audio.\nPlay something and click Scan.",
                         font=("Segoe UI",11),text_color=P["muted"],justify="center").grid(row=0,column=0,pady=20)
            return
        for i,p in enumerate(procs):
            name=friendly(p.name)
            title=p.window_title.strip() if p.window_title.strip() else ""
            label=f"{name}" + (f" — {title[:30]}" if title and title!=name else "")
            row=SourceRow(self.app_scroll,label,"app",on_toggle=self._app_tog,on_vol=self._app_vol,on_rm=self._app_rm)
            row.pid=p.pid; row.source_id=f"app_{p.pid}"
            row.grid(row=i,column=0,sticky="ew",pady=2,padx=4)
            row.animate_in()
            self.app_rows[p.pid]=row
            self.router.add_app(p)

    def _app_tog(self,r): self.router.set_app_active(r.pid, bool(r.tog.get()))
    def _app_vol(self,r,v):
        self.router.set_app_volume(r.pid, v/100.0); r.vlbl.configure(text=f"{int(v)}%")
    def _app_rm(self,r):
        self.router.remove_app(r.pid); r.destroy(); del self.app_rows[r.pid]

    def _add_mic(self):
        dev=self._mics.get(self.mic_var.get())
        if not dev: return
        sid=self.router.add_mic(dev)
        row=SourceRow(self.mic_scroll,dev["name"],"mic",on_toggle=self._mic_tog,on_vol=self._mic_vol,on_rm=self._mic_rm)
        row.source_id=sid; row.grid(row=len(self.mic_rows),column=0,sticky="ew",pady=2,padx=4)
        self.mic_rows[sid]=row

    def _mic_tog(self,r):
        with self.router._lock:
            s=self.router.mic_sources.get(r.source_id)
            if s: s.active=bool(r.tog.get())
    def _mic_vol(self,r,v):
        with self.router._lock:
            s=self.router.mic_sources.get(r.source_id)
            if s: s.volume=v/100.0
        r.vlbl.configure(text=f"{int(v)}%")
    def _mic_rm(self,r):
        self.router.remove_mic(r.source_id); r.destroy(); del self.mic_rows[r.source_id]

    def _start(self):
        if not self._cable_dev: self.foot.configure(text="⚠ VB-Cable not found"); return
        lb=self._loops.get(self.loop_var.get())
        if not lb: self.foot.configure(text="⚠ Select a loopback device"); return
        ok=self.router.start(lb, self._cable_dev)
        if not ok: self.foot.configure(text="⚠ Failed to start"); return
        self._running=True
        self.status_lbl.configure(text="● Routing",text_color=P["green"])
        self.start_btn.configure(state="disabled",fg_color=P["card2"])
        self.stop_btn.configure(state="normal",fg_color=P["red"])
        self.foot.configure(text=f"Routing → {SONIXX}")
        self._update_peaks()

    def _stop(self):
        self.router.stop(); self._running=False
        self.status_lbl.configure(text="● Stopped",text_color=P["red"])
        self.start_btn.configure(state="normal",fg_color=P["accent"])
        self.stop_btn.configure(state="disabled",fg_color=P["red_d"])
        self.foot.configure(text="Stopped")

    def _mvol(self,v): self.router.master_vol=v/100.0; self.m_lbl.configure(text=f"{int(v)}%")
    def _mute(self):
        self.router.master_mute=not self.router.master_mute
        if self.router.master_mute: 
            self.master_mute_btn.configure(text="🔇",fg_color=P["red_d"],text_color=P["red"])
        else: 
            self.master_mute_btn.configure(text="🔊",fg_color=P["card2"],text_color=P["dim"])
    
    def _save_settings_ui(self):
        from app.driver import set_startup
        self.settings["tray"] = bool(self.tray_sw.get())
        self.settings["startup"] = bool(self.start_sw.get())
        self._save_settings()
        set_startup(self.settings["startup"])

    def _rename(self):
        ok,msg=rename_to_sonixx()
        self.rename_btn.configure(text=f"{'✓' if ok else '✕'} {msg}",text_color=P["green"] if ok else P["orange"])

    def _update_peaks(self):
        if not self._running: return
        for pid,r in self.app_rows.items():
            with self.router._lock:
                s=self.router.app_sources.get(pid)
            if s: r.set_peak(s.get_peak_linear())
        for sid,r in self.mic_rows.items():
            with self.router._lock:
                s=self.router.mic_sources.get(sid)
            if s: r.set_peak(s.get_peak())
        w=self.m_peak.winfo_width()
        self.m_peak.delete("all"); self.m_peak.create_rectangle(0,0,w,10,fill=P["card2"],outline="")
        bw=int(w*min(self.router.mix_peak,1.0))
        if bw>0:
            c=P["green"] if self.router.mix_peak<0.7 else (P["orange"] if self.router.mix_peak<0.9 else P["red"])
            self.m_peak.create_rectangle(0,0,bw,10,fill=c,outline="")
        self._peak_job=self.after(50,self._update_peaks)

    def on_close(self):
        self._running=False
        if self._peak_job: self.after_cancel(self._peak_job)
        self.router.cleanup()
        try: self.pa.terminate()
        except: pass
        self.destroy()
