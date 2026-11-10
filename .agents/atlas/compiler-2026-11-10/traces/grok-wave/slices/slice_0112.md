slice_0112

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0863 | image-macho/src/dyld_linking.rs:193-210 | CLOSED | Invalid relocation handles are skipped at thunk install, but the same FinalImage still fails in apply_relocations before emit succeeds. |
| A0864 | image-pe/src/imports.rs:95 | REACHABLE | UEFI subsystem 10..=12 is rejected only on the Normalized Foreign branch; StringBackedBootstrap and Component imports still install on a UEFI subsystem PE. |
| A0865 | image-pe/src/headers.rs:42-88; sections.rs:51-86,183 | REACHABLE | PE header and relocation builders cast memory lengths to u16/u32 with no overflow check before write. |
| A0866 | image-pe/src/lib.rs:171-361 | REACHABLE | emit_pe_x86_64 returns container bytes with no PE re-read; post-emit validation skips Coff container mapping unlike Mach-O. |
| A0867 | image/src/lib.rs:221 | UNDECIDED | Address sums symbol offset into the section base without a size bound; object-plan admission of out-of-range offsets was not fully traced. |
| A0868 | isa-aarch64/src/machine_effects.rs:993-998,1187 | CLOSED | Catalog is built and validated as one unit; end users cannot introduce a new semantic that only hits the default encoded_effects arm. |
| A0869 | isa-aarch64/src/post_handoff_writer.rs:104-126 | CLOSED | layout_plans::validate_fragment caps container width at 64 bits before the multi-byte load/store loop runs. |

### A0863

1. Read `install_import_thunks` in full in `omega-rust/omega/compilation/backend/images/image-macho/src/dyld_linking.rs:75-250` (read).
2. Search `\binstall_import_thunks\b` under `omega-rust/`: 4 files, 25 matches (read count via ripgrep).
3. Production caller: `emit_macho_container_contents` at `image-macho/src/lib.rs:738` (read); that path always calls `image::apply_relocations` at `lib.rs:783` before return (read). PE has a separate `install_import_thunks` in `image-pe/src/imports.rs` (not this entry’s function).
4. Input on the accepting branch: `FinalImage` relocation rows whose `symbol_handle` is `Handle::invalid()` after `copy_object_relocations` downgrades non-resolving handles at `image/src/builder.rs:110-114` (read); loop at `dyld_linking.rs:193-199` executes `continue` (read).
5. **Verdict: CLOSED** — Check `apply_relocations` at `image/src/relocations.rs:24-37` (read): when `final_image_symbol_address` returns `None`, emission returns `Diagnostic::error` for an unknown symbol; every Mach-O emit path that runs thunk install also runs this relocation pass (`image-macho/src/lib.rs:738,783`).

### A0864

1. Read `install_import_thunks` in full in `image-pe/src/imports.rs:37-181` (read).
2. Search `\binstall_import_thunks\b` under `omega-rust/`: hits include production call `image-pe/src/lib.rs:183` and tests in `imports.rs` (read).
3. Caller `emit_pe_x86_64` at `lib.rs:178-206` passes `subsystem` through unchanged (read); upstream `emit_direct_executable_image_signed` at `image-emission/src/image.rs:1848` supplies the build subsystem (read).
4. Input on the accepting branch: `FinalImage` import with `FinalImageImportPlan::StringBackedBootstrap` or `Normalized(Component(_))`, and `subsystem` in `10..=12`; only the `Foreign(locator)` arm tests `matches!(subsystem, 10..=12)` at `imports.rs:95-100` (read). Component lacks that test at `imports.rs:123-137` (read).
5. **Verdict: REACHABLE** — A UEFI PE image (subsystem 10..=12) carrying a string-backed bootstrap import is not rejected in this function and would proceed to thunk installation; the normalized-foreign case is rejected in tests at `imports.rs:599-607` (read) but that guard is absent on the other import-plan arms.

### A0865

1. Read `write_pe_headers` at `headers.rs:39-104` and `write_section_header` at `headers.rs:106-129` (read); `plan_pe_sections` and `build_base_relocations` at `sections.rs:50-86,167-184` (read).
2. Casts include `input.section_count as u16` (`headers.rs:42`), `size_of_code as u32` (`:52`), `import_directory_size as u32` (`:88`), `sections.rs:51` (`image.memory.text.len() as u32`), `relocation.offset as u32` (`:183`) (all read).
3. No caller in these files validates `usize` values against `u16::MAX` or `u32::MAX` before cast (read).
4. Input on the accepting branch: `FinalImage` / `PeSections` with section or RVA geometry whose true length exceeds `u32::MAX` (or section count above 65535 for the u16 cast).
5. **Verdict: REACHABLE** — Truncation in header or relocation encoding can yield a structurally wrong PE while `emit_pe_x86_64` still returns `Ok` (`lib.rs:343-360`, read); post-emit checks in `image-emission/src/image.rs:803-877` do not compare serialized PE header fields to planned RVAs for Coff (read).

