# Machine Bytes To Object Plan

[Pipeline](../pipeline.md) | Previous: [Machine Instructions To Machine Bytes](machine_instructions_to_machine_bytes.md) | Next: [Object Plan To Relocations](object_plan_to_relocations.md)

This stage turns encoded machine bytes, data bytes, layout facts, and host ABI imports into an object-file plan made of sections and symbols.

The clean terminal-Psi lane enters the same object model through
`omega-terminal-image-emission`. It consumes `TerminalMachineCodePlan` directly,
retains the exact terminal semantic identity and per-function Psi provenance,
and does not reconstruct an `EncodedMachineCode` carrier.
Its scalar slice owns canonical-order text functions, their symbols, and typed
internal-call sites. Object construction accepts those sites only when their
operation identities occur in the function's retained Psi provenance and the
named architecture-native immediate field is still unpatched.

## Stage Contract

Input: encoded machine bytes under `EncodedMachineCode`, target data, layout,
host ABI, and entry-point metadata.

Output: object plan with sections, symbols, and entry symbol under
`ObjectFileLayout`.

Primary responsibility: construct artifact-level sections and symbols for text, data, bss, imports, runtime frame storage, machine storage, and the entry function.

Address-free local dynamic-conformance tables arrive as ordinary target-data
objects: pointer-aligned zero-filled slot bytes plus a private symbol. Object
planning publishes their exact existing object spans and does not rediscover
trait rows or choose realization addresses. The following relocation stage
owns binding each retained address-free row target to a private function.

Backend orchestration shape: the aggregate `BackendPlan` keeps symbolic machine
instructions, encoded machine bytes, object layout, and relocation records under
`BackendArtifactRoots`. Individual stages still own their artifact type, but the
orchestration root now makes the final artifact chain visible as one spine and
exposes semantic-summary accessors for artifact-time diagnostics. Artifact
fixtures and skeletons should use `BackendArtifactRoots::with_roots` or the
target-aware empty constructor rather than assembling the artifact spine
field-by-field.

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
  planning and artifact fixtures should join those roots through
  `ObjectFileLayout::with_roots` and `ObjectPlan::with_layout`, not hand-build
  their arena roots.
- `omega-object-file/src/sections.rs` owns section records.
- `omega-object-file/src/symbols.rs` owns symbol records and handles.
- `omega-object-file/src/names.rs` owns target-specific object symbol and section names.
- `omega-terminal-image-emission/src/lib.rs` owns the clean terminal-Psi object
  artifact, canonical function-span validation, Omega object-container
  emission, and handoff to the shared final-image model.

## Known Gaps

- Artifact-time diagnostics can now query preserved semantic evidence through
  `BackendArtifactRoots`; the remaining gap is adding concrete diagnostics that
  use that view instead of reaching directly through encoded-machine internals.
