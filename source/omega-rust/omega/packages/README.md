# Omega Package Manager

This directory is the `omega-package-manager` crate. Start at
[`src/lib.rs`](src/lib.rs), then enter the operation you are following.

```text
packages/
├── Cargo.toml                  package-manager crate
├── README.md                   this entrance and owner map
├── src/
│   ├── lib.rs                 Rust entrance
│   ├── operations/            complete compiler and user operations
│   │   ├── prepare_project.rs ordinary package-aware compilation
│   │   └── inspect_source/    acquire and inspect without admission
│   ├── declarations/          checked package declarations from build.omg
│   ├── resolution/            exact source selection and dependency closure
│   └── review/                compile, compare, audit, and decide
├── tests/                     manager integration tests
├── source/                    hostile source acquisition and custody
├── source-execution/          confined native resolver processes
├── evidence/                  checked compiler state as inert evidence
└── advisory/                  optional model-facing recommendations
```

`operations` is the only place for a complete package operation. The `omega`
binary delegates to it instead of reconstructing package policy. Install and
update will live there once their remaining acceptance and transaction gates
are closed.

`declarations` reads the statically checked package declarations in
`build.omg`; Omega has no separate package manifest. `resolution` binds those
declarations to immutable source custody and reconciles one exact closure.
`review` turns that closure into compiler-issued evidence and root-owned review
decisions. Review evidence is not an accepted lock and cannot admit a package.

The supporting crates have one-way responsibilities:

```text
package manager ──→ source ──→ source execution
                └─→ review evidence
review advisory ──→ package manager
```

Design and security references:

- [`package_manager_first_draft.md`](../../../../wiki/design_briefs/package_manager_first_draft.md)
- [`build_and_package_model.md`](../../../../wiki/design_briefs/build_and_package_model.md)
- [`SOURCE_RESOLVER_SECURITY.md`](source/SOURCE_RESOLVER_SECURITY.md)
- [`TASKS_PACKAGE_MANAGER.md`](../../../../TASKS_PACKAGE_MANAGER.md)
