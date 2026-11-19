# Omega Resolver Execution

This crate prepares the Git commands that package-source resolution runs:
the executable, phases and working roots. Structured command building, resource limits, bounded
output capture and process-container cleanup live in `bounded-process`.
Callers open a backend with the absolute Git path fixed before package
input is read, then pick one compiler-defined phase.

## Start here

[resolver_execution.rs](src/resolver_execution.rs) holds the fixed executable,
the closed phase list, root validation, command preparation and resource
limits. `prepare(phase, working_root)` takes one working root for every phase;
no phase has an optional root.

The [path custody checks](src/path_custody.rs) require absolute, lexically
canonical, bounded paths and keep the executable outside package-controlled
roots and their canonical targets. Tests live beside them. `lib.rs` exports
these operations and the shared bounded-process types.

Dependencies run one way on purpose. Building a request yields an opaque
prepared command, and running the process consumes it. Package declarations
never reach this crate. A prepared command or completion value is ordinary
control-flow state, not canonical evidence and not source identity.

## Phase authority

`RepositoryInitialization` and `RepositoryInspection` are local-only phases:
their compiler-chosen arguments ask for no network transport, and only
initialization changes the repository. Neither gets a Seatbelt or Landlock
executable or filesystem policy.

`TransportDiscovery` and `Fetch` run through the host. The system Git and the
processes it starts use the user's normal Git/SSH configuration, credentials,
proxies, executable lookup, network and writable state. Omega does not pick
transport helpers, force a proxy or SSH command, claim the real network route,
or count total bytes transferred. Those are host inputs, not package authority
or identity.

The acquisition code fixes the primary Git path before reading
package-controlled input and passes that absolute path here. This crate never
looks up a bare command name from a package or repository directory. That path does not
vouch for helpers host Git configuration starts.

So package input cannot choose the executable, and Omega does not pretend
to reproduce or safely control the host's transport setup.

## Enforced boundary

`ResolverExecutionBackend::open` takes one exact absolute executable path and
refuses it if it lies under any supplied package-controlled root. Every later
preparation reuses that stored path; no call looks up or replaces it.
Preparing a phase sets the working directory to its validated root. Commands
carry only structured arguments, environment changes, and null or piped
standard streams chosen by the compiler. Implicitly inherited streams are
refused, and spawning consumes the prepared command.

Completion returns only after the primary process is reaped and
bounded-process has terminated its process container or seen it gone.
Callers may read completion for control flow and diagnostics; it is never
hashed or canonically encoded.

Process lifecycle and capture live in
`foundation/bounded-process`; this crate supplies the resolver's exact
limits and which root each phase may use. Windows uses a kill-on-close Job Object with
process, memory and CPU limits. Unix sets concrete resource limits, starts a
new process group and kills that group during cleanup; it does not claim to
control a child that deliberately leaves the group. Seatbelt, Landlock
filesystem control, executable/write allowlists, native-guarantee tables and
canonical records of commands, policies or completions do not exist.
Stronger host or CI isolation is the operator's job, not package evidence.

The [resolver security contract](../../../../wiki/spec/packages/resolver_security.md)
defines the source-resolution rules this crate enforces.
