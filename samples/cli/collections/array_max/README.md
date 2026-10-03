# Array maximum

The program scans `[4, 15, 8, 42, 16, 23]` through a shared fixed-array
parameter carried around a runtime-indexed state loop. Its maximum is 42,
not the final element. It prints three lines, accepts Enter or EOF, and
exits 42.

After ordinary package review:

```sh
cargo run -p omega -- run --both samples/cli/collections/array_max/main.omg
```

On macOS ARM64 at `57334c2f117`, the public command reaches pending package
decisions and exits 200 after 108.82 seconds. No decisions were accepted
by the compiler repair. Complete the generated review before claiming this
ordinary sample invocation runs.

The registered regression uses the byte-identical sample body with equivalent
Build roots and an exact stdout/exit oracle:

```sh
python3 tools/corpus_gate.py --both --filter collections/runtime_shared_fixed_array_cycle_exit
```

That route uses disposable test admission, not a dependency audit. On macOS
ARM64 both native execution and checked interpretation produce exit 42 and
the complete expected stdout. Other hosts remain unverified.
