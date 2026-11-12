# intrinsic_machine_param

A provider satisfied by the compiler itself: `IntrinsicWorkerProvider::execute<T, Arguments, machine Target>` fills the trait row `satisfies WorkerProvider::execute` `via ForeignBinding::CompilerIntrinsic` — no authored body. The where clause pins the worker's shape `machine Target(arguments: Arguments) -> T suspends blocks;` — dual-axis machine symbols as generic parameters.

## Learnings

- `via ForeignBinding::CompilerIntrinsic` vends the body; the provider declaration is the whole implementation.
- `machine Target` params compose with ordinary `T`/`Arguments` type params in one generic list.

Compile-checked only: run a check target, not native.
