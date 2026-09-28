# Trapping overflow

This program sets an `i32 in Trapping` to its maximum and increments it.
Successful execution means the overflow traps; reaching `exit_process(70)`
is a failure. The build binds `Main::main` for all four hosted targets.

From the repository root, after completing the ordinary package review:

```sh
omega run samples/cli/probes/trapping_probe/main.omg
```

The command is the same in PowerShell. Expected native termination:

- macOS AArch64 and Linux AArch64: `SIGTRAP` (signal 5).
- Linux x86-64: `SIGILL` (signal 4).
- Windows x86-64: `STATUS_ILLEGAL_INSTRUCTION` (`0xC000001D`).

There is no program stdout. Compiler warnings that the computation always
overflows are expected. On Unix, the CLI reports `native signal 5` or
`native signal 4` and itself exits 255. That CLI exit alone is not the oracle:
check the native signal, not just a nonzero status.

The sample gate currently reports this program as `no-oracle` after building
and running it because it only scores ordinary exit/output annotations. The
matching language regression has an explicit `trap` oracle:

```sh
python3 tools/corpus_gate.py --native --filter arithmetic/runtime_trapping_overflow_traps
```

Use `python` in PowerShell when that is the Python 3 executable.
