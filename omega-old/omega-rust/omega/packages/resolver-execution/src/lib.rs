/*
Process custody for compiler-owned package-source resolution.

We freeze one absolute, lexically canonical resolver executable path outside
all package-controlled roots before any package input can influence command
construction, then prepare per-phase `BoundedProcessPrepared` invocations bound
to one validated working directory.

Why delegate child lifecycle and resource ceilings to `bounded-process`?
Resolver execution needs the exact same process-group / Windows Job Object
containment, descriptor hygiene, and CPU/memory/file ceilings as other bounded
helper processes, while keeping executable custody and working-root separation
specific to source-resolution phases (`TransportDiscovery`,
`RepositoryInitialization`, `Fetch`, `RepositoryInspection`). Nothing in this
crate produces canonical package evidence; git command policy belongs to
`source-acquisition`.
*/

#![deny(unsafe_op_in_unsafe_fn)]

mod resolver_execution;

pub use bounded_process::{
    BoundedProcessChild as ResolverExecutionChild,
    BoundedProcessCompletion as ResolverExecutionCompletion,
    BoundedProcessExitStatus as ResolverExecutionExitStatus,
    BoundedProcessPrepared as ResolverPreparedExecution,
};
pub use resolver_execution::{ResolverExecutionBackend, ResolverExecutionPhase};
