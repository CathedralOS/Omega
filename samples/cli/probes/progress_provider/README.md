# progress_provider

First sample wiring a progress-qualified provider through
the build declaration: `domain SchedulerHandle::WeakFair
satisfies ProgressProfile established by
Scheduler::grant`, per-target satisfies impls via
`ForeignBinding::Syscall(1)` /
`ForeignBinding::VtableField(field)`, entry
`requires main.scheduler in SchedulerHandle::WeakFair`,
and `Build::select_provider<Scheduler,
SchedulerProvider>` in build.omg.

## Learnings

`Build::select_provider` resolves the earlier
`Binding<T>`-field gap from `multi_effect_trait`: a
provider selected in build.omg is the Fused route that
lets a `Binding<Scheduler>` carrier field admit value
calls at `--check` — no package dependency needed.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
