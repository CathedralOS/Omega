# Raw descriptor output through the public command

```sh
omega run --both tests/omega/pass/host/runtime_descriptor_binary_output_exit/main.omg
```

On macOS AArch64 both engines exit zero. Native stdout is exactly the four
bytes `FF 00 80 0A`; native stderr begins with `00 FE 81 0A`, followed by CLI
diagnostics. Neither stream is text. Replacement characters, omitted NUL bytes
or cross-stream mixing violate the authored result.

The fixture's command check verifies raw output for ordinary execution and
comparison, not just the internal capture. It also checks closed stdout and
stderr pipes report command errors without a panic:

```sh
python3 tests/omega/pass/host/runtime_descriptor_binary_output_exit/check_cli.py --omega /path/to/fresh/omega
```

The corpus exit oracle independently constrains native/interpreter execution; it alone
does not verify the CLI byte handoff.
