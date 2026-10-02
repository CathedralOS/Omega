# Print number

From the repository root, compile and execute on the host:

```text
mbx run -p omega -- run samples/cli/basics/print_number/main.omg
```

Use Cargo when `mbx` is unavailable. Prepare and review a fresh project through
the ordinary [package commands](../../../../omega-rust/omega/application/cli/README.md#command-surface).

The program sums `[10, 20, 30, 40, 50]`, converts the result into decimal bytes,
and must print a standalone `150` line and exit 0. The closing banner also
contains `150`; that banner alone does not verify the computation. Press Enter
at the final prompt, or supply EOF for a noninteractive run.

Add `--both` after `run` to execute the native and checked-interpreter routes.
