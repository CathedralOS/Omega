# Structural array-element stores

This program replaces a runtime-selected record element with an owned parameter,
then stores a locally constructed record and a payloadless case at fixed indices.
It must compile and exit 0; it does not independently observe the stored contents.
The stage tests verify exact source identity, field extent, ABI transport and
rejection of substituted values.

After building `omega`, prepare this checkout's package review and run on macOS:

```sh
python3 tools/benchmark/benchmark.py prepare --root tests/omega/pass/structural/element_store/main.omg --target macos_arm64 --omega target/release/omega
target/release/omega run --both tests/omega/pass/structural/element_store/main.omg
```

For Windows use `python`, `--target windows_x86_64`, and
`target/release/omega.exe`. Omit `--target` from `omega run`: an explicit target
requests compilation only. Native execution is also enforced by the corpus gate:

```sh
python3 tools/corpus_gate.py --native --filter structural/element_store
```
