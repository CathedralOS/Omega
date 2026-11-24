# Atlas packet unit_020: native stage 14 object plans and the dynamic ELF writer

Packet complete. Every correctness claim in `records/unit_020.md` has a disposition below, and none shows a
wrong result. The compiler accepts no bad object or ELF file through any path the record describes. Four
claims were stale source comments or a duplicate helper. This packet's code commit corrects them without
changing behavior. No claim stays open.

Evidence baseline: `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. The survey revision is
`4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Since the survey, five merged changes touched the unit's files:
[#5376](https://github.com/CathedralOS/Omega/pull/5376) (`dynamic_executable.rs`, `load_placement.rs`),
[#5353](https://github.com/CathedralOS/Omega/pull/5353) (`imports.rs` comment),
[#5304](https://github.com/CathedralOS/Omega/pull/5304) (`object_plan.rs`, `file_assembly.rs`),
[#5280](https://github.com/CathedralOS/Omega/pull/5280) (`relocation_free_object/mod.rs`) and
[#5246](https://github.com/CathedralOS/Omega/pull/5246) (Windows ARM64: `target_matrix.rs`,
`relocation_free_object/`, `load_placement.rs`). Gap packets `unit_020_gap_007` and `unit_020_gap_008` own the
ranges that `second_pass_ranges.tsv` lists for unit 20. This packet covers only the ranges the record read.

Paths: `S14` = `source-rust/subsystems/compiler/psi-to-native/pipeline/14_placed-functions-to-objects/src`,
`ELF` = `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-elf/src`,
`IMG` = `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image`,
`R` = `records/unit_020.md`, `D11` = `digests/11-native-stages-11-15.md`.

## Claims

| # | Original claim | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | `R` `S14/target_matrix.rs` §3 and §9: `(Aarch64, Coff)` returns `None`, and callers map it to `NonCanonicalTarget`. | `S14/target_matrix.rs:36-58` | [#5246](https://github.com/CathedralOS/Omega/pull/5246) added the `windows_arm64` target. `(Aarch64, Elf \| Coff)` now returns `main`, `.text`, alignment 4. The fixture `windows_arm64_objects_use_the_coff_names_and_aarch64_alignment` pins the row. Every `(Architecture, ObjectFormat)` pair now has a row. | **Fixed** in #5246. The header comment still said five pairs and `None` for `(Aarch64, Coff)`. This packet corrects the comment. |
| 2 | `R` `ELF/file_assembly.rs` §9(a), five-things item 2; `D11:289` (§10 item 4); `D11:278` (header disagreement). Stage 22 does not re-check the final bytes in production, though the header says it re-verifies them. | `ELF/file_assembly.rs:184-251` (`admit_elf_dynamic_executable`, `derive_executable_output`), `:352-393` (`#[cfg(test)] validate_executable_output`) | Admission accepts only a `ValidatedElfAssembledDynamicFile`. That type exists only after stage 21 replays and seals the assembled file. Admission then derives the output from it. The test-only validator repeats that derivation and also checks the `\x7fELF` class bytes. The [native pipeline rule](../../../../../AGENTS.md) "Verify at trust boundaries" says default compilation does not re-verify its own stage output. | **Intended.** The stale header claimed a second check. This packet corrects the comment. |
| 3 | `R` `ELF/imports.rs` §9(a), five-things item 4; `D11:291` (§10 item 6). Unreferenced imports are skipped, but the header says they are rejected. | `ELF/imports.rs:1-6`, `:162-175` | The existing area ledger row "11, ELF imports without relocations" records the comment correction from [#5353](https://github.com/CathedralOS/Omega/pull/5353). | **Duplicate** of that row. Already fixed. |
| 4 | `R` `ELF/imports.rs` §9(b), five-things item 4; `D11:291`. A relocation that names an invalid symbol handle is skipped with `continue` and passes the import collector. | `ELF/imports.rs:178-196`; `ELF/file_assembly.rs:2476-2511`; `ELF/static_executable.rs:223`; `ELF/shared_component.rs:309`; `IMG/src/relocations.rs:24-37`; `IMG/src/lib.rs:211-233` | The collector checks only import rows. A relocation that names no import is image-local. All three ELF routes (dynamic `relocate_image_local_memory`, static and shared component) pass the image-local relocations to `image::apply_relocations`. There `final_image_symbol_address` returns `None` for a handle with no symbol, and the call fails with "relocation references unknown symbol". The fixture `IMG/tests/relocation_rejections.rs:171-187` applies a relocation with `ObjectSymbolHandle::invalid()` and expects that refusal with the text unchanged. | **Intended.** A later stage refuses the program. |
| 5 | `R` `S14/object_plan.rs` §9. `ObjectFileLayout::with_capacity*` sets `entry_symbol` to an invalid handle, and `RelocationRecord::default` uses invalid handles. | `S14/object_plan.rs:66-95`, `:432-446`; producers `S14/body_object.rs:112`, `S14/bridge_object.rs:114`, `image-emission/src/entries.rs:2656`; consumer `IMG/src/lib.rs:242-256` | The invalid handle is the zero state allowed by the data-oriented storage rule. Each producer sets the entry before publishing. If none is set, `final_image_entry_symbol` refuses the image with "entry symbol ... is missing from the final image". No production code builds a `RelocationRecord` from `Default`. Every constructor (`body_object/code_descriptors.rs:202`, `image_addresses.rs:11`, `native_code_layouts.rs:146`, `:193`, `object_plan.rs:186`, `:237`, `bridge_object.rs:120`) names every field. | **Intended.** |
| 6 | `R` `S14/relocation_free_object/publication.rs` §9. `decode` does not check the manifest statistics against an object. A manifest with wrong counts and a self-consistent identity decodes. | `S14/relocation_free_object/publication.rs:142-240`; `S14/fragment_container.rs:166-206` | Only the module's tests call `FunctionFragmentObjectContainerManifest::decode`. The production validator `validate_optimized_relocation_free_object_container` rebuilds the manifest, statistics included, from the object and container (`construct_manifest`). It refuses any difference with `ManifestMismatch`. | **Intended.** Statistics are bound where an object is available. |
| 7 | `R` `S14/relocation_free_object/mod.rs` §6. The register decode table is written by hand and duplicates `MachineRegister::wire_class_index`. | `S14/relocation_free_object/mod.rs:843-870`; `targets/calling-conventions/src/plans/vocabulary.rs:50-72` | The decoder is the inverse of `wire_class_index`. All 19 variants and the index ranges (x86 0–15, xmm 0–15, x 0–30, v 0–31) agree. An unknown pair is refused with `InvalidCallbackDestination`. | **Duplicate** code that gives the same answer. Not a correctness defect. |
| 8 | `R` `S14/relocation_free_object/mod.rs` §9; five-things item 5; `D11:277` (symbol names). The validator expects `__omega_terminal_machine_{id}`, while body objects use `omega_terminal_machine_{id}`. | `S14/relocation_free_object/mod.rs:244-246`; `S14/body_object.rs:103`; `image-emission/src/function_fragments/validation.rs:569` | The relocation-free plan is published only inside the `OMGTRO` fragment container. Its manifest records the executable image as `Unavailable`. Body objects feed the image writers. Each validator checks its own producer's spelling, and no code compares names across the two. | **Intended** (separate representations). The naming difference remains in the D11 §9 disagreement list, a naming question rather than a wrong result. |
| 9 | `R` `ELF/dynamic_executable.rs` §9. The third `seal_stage!` form replays with `\|_\| true` and an empty message. | `ELF/dynamic_executable.rs:349-371` | That form serves stages with no fingerprint field. It still runs the stage's full `$validate` replay. Only the fingerprint comparison is absent, because there is no fingerprint to compare. | **Intended.** |
| 10 | `R` `ELF/dynamic_executable.rs` §6, `ELF/procedure_linkage.rs` §6, `S14/object_plan.rs` §6. `image_section_tag` and `procedure_linkage`'s private `section_tag` are the same table. | `ELF/dynamic_executable.rs:684`; `ELF/procedure_linkage.rs:564`, formerly `:571-578` | The two tables gave identical bytes. Line `:2528` of the same file already used the shared function. | **Fixed** in this packet: the relocation fingerprint now calls `crate::dynamic_executable::image_section_tag`. The fingerprint bytes are unchanged. The `EntryFixupKind` mapping (`S14/program_entry_object.rs:24-29`) and the import resolution mapping (`object_plan.rs:531-538`) convert different source enums. They are not copies. |
| 11 | `R` `ELF/lib.rs` §9. `entry_symbol::tests` is an empty module. | `ELF/lib.rs:111-124` | It held no tests. | **Fixed** in this packet: the empty module is deleted. |
| 12 | `R` `S14/relocation_free_object/text_section.rs` §9. `import_symbols[&(...)]` panics if a call is missing. | `S14/relocation_free_object/text_section.rs:95-117` | The record's own trace holds. The map is built from the same call list it indexes. | **Intended.** |
| 13 | `R` `ELF/load_placement.rs` §9. The interpreter header uses `unwrap_or(0)` after an `ok_or_else` on the same value. | `ELF/load_placement.rs:807-812` | The `?` at `:807-811` returns first when the value is `None`, so `unwrap_or(0)` never supplies 0. | **Intended** (unreachable default). |
| 14 | `R` `ELF/file_assembly.rs` §9(b). `recover_retained_image` discards each stage's contents. | `ELF/file_assembly.rs:403-427`, `:197-204` | The function returns the input `FinalImage` retained through the stages. Admission then replaces its text and data with the final output bytes. The discarded contents are already in `output`. | **Intended.** |
| 15 | `R` `ELF/import_sections.rs` §9. `validate_payloads` skips symbol 0 when it checks names. | `ELF/import_sections.rs:646-650` | ELF reserves dynamic symbol 0 as the null row. | **Intended.** |
| 16 | `R` `ELF/procedure_linkage.rs` §7. `derive_fixups` and `place_import_fixups` are two derivations of one fixup list. | `ELF/procedure_linkage.rs:1999`, `:2122`, `:2207`, `:2731` | Production builds the list with `place_import_fixups`. The stage's `validate_contents` runs on every compile through `seal_stage!` and compares it with `derive_fixups`. This is the deliberate independent replay that five-things item 1 describes. | **Intended.** |
| 17 | `R` `S14/relocation_free_object/mod.rs` §8; `D11:273`. `OMGTRO` is documented as version 5 but written as 6. | `S14/relocation_free_object/mod.rs:37`, `:500` | The digest recorded this disagreement as the stale `S14/lib.rs` comment. `S14/lib.rs` lies outside unit 020 (unit 019). | **Duplicate** of the D11 §9 entry. It belongs to unit 019's packet. |
| 18 | `R` "What I am unsure about" 3. Do callers of `emit_elf_dynamic_executable` re-check the bytes after stage 22? | See claim 2 | Claim 2 shows the bytes have already been sealed when stage 22 runs. A caller's second check is not required. | **Intended.** No further claim. |

The remaining record entries (§1–§8 descriptions, crate-shape counts, call counts and "unused" counts) state
structure rather than a failure. Two "unsure" items, about `names.rs`, `bridge_object.rs`, `fragment_container.rs`,
`shared_component.rs` and `static_executable.rs`, ask about files outside this unit. Claims 4–6 and 8 trace the
parts they asked about.

## Code changes

None of the changes alters behavior. One commit, `backend: correct stale ELF and object-target comments and
share one section tag`, contains them:
- `S14/target_matrix.rs` header: every pair has a row, and `(Aarch64, Coff)` serves `windows_arm64` (claim 1).
- `ELF/file_assembly.rs` header: admission derives the output from the sealed file, and the byte replay is
  test-only (claim 2).
- `ELF/procedure_linkage.rs`: the private `section_tag` copy is deleted in favor of `image_section_tag` (claim 10).
- `ELF/lib.rs`: the empty `entry_symbol::tests` module is deleted (claim 11).

These changes need no owning fixture because no output changes. The existing fingerprint and replay tests in
`image-elf` cover the shared tag.

## Validation

- I read each cited location in the current tree. `git log 4f3c255d..24ad2700` over the unit's 14 files lists
  the five changes named above.
- `main` did not build any native crate when this review began: #5393 added `Proposition::DeclarationIdentity`,
  and the `optimization-unit` identity encoder had no arm for it (E0004 at `identity.rs:267`). I found this while
  running the tests below. [#5427](https://github.com/CathedralOS/Omega/pull/5427) repaired it and merged during
  this review. This branch merges that `main`, so it carries no copy of the repair.
- `mbx test -p image-elf -p object-file`: 138 `image-elf` unit tests, 28 `object-file` unit tests and 1
  `object-file` integration test passed. They ran on this change with a local copy of the same encoder arm,
  then again after merging `main`.
- `mbx clippy -p image-elf -p object-file --all-targets -- -D warnings`: clean.
- I searched open PRs for this unit, `image-elf` and the declaration Identity break. The only overlap was #5427.
  The unit 021 packet (#5412) covers neighboring ELF files and shares no claim with this packet.
