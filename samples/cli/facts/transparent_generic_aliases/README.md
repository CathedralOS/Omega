# Transparent generic alias witnesses

These focused Rust-producer witnesses exercise transparent aliases without wrapper storage or a distinct alias ABI identity. Each subfolder owns its programs, intended-invalid controls and actual producer/host observations.

| Witness | Exercised behavior | Observed host |
| --- | --- | --- |
| [Imported representation](imported_representation/README.md) | Partial application, computed const policies, caller-name hygiene, explicit-carrier interchange and byte offsets | Linux x86_64 |
| [Borrowed view](borrowed_view/README.md) | Own lifetime/Type telescope, explicit-reference interchange and distinct returned-input custody | Linux x86_64 |
| [Runtime subject](runtime_subject/README.md) | Value-subject capture across mutation and erased argument carrier obligations | Linux x86_64 |
| [Machine contracts](machine_selection/README.md) | Nested alias-owned callable promises, selected callback results and erased argument obligations | Linux x86_64 |
| [RHS const expression](rhs_const_expression/README.md) | Declaration-owned Const occurrence substitution, explicit array interchange and Exact overflow | Linux x86_64 |

The machine witness expands to `u64`; these erased aliases are the same runtime type. The existing restriction on finite-layout data with machine binders remains in force.

The parent [LIBRARY-REPRESENTATION-FAMILIES task](../../../../TASKS.md) remains open: complete nine-place rows, remaining binder compositions, qualified alias cases/patterns and exposure, alias-RHS expression operands, captured Product/Build alias transport, other hosts and the Omega-written producer are not established by these observations.
