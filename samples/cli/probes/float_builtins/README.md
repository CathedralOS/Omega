# Ordinary float helpers

Float helper probe with an explicit standard-library dependency.

- float_builtins: imported carrier-specific min/max/sqrt machines on f32/f64;
  anonymous literals land at the selected declaration's format.
- target_scoped_machines: `linux_x86_64 machine probe()` —
  target-prefixed bodies; Psi checks every target's body, only
  realization narrows.
- drop_hook_body: owner-attached `Guard::drop(guard: &mut Guard)`
  with a multi-statement hook calling machines and writing
  fields; `drop(local)` disposal.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/float_builtins/main.omg
mbx run -p omega -- update --project samples/cli/probes/float_builtins
```

The imported helper program checks. Native realization on macOS arm64 still
rejects the `Main::main` state graph's conditional guard (`state graph:
terminator: conditional successors: guard expression`). The cause is the
guard's own `f32`/`f64` comparisons, not the std helpers: this program does not
import `omega::language::core::float_operations`, so each `==` checks through
its builtin-fallback meaning, and a builtin float comparison has no Terminal
route in any position (the reduction is
`tests/omega/pending/arithmetic/float_computation_no_route`, owned by
OPERATOR-BOUNDARY-CALLS, whose board entry says to preserve declared operator
selection with no spelling-based fallback). The same program with that import
builds natively on macOS arm64. Whether this probe should import the selected
float operations or the builtin fallback should execute is an owner decision;
until it is made this probe is not a passing native example. `omega.lock`
publishes locally and is ignored.
