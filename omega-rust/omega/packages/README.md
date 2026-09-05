# Omega Package Subsystem

Start in [`manager/src/operations/`](manager/src/operations/) to follow a
complete package operation. The other two directories are subordinate security
boundaries used by those operations:

```text
packages/
├── README.md             this subsystem map
├── manager/              complete compiler and user package workflows
├── sources/              hostile source acquisition and bounded resolvers
└── review/               compiler-issued facts and optional audit advice
```

The ratified install/update model is ordinary repository acquisition and graph
resolution, with compiler-derived reachability, unsafe API, and assumption
review. `omega.lock` records exact pins, the graph, accepted review baselines,
and decisions. The project trusts whoever lands that file. Installation does
not require a sealed or certified `PackageInstance`, or certificates proving
lock acceptance. Compiler proof/reach checks and native artifact checks remain
independent of installation.

The current implementation flow is:

```text
build.omg declaration
    -> manager resolves the exact dependency closure
    -> sources acquires immutable source custody
    -> compiler checks the selected closure
    -> review records and compares compiler-issued facts
    -> manager admission rechecks custody, evidence, and root-owned policy
    -> operations return review results or continue retained native compilation
```

The manager's additional evidence-promotion and policy-replay machinery exists
today, but is redundant machinery to simplify, not a required future stage of
install/update. Source verification and compiler checks retain their own
purposes. Review evidence does not prove acceptance or that an audit occurred,
and advisory output cannot alter policy. Complete operations belong only to
`manager/`; the `omega` binary delegates to those operations.

Within one admission operation, question construction, result composition, and
payload assembly share the same source-ordered borrowed reviews. They do not
rebuild package-key associations or compare copies of the same immutable
compiler results. Live source custody, requested target, result metadata,
generated-source identity, and exact project decisions retain their distinct
checks. This does not replace the remaining install/update transaction work.

Fresh review compilation constructs each ledger and result set without then
reconstructing it solely to compare that output with itself. Result construction
still rejoins and rechecks compiler contract-assumption certificates. Public
validators continue to reconstruct independently for supplied or recovered
evidence.

`CanonicalSourceClosureSubject` has binary and line-oriented text encodings of
the same resolved graph. The text names the exact target, root role and request,
source-qualified packages and immutable revisions/content, workspace navigation,
authored dependency requests, and selected alias edges. Recovery applies the
same graph checks and requires neither the source checkout nor a compiler run.
This source record is not an accepted lock: accepted policy baselines and
decisions, locked resolution, and transaction publication remain separate work.

An accepted policy baseline must not embed the existing review capsule. That
capsule includes compiler proof and build-replay data. Selected provider plans,
terminal permissions, external supplies, calling applications, and representation
declarations, availability, selections, and demands have receipt-free structural
projections and bounded component encodings. Provider policy retains exact service
signatures, complete calling applications, binding producers, grants, and
family links. Terminal permissions retain complete service schemas and generic
telescopes independently of provider demand. Callable policy retains complete
signatures, contracts and lifetime bindings, published promises and checked
summaries, entry mutation, and separately normalized reachable capability flows.
Its typed crash guards preserve foreign owners without private derivation
coordinates. Complete baseline composition, recovery, and comparison remain open.
Dropping audit-relevant families or retaining their reconstruction receipts
would both be incorrect.

Historical project decisions have a separate bounded text section under
`manager/src/lock/decisions`. It is scoped to the retained source subject and
loads without old source or old compiler conflicts. It cannot stand in for
fresh root-policy resolution or the full normalized accepted baseline.

Design and security references:

- [`package_manager_first_draft.md`](../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../wiki/design_briefs/build_and_package_model.md)
- [`SOURCE_RESOLVER_SECURITY.md`](sources/acquisition/SOURCE_RESOLVER_SECURITY.md)
- [`TASKS_PACKAGE_MANAGER.md`](../../../TASKS_PACKAGE_MANAGER.md)
