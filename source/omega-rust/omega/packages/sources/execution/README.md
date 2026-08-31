# Omega Resolver Execution

This crate owns native process preparation and lifecycle for package-source
resolution. D43 narrows it to structured command construction, concrete
resource limits, and whole-process-tree cleanup. Callers choose one
compiler-defined phase and supply the absolute Git path frozen before package
input. The legacy guarantee, canonical execution-observation, and
filesystem/executable-confinement surfaces are implementation debt to delete.

## Structure

```text
src/
├── lib.rs          public entrance and closed reexports
├── model/          transitional phase/policy carriers; retain only runtime needs
├── request.rs      validated executable and custody-path requests
├── backend/        host selection, request validation, and preparation
├── prepared.rs     opaque structured command configuration
├── process/        limits, descriptor custody, lifecycle, and completion
└── confinement/    migration area: delete attestation/filesystem policy;
                    retain only honest limits and cleanup in their proper owner
```

The dependency direction remains deliberate. Request construction feeds an
opaque prepared command, and process execution consumes it. Package
declarations never reach this crate. A preparation or completion value is an
internal control-flow carrier, not canonical evidence and not source identity.

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

Every prepared execution has explicit arguments, environment changes, working
directory, standard-stream dispositions, deadlines, and process-resource
ceilings. Standard streams are compiler-owned null handles or pipes; arbitrary
caller-opened handles and implicit stream inheritance are not representable.
Spawning consumes the prepared value.

Successful completion is returned only after the primary process has been
reaped and the backend has terminated or observed the absence of the owned
process tree. The caller may inspect the exit status and bounded output needed
for ordinary control flow and diagnostics; those facts are not hashed into an
execution receipt.

Native mechanisms survive only when they directly implement an honest bound or
lifecycle guarantee. Windows may use a kill-on-close Job Object with process,
memory, and CPU ceilings; Unix may apply concrete resource limits. Seatbelt,
Landlock filesystem mediation, executable/write allowlists, native-guarantee
matrices, and canonical policy/completion observations are retired. Stronger
host or CI isolation is an operator concern, not package evidence.

The full source-resolution contract is maintained in
[`SOURCE_RESOLVER_SECURITY.md`](../acquisition/SOURCE_RESOLVER_SECURITY.md).
