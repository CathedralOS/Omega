# Object Plan To Relocations

[Pipeline](../pipeline.md) | Previous: [Machine Bytes To Object Plan](machine_bytes_to_object_plan.md) | Next: [Object Relocations To Final Image](object_relocations_to_final_image.md)

This stage derives relocation records from selected target operations, assigned runtime-value homes, encoded instruction offsets, object symbols, data objects, and host ABI bindings.

## Stage Contract

Input: target operations, assigned target operations, encoded machine bytes,
target data, object plan with artifact layout under `ObjectFileLayout`, and
host ABI.

Output: relocation plan with records under `RelocationRecordSet`.

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

- `omega-relocations/src/lib.rs` owns the public relocation-planning API surface only.
- `omega-relocations/src/input.rs` owns relocation-planning input DTOs.
- `omega-object-file/src/plan.rs` is the input object representation:
  sections, symbols, and entry symbol live under `ObjectFileLayout`.
- `omega-relocations/src/builder.rs` owns the relocation-planning entrypoint and per-function walk.
- `omega-relocations/src/lookups.rs` owns selected-instruction offset lookup.
- `omega-relocations/src/data_addresses.rs` owns scanning assigned operands for data/storage address references.
- `omega-relocations/src/data_address_records.rs` owns target-specific data-address relocation record facts.
- `omega-relocations/src/dynamic_conformances.rs` owns private dynamic-table
  slot validation and exact address-free realization-state to private-function
  symbol joins. It emits one data-section `Absolute64` materialization record
  per pointer slot and never uses a short source spelling as function
  authority.
- `omega-relocations/src/offsets/*` owns target-specific relocation offset math by family: data addresses, external calls, runtime frame indexing, runtime storage, and runtime text. `offsets/runtime_storage/*` keeps compare, copy, string-descriptor, and write/binary operand offset math split by relocation family. `offsets/runtime_text/*` keeps append, materialize, and host-backed line-read offset math split by runtime-text relocation family.
- `omega-relocations/src/instruction_records/mod.rs` routes selected instructions to focused relocation families.
- `omega-relocations/src/instruction_records/host_operation.rs` owns host-operation relocation routing, including data-address operand relocation scanning and external import call relocation records.
- `omega-relocations/src/instruction_records/runtime_storage*.rs` owns runtime storage relocation families: address, compare, copy, string descriptor, and write references.
- `omega-relocations/src/instruction_records/runtime_text*.rs` owns runtime text relocation families: append, compare, materialize, host-backed line read, and literal write references.
- `omega-relocations/src/instruction_records/runtime_values.rs` owns recursive runtime-value operand relocation extraction.
- `omega-object-file/src/relocations.rs` owns relocation-plan and relocation-record data:
  patch records live under `RelocationRecordSet`, keeping artifact relocation
  shape explicit at the plan root. Root construction should join target and
  record-set roots through `RelocationPlan::with_roots`, while callers should
  use helpers such as `with_target`, `with_record_capacity`, `push_record`,
  `record_count`, and `records` instead of constructing or walking the record
  arena directly. Origins distinguish selected instructions, full-width
  semantic-operation identities, and materialized objects; a semantic identity
  must never be narrowed into an instruction index.
- `omega-object-file/src/container.rs` owns Omega object-container serialization orchestration.
- `omega-object-file/src/container/bytes.rs` owns primitive byte writing for the Omega object container.
- `omega-object-file/src/container/ids.rs` owns stable object-container enum IDs.
- `omega-object-file/src/container/sections.rs` owns section-size facts used by object-container summaries.
- `omega-object-file/src/container/symbols.rs` and `container/relocations.rs` own symbol and relocation metadata serialization for the Omega object container.

## Known Gaps

- Keep runtime text and runtime storage relocation families aligned with instruction-selection families as new selected instructions land.
- Boundary summaries are preserved beside this stage, but target policy validation still needs explicit linkage between source boundary edges and lowered host-operation relocations.
