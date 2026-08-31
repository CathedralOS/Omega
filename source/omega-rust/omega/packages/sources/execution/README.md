# Omega Resolver Execution

This crate owns native process preparation and lifecycle for package-source
resolution. It is deliberately limited to structured command construction,
concrete resource limits, and process-container cleanup. Callers choose one
compiler-defined phase and open a backend around the absolute Git path frozen
before package input.

## Structure

```text
src/
├── lib.rs          public entrance and closed reexports
├── phase.rs        compiler-owned source-resolution phases
├── request.rs      validated executable and custody-path requests
├── backend/        executable selection, request validation, and preparation
├── prepared.rs     opaque structured command configuration
└── process/        limits, descriptor custody, lifecycle, completion, and
                    the Windows Job Object implementation
```

The dependency direction remains deliberate. Request construction feeds an
opaque prepared command, and process execution consumes it. Package
declarations never reach this crate. A prepared command or completion value is
ordinary control-flow state, not canonical evidence and not source identity.

## Phase authority

`RepositoryInitialization` and `RepositoryInspection` are local-only phases:
compiler-owned arguments request no transport and only initialization requests
repository mutation. They receive no universal Seatbelt/Landlock executable or
filesystem policy.

`TransportDiscovery` and `Fetch` are host-routed phases. The selected system
Git and its descendants use the invoking user's ordinary Git/SSH configuration,
credentials, proxies, executable lookup, network, and writable state. Omega
does not preselect transport helpers, force a proxy or SSH command, claim the
actual endpoint route, or measure aggregate transport bytes. Those are host
inputs, not package authority and not package identity.

The acquisition owner freezes the primary Git path before package-controlled
input is processed and supplies that absolute path here. This crate never
performs a bare-name lookup from a package or repository working directory.
The frozen primary coordinate does not close or attest the descendant helper
graph selected by ordinary host Git configuration.

This distinction prevents package input from selecting the executable without
pretending that Omega can reproduce or safely broker the host's complete
transport ecosystem.

## Enforced boundary

`ResolverExecutionBackend::open` accepts one exact absolute executable path and
rejects it if it is beneath any supplied package-controlled root. Every later
preparation reuses that stored path; there is no executable lookup or per-call
replacement. Phase preparation fixes the working directory to its validated
root. Commands expose only structured arguments, environment changes, and
compiler-owned null or piped standard streams. Implicit stream inheritance is
rejected, and spawning consumes the prepared value.

Completion is returned only after the primary process has been reaped and the
backend has terminated or observed the absence of the owned process container.
The caller may inspect exit status and bounded output for ordinary control flow
and diagnostics; the crate does not hash or canonically encode those facts.

Native mechanisms survive only when they directly implement an honest bound or
lifecycle guarantee. Windows uses a kill-on-close Job Object with process,
memory, and CPU ceilings. Unix applies concrete resource limits, starts a new
process group, and kills that group during cleanup; it does not claim custody
of a descendant that deliberately detaches from the group. Seatbelt,
Landlock filesystem mediation, executable/write allowlists, native-guarantee
matrices, and canonical command/policy/completion observations are absent.
Stronger host or CI isolation is an operator concern, not package evidence.

The full source-resolution contract is maintained in
[`SOURCE_RESOLVER_SECURITY.md`](../acquisition/SOURCE_RESOLVER_SECURITY.md).
