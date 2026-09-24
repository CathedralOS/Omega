# Dutch flag

Dijkstra's three-pointer in-place partition over a real enum array: everything
below `w` is Red, everything from `b` up is Blue, and the White cursor walks
the unknown middle until it meets the fence. `[White, Blue, Red]` becomes
`[Red, White, Blue]`; the program exits **70**.

The sample uses the field-counter idiom: plain `Exact` counters plus
literal re-guard states dominate every runtime-indexed access, and the
`confused(9x)` arms stand as honest runtime checks for the loop invariants a
verified port would carry as `requires`/`ensures` facts.

## Host status

- Checked trees: `dutch_flag_sample_reaches_checked_trees` passes.
- Linux x86-64 native (compiler-library regression,
  `OMEGA_SAMPLE_RUNTIME_FILTER` scoped to this sample): compilation fails at
  selected `ProgramEntry` establishment with "rejoins 0 Terminal attachment
  identities; expected one" — an entry/receiver-custody gap routed to
  **ENTRY-CONTENT-ROOTS** per the SAMPLE-CORPUS table, not a sample defect.
- Other hosted targets (Windows x86-64, Linux ARM64, macOS ARM64) remain
  unverified; coverage tracking lives in
  [SAMPLE-CORPUS](../../../../TASKS.md).

## Run the regression

```sh
OMEGA_SAMPLE_RUNTIME_FILTER=dutch_flag cargo nextest run -p compiler \
  --test samples_compile --no-fail-fast \
  -E 'test(=samples_with_documented_exit_run_correctly)'
```

In PowerShell set `$env:OMEGA_SAMPLE_RUNTIME_FILTER='dutch_flag'` first, run
the same command on one line, then remove the variable. Use `mbx` in place of
`cargo` when available. The harness supplies test-owned acceptance, closes
stdin, and checks the documented exit — it does not replace the ordinary CLI
package review route.
