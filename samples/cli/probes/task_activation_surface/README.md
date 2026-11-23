# task_activation_surface

The sample keeps a local token across a call to `Sleeper::park` and finishes a task through the public `TaskLifecycle::finish` requirement.

## Gaps found while authoring (filed)

`TaskRuntime::start`/`try_start` traits are package-private —
the satisfies-impl activation route (`start<Worker::run>`) is
unreachable from ordinary packages; only `TaskLifecycle::finish` /
`TaskLifecycle::request_cancel` requirements are public.

Compile-checked only: `use omega::language::core::task` and
`core::binding` are depend-free imports.
