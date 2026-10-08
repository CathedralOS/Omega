# square_tone

Synthesizes one second of a ~444 Hz square wave (18-sample period: 9 samples
high, 9 low) as unsigned 8-bit mono PCM at 8000 Hz, and writes it to
`square_tone.wav` — one fixed `[u8; 8044]` buffer holding the canonical
44-byte RIFF header plus 8000 samples, written once through the raw
`PosixFilesystemHost` seam (`create` + `write` + `close`), path relative to
the current directory.

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
Expected exit: `0` after printing `wrote square_tone.wav`; `1` on a write
error. Verify with `python ../verify_wav.py square_tone.wav` — expect
`channels=1 sampwidth=1 rate=8000 frames=8000` and an estimate near 444 Hz.
Any media player opens the WAV directly.
