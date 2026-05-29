# Object Relocations To Final Image

[Pipeline](../pipeline.md) | Previous: [Object Plan To Relocations](object_plan_to_relocations.md) | Next: none

This stage combines encoded machine bytes, data bytes, object sections/symbols, relocation records, and target image rules into final executable image data.

## Stage Contract

Input: target, object plan, relocation plan, encoded text bytes, and data bytes.

Output: final image model and emitted executable image output.

Primary responsibility: preserve object-level symbols/imports/relocations in a final-image representation, apply target image layout and fixups, and emit executable bytes for supported formats.

## Semantic Ownership

This stage owns final artifact image data. It does not create source-level semantics; it maps already-lowered artifact metadata into loader-visible sections, addresses, imports, fixups, headers, and output summaries.

| Noun | Ownership |
| --- | --- |
| Places | Final section addresses, symbol addresses, and loader-visible storage locations only. |
| Values | Not owned; runtime value semantics must already be lowered or preserved as sibling metadata. |
| Facts | Final image layout facts: section addresses, sizes, alignments, import tables, and relocation application facts. |
| Loans | Not active. |
| Moves | Not owned. |
| Drops | Not owned. |
| Calls | Final import thunks, entry-point addresses, and call relocation fixups only. |
| Transitions | Final branch/call relocation fixups only. |
| Effects | Artifact output effects only: executable image bytes and format-specific loader metadata. |
| Boundary edges | Final host-import/image-boundary metadata only; source boundary policy must already be validated or preserved separately. |

## Ownership Rules

Must own:

- Final image model construction from object sections, symbols, imports, and relocation records.
- Target-specific executable image layout for ELF, Mach-O, PE/COFF, and future formats.
- Final relocation/fixup application against concrete image addresses.
- Emitted image output records for direct executable construction.

Must not own:

- Object section/symbol planning.
- Relocation record discovery.
- Instruction encoding.
- Source-level effect, capability, borrow, or proof validation.

## Implementation Map

- `omega-image/src/lib.rs` owns the public final-image API surface only.
- `omega-image/src/model.rs` owns final-image arena-backed data records.
- `omega-image/src/builder.rs` owns object-plan and relocation-plan conversion into `FinalImage`.
- `omega-image/src/symbols.rs` owns final-image symbol handle mapping, symbol names, import checks, and address queries.
- `omega-image/src/output.rs` owns emitted image output DTOs.
- `omega-image/src/*_relocations.rs` owns architecture-specific final relocation patching helpers.
- `omega-image-emission/src/lib.rs` owns target-to-image-writer dispatch and checked direct-executable emission.
- `omega-image-pe/src/lib.rs` owns PE emission orchestration; PE constants, byte writing, layout helpers, imports, and headers live in focused sibling modules.
- `omega-image-elf`, `omega-image-macho`, and the remaining PE modules own format-specific executable layout and byte writing.

## Known Gaps

- The image writers still need the same single-responsibility pressure as earlier backend crates; PE and Mach-O roots are likely next cleanup seams.
- Source boundary summaries and final host imports are not yet explicitly linked by a validation/reporting pass.
- Direct executable image output exists, but object-container output and linker/image flows still need clearer relationship docs.
