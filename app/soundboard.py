"""Soundboard Module — Pre-decoded audio slots and real-time sound effect mixing."""
import os, shutil, threading
import numpy as np
import miniaudio

APP_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SOUNDS_DIR = os.path.join(APP_DIR, "sounds")
os.makedirs(SOUNDS_DIR, exist_ok=True)


class SoundSlot:
    """Represents a single soundboard slot (1..9)."""

    def __init__(self, slot_id: int, name: str = "", file_path: str = "", volume: float = 1.0):
        self.slot_id = slot_id
        self.name = name or f"Sound #{slot_id}"
        self.file_path = file_path
        self.volume = max(0.0, min(2.0, volume))
        self.buffer = None  # Float32 stereo numpy array at 48000Hz
        self.lock = threading.Lock()

        if self.file_path and os.path.exists(self.file_path):
            self.load_file(self.file_path)

    def load_file(self, path: str) -> bool:
        """Decode audio file (MP3, WAV, FLAC, OGG) to 48kHz stereo Float32 NumPy array."""
        if not path or not os.path.exists(path):
            return False

        # Copy to managed local sound directory so user can delete/move original file
        try:
            abs_path = os.path.abspath(path)
            abs_managed = os.path.abspath(SOUNDS_DIR)
            if not abs_path.startswith(abs_managed):
                ext = os.path.splitext(path)[1]
                target_path = os.path.join(SOUNDS_DIR, f"slot_{self.slot_id}{ext}")
                import shutil
                shutil.copy2(path, target_path)
                path = target_path
        except Exception as e:
            print(f"[SoundSlot] Copy warning for '{path}': {e}")

        try:
            decoded = miniaudio.decode_file(
                path,
                output_format=miniaudio.SampleFormat.SIGNED16,
                nchannels=2,
                sample_rate=48000
            )
            raw_pcm = np.frombuffer(decoded.samples, dtype=np.int16)
            pcm_float = raw_pcm.astype(np.float32) / 32768.0

            with self.lock:
                self.file_path = path
                if not self.name or self.name.startswith("Sound #"):
                    self.name = os.path.splitext(os.path.basename(path))[0].title()
                self.buffer = pcm_float
            return True
        except Exception as e:
            print(f"[SoundSlot] Failed to decode '{path}': {e}")
            with self.lock:
                self.buffer = None
            return False

    def clear(self):
        with self.lock:
            self.file_path = ""
            self.buffer = None


class SoundboardManager:
    """Manages 9 soundboard slots and thread-safe mixing into the output audio stream."""

    def __init__(self):
        self.slots: dict[int, SoundSlot] = {i: SoundSlot(i) for i in range(1, 10)}
        self.playbacks = []  # list of dicts: {"slot_id": i, "buffer": arr, "cursor": int, "volume": float}
        self.lock = threading.Lock()

    def get_slot(self, slot_id: int) -> SoundSlot:
        return self.slots.get(slot_id)

    def configure_slot(self, slot_id: int, name: str = "", file_path: str = "", volume: float = 1.0) -> bool:
        slot = self.get_slot(slot_id)
        if not slot:
            return False

        slot.name = name or f"Sound #{slot_id}"
        slot.volume = max(0.0, min(2.0, volume))
        if file_path:
            return slot.load_file(file_path)
        else:
            slot.clear()
            return True

    def trigger_slot(self, slot_id: int) -> bool:
        """Trigger instant playback of a sound slot."""
        slot = self.get_slot(slot_id)
        if not slot:
            return False

        with slot.lock:
            buf = slot.buffer
            vol = slot.volume
            name = slot.name

        if buf is None or len(buf) == 0:
            print(f"[Soundboard] Slot #{slot_id} is empty.")
            return False

        with self.lock:
            # Stop existing playback of same slot if re-triggered
            self.playbacks = [p for p in self.playbacks if p["slot_id"] != slot_id]
            self.playbacks.append({
                "slot_id": slot_id,
                "buffer": buf,
                "cursor": 0,
                "volume": vol
            })

        print(f"[Soundboard] Playing Slot #{slot_id} ('{name}')")
        return True

    def stop_slot(self, slot_id: int):
        with self.lock:
            self.playbacks = [p for p in self.playbacks if p["slot_id"] != slot_id]

    def stop_all(self):
        with self.lock:
            self.playbacks.clear()

    def is_playing(self, slot_id: int) -> bool:
        with self.lock:
            return any(p["slot_id"] == slot_id for p in self.playbacks)

    def mix_audio(self, frame_count: int) -> np.ndarray | None:
        """Mix active soundboard playbacks for frame_count frames. Returns stereo Float32 NumPy array or None."""
        with self.lock:
            if not self.playbacks:
                return None
            current_playbacks = list(self.playbacks)

        samples_needed = frame_count * 2
        mixed = np.zeros(samples_needed, dtype=np.float32)
        finished_slots = []

        for p in current_playbacks:
            buf = p["buffer"]
            cur = p["cursor"]
            vol = p["volume"]

            rem_samples = len(buf) - cur
            take_samples = min(samples_needed, rem_samples)

            if take_samples > 0:
                chunk = buf[cur:cur + take_samples] * vol
                mixed[:take_samples] += chunk
                p["cursor"] += take_samples

            if p["cursor"] >= len(buf):
                finished_slots.append(p["slot_id"])

        if finished_slots:
            with self.lock:
                self.playbacks = [p for p in self.playbacks if p["slot_id"] not in finished_slots]

        return mixed
