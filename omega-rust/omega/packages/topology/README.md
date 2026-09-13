# Checked Boundary Topology Plan

This crate is the Rust reference owner of the package-defined deployment plan
described by the [topology contract](../../../../../wiki/spec/packages/topology.md).
It is not a compiler pipeline stage: nothing in the compiler calls it, and its
existence grants no routing authority. The eventual build-only Omega package
produces plans under this same versioned schema; this crate owns the wire
tables, the independent source-free verification route, and the reference
`no_route`/`only_via` predicates used to check that package's output.

Start at [`src/lib.rs`](src/lib.rs), then enter the area you are following:

```text
topology/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs       public entrance and trust-boundary notes
    ├── model/       exact plan and owner-request data
    ├── graph/       bounded deterministic normalization and traversals
    ├── predicate/   selector validation, reference predicates, certificates
    ├── codec/       versioned canonical wire tables and cursors
    ├── compose/     producer reference: evaluate required policies, emit a plan
    └── verify/      source-free consumer: reconstruct, compare, replay
```

The crate distinguishes `composition checked` from `installation admitted`.
`verify_plan` can only produce the first; no API here installs code, binds
endpoints, or claims confinement. Correspondingly, a verified plan is evidence
for the installer's own recheck, never a reusable authorization.
