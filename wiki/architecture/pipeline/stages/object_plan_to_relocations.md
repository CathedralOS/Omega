# Object Plan To Relocations

[Pipeline](../pipeline.md) | Previous: [Machine Bytes To Object Plan](machine_bytes_to_object_plan.md) | Next: [Target Operations To Machine Program](target_operations_to_machine_program.md)

This stage derives relocation records from selected target operations, assigned runtime-value homes, encoded instruction offsets, object symbols, data objects, and host ABI bindings.

## Stage Contract

Input: target operations, assigned target operations, encoded machine bytes, target data, object plan, and host ABI.

Output: relocation plan.

Primary responsibility: map instruction/data references to object symbols and text offsets so object/container/image emission can patch or describe unresolved addresses.

## Semantic Ownership

This stage owns relocation records only. It reads semantic-adjacent metadata to find the correct selected instruction, host operation, runtime text, data object, or storage symbol, but it does not become the owner of source-level calls, transitions, effects, or boundary contracts.

| Noun | Ownership |
| --- | --- |
| Places | Artifact symbol references and storage-region relocations only. |
| Values | Not owned; value summaries remain sibling metadata. |
| Facts | Not owned except relocation width/kind/offset facts. |
| Loans | Not active. |
| Moves | Not owned; ownership summaries remain sibling metadata. |
| Drops | Not owned; ownership summaries remain sibling metadata. |
| Calls | Relocation records for host calls/imports and branch/call targets. |
| Transitions | Relocation records for branch/call targets when required. |
| Effects | Relocation records for artifact-level references only. |
| Boundary edges | Not owned; boundary summaries remain sibling metadata. |

## Ownership Rules

Must own:

- Mapping selected instruction indices to text offsets.
- Host-operation, runtime-text, data-object, and storage-symbol relocation records.
- Target-specific relocation kind and byte-width selection.

Must not own:

- Object section/symbol construction.
- Instruction byte emission.
- Source-level boundary policy validation, borrow checking, or proof discharge.

## Implementation Map

- `omega-relocations/src/lib.rs` owns the relocation-planning entrypoint and per-function walk.
- `omega-relocations/src/lookups.rs` owns selected-instruction offset lookup.
- `omega-relocations/src/data_addresses.rs` owns data/storage address relocations.
- `omega-relocations/src/instruction_records/*` owns instruction-family relocation extraction.
- `omega-object-file/src/relocations.rs` owns relocation-plan and relocation-record data.

## Known Gaps

- `instruction_records/mod.rs` is still a large dispatch table and should be split by selected instruction family.
- Runtime-text relocation helpers are still relatively dense; split further if new text/storage families land.
- Boundary summaries are preserved beside this stage, but target policy validation still needs explicit linkage between source boundary edges and lowered host-operation relocations.
