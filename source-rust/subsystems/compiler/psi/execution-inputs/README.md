# Inputs to Psi execution

An interpreter run and a native compilation can start from the same published Psi program. Before either consumes an external value, its request must bind that value to the declarations it serves. These checks also serve Build evaluation when Build runs a published program.

| Input | Check |
| --- | --- |
| Target and layouts | [Bind the target](target-layouts/admission.rs) and verify its selected layouts against the program. |
| Providers | [Admit provider selections](providers/selections.rs) against one callable catalog and its required bindings. |
| Borrowed entry memory | [Check entry memory](entry-memory/entry_memory.rs) against the selected row and its admitted target layout. |
| Scalar applications | [Check scalar observations](scalar-applications/scalar_applications.rs), prerequisites and entry or ABI scope against the same provider and layout selections. |

The [Build scalar evaluator](../../build/build-time/scalars/execution.rs) runs a program’s requested scalar calculations and records the results. The scalar checks above bind each result to the application it serves.

The host supplies these inputs when it requests [native code or an interpreter runtime](../../compiler.rs). Their portable declarations are recorded in the [Psi program](../psi.rs).
