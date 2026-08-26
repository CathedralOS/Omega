# Object Relocations To Final Image

[Pipeline](../pipeline.md) | Previous: [Object Plan To Relocations](object_plan_to_relocations.md) | Next: none

This stage combines encoded machine bytes, data bytes, object sections/symbols, relocation records, and target image rules into final executable image data.

The clean terminal-Psi scalar lane also uses this model and the format writers.
It currently admits no imports and only provenance-bound internal-call
relocations. Final compiler text may differ from the owned terminal machine
bytes only in those relocations' architecture-specific immediate bits, and
every executable byte must belong to one provenance-bearing function region.

## Stage Contract

Input: target, object layout under `ObjectFileLayout`, relocation records under
`RelocationRecordSet`, encoded text bytes, and data bytes.

Output: final image model with memory, symbol/import, and relocation data under
named roots, plus emitted executable image output.

Primary responsibility: preserve object-level symbols/imports/relocations in a final-image representation, apply target image layout and fixups, and emit executable bytes for supported formats.

Private dynamic-conformance table slots arrive as ordinary initialized-data
`Absolute64` relocations targeting exact compiler-private function symbols.
Both AArch64 and x86-64 final-image paths apply that existing relocation form;
this stage does not inspect trait rows or reconstruct dynamic-table identity.
For PIE Mach-O output, the same typed relocation sites additionally become
dyld pointer-rebase opcodes. The writer retains the exact internal symbol and
addend, rejects malformed or duplicate data sites, and replays each patched
preferred pointer before publication; it does not disable ASLR or synthesize a
runtime slide.

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
- `omega-image/src/model/` owns final-image arena-backed data records:
  `memory.rs` owns bytes and BSS facts, `symbols.rs` owns
  entry/symbol/import facts, `relocations.rs` owns final fixups, `layout.rs`
  owns final section addresses, and `root.rs` owns `FinalImage` construction.
  Root construction should join memory, symbol-table, and relocation-table
  roots through `FinalImage::with_roots` or the capacity constructor so callers
  do not manually assemble every sub-root arena.
- `omega-image/src/builder.rs` owns object-plan and relocation-plan conversion into `FinalImage`.
- `omega-image/src/builder/copies.rs` owns object symbol, import, and
  relocation copying into final-image arenas.
- `omega-object-file/src/plan.rs` is the input object representation:
  sections, symbols, and entry symbol live under `ObjectFileLayout`.
- `omega-object-file/src/relocations.rs` is the input relocation representation:
  patch records live under `RelocationRecordSet`.
- `omega-image/src/builder/sections.rs` owns object section size/alignment
  lookup for final-image construction.
- `omega-image/src/tests.rs` owns final-image construction canaries so
  `builder.rs` stays focused on conversion orchestration.
- `omega-image/src/symbols.rs` owns final-image symbol handle mapping, symbol names, import checks, and address queries.
- `omega-image/src/output.rs` owns emitted image output DTOs.
- `omega-image/src/*_relocations.rs` owns architecture-specific final relocation math.
- `omega-image/src/patch_bytes.rs` owns checked text-section byte reads/writes for final relocation patching.
- `omega-image/src/relocation_envelope.rs` proves that final compiler text
  changed only in the exact bits named by checked relocation records.
- `omega-image-emission/src/lib.rs` owns the public direct-image emission API surface only.
- `omega-image-emission/src/input.rs` owns the executable image input DTO.
- `omega-image-emission/src/support.rs` owns direct image writer support facts by target.
- `omega-image-emission/src/dispatch.rs` owns target-to-image-writer dispatch and final-image construction handoff.
- `omega-image-emission/src/checked.rs` orchestrates planned-vs-encoded byte
  validation before direct image emission.
- `omega-image-emission/src/checked/tests.rs` is the regression-corpus root;
  final-image/instruction-boundary validation, place replay, and runtime-guard/
  checked-assembly fixtures live in separate subordinate test modules.
- `omega-image-emission/src/checked/assembly.rs` owns checked-assembly
  footprints, operand-loader semantics, exact instruction bytes, and their
  retained relocation checks.
- `omega-image-emission/src/checked/atomic_replay.rs` owns compiler atomic
  operation replay and recursive runtime-operand storage-site derivation.
- `omega-image-emission/src/checked/runtime_imports.rs` owns imported-call
  replay.
- `omega-image-emission/src/checked/runtime_imports/indirect_calls.rs` owns
  table- and vtable-field indirect-call replay.
