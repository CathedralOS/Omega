# Print squares

Print the squares 1 through 9 as three-digit decimal text, ending in `081`,
then read Enter and exit 0. The program retains its nested state cycles,
runtime-indexed UTF-8 buffer writes, and ordinary Console calls.

From the repository root, compile and execute on the host:

```sh
mbx run -p omega -- run samples/cli/basics/print_squares/main.omg
```

That command works in macOS's shell and Windows PowerShell. Press Enter at
the final prompt. For a noninteractive macOS run, redirect stdin from
`/dev/null`; EOF also lets the program finish.

The computed rows must be exactly:

```text
001
004
009
016
025
036
049
064
081
```

The final banner also contains `081`, so that substring alone is not a numeric
output oracle. Require all nine rows in order and exit 0.

Use Cargo when `mbx` is unavailable. Package review remains part of the command;
do not manufacture acceptance to advance the sample. An explicit `run --target`
only compiles for that target, without executing the image.

The complete EOF-driven run is verified on macOS ARM64. Matching-host execution
on the other authored targets remains under
[SAMPLE-CORPUS](../../../../TASKS.md); cross-target compilation is not execution
evidence.
