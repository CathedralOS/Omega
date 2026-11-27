# Trapping overflow

This program sets an `i32 in Trapping` to its maximum and increments it.
Successful execution means the overflow traps; reaching `exit_process(70)`
is a failure. The build binds `Main::main` for all four hosted targets.

From the repository root, after completing the ordinary package review:

```sh
omega run samples/cli/probes/trapping_probe/main.omg
```

The interpreter must report a crash and exit 203. To verify the compiled
trap separately, build with `omega samples/cli/probes/trapping_probe/main.omg`,
launch the published image and check the host process status:

- macOS AArch64 and Linux AArch64: `SIGTRAP` (signal 5).
- Linux x86-64: `SIGILL` (signal 4).
- Windows x86-64: `STATUS_ILLEGAL_INSTRUCTION` (`0xC000001D`).

There is no program stdout. Compiler warnings that the computation always
overflows are expected. On Unix, check the signal delivered to the published program.

The sample gate currently reports this program as `no-oracle` after building
and running it because it only scores ordinary exit/output annotations.

Use `python` in PowerShell when that is the Python 3 executable.
