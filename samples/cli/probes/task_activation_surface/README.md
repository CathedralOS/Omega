# task_activation_surface

First sample on the task-runtime surface: `suspend
Sleeper::park(7)` inside a `suspends;` machine, `suspend
block TaskLifecycle::finish<T>(task)` on the public requirement, and
`Token in Carry::AcrossSuspend` domain-qualified payloads.

## Gaps found while authoring (filed)

`TaskRuntime::start`/`try_start` traits are package-private —
the satisfies-impl activation route (`start<Worker::run>`) is
unreachable from ordinary packages; only `TaskLifecycle::finish` /
`TaskLifecycle::request_cancel` requirements are public.

Compile-checked only: `use omega::language::core::task` and
`core::binding` are depend-free imports.
