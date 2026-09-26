# Checked Boundary Topology Plan

This crate is the Rust reference owner of the package-defined deployment plan
described by the [topology contract](../../../../wiki/spec/packages/topology.md).
It is not a compiler pipeline stage: nothing in the compiler calls it, and its
existence grants no routing authority. The eventual build-only Omega package
produces plans under this same versioned schema; this crate owns the wire
tables, the independent source-free verification route, and the reference
`no_route`/`only_via` predicates used to check that package's output.

Choose the operation directly from `src/`:

```text
src/
├── lib.rs                    Rust wiring, public exports, trust-boundary notes
├── deployment_plan.rs        exact plan, request, endpoints, and policy records
├── deployment_plan/
│   ├── graph.rs              bounded normalization and traversals
│   ├── predicate.rs          reference policies and certificate checking
│   └── codec.rs              versioned canonical bytes
├── plan_composition.rs       producer: validate roster, evaluate policies, emit plan
├── plan_verification.rs      independent consumer: reconstruct, compare, replay
├── topology_installation.rs  prepare → activate → quiesce/replace; owns custody
└── topology_installation/
    ├── mediation.rs          authorize each request/response against installed routes
    ├── pipe_adapter.rs       private-pipe provider contract and reference adapter
    └── frame.rs              bounded frame codec
```

The operation files contain their actual sequencing, not dispatch façades.
Installation starts with the lifecycle operations; the authority, custody, and
failure records they use follow in the same owner. Per-message mediation cannot
mint installation authority. Plan construction and independent verification
remain separate entrypoints with different input and trust contracts.

The crate distinguishes `composition checked` from `installation admitted`.
`verify_plan` can only produce the first. Preparation and activation in
`topology_installation` produce the second for the private-pipe profile by
joining a checked plan to an independently
authorized request, assigning one dedicated pipe pair per binding, and
gating application entry on complete admitted coverage. Correspondingly, a
verified plan is evidence for the installer's own recheck, never a reusable
authorization — and a receipt is evidence for one installation occurrence,
not the next.

Installation intent is the copyable `InstallationRequest`; it is not a live
capability. The supervisor retains one `InstallationLifecycle` and uses
`authorize` to issue an opaque, non-clonable `InstallationAuthorization`
independently of the candidate plan. Occurrences increase strictly within that
lifecycle, including after preparation failure, disarm, failed activation, and
retirement. A fresh issuance supersedes older pending intent. Preparation owns
the token until activation checks its issuing lifecycle and current occurrence;
replacement performs that check before stopping the old installation. Failed
replacement preflight returns both the unchanged old installation and pending
endpoint custody.

This local issuance state has no global registry or persistence protocol. The
supervisor remains responsible for retaining its lifecycle, admitting executable
closure, and establishing actual process confinement, endpoint delivery, and
startup gating. A fresh lifecycle cannot stand in for the same supervisor's
current authority. These reference tests do not establish the contract's
three-process payment installation: its real executable/component admission and
OS-backed supervisor remain required.
