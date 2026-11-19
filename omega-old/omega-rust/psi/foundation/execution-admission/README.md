# Execution admission

One `omega` invocation owns one `ExecutionBudget`: a fixed set of
large-stack worker threads that every parallel compiler stage shares.
Source parsing (Psi 01) and native completion (Omega) both run on it, so the
invocation's active workers and their 256 MiB stack reservations are bounded
once, not once per stage. The crate is target-neutral, so `psi/` can use it
without importing `omega/` orchestration.

The CLI creates the budget at its entry point and runs the whole invocation
inside it with `ExecutionBudget::run`. Stages reach it through
`within_budget` and `current_workers`. A caller outside any budget (a unit
test, a library embedding) gets a temporary single-worker budget, so its work
runs in sequence on a large stack instead of Rayon's small-stack global pool.

The worker count follows Rayon's default (`RAYON_NUM_THREADS`, otherwise the
logical CPUs the process may use, which honors affinity masks and cgroup CPU
quotas). This crate does not bound memory: stages that hold large intermediates per
job, such as native completion, keep their own in-flight cap below this
worker bound.
