slice_0110

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0847 | ELF/imports.rs:178-185 | CLOSED | Invalid-handle relocations are intentional non-symbol rows; import rows with valid handles are checked at 186-196 before ELF consumers proceed. |
| A0848 | ELF/file_assembly.rs:2217-2337;2323-2335;2409-2413 | CLOSED | Stage 20 derives and validates in one `plan_stage` call; no gap for compiler-produced envelopes between derive and `validate_contents`. |
| A0849 | ff:3988-4112 | CLOSED | Production `ObjectArtifact` construction always sets `fragment_replay: Some`, so `validate` always enters the full replay path at 4107-4109. |
| A0850 | ff:18745;18864-18865;18995-19007 | REACHABLE | `admit` admits those ops without selected checks; attribution replay does not prove instruction semantics match the abstract operation. |
| A0851 | ff:18988;18992 | REACHABLE | Empty `cleanup_actions` makes `Return` / `ReturnUnit` admit with `true` and no selected-return or terminator replay on that arm. |
| A0852 | ff:18729 | REACHABLE | `PortWrite` admits when `.any` selected instruction matches port, value, and provenance, so duplicate matches are not rejected. |
| A0853 | ff:16982-16984 | REACHABLE | `runtime_footprint_retained` returns `true` when `runtime_indices()` is empty without consulting memory-access rows. |
| A0854 | ff:4717-4731;8898 | CLOSED | Installation-owned descriptor hardware is omitted from `DescriptorReplay` only when `installed_interrupts::validate` already ran on the same function in `admit`. |

### A0847

1. Read (full function): `canonical_referenced_imports` in `omega-rust/omega/compilation/backend/images/image-elf/src/imports.rs:43-200`.
2. Search: `rg '\bcanonical_referenced_imports\b' /agent/repos/omega/omega-rust` — 12 hits (definition, 3 production call sites in `static_executable.rs:184`, `shared_component.rs:95`, `import_sections.rs:88`, plus tests in `imports.rs`).
3. Read callers: `import_sections.rs:88-91` (`plan_elf_dynamic_link_inputs`), `static_executable.rs:184-216`, `shared_component.rs:95`.
4. Read (inferred upstream): `image/src/builder.rs:97-118` downgrades relocations whose object symbol fails `symbols.is_valid` to `Handle::invalid()`; module comment at `image/src/lib.rs:15-16` states this is deliberate.
5. Input on the accepting branch: a relocation in `image.relocation_table` whose `symbol_handle` fails `image.symbol_table.symbols.is_valid` at 179-183.
6. Verdict **CLOSED**: check at `imports.rs:186-196` — for relocations with **valid** handles on import symbols, the function errors if no matching import request exists; the `continue` at 184 only skips intentionally non-symbol relocations. Dynamic planning still eventually applies relocations via `image::apply_relocations` (`relocations.rs:24-37`), which errors on unresolved symbol addresses for relocations that are actually applied.

### A0848

1. Read (full function): `validate_contents` in `file_assembly.rs:2217-2337`; `validate_unchanged_bytes` 2366-2391; fingerprint `non_authoritative_resolved_linkage_compatibility_fingerprint` 2401-2427.
2. Entry matches code (read): `require` at 2227-2250 length-checks procedure-linkage, GOT, and procedure-relocation bytes but not `general_relocation_bytes`; loop at 2323-2335 omits `GeneralRelocation`; fingerprint at 2409-2413 hashes four byte blobs, not `general_relocation_bytes`.
3. Read stage wiring: `emit_elf_dynamic_executable` stage 20 is `apply_elf_procedure_linkage_fixups` (`dynamic_executable.rs:657-658`); `apply_elf_procedure_linkage_fixups` uses `plan_stage!` with `derive: derive_contents` (`file_assembly.rs:3543-3553`); `checked::plan_candidate` + `validate_candidate` (`dynamic_executable.rs:395-384`).
4. Input on accepting branch: `ElfResolvedProcedureLinkageContents` whose `general_relocation_bytes` differ from upstream outside procedure fixup-owned words while other checks pass.
5. Verdict **CLOSED**: every production path runs `derive_contents` (`2119-2164`) and `validate_contents` in one `plan_stage` invocation with no intervening mutation; upstream `.rela.dyn` bytes enter via `indexed_row_bytes` at 2146-2150 from the sealed envelope produced by earlier dynamic-ELF stages in the same `emit_elf_dynamic_executable` call (`614-662`).

### A0849

