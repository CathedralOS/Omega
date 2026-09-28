# Checked time arithmetic

The unchanged program observes all eight Instant/SystemTime arithmetic legs
listed in `main.omg`: carry, borrow, signed epoch arithmetic and overflow.
Success exits 70; a failure exits with its leg number. No host clock is read.

The repeated result-field replacements also exercise borrowed-storage restoration
across branching control flow. Displaced unrestricted values carry no disposal
debt after the field is repaired. Their availability still requires the exact
producer to dominate a use; affine values retain their disposal obligations.

After building `omega`, prepare package review and execute both engines on macOS:

```sh
python3 tools/benchmark/benchmark.py prepare --root tests/omega/pass/time/runtime_checked_time_arith_exit/main.omg --target macos_arm64 --omega target/release/omega
target/release/omega run --both tests/omega/pass/time/runtime_checked_time_arith_exit/main.omg
```

For Windows use `python`, `windows_x86_64`, and `target/release/omega.exe`.
Omit `--target` from `omega run` so it executes on the matching host rather
than only producing a cross-target artifact.

The maintained runtime regression is:

```sh
python3 tools/corpus_gate.py --native --filter time/runtime_checked_time_arith_exit
```

`expected_native_exit.txt` enforces the exit-70 result before golden recording.
