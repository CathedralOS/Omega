# Omega Package Subsystem

Start in [`manager/src/operations/`](manager/src/operations/) to follow a
complete package operation.

```text
packages/
├── README.md             subsystem map
├── manager/              package workflows, declarations, graph, and lock
├── sources/              repository acquisition and immutable source snapshots
└── review/               compiler-derived findings and optional audit advice
```

The goal is Cargo-like repository install/update with compiler-derived
reachability, unsafe API, and assumption review. Packages declare their names
and dependencies in `build.omg`; the compiler derives their capabilities.
`omega.lock` records exact pins, the graph, accepted policy, and decisions.
The project trusts whoever lands it. It is not an audit certificate.

Follow the supported source-change flow through:

1. `manager/src/declarations/`: read checked declarations and plan a dependency edit.
2. `manager/src/operations/stage_build_edit.rs`: stage proposed build bytes
   while retaining the original live project identity.
3. `manager/src/resolution/`: acquire and reconcile the complete source graph.
   Pin-aware resolution preserves unchanged Git requests during installs and
   selective updates; repository workspace members move together.
4. `manager/src/operations/package_change.rs`: check the candidate and compare
   compiler findings with accepted policy.
5. `manager/src/review/`: render exact findings and recover per-change project
   decisions from the editable review document.
6. `manager/src/operations/publication/`: recheck the reviewed candidate and
   project files, then publish a recoverable `build.omg`/`omega.lock` pair.

`omega install` and `omega update` use that flow, including selective updates,
per-target review files, `--resume`, and recoverable publication. Start at the
[command operation](manager/src/operations/package_commands/README.md) for usage
and its source map. The [task board](../../../TASKS_PACKAGE_MANAGER.md) contains
only remaining work, including source audit integration. Install can select a
Git workspace member with `--package <declared-name>`; its declared name still
supplies the default import alias.

The lock codec stores readable, receipt-free policy baselines and historical
decisions beside exact source graphs. Old source is not required to read or
compare those baselines. Locked recovery acquires the recorded commit rather
than refreshing selectors; fresh checking reports changes in compiler findings
without claiming to certify prior acceptance.

Ordinary project preparation uses accepted dependency pins while allowing edits
to the application's own source. Changed dependency declarations or local
dependency contents require `omega update`. Compiler admission policy lives in
`omega.admissions`, separately from package pins; `--accept-admissions` cannot
replace the package lock.

Native compilation uses a separate manager admission and compiler handoff
path. Its proof, reachability, ABI, and artifact checks are not an additional
install/update certification requirement. Invalid or unsupported source still
rejects during candidate checking.

Design and acquisition references:

- [Build And Package Model](../../../wiki/design_briefs/build_and_package_model.md)
- [Package Manager First Draft](../../../wiki/design_briefs/package_manager_first_draft.md)
- [Source Resolver Security](sources/acquisition/SOURCE_RESOLVER_SECURITY.md)
