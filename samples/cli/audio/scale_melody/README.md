# scale_melody

Synthesizes a two-second C-major scale — eight square-wave notes of 2000
samples each (C4 D4 E4 F4 G4 A4 B4 C5) — as unsigned 8-bit mono PCM at
8000 Hz, and writes it to `scale_melody.wav` — one fixed `[u8; 16044]` buffer
holding the canonical 44-byte RIFF header plus 16000 samples, written once
through the portable `WavOutput` binding in `../wav_output`, path relative to
the current directory. `WavOutput` is satisfied per target over the raw
filesystem family: `PosixFilesystemHost` on linux/macOS,
`WindowsFilesystemHost` (CreateFileW) on windows_x86_64.

After the WAV is written the program plays it on the host speaker through
the portable `AudioPlayback` binding in `../audio_playback`, which blocks
until playback finishes: winmm `PlaySoundW` (`SND_MEMORY|SND_NODEFAULT`) on
windows_x86_64, ALSA (`snd_pcm_*` via `libasound.so.2`) on linux_x86_64 and
linux_arm64. macOS has no CoreAudio provider yet — the WAV is still written
and playback reports a failure there.

Run from this directory on Windows, Linux, or macOS:

```sh
omega update --project .   # then complete the review it requests
omega run main.omg
```

`run` with an explicit non-host target compiles without executing; invoke the
kept `build dir`/`omega-program` binary from this directory on that platform
(e.g. `omega run --target linux_x86_64 --keep main.omg` on Windows, then run
the ELF under WSL).
Expected exit: `0` after printing `played scale_melody.wav`; `1` on a write
error, `2` on a playback failure (the WAV is still written). Verify with `python ../verify_wav.py scale_melody.wav --blocks=2000`
— expect `channels=1 sampwidth=1 rate=8000 frames=16000` and eight
estimates near 8000/31, 8000/27, 8000/24, 8000/23, 8000/20, 8000/18,
8000/16, 8000/15 Hz. Any media player opens the WAV directly.