### A0866

1. Read `emit_pe_x86_64` end-to-end `lib.rs:178-361` (read): builds `bytes` via `write_*` and `resize`/`extend`, returns `ExecutableImageOutput` without rereading `bytes`.
2. Search for PE container replay in `image-pe/`: none beyond unit tests in `lib.rs` tests module (read `lib.rs:363+` header).
3. Caller `emit_direct_executable_image_signed` invokes `validate_terminal_image` at `image-emission/src/image.rs:1851-1858` (read); Coff branch has no analogue to Mach-O `validate_macho_container_mapping` at `:842-864` (read).
4. Input on the accepting branch: any `FinalImage` the planner accepts; correctness of DOS/COFF/optional header and section table is taken from in-memory `PeSections` / `PeHeaderInput` only.
5. **Verdict: REACHABLE** — Compilation can succeed with PE container bytes never validated by a format replay; a planner/header mismatch would not be caught by the Coff path of `validate_terminal_image`.

### A0867

1. Read `final_image_symbol_address` at `image/src/lib.rs:208-222` (read).
2. Search `final_image_symbol_address` under `omega-rust/`: definitions plus uses in `image/src/relocations.rs:26` and `image-macho/src/dyld_linking.rs:2336` (read).
3. Read `apply_relocations` at `relocations.rs:17-119` (read): uses computed address directly; no comparison to symbol `size` or section length at this site.
4. Input on the accepting branch: valid `FinalImageSymbolHandle` with `section` Text/Data/Bss and `offset` not validated against `image.memory` section length at `:221`.
5. **Verdict: UNDECIDED** — The helper clearly omits a bounds check (read), but this slice did not trace whether `ObjectPlan` / fragment admission always rejects symbols whose offset lies past section storage before `build_final_image` (`builder.rs:17-33`, read only).

### A0868

1. Read `encoded_effects` default arm at `machine_effects.rs:993-998` (read) and `size` default at `:1187` (read); read full `declaration` at `:617-708` and catalog builder `:271-400` (read).
2. Default encoded arm supplies empty implicit uses/defs, `NeverV1` trap, fall-through, 4-byte size only when a semantic reaches `encoded_effects` but not the explicit implicit match at `:880-992` (read).
3. Catalog entries come only from `selected_keys.declaration_keys()` mapped through `declaration(semantic, &selected_keys)` at `:286-399` (read); not a per-program input.
4. `aarch64_machine_effect_catalog` is validated at build time in-module at `:235` and in pipeline `row_encoding.rs:1377` (grep, inferred not read in full).
5. **Verdict: CLOSED** — End-user programs cannot add a `MachineSemanticKind`; maintainer edits are checked by `validate_aarch64_machine_effect_catalog` when the catalog is built. The reader’s “added to reads/writes but not effect table” scenario is a static catalog consistency question, not an input-acceptance hole at compile time.

### A0869

1. Read `append_fragment`, `load`, and `store` in full in `post_handoff_writer.rs:14-139` (read).
2. `append_fragment` calls `layout_plans::validate_fragment` at `:48-57` before `count = container_width / 8` at `:59` (read).
3. Read `validate_fragment` at `psi/foundation/layout-plans/src/materialization.rs:1351-1393` (read): rejects `container_width_bits > 64`.
4. Plan admission calls the same helper at `post-handoff-writer-plan/src/lib.rs:244-252` (read).
5. **Verdict: CLOSED** — Container widths above 8 bytes cannot pass `validate_fragment`; the `_` branches at `post_handoff_writer.rs:104-117` only handle counts 3,5,6,7 within the ≤64-bit cap, not “larger than 8 bytes” in the entry’s sense.

### What I did not read

- Full body of `validate_isa_machine_effect_catalog` in `selected-instructions/src/effects/catalog/validation.rs`.
- Complete `object-file` / fragment admission rules for symbol offset versus section size.
- Full compiler path from Omega source to UEFI PE with `StringBackedBootstrap` imports (only PE import installer and one Foreign-locator test read).
- Callees of `emit_macho_container_contents` beyond `image-macho/src/lib.rs:700-911`.
- `encode_generated_post_handoff_writer_bytes` below `post_handoff_writer.rs:180`.

SLICE COMPLETE
