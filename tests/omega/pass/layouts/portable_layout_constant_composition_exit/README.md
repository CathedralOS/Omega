# Layout-dependent constant composition

Parent: `PIPELINE-ARCHITECTURE-ENFORCEMENT` in `TASKS.md`.

A layout observation remains a dependency through an ordinary helper, a fixed
array extent, a named constant initializer and a ground call contract. The
program runs only after its exact layout establishes `TOTAL == 24`; its
[reject twin](../../../fail/layouts/portable_layout_constant_composition/main.omg)
removes one input field without changing that requirement or the consuming
subtraction.

The original compiler tried to invoke `extent` during independent array-length
evaluation, before target layout closure, and refused its retained query as a
call with no machine identity. Early jobs must inspect their demanded declaration
dependencies as well as call edges. Deferral does not grant execution or prove
the predicate; ordinary target closure and invocation admission remain required.

This fixture retains that prerequisite repair. The larger architecture outcome
remains open: two-target compilation must share one immutable Terminal artifact,
with source-free target closure and execution after reload. Single-target native
and Terminal agreement alone does not establish that acceptance.
