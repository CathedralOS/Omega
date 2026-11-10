# slice_0112

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0863 | MO/dyld_linking.rs:193-210 | CLOSED | `apply_relocations` later errors on invalid relocation symbol handles on every Mach-O emit path |
| A0864 | PE/imports.rs:95 | REACHABLE | `StringBackedBootstrap` and `Component` import plans ignore UEFI subsystem 10..=12 |
| A0865 | PE/headers.rs:42 | REACHABLE | PE header/section planning uses truncating `as u32`/`as u16` casts without prior size caps |
| A0866 | PE/lib.rs:171-361 | REACHABLE | PE emission returns `Ok(ExecutableImageOutput)` without re-reading or validating written container bytes |
| A0867 | IM/lib.rs:221 | REACHABLE | Symbol addresses add `offset` with no check against section byte length |
| A0868 | A64/machine_effects.rs:993-998 | REACHABLE | Default `encoded_effects`/`size` arms declare never-trap, 4-byte semantics for unmatched variants |
| A0869 | A64/post_handoff_writer.rs:104-126 | CLOSED | `validate_fragment` and `validate_step` cap container width at 64 bits (8 bytes), so the generic byte loop never sees `count > 8` |

## A0863

**Cited site (read):** In `install_import_thunks` at `omega-rust/omega/compilation/backend/images/image-macho/src/dyld_linking.rs:75-268`, the relocation preflight loop at :193-210 uses `continue` when `!image.symbol_table.symbols.is_valid(relocation.symbol_handle)` (:197-199), skipping import-consistency checks for that record.

**Callers (read):** Search `install_import_thunks` in `omega-rust` (grep, 14 name hits): production call sites are `image-macho/src/lib.rs:738` inside `emit_macho_container_contents` and `image-pe/src/imports.rs:37` (PE path, unrelated to this row); remaining hits are tests and re-exports in `dyld_linking.rs` and `image-pe/src/imports.rs`.

**Closing path (read):** `emit_macho_container_contents` calls `install_import_thunks` at :738, then `image::apply_relocations` at :783. `apply_relocations` in `image/src/relocations.rs:17-119` resolves each relocation via `final_image_symbol_address` (:25-26); when that returns `None`, it returns `Diagnostic::error` for unknown symbols (:35-37) unless the symbol is an unresolved import (:29-32). An invalid handle yields `None` from `final_image_symbol_address` (`image/src/lib.rs:213-218`) and fails emission before bytes are returned.

**Input (plain words):** Any crafted `FinalImage` carrying a relocation whose symbol handle is not valid still reaches the skip at :197-199 during thunk installation, but compilation does not succeed: the same image fails at `apply_relocations` with an unknown-symbol error.

**Entry description:** Matches the code at :197-199; the digest’s concern about skipping is real at thunk installation, but acceptance is blocked downstream.

## A0864

**Cited site (read):** In `install_import_thunks` at `omega-rust/omega/compilation/backend/images/image-pe/src/imports.rs:37-181`, the `Normalized` `Foreign` arm rejects when `matches!(subsystem, 10..=12)` at :95-100. The `StringBackedBootstrap` arm (:81-90) and `Normalized` `Component` arm (:123-137) have no subsystem guard.

**Callers (read):** Search `install_import_thunks` in `omega-rust` (grep, 14 hits): production `image-pe/src/lib.rs:183` in `emit_pe_x86_64`; tests include `locator_mutation_changes_pe_import_bytes_and_target_drift_rejects_before_installation` rejecting normalized foreign on UEFI (:599-612) but no bootstrap-on-UEFI rejection.

**Closing check absent (read):** `emit_pe_x86_64` (`PE/lib.rs:178-361`) always calls `install_import_thunks` at :183 with the caller-supplied `subsystem`; `image-emission/src/image.rs:1848` passes the subsystem for COFF executables.

**Input (plain words):** A Windows COFF `FinalImage` for `NativeTarget::uefi_x64()` with subsystem 10 (EFI application) and a `StringBackedBootstrap` or `Component` import row is not rejected in `install_import_thunks` and proceeds through PE emission with an import directory, despite the foreign-locator path rejecting subsystem 10..=12.

**Entry description:** Accurate for the foreign-locator branch; string-backed and component paths omit the UEFI subsystem check.

## A0865

**Cited sites (read):** `PE/headers.rs:39-104` casts `section_count`, section sizes, and directory sizes with `as u16`/`as u32` (:42, :52-53, :67-68, :84, :96, etc.). `PE/sections.rs:50-86` uses `as u32` for RVA arithmetic (:51, :57, :60-61, :86) and `sections/relocations.rs:183` uses `as u32` for relocation RVAs. Mach-O uses `u32::try_from` in many header writers (`dyld_linking.rs` grep), contrast read only.

**Callers (read):** `write_pe_headers` is called from `emit_pe_x86_64` at `PE/lib.rs:248-268`; `plan_pe_sections` at :201 uses the same casts before headers are written.

**Upstream (read):** No `u32::MAX` or truncation guard on `FinalImage` memory lengths was found in `image/src/final_image.rs` or the PE planner before these casts (grep `u32::MAX` under `images/image-pe` and `images/image` returned no emission guards).

