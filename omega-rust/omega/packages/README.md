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

Design and security references:

- [`package_manager_first_draft.md`](../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../wiki/design_briefs/build_and_package_model.md)
- [`SOURCE_RESOLVER_SECURITY.md`](sources/acquisition/SOURCE_RESOLVER_SECURITY.md)
- [`TASKS_PACKAGE_MANAGER.md`](../../../TASKS_PACKAGE_MANAGER.md)
