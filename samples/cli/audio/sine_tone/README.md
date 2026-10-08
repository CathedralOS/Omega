# sine_tone

Synthesizes one second of a ~440 Hz sine wave as unsigned 8-bit mono PCM at
8000 Hz, and writes it to `sine_tone.wav` — one fixed `[u8; 8044]` buffer
holding the canonical 44-byte RIFF header plus 8000 samples, written once
through the raw `PosixFilesystemHost` seam (`create` + `write` + `close`),
path relative to the current directory.

The oscillator is a 16.16 fixed-point phase accumulator (increment 115343)
stepping through a 32-entry one-cycle table centered on 128 with amplitude
100. The table is a value-position `match` on the wrapped index rather than a
dynamic array subscript, so the read needs no bounds proof.

The portable `Filesystem` adapter's `FilesystemHost` boundary has no realized
provider on this toolchain yet (the minted realization table covers only
scalar linux_x86_64 leaves, and no Windows provider exists), so this sample
binds the POSIX hosted entries only, like the `native_fs_*` canaries.

Run from this directory:

```sh
omega update --project . --target linux_x86_64   # then complete the review it requests
omega --check --target linux_x86_64 main.omg
omega run --target linux_x86_64 --keep main.omg
```

`run` with an explicit non-host target compiles without executing; invoke the
kept `build dir`/`omega-program` ELF from this directory on Linux (or WSL).
Expected exit: `0` after printing `wrote sine_tone.wav`; `1` on a write
error. Verify with `python ../verify_wav.py sine_tone.wav` — expect
`channels=1 sampwidth=1 rate=8000 frames=8000` and an estimate near 440 Hz.
Any media player opens the WAV directly.