**Input (plain words):** A `FinalImage` whose text, data, BSS, or computed `size_of_image` exceeds what fits in the fields written via `as u32`/`as u16` would emit a PE whose headers truncate high bits instead of failing closed at those casts.

**Entry description:** Matches unchecked truncating casts at the cited lines.

## A0866

**Cited site (read):** `emit_pe_x86_64` at `PE/lib.rs:178-361` builds `bytes` in memory (:246-341) and returns `Ok(ExecutableImageOutput { bytes, ... })` at :343-360 without parsing the PE back.

**Contrast (read):** Mach-O `emit_macho_container_contents` re-validates after assembly at `image-macho/src/lib.rs:876-902` (`validate_macho_container_mapping`, `validate_macho_loader_fixups_for_segment`, optional export trie). Digest preamble area_02 line 32 states PE output is not re-read.

**Callers (read):** `emit_pe_x86_64_executable` (:171-175) is invoked from `image-emission/src/image.rs:1848` for COFF direct executables and from `shared_component.rs` via `emit_pe_x86_64`.

**Input (plain words):** Any `FinalImage` the planner maps into header and section tables is accepted as a successful PE if in-memory writes do not error, even when those bytes would not match a re-read PE validator, because no post-write container verification runs in `image-pe`.

**Entry description:** Accurate; correctness rests on planner and in-process writes only.

## A0867

**Cited site (read):** `final_image_symbol_address` in `image/src/lib.rs:208-222` returns `Some(section_address + symbol.offset as u64)` with no comparison of `offset` or `size` to the section’s byte length.

**Callers (read):** Search `final_image_symbol_address` in `omega-rust` (grep, 4 files, 6 hits): production use in `image/src/relocations.rs:25-26` inside `apply_relocations`; Mach-O dyld validation/testing at `dyld_linking.rs:2336`; re-export at `image/src/lib.rs:297`.

**Downstream (read):** `apply_relocations` uses the computed address as the relocation target (:39-45) and patches bytes when relocation offsets are in-bounds for the section slice; it does not validate that the symbol’s own offset lies inside the section.

**Input (plain words):** A `FinalImage` whose symbol table marks a defined symbol in `.text` with an `offset` beyond `memory.text.len()` still yields a symbol address used for relocation patching, so a program that should be rejected for an out-of-bounds symbol placement can complete PE/Mach-O relocation application.

**Entry description:** Matches missing bounds check at :221.

## A0868

**Cited sites (read):** In `encoded_effects` (`machine_effects.rs:711-1050`), operand read/write arms end with explicit variants (:729-878) while implicit trap/control arms use a default `_ =>` at :993-998 assigning empty implicit uses/defs, `NeverV1` trap, and `FallThroughV1`. `size` (`:1150-1188`) defaults `_ => MachineSizeKnowledge::ExactBytes(4)` at :1187. `declaration` (:617-708) feeds both through `form_alternative!` at :707.

**Catalog path (read):** `aarch64_machine_effect_catalog` (:271-288) maps every selected semantic through `declaration`; `validate_aarch64_machine_effect_catalog` is invoked from encoding paths (`selected_form_encoding.rs` grep, multiple sites).

**Input (plain words):** If a `MachineSemanticKind` variant gains operand read/write entries in the match at :729-878 but no dedicated implicit-effect arm before :993, the catalog records it as non-trapping with four-byte size and no implicit register effects—so unsound metadata for a trapping or clobbering operation would be accepted at catalog build time.

**Entry description:** Accurate; default arms at :993-998 and :1187 create the stated pure/minimal declaration hazard.

## A0869

**Cited sites (read):** `load` and `store` in `post_handoff_writer.rs:93-138` handle `count` 1, 2, 4, 8 in dedicated arms (:101-103, :123-125); the `_` arms implement per-byte loops (:104-116, :126-135) that assemble wider values with 8-bit steps.

**Validation before use (read):** `append_fragment` calls `layout_plans::validate_fragment` at :48-56; `count = container_width / 8` at :59 with `container_width` capped to 64 bits in `psi/foundation/layout-plans/src/materialization.rs:1360-1367`. `encode_generated_post_handoff_writer_bytes` validates each step with `validate_step` (`post-handoff-writer-plan/src/lib.rs:510-515`) allowing only `container_width_bits` in `8 | 16 | 32 | 64`.

**Input (plain words):** Fragment insertion always runs through `append_fragment` or plan validation that limits container width to at most eight bytes, so the generic loops for `count > 8` are dead on production paths; only sub-8-bit field extractions use the loop with `count` at most eight.

**Entry description:** Partially right about the loop logic; larger containers are blocked before `load`/`store`, not left to runtime discovery alone.

## What I did not read

- Full `area_02.md` beyond the preamble (lines 1-38 read).
- `git show 457adf4a11f:` snapshots of cited paths (current checkout read; line numbers matched stated functions).
- `MO/dyld_linking.rs` beyond `install_import_thunks` and cited validator excerpts.
- `selected_form_encoding.rs` beyond grep hits for catalog validation.
- `image-emission` `build_final_image` in full (inferred bounds from grep samples only).

SLICE COMPLETE