- `omega-image-emission/src/checked/runtime_imports/runtime_io.rs` owns runtime
  byte, line, and text-boundary replay.
- `omega-image-emission/src/checked/runtime_imports/syscalls.rs` owns outbound
  syscall replay and exact storage/data relocation-target derivation.
- `omega-image-emission/src/checked/footprints.rs` owns compiler atomic and
  instruction footprint family dispatch, body/fixed-mechanics partition
  validation, and exact footprint composition.
- `omega-image-emission/src/checked/footprints/{control_entry,storage_place,
  outbound_calls,buffer_wire_text}.rs` own the four closed compiler-instruction
  footprint families consumed by that dispatcher.
- `omega-image-emission/src/checked/instruction_relocations.rs` owns the closed
  compiler instruction-relocation recipe vocabulary and replays each recipe
  against exact final bytes, symbols, and relocation sites.
- `omega-image-emission/src/checked/instruction_specs.rs` reconstructs the
  exhaustive expected-byte, class, position, and relocation recipe tuple for
  every retained compiler instruction validation kind.
- `omega-image-emission/src/checked/instruction_specs/arithmetic_convert.rs`
  owns binary-arithmetic and scalar-conversion write instruction
  specifications.
- `omega-image-emission/src/checked/instruction_specs/buffer_wire_text.rs` owns
  bit-field, bounded-buffer, wire-codec, and text-materialization instruction
  specifications.
- `omega-image-emission/src/checked/instruction_specs/control_entry.rs` owns
  fixed mechanics, runtime guards, return transport, entry transport, and
  dispatch-tail instruction specifications.
- `omega-image-emission/src/checked/instruction_specs/outbound_calls.rs` owns
  imported-call, runtime-I/O, indirect-call, and syscall instruction
  specifications.
- `omega-image-emission/src/checked/instruction_specs/storage_place.rs` owns
  compiler atomic, place-copy, place-write, and storage-result instruction
  specifications.
- `omega-image-emission/src/checked/place_copy_shapes.rs` owns the closed
  compiler place-copy shape vocabulary and its exact classifier.
- `omega-image-emission/src/checked/place_copy_offsets.rs` owns indexed and
  pointee offset decomposition shared by copy and write replay.
- `omega-image-emission/src/checked/place_copy_sites.rs` maps retained
  place-pair and place-copy shapes to exact architecture-specific relocation
  address sites.
- `omega-image-emission/src/checked/place_write_shapes.rs` owns the closed
  compiler place-write shape vocabulary and its exact classifier family.
- `omega-image-emission/src/checked/place_write_sites.rs` encodes retained
  place writes and derives their exact register and relocation sites.
- `omega-image-emission/src/checked/relocations.rs` owns exact compiler
  relocation-set validation, relocation-symbol custody, and validation that
  non-relocated instruction bits remain unchanged.
- `omega-image-elf/src/lib.rs` owns ELF emission orchestration; ELF constants, byte writing, section/address planning, entry-symbol lookup, layout helpers, and header/program-header writing live in focused sibling modules.
- `omega-image-pe/src/lib.rs` owns PE emission orchestration; PE constants, byte writing, section/RVA planning, imports, entry-symbol lookup, and headers live in focused sibling modules.
- `omega-image-macho/src/lib.rs` owns Mach-O emission orchestration; image command/section/linkedit planning, import thunks, bind info, typed internal-pointer rebase info, AArch64 thunk patching, and entry-symbol lookup live in focused sibling modules.
- The remaining ELF, Mach-O, and PE modules own format-specific executable layout and byte writing.
- `omega-terminal-image-emission/src/lib.rs` dispatches the clean terminal-Psi
  artifact to those writers and publishes relocation-envelope validation
  evidence while retaining terminal semantic identity alongside each
  object/image output.
- `omega-terminal-image-emission/src/installation.rs` owns the canonical typed
  installation-record payload over the sealed image: target facts, profile
  decision, selected provider plans, image digest, and text-validation evidence.
  It is manifest metadata, not the executable admission/placement ladder.

## Known Gaps

- The image writers still need the same single-responsibility pressure as earlier backend crates; remaining high-level image orchestration is still a cleanup seam.
- Source boundary summaries and final host imports are not yet explicitly linked by a validation/reporting pass.
- Direct executable image output exists, but object-container output and linker/image flows still need clearer relationship docs.
