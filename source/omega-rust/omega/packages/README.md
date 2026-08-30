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

The ordinary flow is:

```text
build.omg declaration
    -> manager resolves the exact dependency closure
    -> sources acquires immutable source custody
    -> compiler checks the selected closure
    -> review records and compares compiler-issued facts
    -> manager applies root-owned policy and transaction rules
```

No supporting crate admits a package. Source custody does not imply trust,
review evidence does not imply acceptance, and advisory output cannot alter
policy. Complete operations belong only to `manager/`; the `omega` binary
delegates to those operations rather than rebuilding package policy.

Design and security references:

- [`package_manager_first_draft.md`](../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../wiki/design_briefs/build_and_package_model.md)
- [`SOURCE_RESOLVER_SECURITY.md`](sources/acquisition/SOURCE_RESOLVER_SECURITY.md)
- [`TASKS_PACKAGE_MANAGER.md`](../../../../TASKS_PACKAGE_MANAGER.md)
