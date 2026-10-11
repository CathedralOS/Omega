# suspend_carry_task

`suspends;` machine suspends through `boundary machine Sleeper::park(...) suspends`; `Token in Carry::AcrossSuspend` value survives the suspend edge.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.

## Gaps found while authoring (filed)

- `satisfies TaskRuntime::start via ForeignBinding::CompilerIntrinsic`
  — the `TaskRuntime` trait itself is package-private; user packages
  cannot author task-runtime providers (corpus-only surface).
