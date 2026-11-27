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
module produces canonical package evidence; git command policy belongs to
`git_command`.
*/

mod path_custody;
mod resolver_execution;

pub use path_custody::canonicalize_exclusion_root;
pub use resolver_execution::{ResolverExecutionBackend, ResolverExecutionPhase};
