"""Audio Router - Captures app audio via WASAPI loopback, outputs to virtual cable."""
import threading, queue, time, os, tempfile
import numpy as np
import pyaudiowpatch as pyaudio
from process_audio_capture import ProcessAudioCapture, AudioProcess, PacCaptureMode

CHUNK = 1024
FMT = pyaudio.paFloat32


class AppAudioSource:
    """Captures audio from a specific app by writing to a named pipe and streaming it.
    Also provides level monitoring via the PAC DLL."""

    def __init__(self, proc: AudioProcess):
        self.proc = proc
        self.pid = proc.pid
        self.name = proc.name
        self.display = proc.window_title or proc.name.replace(".exe", "")
        self.volume = 1.0
        self.active = False
        self.muted = False
        self.peak_db = -60.0
        self._capture = None
        self._level_lock = threading.Lock()
        self._lock = threading.Lock()
        self.buffer = None
        self.pipe_name = fr'\\.\pipe\sonixx_app_{self.pid}_{id(self)}'
        self._pipe = None
        self._pipe_thread = None
        self._running = False
        self._buffering = True

    def start_monitoring(self):
        """Start per-process level monitoring and capture via PAC DLL to a named pipe."""
        import ctypes
        from ctypes.wintypes import DWORD, LPCWSTR, LPVOID, HANDLE

        self._running = True
        kernel32 = ctypes.windll.kernel32
        PIPE_ACCESS_INBOUND = 1
        PIPE_TYPE_BYTE = 0
        PIPE_WAIT = 0
        
        # Create named pipe
        self._pipe = kernel32.CreateNamedPipeW(
            self.pipe_name, PIPE_ACCESS_INBOUND, PIPE_TYPE_BYTE | PIPE_WAIT,
            1, 65536, 65536, 0, None)
            
        self._q = queue.Queue(maxsize=50)

        def pipe_reader():
            kernel32.ConnectNamedPipe(self._pipe, None)
            buf = ctypes.create_string_buffer(8192)
            read = DWORD()
            # Read and discard WAV header (usually 68 bytes)
            kernel32.ReadFile(self._pipe, buf, 68, ctypes.byref(read), None)
            
            byte_buffer = bytearray()
            while self._running:
                res = kernel32.ReadFile(self._pipe, buf, 8192, ctypes.byref(read), None)
                if not res or read.value == 0:
                    time.sleep(0.005)
                    continue
                
                byte_buffer.extend(buf[:read.value])
                
                # Extract exact chunks of 8192 bytes (1024 stereo 32-bit float frames)
                while len(byte_buffer) >= 8192:
                    chunk = byte_buffer[:8192]
                    del byte_buffer[:8192]
                    
                    audio_data = np.frombuffer(chunk, dtype=np.float32).copy()
                    try:
                        self._q.put_nowait(audio_data)
                    except queue.Full:
                        try:
                            self._q.get_nowait()
                            self._q.put_nowait(audio_data)
                        except Exception:
                            pass

        self._pipe_thread = threading.Thread(target=pipe_reader, daemon=True)
        self._pipe_thread.start()

        try:
            def on_level(db):
                with self._level_lock:
                    self.peak_db = db
            self._capture = ProcessAudioCapture(
                pid=self.pid, output_path=self.pipe_name,
                mode=PacCaptureMode.INCLUDE, level_callback=on_level)
            self._capture.start()
        except Exception as e:
            print(f"[AppSource] Capture failed for {self.name}: {e}")
            self._capture = None

    def read(self):
        with self._lock:
            if not self.active or self.muted:
                # Discard queue to prevent backlog while muted
                while not self._q.empty():
                    try: self._q.get_nowait()
                    except: break
                self._buffering = True
                return None
        
        # Jitter buffer logic: pre-buffer 10 chunks (~210ms)
        if self._buffering:
            if self._q.qsize() >= 10:
                self._buffering = False
            else:
                return np.zeros(2048, dtype=np.float32)

        try:
            data = self._q.get_nowait()
            return data * self.volume
        except queue.Empty:
            # Underrun occurred, resume buffering
            self._buffering = True
            return np.zeros(2048, dtype=np.float32)

    def get_peak_linear(self):
        """Get peak level as 0.0-1.0 linear scale."""
        with self._level_lock:
            if not self.active or self.muted:
                return 0.0
            db = self.peak_db
        if db <= -60:
            return 0.0
        return min(1.0, 10 ** (db / 20.0))

    def stop(self):
        self._running = False
        if self._capture:
            try:
                self._capture.stop()
            except Exception:
                pass
            self._capture = None
        import ctypes
        if self._pipe:
            ctypes.windll.kernel32.CloseHandle(self._pipe)
            self._pipe = None


