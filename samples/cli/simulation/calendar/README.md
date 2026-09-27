# Calendar

Render a 30-day month starting on Wednesday, then read Enter and exit 0.
The program writes decimal digits into a 21-byte UTF-8 row using runtime
indices, flushes each complete week, and prints the final partial week.
The explicit `as i32` conversions erase the arithmetic policy after the
wrapping digit additions; the existing narrowing calls still trap on an
out-of-range value.

From the repository root on macOS ARM64:

```sh
mbx run -p omega -- --check --target macos_arm64 samples/cli/simulation/calendar/main.omg
mbx run -p omega -- update --target macos_arm64 --project samples/cli/simulation/calendar
```

If update requests review, inspect the generated source findings and
`samples/cli/simulation/calendar/build/package-manager/review-macos_arm64.txt`.
Record an `accept` or `reject` for each pending decision after reviewing it,
then resume the transaction:

```sh
mbx run -p omega -- update --resume --project samples/cli/simulation/calendar
mbx run -p omega -- run samples/cli/simulation/calendar/main.omg
```

Use Cargo if `mbx` is unavailable. Enter closes the native program.
Leave `--target` off `run` so it executes on the matching host. The same
commands work in PowerShell; select `windows_x86_64` for update/check and
review that target's generated file. The native result below was verified
on macOS ARM64.

The exact header and five rows are shown as quoted strings so the trailing
spaces remain visible. The quotes are not printed. Every string is 21 ASCII
bytes, including the header's final space:

```text
"Su Mo Tu We Th Fr Sa "
"          1  2  3  4 "
" 5  6  7  8  9 10 11 "
"12 13 14 15 16 17 18 "
"19 20 21 22 23 24 25 "
"26 27 28 29 30       "
```

The complete stdout is 277 bytes with LF line endings: the banner
`calendar: render a 30-day month starting on Wednesday as a 7-column grid`,
the six lines above, `done: all 30 dates laid out and flushed (exit 0)`, and
`[press Enter to close]`, each followed by LF. Native execution exits 0;
the CLI reports `native exit 0` on stderr. Acceptance requires all five
rows and the exact header, not merely output containing `30`.
