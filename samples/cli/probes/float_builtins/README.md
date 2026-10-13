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
rejects the `Main::main` state graph's conditional guard; the pre-migration
program also failed native lowering on base `1f63ee648905`. This probe is not a
passing native example. `omega.lock` publishes locally and is ignored.
