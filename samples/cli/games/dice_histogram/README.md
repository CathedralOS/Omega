# Dice histogram

Roll 60 deterministic xorshift32 dice, tally the six faces, and render their
frequencies as a labeled bar chart. From the repository root:

```text
mbx run -p omega -- run --both samples/cli/games/dice_histogram/main.omg
```

Use Cargo if `mbx` is unavailable. Complete the ordinary
[package review](../../../../omega-rust/omega/application/cli/README.md#command-surface)
when requested. Leave `--target` off `run` to execute on the matching host.
Enter or EOF closes the native program.

The seed is 1. The six face counts must be `8, 13, 6, 11, 11, 11`, totaling
60. Each chart row is exactly 24 ASCII bytes before its newline: the face
label, colon and space, the stated number of `#` characters, then spaces to
fill the row. This defines the trailing spaces without relying on Markdown
to preserve them:

| Face | Printed prefix | Trailing spaces |
| --- | --- | --- |
| 1 | `1: ########` | 13 |
| 2 | `2: #############` | 8 |
| 3 | `3: ######` | 15 |
| 4 | `4: ###########` | 10 |
| 5 | `5: ###########` | 10 |
| 6 | `6: ###########` | 10 |

The banner is `dice_histogram: 60 xorshift32 dice rolls tallied into a labeled bar chart`.
After the six rows, print `done: all 6 face frequencies charted (exit 0)`
and `[press Enter to close]`, each on its own line. Native execution exits 0;
the checked interpreter also exits 0. The CLI reports `native exit 0` and
`interp exit: 0` on stderr. A banner, a single `#`, or exit 0
alone does not establish the chart's correctness.

The entry receives an explicit `main: &mut Main` context and forwards it to
the named states. The complete native stdout oracle and both engine exit codes
were verified on macOS ARM64. The other declared hosted targets still require
matching-host execution.