1. Read (full function): `replay::validate` `function_fragments.rs:3988-4112`.
2. Entry matches code (read): six `fragment_replay.is_none()` guards return errors at 3991-4105; otherwise `Ok(())` at 4111 without replay unless 4107-4109 runs.
3. Search: `rg 'fragment_replay:' /agent/repos/omega/omega-rust` — 2 hits; only assignment `fragment_replay: Some(...)` at `3827` in `build_function_fragment_object_artifact`.
4. Read callers of `replay::validate`: `image.rs:224`, `image.rs:1894`; both operate on emission artifacts from the builder.
5. Input on accepting branch: an `ObjectArtifact` with `fragment_replay: None` that fails none of the six shape guards.
6. Verdict **CLOSED**: the sole production constructor always sets `fragment_replay: Some` (`3827`); `validate` then always calls `validate_function_fragment_object_artifact` (`4107-4109`), which runs `source::admit` and full replay (`24426-24432`).

### A0850

1. Read (full `admit` match): `source::admit` `18534-19034`, arms cited at 18735-18746, 18864-18865, 18995-19007.
2. Read (partial module): `attribution` `function_fragments.rs:8-458` — `produce`/`validate` tie instruction spans to `provenance.operations`, not operation kind vs opcode semantics.
3. Read downstream: `validate` calls `attribution::validate` at `24627` after `admit` is invoked from `validate_function_fragment_object_artifact` (`24432`).
4. Input on accepting branch: abstract atomic, constant, or arithmetic operation for which `admit` returns `true` without inspecting `selected`.
5. Verdict **REACHABLE**: a function could carry the correct abstract operation and attribution intervals while selected machine code implements the wrong instruction or drops a store; `admit` does not compare selected instructions for those arms, and attribution `validate` only checks presence of spans for provenance operation ids, not semantic lowering correctness.

### A0851

1. Read: `Return` / `ReturnUnit` arms `function_fragments.rs:18986-18994`.
2. Entry matches code (read): `cleanup_actions.is_empty()` yields admission without `control_flow::retained` or `read_result_cleanup_actions_match`.
3. Read (not fully): `control_flow::retained` `10314+` matches terminators when the nonempty cleanup path runs.
4. Input on accepting branch: `Return` or `ReturnUnit` with empty `cleanup_actions`.
5. Verdict **REACHABLE**: a program whose abstract return has empty cleanup but whose selected lowering uses the wrong return path or register could pass the empty-cleanup arm with no selected-return check on that branch.

### A0852

1. Read: `PortWrite` arm `function_fragments.rs:18728-18732`.
2. Entry matches code (read): uses `.any`, not a uniqueness count.
3. Input on accepting branch: `AbstractOperation::PortWrite` with two or more selected `PortWrite` instructions sharing port, value, and `psi_operation` provenance.
4. Verdict **REACHABLE**: duplicate matching port-write instructions are admitted; a wrong extra matching instruction would not be rejected.

### A0853

1. Read (full function): `runtime_footprint_retained` `16937-17016`; accepting branch `16982-16983`.
2. Read caller: `source::admit` at `18787-18789` (`StructuralByteSequenceFieldLength`).
3. Input on accepting branch: an operation for which `operation.runtime_indices()` is empty.
4. Verdict **REACHABLE**: wrong or missing memory-access rows for a non-indexed operation using this helper can pass `runtime_footprint_retained` when pairing with `structural_fields::retained` alone is insufficient for footprint obligations.

### A0854

1. Read (full function): `dedicated_descriptor_owner` `4717-4730`; skip site `4456-4457` in `DescriptorReplay::new`.
2. Read caller chain: `source::admit` `18597-18607` runs `installed_interrupts::validate` before `DescriptorReplay::new` and the operation loop.
3. Search: `rg '\bdedicated_descriptor_owner\b' /agent/repos/omega/omega-rust` — 2 hits (definition and call at 4456).
4. Input on accepting branch: `dedicated == false` and `operation` in `installations`, so `dedicated_descriptor_owner` returns `Ok(false)` and hardware is skipped.
5. Verdict **CLOSED**: skipped hardware is installation-owned; `installed_interrupts::validate` (`8898+`) runs on the same `(targeted, selected)` pair immediately before descriptor replay in every `admit` path.

### What I did not read

- Full body of `installed_interrupts::validate` past roughly `8900` (descriptor-instruction coverage inferred from call order only).
- `control_flow::retained` and cleanup roster matching beyond the `Return` admit arms.
- Earlier ELF indexed-payload validators that feed stage-20 envelope bytes.
- `native-artifact` physical-evidence paths for arithmetic or atomic operations.
- Every caller of `validate_function_fragment_object_artifact` beyond `replay::validate` and composition early-return at `24407-24421`.

SLICE COMPLETE
