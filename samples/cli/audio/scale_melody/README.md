# scale_melody

Synthesizes a two-second C-major scale — eight square-wave notes of 2000
samples each (C4 D4 E4 F4 G4 A4 B4 C5) — as unsigned 8-bit mono PCM at
8000 Hz, and writes it to `scale_melody.wav` — one fixed `[u8; 16044]` buffer
holding the canonical 44-byte RIFF header plus 16000 samples, written once
through the raw `PosixFilesystemHost` seam (`create` + `write` + `close`),
path relative to the current directory.

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
Expected exit: `0` after printing `wrote scale_melody.wav`; `1` on a write
error. Verify with `python ../verify_wav.py scale_melody.wav --blocks=2000`
— expect `channels=1 sampwidth=1 rate=8000 frames=16000` and eight
estimates near 8000/31, 8000/27, 8000/24, 8000/23, 8000/20, 8000/18,
8000/16, 8000/15 Hz. Any media player opens the WAV directly.
