# Assigned Target Operations To Machine Instructions

[Pipeline](../pipeline.md) | Previous: [Target Operations To Assigned Target Operations](target_operations_to_assigned_target_operations.md) | Next: [Target Operations To Machine Program](target_operations_to_machine_program.md)

This stage converts assigned target operations into symbolic ISA instructions without final object encoding.

## Stage Contract

Input: assigned target operations.

Output: symbolic machine instructions.

Primary responsibility: convert assigned target operations into ISA instruction forms without final object-file encoding.

## Semantic Ownership

This stage owns symbolic instruction shape only. Assigned target operations have
already chosen concrete homes; this stage turns those assigned homes and
operation kinds into inspectable machine-instruction variants without deciding
final bytes, sections, or relocations.

| Noun | Ownership |
| --- | --- |
| Places | Encoded as assigned memory/register operands. |
| Values | Become instruction operands or immediates. |
| Facts | Optional diagnostics/debug metadata only. |
| Loans | Not active. |
| Moves | Become machine copies, loads, stores, or disappear. |
| Drops | Become calls or instruction sequences. |
| Calls | Become symbolic call instructions/sequences. |
| Transitions | Become symbolic jumps, branches, returns, or dispatch mutations. |
| Effects | Represented by instruction/call sequences. |
| Boundary edges | Represented by symbolic imports, calls, syscalls, traps, or runtime sequences. |

## Ownership Rules

Must own:

- Mapping selected instruction kinds to symbolic machine instruction kinds.
- Validating that assigned runtime value homes are shape-compatible before
  symbolic instruction emission.
- Keeping dispatch, host, runtime-storage, and runtime-text instruction-shape
  helpers separate from final object encoding.

Must not own:

- Section layout, relocation application, final image policy, or encoded bytes.
- Register allocation, stack-slot assignment, or calling-convention placement.

## Implementation Map

- `builder.rs` walks assigned target operations and appends symbolic machine
  instructions.
- `shapes.rs` routes selected instruction families to shape-specific helpers.
- `shapes/dispatch.rs` owns dispatch-loop/case/state/return instruction shapes.
- `shapes/host.rs` owns host-operation instruction shapes.
- `shapes/runtime_storage.rs` routes runtime storage selected instructions to
  compare, write, address, and copy shape helpers.
- `shapes/runtime_storage/compare.rs` owns runtime storage compare shapes.
- `shapes/runtime_storage/writes.rs` routes runtime storage write shapes.
  `shapes/runtime_storage/writes/integer.rs`,
  `shapes/runtime_storage/writes/binary.rs`, and
  `shapes/runtime_storage/writes/string.rs` own integer, binary, and string
  write families respectively.
- `shapes/runtime_storage/addresses.rs` owns address-to-runtime-frame write
  shapes.
- `shapes/runtime_storage/copies.rs` owns runtime storage copy shapes.
- `shapes/runtime_text.rs` routes runtime text selected-instruction shapes.
  `shapes/runtime_text/compare.rs`, `shapes/runtime_text/write.rs`,
  `shapes/runtime_text/append.rs`, `shapes/runtime_text/materialize.rs`, and
  `shapes/runtime_text/read.rs` own the compare, write, append, materialize,
  and read families respectively.
- `shapes/validation.rs` owns pre-shape checks that assigned runtime value homes
  are present and compatible with the selected instruction.

## Known Gaps

- `shapes.rs` is still a large dispatch table; continue splitting by selected
  instruction family when a family grows enough to hide intent.
- Keep instruction selection separate from machine encoding.