class MicSource:
    """Microphone input via PyAudio WASAPI."""

    def __init__(self, dev_info, pa):
        self.dev = dev_info
        self.pa = pa
        self.volume = 1.0
        self.active = False
        self.muted = False
        self.stream = None
        self.buffer = None
        self.peak = 0.0
        self.ch = min(self.dev["channels"], 2)
        self._lock = threading.Lock()
        self._q = queue.Queue(maxsize=50)
        self._buffering = True

    def start(self):
        rate = self.dev["rate"]
        try:
            self.stream = self.pa.open(
                format=FMT, channels=self.ch, rate=rate, input=True,
                input_device_index=self.dev["index"],
                frames_per_buffer=CHUNK, stream_callback=self._cb)
            self.stream.start_stream()
        except Exception as e:
            print(f"[MicSource] Failed: {e}")

    def _cb(self, in_data, frame_count, time_info, status):
        audio = np.frombuffer(in_data, dtype=np.float32).copy()
        
        with self._lock:
            self.peak = float(np.max(np.abs(audio))) if len(audio) > 0 else 0.0
            
        # Convert mono to stereo correctly if needed
        if self.ch == 1:
            stereo = np.empty(len(audio) * 2, dtype=np.float32)
            stereo[0::2] = audio
            stereo[1::2] = audio
            audio = stereo

        try:
            self._q.put_nowait(audio)
        except queue.Full:
            try:
                self._q.get_nowait()
                self._q.put_nowait(audio)
            except Exception:
                pass
                
        return (None, pyaudio.paContinue)

    def read(self):
        with self._lock:
            if not self.active or self.muted:
                while not self._q.empty():
                    try: self._q.get_nowait()
                    except: break
                self._buffering = True
                return None

        # Jitter buffer logic: pre-buffer 10 chunks (~210ms)
        if self._buffering:
            if self._q.qsize() >= 10:
                self._buffering = False
            else:
                return np.zeros(2048, dtype=np.float32)

        try:
            data = self._q.get_nowait()
            return data * self.volume
        except queue.Empty:
            self._buffering = True
            return np.zeros(2048, dtype=np.float32)

    def get_peak(self):
        with self._lock:
            return self.peak * self.volume if self.active and not self.muted else 0.0

    def stop(self):
        if self.stream:
            try:
                self.stream.stop_stream()
                self.stream.close()
            except Exception:
                pass
            self.stream = None


