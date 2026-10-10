# target scoped machines

Compile-scope probe. No standard-library dependency.

- float_builtins: builtin `min`/`max`/`sqrt` on f32/f64 —
  no import; anonymous literals land at the annotation format.
- target_scoped_machines: `linux_x86_64 machine probe()` —
  target-prefixed bodies; Psi checks every target's body, only
  realization narrows.
- drop_hook_body: owner-attached `Guard::drop(guard: &mut Guard)`
  with a multi-statement hook calling machines and writing
  fields; `drop(local)` disposal.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/target_scoped_machines/main.omg
mbx run -p omega -- update --project samples/cli/probes/target_scoped_machines
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
