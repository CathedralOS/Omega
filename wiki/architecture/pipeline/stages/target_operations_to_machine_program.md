# Target Operations To Machine Program

[Pipeline](../pipeline.md) | Previous: [Target Operations To Assigned Target Operations](target_operations_to_assigned_target_operations.md) | Next: [Assigned Target Operations To Machine Instructions](assigned_target_operations_to_machine_instructions.md)

This aggregate stage builds the current `MachineProgram` artifact from target-operation data by composing assignment and symbolic instruction emission.

## Stage Contract

Input: target-aware operations.

Output: `MachineProgram` with executable artifact shape under
`MachineProgramCode`.

Primary responsibility: compose target-operation assignment and symbolic machine-instruction emission into the current backend artifact.

## Implementation Map

- `lib.rs` owns the public stage entrypoint only.
- `builder.rs` owns composition of `omega-target-operations-to-assigned-target-operations` and `omega-assigned-target-operations-to-machine-instructions`, then wraps the result as a `MachineProgram`.
- `omega-machine-program/src/plan/` owns the aggregate machine-program
  artifact root: functions and instructions live under `MachineProgramCode`.
  `plan/code.rs` owns root structs and `plan/capacity.rs` owns capacity
  construction.
- `omega-machine-program/src/semantics.rs` owns aggregate machine-program
  semantic aliases. `MachineSemanticSummary` is the preserved backend semantic
  spine, not a new duplicate values/boundaries/ownership container.

## Semantic Ownership

| Noun | Ownership |
| --- | --- |
| Places | Forwarded into assignment and symbolic instruction emission; no new place semantics are created here. |
| Values | Forwarded into assignment, symbolic instruction emission, and the current `MachineProgram` artifact; no new value semantics are created here. |
| Facts | Not active except diagnostics/debug metadata carried by lower stages. |
| Loans | Not active. |
| Moves | Ownership summaries are forwarded through assignment, symbolic instruction emission, and the current `MachineProgram` artifact. |
| Drops | Ownership summaries are forwarded through assignment, symbolic instruction emission, and the current `MachineProgram` artifact. |
| Calls | Already represented by target/assigned/symbolic instruction stages. |
| Transitions | Already represented by target/assigned/symbolic instruction stages. |
| Effects | Already represented by target/assigned/symbolic instruction stages. |
| Boundary edges | Boundary-edge summaries are forwarded through assignment, symbolic instruction emission, and the current `MachineProgram` artifact. |

## Ownership Rules

- Must stay a thin composition bridge while `MachineProgram` is the current backend artifact.
- Must not absorb assignment policy, instruction-shape policy, object encoding, final image layout, semantic validation, proof discharge, or borrow checking.
- Must not absorb object planning, relocation planning, or final-image policy now that those representation crates/stages exist.

## Known Gaps

This is an aggregate bridge, not the final backend architecture. Object-plan, relocation-plan, and direct final-image construction now have their own representation boundaries; this bridge should keep shrinking rather than becoming a second backend truth.
The bridge preserves ownership summaries as metadata, but explicit transfer and
cleanup instruction lowering remains future work.
The bridge also preserves boundary-edge summaries as metadata; those summaries
still need source-level checked boundary contract linkage.
The bridge preserves value summaries as metadata, but explicit storage/drop
consequence lowering remains future work.
