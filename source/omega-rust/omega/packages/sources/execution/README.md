# Omega Resolver Execution

This crate owns native process preparation and lifecycle for package-source
resolution. Callers choose one compiler-defined phase and supply already
verified executable and custody paths. They cannot author sandbox policy text,
invent containment claims, or extract an unrestricted native command.

## Structure

```text
src/
├── lib.rs          public entrance and closed reexports
├── model/          phases, native guarantees, and policy observations
├── request.rs      validated executable and custody-path requests
├── backend/        host selection, request validation, and preparation
├── prepared.rs     opaque command configuration and exact command identity
├── process/        limits, descriptor custody, lifecycle, and completion
└── confinement/    optional native hardening
    ├── macos/          Seatbelt policy for local-only phases
    ├── linux.rs        Landlock and Unix resource-limit integration
    └── windows/        Job Object launch, limits, and whole-job cleanup
```

The dependency direction is deliberate. `model/` and `request.rs` define the
closed vocabulary; `backend/` validates it and produces `prepared.rs` values;
`process/` consumes those values; platform confinement only realizes and
classifies compiler-owned policy. Package declarations never reach this crate.

## Phase authority

`RepositoryInitialization` and `RepositoryInspection` are local-only phases.
They require no host transport chain, deny network and descendants where the
native backend can enforce that policy, and retain narrow filesystem authority.

`TransportDiscovery` and `Fetch` are host-routed phases. The selected system
Git and its descendants use the invoking user's ordinary Git/SSH configuration,
credentials, proxies, executable lookup, network, and writable state. Omega
does not preselect transport helpers, force a proxy or SSH command, claim the
actual endpoint route, or measure aggregate transport bytes. Those are host
inputs, not package authority and not package identity.

This distinction prevents local object inspection from acquiring unnecessary
authority without pretending that Omega can reproduce or safely broker the
host's complete transport ecosystem.

## Enforced boundary

Every prepared execution has explicit arguments, environment changes, working
directory, standard-stream dispositions, deadlines, and process-resource
ceilings. Standard streams are compiler-owned null handles or pipes; arbitrary
caller-opened handles and implicit stream inheritance are not representable.
Spawning consumes the prepared value.

Successful completion is issued only after the primary process has been reaped
and the backend has terminated or observed the absence of the owned process
tree. Policy and completion observations bind the selected primary executable,
phase, command identity, limits, native guarantee dispositions, exit status,
and cleanup result. They report only controls that the backend actually
established.

Native hardening is defense in depth:

- macOS applies compiler-authored Seatbelt policy to the local-only phases;
- Linux applies resource limits and uses Landlock where its incomplete
  filesystem mediation is useful and honestly classifiable; and
- Windows launches through a kill-on-close Job Object with process, memory,
  and CPU ceilings.

Network phases deliberately do not use executable or write allowlists that
would block host-selected Git helpers. Stronger host or CI isolation is an
operator concern; Omega does not convert it into package evidence.

The full source-resolution contract is maintained in
[`SOURCE_RESOLVER_SECURITY.md`](../acquisition/SOURCE_RESOLVER_SECURITY.md).