class AudioRouter:
    """Mixes mic inputs + loopback capture → virtual cable output."""

    def __init__(self, pa):
        self.pa = pa
        self.mic_sources: dict[str, MicSource] = {}
        self.app_sources: dict[int, AppAudioSource] = {}  # pid -> source
        self.loopback_stream = None
        self.output_stream = None
        self.running = False
        self.master_vol = 1.0
        self.master_mute = False
        self.mix_peak = 0.0
        self._q = queue.Queue(maxsize=40)
        self._thread = None
        self._lock = threading.Lock()
        self._loopback_buf = None
        self._loopback_peak = 0.0
        self._sid = 0
        self.monitor_enabled = False
        self.monitor_stream = None
        self._mon_byte_buf = bytearray()
        self._mon_lock = threading.Lock()
        self.loopback_dev = None
        self.output_dev = None
        self.mon_rate = 48000
        self.mics_mute = False
        self.apps_mute = False

    # ── Mic sources ──────────────────────────────────────────────────────
    def add_mic(self, dev_info) -> str:
        self._sid += 1
        sid = f"mic_{self._sid}"
        src = MicSource(dev_info, self.pa)
        with self._lock:
            self.mic_sources[sid] = src
        if self.running:
            src.start()
        return sid

    def remove_mic(self, sid):
        with self._lock:
            src = self.mic_sources.pop(sid, None)
        if src:
            src.stop()

    # ── App sources ──────────────────────────────────────────────────────
    def add_app(self, proc: AudioProcess):
        """Add an app for monitoring. Audio captured via loopback."""
        src = AppAudioSource(proc)
        with self._lock:
            self.app_sources[proc.pid] = src
        if self.running:
            src.start_monitoring()

    def remove_app(self, pid: int):
        with self._lock:
            src = self.app_sources.pop(pid, None)
        if src:
            src.stop()

    def set_app_volume(self, pid, vol):
        with self._lock:
            if pid in self.app_sources:
                self.app_sources[pid].volume = max(0.0, min(2.0, vol))

    def set_app_active(self, pid, on):
        with self._lock:
            if pid in self.app_sources:
                self.app_sources[pid].active = on

    def set_app_muted(self, pid, muted):
        with self._lock:
            if pid in self.app_sources:
                self.app_sources[pid].muted = muted

    # ── Monitor Stream Management ────────────────────────────────────────
    def start_monitor(self):
        """Start streaming monitor mix audio to physical headphones/speakers."""
        self.stop_monitor()
        if not self.running or not self.output_dev:
            return False

        with self._mon_lock:
            self._mon_byte_buf.clear()

        # 1. Try matching the selected loopback device to its physical playback counterpart
        mon_dev_idx = None
        mon_dev_name = None

        if self.loopback_dev and "name" in self.loopback_dev:
            lb_name = self.loopback_dev["name"]
            clean_name = (lb_name.replace(" [Loopback]", "")
                                 .replace(" (loopback)", "")
                                 .replace("[Loopback]", "")
                                 .replace("(loopback)", "")
                                 .strip())
            
            for i in range(self.pa.get_device_count()):
                d = self.pa.get_device_info_by_index(i)
                if d["maxOutputChannels"] > 0:
                    d_name = d["name"]
                    if clean_name.lower() in d_name.lower() or d_name.lower() in clean_name.lower():
                        if "cable" not in d_name.lower() or "cable" in clean_name.lower():
                            mon_dev_idx = i
                            mon_dev_name = d_name
                            break

        # 2. Fallback: check WASAPI default output device (if not VB-Cable)
        if mon_dev_idx is None:
            wasapi_idx = -1
            for i in range(self.pa.get_host_api_count()):
                if "WASAPI" in self.pa.get_host_api_info_by_index(i)["name"]:
                    wasapi_idx = i
                    break
            if wasapi_idx != -1:
                api_info = self.pa.get_host_api_info_by_index(wasapi_idx)
                def_out = api_info["defaultOutputDevice"]
                if def_out >= 0:
                    d = self.pa.get_device_info_by_index(def_out)
                    d_name = d["name"].lower()
                    if "cable" not in d_name and "sonixx" not in d_name:
                        mon_dev_idx = def_out
                        mon_dev_name = d["name"]

        # 3. Fallback: first non-cable WASAPI physical output device
        if mon_dev_idx is None:
            for i in range(self.pa.get_device_count()):
                d = self.pa.get_device_info_by_index(i)
                if d["maxOutputChannels"] > 0 and "cable" not in d["name"].lower() and "sonixx" not in d["name"].lower():
                    mon_dev_idx = i
                    mon_dev_name = d["name"]
                    break

        if mon_dev_idx is None:
            print("[Router] No suitable playback device found for Monitor Mix.")
            return False

        try:
            mon_info = self.pa.get_device_info_by_index(mon_dev_idx)
            self.mon_rate = int(mon_info["defaultSampleRate"])
            out_ch = min(self.output_dev["channels"], 2)

            print(f"[Router] Starting Monitor Mix on '{mon_dev_name}' (idx={mon_dev_idx}, rate={self.mon_rate}Hz)")
            self.monitor_stream = self.pa.open(
                format=FMT, channels=out_ch, rate=self.mon_rate, output=True,
                output_device_index=mon_dev_idx, frames_per_buffer=CHUNK,
                stream_callback=self._monitor_cb)
            self.monitor_stream.start_stream()
            self.monitor_enabled = True
            return True
        except Exception as e:
            print(f"[Router] Monitor start failed on '{mon_dev_name}': {e}")
            self.monitor_stream = None
            return False

    def stop_monitor(self):
        """Stop the monitor mix playback stream."""
        if self.monitor_stream:
            try:
                self.monitor_stream.stop_stream()
                self.monitor_stream.close()
            except Exception:
                pass
            self.monitor_stream = None
        with self._mon_lock:
            self._mon_byte_buf.clear()

    # ── Start / Stop Router ──────────────────────────────────────────────
    def start(self, loopback_dev, output_dev):
        self.stop()
        self.running = True
        self.loopback_dev = loopback_dev
        self.output_dev = output_dev

        # Start mic sources
        with self._lock:
            for src in self.mic_sources.values():
                src.start()
            for src in self.app_sources.values():
                src.start_monitoring()

        # Open loopback capture (system audio)
        lb = loopback_dev
        lb_ch = min(lb["channels"], 2)
        try:
            self.loopback_stream = self.pa.open(
                format=FMT, channels=lb_ch, rate=lb["rate"], input=True,
                input_device_index=lb["index"], frames_per_buffer=CHUNK,
                stream_callback=self._lb_cb)
            self.loopback_stream.start_stream()
        except Exception as e:
            print(f"[Router] Loopback failed: {e}")
            self.running = False
            return False

        # Open output to virtual cable
        out_ch = min(output_dev["channels"], 2)
        try:
            self.output_stream = self.pa.open(
                format=FMT, channels=out_ch, rate=output_dev["rate"], output=True,
                output_device_index=output_dev["index"], frames_per_buffer=CHUNK,
                stream_callback=self._out_cb)
            self.output_stream.start_stream()
        except Exception as e:
            print(f"[Router] Output failed: {e}")
            self.running = False
            return False

        # Open monitor stream if enabled
        if self.monitor_enabled:
            self.start_monitor()

        return True

    def _monitor_cb(self, in_data, frame_count, time_info, status):
        needed = frame_count * 2 * 4  # 2 channels, 4 bytes per float32 sample
        with self._mon_lock:
            if len(self._mon_byte_buf) >= needed:
                chunk = bytes(self._mon_byte_buf[:needed])
                del self._mon_byte_buf[:needed]
            else:
                avail = len(self._mon_byte_buf)
                chunk = bytes(self._mon_byte_buf) + b'\x00' * (needed - avail)
                self._mon_byte_buf.clear()
        return (chunk, pyaudio.paContinue)

    def _lb_cb(self, in_data, frame_count, time_info, status):
        audio = np.frombuffer(in_data, dtype=np.float32).copy()
        self._loopback_buf = audio
        self._loopback_peak = float(np.max(np.abs(audio))) if len(audio) > 0 else 0.0
        return (None, pyaudio.paContinue)

    def _out_cb(self, in_data, frame_count, time_info, status):
        if not self.running:
            return (b'\x00' * (frame_count * 2 * 4), pyaudio.paComplete)
            
        mixed = None
        
        # Add app sources (if apps not muted)
        if not self.apps_mute:
            with self._lock:
                app_list = list(self.app_sources.values())
            for src in app_list:
                buf = src.read()
                if buf is not None:
                    if mixed is None:
                        mixed = buf.copy()
                    else:
                        ml = min(len(mixed), len(buf))
                        mixed[:ml] += buf[:ml]

        # Add mic sources (if mics not muted)
        if not self.mics_mute:
            with self._lock:
                mic_list = list(self.mic_sources.values())
            for src in mic_list:
                buf = src.read()
                if buf is not None:
                    if mixed is None:
                        mixed = buf.copy()
                    else:
                        ml = min(len(mixed), len(buf))
                        mixed[:ml] += buf[:ml]

        if mixed is None:
            mixed = np.zeros(frame_count * 2, dtype=np.float32)

        if self.master_mute:
            mixed[:] = 0
        else:
            mixed *= self.master_vol
            
        np.clip(mixed, -1.0, 1.0, out=mixed)
        self.mix_peak = float(np.max(np.abs(mixed)))

        expected = frame_count * 2 * 4
        out_bytes = mixed.tobytes()
        if len(out_bytes) < expected:
            out_bytes += b'\x00' * (expected - len(out_bytes))
            
        if self.monitor_enabled and self.monitor_stream:
            out_rate = self.output_dev["rate"] if self.output_dev else 48000
            mon_rate = getattr(self, "mon_rate", out_rate)
            
            if mon_rate != out_rate:
                ratio = mon_rate / out_rate
                n_in = len(mixed) // 2
                n_out = int(round(n_in * ratio))
                if n_in > 0 and n_out > 0:
                    x_in = np.linspace(0, 1, n_in, endpoint=False)
                    x_out = np.linspace(0, 1, n_out, endpoint=False)
                    left_out = np.interp(x_out, x_in, mixed[0::2])
                    right_out = np.interp(x_out, x_in, mixed[1::2])
                    mon_mixed = np.empty(n_out * 2, dtype=np.float32)
                    mon_mixed[0::2] = left_out
                    mon_mixed[1::2] = right_out
                    np.clip(mon_mixed, -1.0, 1.0, out=mon_mixed)
                    mon_bytes = mon_mixed.tobytes()
                else:
                    mon_bytes = out_bytes[:expected]
            else:
                mon_bytes = out_bytes[:expected]

            with self._mon_lock:
                if len(self._mon_byte_buf) > 65536:
                    del self._mon_byte_buf[:32768]
                self._mon_byte_buf.extend(mon_bytes)
            
        return (out_bytes[:expected], pyaudio.paContinue)

    def stop(self):
        self.running = False
        self.stop_monitor()
        if self._thread and self._thread.is_alive():
            self._thread.join(timeout=2)
        with self._lock:
            for s in self.mic_sources.values():
                s.stop()
            for s in self.app_sources.values():
                s.stop()
        for stream in [self.loopback_stream, self.output_stream]:
            if stream:
                try:
                    stream.stop_stream()
                    stream.close()
                except Exception:
                    pass
        self.loopback_stream = None
        self.output_stream = None
        while not self._q.empty():
            try:
                self._q.get_nowait()
            except queue.Empty:
                break
        self.mix_peak = 0.0

    def cleanup(self):
        self.stop()
        with self._lock:
            self.mic_sources.clear()
            self.app_sources.clear()

