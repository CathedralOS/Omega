# sine_tone

Synthesizes one second of a ~440 Hz sine wave as unsigned 8-bit mono PCM at
8000 Hz, and writes it to `sine_tone.wav` — one fixed `[u8; 8044]` buffer
holding the canonical 44-byte RIFF header plus 8000 samples, written once
through the portable `WavOutput` binding in `../wav_output`, path relative to
the current directory. `WavOutput` is satisfied per target over the raw
filesystem family: `PosixFilesystemHost` on linux/macOS,
`WindowsFilesystemHost` (CreateFileW) on windows_x86_64.

The oscillator is a 16.16 fixed-point phase accumulator (increment 115343)
stepping through a 32-entry one-cycle table centered on 128 with amplitude
100. The table is a value-position `match` on the wrapped index rather than a
dynamic array subscript, so the read needs no bounds proof.

Run from this directory on Windows, Linux, or macOS:

```sh
omega update --project .   # then complete the review it requests
omega run main.omg
```

`run` with an explicit non-host target compiles without executing; invoke the
kept `build dir`/`omega-program` binary from this directory on that platform
(e.g. `omega run --target linux_x86_64 --keep main.omg` on Windows, then run
the ELF under WSL).
Expected exit: `0` after printing `wrote sine_tone.wav`; `1` on a write
error. Verify with `python ../verify_wav.py sine_tone.wav` — expect
`channels=1 sampwidth=1 rate=8000 frames=8000` and an estimate near 440 Hz.
Any media player opens the WAV directly.
