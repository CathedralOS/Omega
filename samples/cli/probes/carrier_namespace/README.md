# Carrier Namespace

Historical implementation probe, not accepted source guidance. The
`no-data-code-attachment` decision retires its data-qualified machine
declarations, including when their parameters name unrelated data.
`STATIC-MACHINE-MIGRATION` in [TASKS.md](../../../../TASKS.md) owns migration to
independent free/module machines while preserving explicit borrow behavior.
The observations below describe the legacy implementation only.

Compile-scope probe: attached machines are namespaced statics. No
standard-library dependency.

- `machine Main::bump(reward: &mut Reward)` — attached machine whose
  only parameter is an unrelated data type; the receiver need not be
  the carrier.
- `machine count(bytes: &[u8])` — free structural-parameter machine
  called on `&main.buf` (ambient receiver beside the forwarded
  carrier).

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/carrier_namespace/main.omg
mbx run -p omega -- update --project samples/cli/probes/carrier_namespace
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
