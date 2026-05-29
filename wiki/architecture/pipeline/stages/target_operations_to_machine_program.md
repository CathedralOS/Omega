# Target Operations To Machine Program

[Pipeline](../pipeline.md) | Previous: [Assigned Target Operations To Machine Instructions](assigned_target_operations_to_machine_instructions.md) | Next: none

This aggregate stage builds the current `MachineProgram` artifact from target-operation data by composing assignment and symbolic instruction emission.

## Stage Contract

Input: target-aware operations.

Output: `MachineProgram`.

Primary responsibility: compose target-operation assignment and symbolic machine-instruction emission into the current backend artifact.

## Implementation Map

- `lib.rs` owns the public stage entrypoint only.
- `builder.rs` owns composition of `omega-target-operations-to-assigned-target-operations` and `omega-assigned-target-operations-to-machine-instructions`, then wraps the result as a `MachineProgram`.

## Semantic Ownership

- Places: forwarded into assignment and symbolic instruction emission; no new place semantics are created here.
- Values: forwarded into assignment and symbolic instruction emission; no new value semantics are created here.
- Facts: not active except diagnostics/debug metadata carried by lower stages.
- Loans: not active.
- Moves: ownership summaries are forwarded through assignment, symbolic instruction emission, and the current `MachineProgram` artifact.
- Drops: ownership summaries are forwarded through assignment, symbolic instruction emission, and the current `MachineProgram` artifact.
- Calls: already represented by target/assigned/symbolic instruction stages.
- Transitions: already represented by target/assigned/symbolic instruction stages.
- Effects: already represented by target/assigned/symbolic instruction stages.
- Boundary edges: boundary-edge summaries are forwarded through assignment,
  symbolic instruction emission, and the current `MachineProgram` artifact.

## Ownership Rules

- Must stay a thin composition bridge while `MachineProgram` is the current backend artifact.
- Must not absorb assignment policy, instruction-shape policy, object encoding, final image layout, semantic validation, proof discharge, or borrow checking.
- Must not pretend object files or final images are current pipeline outputs until those representation crates/stages exist.

## Known Gaps

This is an aggregate bridge, not the final backend architecture. Object-file emission and direct final-image construction remain future representation boundaries, but they should be documented as future backend work rather than current pipeline stages.
The bridge preserves ownership summaries as metadata, but explicit transfer and
cleanup instruction lowering remains future work.
The bridge also preserves boundary-edge summaries as metadata; those summaries
still need source-level checked boundary contract linkage.
