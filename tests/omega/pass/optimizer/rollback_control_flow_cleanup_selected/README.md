# ControlFlowCleanup rollback, selected leg

`build.omg` authors `Optimization::ControlFlowCleanup` by exact name; `rollback.txt`
declares the same rule, so the request's `OptimizationRollback` overlay
subtracts it before Terminal production.

On the check leg the corpus runner compiles this fixture to its retained
Terminal artifact twice -- with and without the overlay -- and records:

- `rollback:requested=[ControlFlowCleanup];applied=[ControlFlowCleanup];effective=[]` -- the
  settled receipt the rolled build's report publishes;
- `unrolled-receipt:none` -- the ordinary build settles no overlay;
- `unrolled-semantic:`/`rolled-back-semantic:` -- the manifest
  fingerprints of the two semantic modules;
- `module:differs` -- the vacuity guard: the authored rule demonstrably
  rewrote the module, so rolling it back is what restores the
  never-selected module rather than an identity rewrite proving nothing.

The `rollback_control_flow_cleanup_control` fixture compiles the same `main.omg`
with no authored selection: this row's `rolled-back-semantic` equals the
control's fingerprints in the golden, pinning that the rolled-back build
byte-for-byte rejoins a build that never selected the rule.

On the native leg the request's overlay rides the real product route, so
`built ... exit:0` records the rolled-back executable actually running.
