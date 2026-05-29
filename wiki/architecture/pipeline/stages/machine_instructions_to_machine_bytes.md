# Machine Instructions To Machine Bytes

[Pipeline](../pipeline.md) | Previous: [Assigned Target Operations To Machine Instructions](assigned_target_operations_to_machine_instructions.md) | Next: [Machine Bytes To Object Plan](machine_bytes_to_object_plan.md)

This stage encodes symbolic machine instructions into target machine bytes while preserving semantic summaries for diagnostics, reports, relocation planning, and later artifact policy.

## Stage Contract

Input: symbolic machine instructions plus assigned target-operation context.

Output: encoded machine bytes.

Primary responsibility: compute instruction widths, lay out symbolic
instructions from `MachineInstructionCode`, encode their final byte spans, and
retain the semantic summaries already attached to the symbolic instruction
plan.

## Semantic Ownership

This stage owns byte encoding and instruction byte spans. It does not create new language semantics; values, ownership events, and boundary edges are preserved as metadata so later object/image stages can still report or validate what those bytes came from.

| Noun | Ownership |
| --- | --- |
| Places | Already lowered into assigned memory/register operands; encoded only as bytes and offsets. |
| Values | Preserved as encoded-machine metadata; byte emission does not create new value facts. |
| Facts | Diagnostic/debug metadata only. |
| Loans | Not active. |
| Moves | Preserved as ownership metadata; explicit copy/drop lowering is still pending. |
| Drops | Preserved as ownership metadata; explicit cleanup byte emission is still pending. |
| Calls | Encoded into call/syscall/import sequences. |
| Transitions | Encoded into branches, dispatch mutations, and returns. |
| Effects | Encoded as concrete instruction bytes for the already-selected operation sequences. |
| Boundary edges | Preserved as encoded-machine metadata beside host-operation byte sequences. |

## Ownership Rules

Must own:

- Instruction width calculation for the selected target.
- Byte layout for a function's symbolic instruction span.
- Final byte emission for fixed and target-encoded instruction families.
- Preserving semantic summaries from `MachineInstructionPlan` into `EncodedMachinePlan`.

Must not own:

- Register allocation, stack-slot assignment, or ABI placement.
- Symbol selection, section layout, relocation records, or final image policy.
- Borrow checking, proof discharge, or source-level effect validation.

## Implementation Map

- `omega-machine-emission/src/emitter.rs` owns the public stage entrypoint and
  assembles target identity, encoded code, and preserved semantic summaries
  into the encoded plan.
- `omega-machine-emission/src/code.rs` owns `EncodedMachineCode` construction,
  including encoded function ranges, instruction byte spans, final byte count,
  and delegation to function/instruction byte insertion helpers.
- `omega-machine-emission/src/instruction_bytes.rs` owns function-local byte
  insertion, fixed instruction encodings, target-encoded instruction fallback,
  and encoded-width validation.
- `omega-machine-instructions/src/plan.rs` is the input representation root:
  symbolic executable instruction shape lives under `MachineInstructionCode`,
  while preserved semantic evidence lives under
  `MachineInstructionSemanticSummary`.
- `omega-machine-emission/src/layout.rs` owns instruction width and byte-offset layout.
- `omega-machine-emission/src/encoding.rs` and `encoding/*` own target byte emission helpers.
- `omega-machine-emission/src/branch_distances.rs` and submodules own byte-distance queries used by branch encoding.
- `omega-machine-bytes/src/plan.rs` is the output representation root:
  encoded executable byte shape lives under `EncodedMachineCode`, while
  preserved semantic evidence lives under `EncodedMachineSemanticSummary`.
- `omega-machine-bytes/src/semantics.rs` owns encoded-stage semantic aliases.
  `EncodedMachineSemanticSummary` is the preserved backend semantic spine, not
  a new duplicate values/boundaries/ownership container.
- `omega-machine-bytes/src/functions.rs` owns encoded function ranges.
- `omega-machine-bytes/src/instructions.rs` owns encoded instruction byte spans.

## Known Gaps

- Move/drop summaries are preserved only as metadata; cleanup behavior still needs explicit lowering before or during instruction selection.
- Value summaries are preserved only as metadata; storage/drop consequences still need deliberate lowering.
- Boundary summaries preserve both source boundary edges and lowered host-operation edges, but target policy validation still needs to link those layers explicitly.
- Object planning and relocation planning consume encoded bytes today, but their semantic ownership docs still need the same compact ownership-table treatment.
