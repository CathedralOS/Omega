# Machine Bytes To Object Plan

[Pipeline](../pipeline.md) | Previous: [Machine Instructions To Machine Bytes](machine_instructions_to_machine_bytes.md) | Next: [Object Plan To Relocations](object_plan_to_relocations.md)

This stage turns encoded machine bytes, data bytes, layout facts, and host ABI imports into an object-file plan made of sections and symbols.

## Stage Contract

Input: encoded machine bytes under `EncodedMachineCode`, target data, layout,
host ABI, and entry-point metadata.

Output: object plan with sections, symbols, and entry symbol under
`ObjectFileLayout`.

Primary responsibility: construct artifact-level sections and symbols for text, data, bss, imports, runtime frame storage, machine storage, and the entry function.

Backend orchestration shape: the aggregate `BackendPlan` keeps symbolic machine
instructions, encoded machine bytes, object layout, and relocation records under
`BackendArtifactRoots`. Individual stages still own their artifact type, but the
orchestration root now makes the final artifact chain visible as one spine.

## Semantic Ownership

This stage owns object sections and symbol metadata. It does not own source-level values, facts, loans, moves, drops, or boundary semantics; those remain sibling metadata on the encoded-machine/backend plan until a later reporting or validation stage consumes them.

| Noun | Ownership |
| --- | --- |
| Places | Artifact storage symbols and section offsets only. |
| Values | Not owned; encoded-machine value summaries remain sibling metadata. |
| Facts | Not owned except artifact sizing/alignment facts. |
| Loans | Not active. |
| Moves | Not owned; encoded-machine ownership summaries remain sibling metadata. |
| Drops | Not owned; encoded-machine ownership summaries remain sibling metadata. |
| Calls | Import and entry symbols only. |
| Transitions | Entry symbol offset only. |
| Effects | Artifact shape for host imports and sections only. |
| Boundary edges | Not owned; encoded-machine boundary summaries remain sibling metadata. |

## Ownership Rules

Must own:

- Text/data/bss section records.
- Entry, import, machine-storage, runtime-frame, and data-object symbols.
- Object-level offsets, sizes, and alignments.

Must not own:

- Instruction encoding, relocation records, final image policy, or loader metadata.
- Borrow checking, proof discharge, effect validation, or boundary contract validation.
- Semantic summaries that belong to encoded-machine or earlier representations.

## Implementation Map

- `omega-object-file-planning/src/lib.rs` owns the public stage boundary only.
- `omega-object-file-planning/src/builder.rs` owns object-plan orchestration.
- `omega-object-file-planning/src/entry.rs` owns entry machine layout and
  encoded entry-function lookup diagnostics.
- `omega-object-file-planning/src/sections.rs` owns text/data/bss section sizing and runtime-frame offset placement.
- `omega-object-file-planning/src/symbols.rs` owns entry, storage, import, runtime-frame, and data-object symbol construction.
- `omega-object-file-planning/src/tests.rs` owns object-planning canaries so
  orchestration code stays separate from test fixtures.
- `omega-machine-bytes/src/plan.rs` is the input representation root:
  encoded executable byte shape lives under `EncodedMachineCode`, while
  preserved semantic evidence lives under `EncodedMachineSemanticSummary`.
- `omega-machine-bytes/src/semantics.rs` keeps encoded semantic summary names
  as aliases over the preserved backend semantic spine.
- `omega-object-file/src/plan.rs` owns the object-plan container: artifact
  sections, symbols, and entry symbol live under `ObjectFileLayout`. Object
  planning should use the representation-level constructors for `ObjectPlan`
  and `ObjectFileLayout`, not hand-build their arena roots.
- `omega-object-file/src/sections.rs` owns section records.
- `omega-object-file/src/symbols.rs` owns symbol records and handles.
- `omega-object-file/src/names.rs` owns target-specific object symbol and section names.

## Known Gaps

- The stage docs now state the semantic cutoff, but diagnostics/reporting still need a clear consumer for encoded-machine semantic summaries at artifact time.
