# Atomic case input

Run on macOS ARM64:

```sh
omega run --both main.omg < input.txt
```

A returned empty case keeps ordinary placed storage across a helper loan.
Eight input bytes select a declared atomic case and establish its original
cell. A subsequent atomic event observes the new value. Shared output may
observe the tag but not snapshot the atomic resident; exclusive output can
observe both. The input's final newline is the counter's high byte, not an
extra byte after the read. Expected exit is zero in both engines; stdout is
eight zero bytes followed by `A\n`.
