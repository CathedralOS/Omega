# Target To Register Environment

[Pipeline](../pipeline.md) | Next: [Target Operations To Selected Instructions](target_operations_to_selected_instructions.md)

This stage turns one exact native target into the validated register model,
constraint catalog, and active reservation profile consumed by instruction
selection and allocation. It is target-owned preparation, not instruction
selection and not an optimization pass.

## Stage Contract

Input: one exact `NativeTarget` and, for decoded or custom inputs, the raw
physical-register model, constraint catalog, and reservation profile.

Output: `ValidatedTargetRegisterEnvironment`, binding the target, physical
model, target-semantic constraints, selected keys, reservations, and the
independently validated environment identity.

Primary responsibility: join clean ISA-owned register facts into a single
target-specific carrier before target-neutral instruction selection runs.
Selection receives this carrier as input; it must not import ISA crates or
silently reconstruct the environment.

## Implementation Map

- `pipeline/target-to-register-environment/src/lib.rs` owns construction
  and the public validation entrances.
- `src/catalog.rs` selects target-owned physical models, constraints, and the
  conservative baseline reservation profile.
- `src/validation.rs` performs the architecture-neutral join checks and then
  invokes the selected ISA's semantic validator.
- `src/model.rs` owns the inseparable validated carrier and typed failures.

Production coordination constructs the register environment as its own stage,
then passes the validated carrier to instruction selection. Selection never
reconstructs the environment implicitly.
