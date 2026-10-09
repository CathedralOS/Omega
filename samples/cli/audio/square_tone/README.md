# square_tone

Synthesizes one second of a ~444 Hz square wave (18-sample period: 9 samples
high, 9 low) as unsigned 8-bit mono PCM at 8000 Hz, and writes it to
`square_tone.wav` — one fixed `[u8; 8044]` buffer holding the canonical
44-byte RIFF header plus 8000 samples, written once through the portable
`WavOutput` binding in `../wav_output`, path relative to the current
directory. `WavOutput` is satisfied per target over the raw filesystem
family: `PosixFilesystemHost` on linux/macOS, `WindowsFilesystemHost`
(CreateFileW) on windows_x86_64.

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
Expected exit: `0` after printing `played square_tone.wav`; `1` on a write
error, `2` on a playback failure (the WAV is still written). Verify with `python ../verify_wav.py square_tone.wav` — expect
`channels=1 sampwidth=1 rate=8000 frames=8000` and an estimate near 444 Hz.
Any media player opens the WAV directly.
