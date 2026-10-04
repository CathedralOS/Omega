# Projection after checking a record's evaluated fields

The selected field is constant; its sibling is not. Checking must validate the
sibling's division using the machine's nonzero requirement without mistaking
failure to fold the whole record for a lost field binding. Selecting `first`
does not make the other initializer unevaluated.

Both native execution and the checked interpreter must exit 70, observing 7.
Current frontier: formation checking succeeds and the interpreter exits 70,
but native production still rejects with `scalar computation needs one checked
expression and one source binding`. `STATE-LOCAL-VALUE-FRONTIER` owns connecting
the computed record receiver to ordinary structural materialization and field
observation. The native oracle is deliberately not recorded as a rejection.
The invalid control is `fail/arithmetic/exact_aggregate_divisor_unproven`;
unit coverage also checks the same projection without the nonzero requirement,
with both anonymous and explicitly typed selected initializers.

```text
python3 tools/corpus_gate.py --both --filter arithmetic/runtime_exact_projection_sibling_exit
```

For the ordinary CLI, first run `omega update --project
tests/omega/pass/arithmetic/runtime_exact_projection_sibling_exit`, review the
reported package and Console-binding policy changes, then accept them with
`omega update --resume --project
tests/omega/pass/arithmetic/runtime_exact_projection_sibling_exit`.

```text
omega run --both tests/omega/pass/arithmetic/runtime_exact_projection_sibling_exit/main.omg
```

`--both` reads standard input to EOF. This fixture needs no input: redirect
`</dev/null` on macOS/Linux, or use `cmd /c "omega run --both
tests/omega/pass/arithmetic/runtime_exact_projection_sibling_exit/main.omg <NUL"`
on Windows. Exit 70 is the authored success result, not a compiler failure.
