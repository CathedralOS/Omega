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
| Boundary edges | Preserved as encoded-machine metadata beside host-operation byte sequences, including policy-check records. |

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
- `omega-machine-emission/src/semantics.rs` owns encoded semantic summary
  assembly. It should preserve value, boundary, and ownership roots through the
  shared semantic-summary constructor instead of cloning the whole summary as an
  opaque blob.
- `omega-machine-emission/src/code.rs` owns `EncodedMachineCode` construction,
  including encoded function ranges, instruction byte spans, final byte count,
  and delegation to function/instruction byte insertion helpers.
- `omega-machine-emission/src/instruction_bytes.rs` owns function-local byte
  insertion, fixed instruction encodings, target-encoded instruction fallback,
  and encoded-width validation.
- `omega-machine-instructions/src/plan/` is the input representation root:
  symbolic executable instruction shape lives under `MachineInstructionCode`,
  while preserved semantic evidence lives under
  `MachineInstructionSemanticSummary`. `plan/code.rs` owns root structs and
  `plan/capacity.rs` owns capacity construction.
- `omega-machine-emission/src/layout.rs` owns instruction width and byte-offset layout.
- `omega-machine-emission/src/encoding.rs` and `encoding/*` own target byte emission helpers.
- `omega-machine-emission/src/branch_distances.rs` and submodules own byte-distance queries used by branch encoding.
- `omega-isa-x86_64/src/lib.rs` remains the public x86-64 encoding surface and
  re-exports focused implementation modules. `function_frame.rs` owns the
  ordinary saved-register/MXCSR entry and return envelope;
  `function_boundary.rs` owns entry argument/aggregate unmarshalling and scalar
  result materialization; and
  `privileged_effects.rs` owns halt, fences, interrupt control, descriptor-table
  loading, flags, model-specific/control-register access, and port I/O. Those
  modules retain their exact width, byte, clobber, and machine-state contracts;
  `atomics.rs` separately owns load/store and read-modify-write encodings,
  including the exact prior-value result and relocation-site calculations;
  `syscalls.rs` owns Linux syscall register marshalling, value returns,
  timespec adapters, and their exact data-relocation offsets; `wire.rs` owns
  Compact Binary append/read, scalar, byte-slice, nested, repeated-field,
  predicate, and UTF-8 encodings together with their exact widths, clobbers,
  machine state, and page offsets; `runtime_text.rs` owns stored/literal append,
  materialization/comparison, Win64/Linux line-read adapters, and bounded text
  carriers together with their exact relocation offsets; `host_calls.rs` owns
  generic host dispatch, authored imports, normalized Win64/System V argument
  and result placement, direct/vtable/table calls, byte I/O, and exact
  relocation-site replay. Its ABI regression corpus is compiled separately
  under `host_calls/tests.rs`. `runtime_storage/scalar.rs` owns runtime value
  comparison and operand replay, binary arithmetic, conversion, and text
  equality; `runtime_storage/places.rs` owns integer, bit-field, indexed-place
  writes and copy-layout contracts. Scalar policy regressions are compiled
  separately under `runtime_storage/scalar_tests.rs`; `dispatch.rs` owns
  dispatch-loop, case-entry, state-write, case-leave, and storage-backed static
  guard encodings together with their exact widths and machine-state effects.
  `encoding_primitives.rs` owns the crate-internal register moves,
  loads/stores, checked displacements, copy-chunk iteration, and atomic byte
  helpers shared by those families. The crate root is only the public module
  and relocation-contract surface; it does not reconstruct any encoding
  family.
- `omega-isa-aarch64/src/aarch64/runtime_storage.rs` owns the remaining AArch64
  shared address formation, raw storage load/result-write primitives, scratch
  register contracts, and runtime-storage production orchestration.
  `runtime_storage/runtime_values.rs` owns recursive runtime operand replay,
  text equality, integer and floating arithmetic, arithmetic-domain policy,
  classification, and their exact byte-size/width contracts.
  `runtime_storage/atomics.rs` owns
  atomic load/store and read-modify-write encoding, ordering selection,
  observed-prior result writes, exact widths, and result relocation-site
  offsets behind the unchanged public re-exports.
  `runtime_storage/conversion.rs` owns scalar integer/float conversion,
  direct/indexed/pointee result placement, float-to-integer trap and saturation
  policy, and the shared recursive-operand conversion evaluator.
  `runtime_storage/comparison.rs` owns direct place-pair, place-value, and
  computed-value comparisons, their register and machine-state contracts, and
  exact operator-to-failure-branch mapping.
  `runtime_storage/scalar_writes.rs` owns recursive-operand register and
  machine-state contracts plus immediate integer, bit-field, direct binary,
  pointee binary, saturation, and trapping writes. Its AArch64 signed 64-bit
  Saturating divide/remainder sequence keeps a fixed-width guarded `-1` split:
  `MIN / -1` selects `MAX`, the paired remainder selects zero, and other
  nonzero divisors retain `SDIV`/`MSUB`; unsigned division retains `UDIV`.
  Production and the width twin agree for direct writes and recursive operands,
  while target-neutral formation remains responsible for excluding zero.
  `runtime_storage/bounded_buffers.rs` owns direct, pointee, indexed, and
  double-indexed bounded-buffer writes plus literal and source appends behind
  the unchanged public re-exports. `runtime_storage/string_writes.rs` owns
  direct, pointee, indexed, and double-indexed string-descriptor writes plus
  their closed register and machine-state ceilings.
  `runtime_storage/address_writes.rs` owns direct, pointee, frame-indexed,
  machine-indexed, and double-indexed place-address writes plus their exact
  clobber and machine-state ceilings. `runtime_storage/indexed_writes.rs` owns
  descriptor, pointee, frame, and machine single- and double-indexed integer
  and binary result writes, including their operand and clobber contracts.
  `runtime_storage/storage_copies.rs` owns direct, pointee, single- and
  double-indexed, cross-region, and indexed-pair copy encoders plus exact chunk
  partitioning and clobber contracts. Byte, width, clobber,
  atomic-ordering, conversion-policy, indexed-place, and floating-policy
  regressions are compiled separately through `runtime_storage_tests.rs`;
  production does not embed that second responsibility.
- `omega-machine-bytes/src/plan/` is the output representation root:
  encoded executable byte shape lives under `EncodedMachineCode`, while
  preserved semantic evidence lives under `EncodedMachineSemanticSummary`.
  Constructors should initialize that semantic root through the shared
  semantic-summary constructor, not an opaque default. `plan/code.rs` owns the
  root structs and `plan/capacity.rs` owns capacity construction.
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
