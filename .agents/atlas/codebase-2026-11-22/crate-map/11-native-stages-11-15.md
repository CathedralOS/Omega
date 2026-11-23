# Crate map: 11-native-stages-11-15

All paths are relative to `C:/Development/Omega/.agents/worktree/worker-2/` (detached at `4f3c255d0559`).
`P` below means `source-rust/subsystems/compiler/psi-to-native/pipeline`. Line and file counts come from
`.agents/atlas/codebase-2026-11-22/crates.tsv` (13 rows with area `11-native-stages-11-15`).
Dependency lists come from each `Cargo.toml` (read). "Dependents" were counted with a Python script
(`tomllib` over every tracked `Cargo.toml` outside `_recycling/`, run once, results kept in scratch) that
splits production from dev dependencies; names under `psi-to-native` are written without that prefix.
Test counts are `grep -c '#\[test\]'` over the source files; no test was run.

## 1. What the area does

Native stages 11 to 15 turn the physical machine program of one function body into bytes and then into a
finished executable or shared-component file for the receiving target. Stage 11 encodes each selected
instruction and its frame prologue/epilogue recipes into machine bytes (x86-64 and AArch64 encoders live
here). Stage 12 fixes each instruction's offset inside its function and relaxes x86 conditional branches.
Stage 13 inserts the frame bytes, computes the whole-function exit contract, places all functions into one
text section and resolves internal calls. Stage 14 wraps that text section into a relocation-free object
(`OMGTRO`) and the linkable `ObjectPlan`/`RelocationPlan`. Stage 15 composes body objects with entry bridges
and imports, builds a format-neutral `FinalImage`, writes it as ELF, Mach-O or PE, and wraps the result in a
self-verifying native artifact and publication receipts. Input at stage 11 is a `BoundFunctionBody` from
stage 10; output at stage 15 is `NativeArtifact`/`ExecutableImage` bytes plus receipts. The body-level stages
11 to 14 are called per body by `psi-to-native/driver/compile_body.rs` (read); stage 15 is called from the
driver's image path (`driver/image/object_emission.rs:260`, `driver/mod.rs:149`, read).

## 2. Crates in pipeline order

### 2.1 `native-encoding` (stage 11 root crate)
- Path: `P/11_bound-framed-machine-to-selected-form-encoding`; 14 files, 3806 lines (`crates.tsv`).
- Purpose: Encodes every row of a bound, framed physical program into bytes plus a decoded footprint, routes
  each row by `SelectedInstructionKind` to an ISA encoder, and encodes the frame protocol recipes. It also
  encodes the non-body pieces that stage 10/09 plan: hosted entry, imports, native entry and foreign bridges,
  physical stack switch, Windows internet exports.
- Consumes: `bound_framed_machine::BoundFunctionBody` (parameter of `encode_bound_function_body`,
  `src/lib.rs:59`). Produces: `StagedOptimizedSelectedFormEncoding<EncodedBodyPublication<_>>`
  (`src/lib.rs:113`), which wraps `machine_code::EncodedMachineProgram` and
  `machine_code::layout::EncodedBodyContracts`.
- Entry points: `encode_bound_function_body` (`src/lib.rs:59`, called at `driver/compile_body.rs:193`);
  `adapters::encode_instructions` (`src/adapters.rs:41`); `callable_entry::encode_callable_entry_transport`
  (`src/callable_entry.rs:26`) and `encode_private_function_entry` (`:45`);
  `hosted_entry::encode_hosted_entry` (`src/hosted_entry.rs:8`);
  `hosted_program::encode_hosted_program_entry` (`src/hosted_program.rs:28`);
  `imports::encode_import_program` (`src/imports.rs:19`);
  `native_entry_bridge::encode_native_entry_bridge` (`:25`); `native_foreign_bridge::encode_foreign_adapter`
  (`:28`) and `encode_native_foreign_bridge` (`:45`); `physical_entry::encode_physical_entry` (`:19`);
  `physical_stack_switch::encode` (`:14`); `windows_internet::encode_windows_internet_provider` (`:53`).
- Production dependencies in this repo: `bound-framed-machine`, `function-identity`, `diagnostics`, `arena`,
  `runtime-abi`, `register-environment`, `frame-admission`, `physical-recipes`, `encoding-admission`,
  `target-vocabulary`, `calling-conventions`, `machine-code`, `post-allocation-machine`, `isa-aarch64`,
  `isa-x86_64`, `register-model`, `selected-instructions`, `target`, `semantic-vocabulary`.
- Dependents (production): 2 (`psi-to-native` driver, stage 12). Dev only: `image-emission`, `image-pe` (windows).
- Modules: `row_encoding` (1936 lines, route per instruction kind, `src/row_encoding.rs:1`); `control_encoding`
  (all branch forms before layout); `frame_protocol` (frame recipes to bytes, defines
  `FunctionBodyEncodingError`); `adapters`; `hosted_entry`, `hosted_program`, `imports`;
  `native_entry_bridge`, `native_foreign_bridge`, `callable_entry`, `physical_entry`, `physical_stack_switch`;
  `windows_internet`.
- Tests: 15 `#[test]` (12 in `row_encoding.rs`, 2 in `adapters.rs`, 1 in `windows_internet.rs`); doc tests in
  `src/lib.rs:19-58` are `compile_fail` checks that the encoder cannot be reached without a bound body.
  Dev-deps `entry-planning`, `register-homes`.
- Open work: no `TASKS.md` or `OWNER_CODE_CONCERNS.md` entry names it (grepped).

### 2.2 `isa-aarch64` (stage 11 nested)
- Path: `P/11_.../isa-aarch64`; 9 files, 19382 lines. 17625 of those are `src/selected_form_encoding.rs`.
- Purpose: Encoder, structural decoder and validator for every AArch64 selected instruction form, plus frame
  prologue/epilogue protocol, FPCR custody, 64-bit immediate offset steps and hosted Linux/Darwin syscall
  leaves. Each byte producer is paired with an independent decoder in the same crate (`src/lib.rs:11-17`).
- Consumes `selected_instructions` forms, `register_model` homes, `normalized-foreign-call-plan` plans.
  Produces instruction word bytes and `SelectedFormDecodedFootprint` for `machine-code`.
- Entry points: per-form `encode_aarch64_selected_*_form` / `validate_aarch64_selected_*_form` inside
  `src/selected_form_encoding.rs` (for example `:922`, `:959`, `:4162`, `:6838`, `:7454`, `:9879`, `:12143`);
  frame protocol functions re-exported from `src/frame_protocol.rs`; `decode_aarch64_generated_entry`
  re-exported from `target-aarch64` (`src/lib.rs:43`).
- Production dependencies: `target-aarch64`, `wire`, `normalized-foreign-call-plan`, `encoded-scalar-plan`,
  `calling-conventions`, `optimization-core`, `register-model`, `target`, `selected-instructions`,
  `semantic-vocabulary`, `diagnostics`, `layout-plans`.
- Dependents (production): 2 (`native-encoding`, `encoding-admission`). Dev: `psi-to-native`, stages 12 and 13,
  `target-aarch64`.
- Modules: `selected_form_encoding` (all forms; subdir `normalized_foreign_call/callback_field.rs`);
  `frame_protocol` (679); `hosted_sequences` (370); `immediate_offset` (239); `floating_control` (99);
  `machine_effects` and `saturating_forms` are one-line files that re-export from `target-aarch64`
  (`src/lib.rs:44,48` read).
- Tests: 141 `#[test]` (118 in `selected_form_encoding.rs`); dev-dep `isa-conformance`
  (`psi-to-native/tests/isa-conformance`).
- Open work: none named.

### 2.3 `isa-x86_64` (stage 11 nested)
- Path: `P/11_.../isa-x86_64`; 7 files, 23524 lines (21917 in `src/selected_form_encoding.rs`).
- Purpose: Same role as `isa-aarch64` for x86-64: selected-form encoders/decoders/validators, frame and MXCSR
  protocols, IEEE scalar and FMA3 materialization, hosted Linux/Darwin syscall leaves, interrupt/table
  sequences (`src/lib.rs:1-31`).
- Entry points: `encode_x86_64_selected_*_form` / `validate_x86_64_selected_*_form` in
  `src/selected_form_encoding.rs` (for example `:843`, `:8460`, `:10793`); `decode_vfmadd132_scalar`
  (`src/ieee_float.rs`, re-exported `src/lib.rs:116`); fixed byte constants `ORDINARY_RETURN_BYTES` etc.
  (`src/lib.rs:44-46`); `ORDINARY_DIRECT_CALL_BYTES` and `PRIVILEGED_STOP_BYTES` re-exported from `target-x86_64`.
- Production dependencies: `target-x86_64`, `wire`, `normalized-foreign-call-plan`, `x86_64-encoding`,
  `encoded-scalar-plan`, `calling-conventions`, `optimization-core`, `register-model`, `target`,
  `selected-instructions`, `semantic-vocabulary`, `diagnostics`, `layout-plans`.
- Dependents (production): 2 (`native-encoding`, `encoding-admission`). Dev: `psi-to-native`, stages 12 and 13,
  `image-emission`, `target-x86_64`.
- Modules: `selected_form_encoding`; `frame_protocol` (660); `hosted_kernel` (324); `ieee_float` (257);
  `machine_effects` (one-line re-export).
- Tests: 168 `#[test]` (149 in `selected_form_encoding.rs`); dev-dep `isa-conformance`.
- Open work: none named.

### 2.4 `x86_64-encoding` (stage 11 nested)
- Path: `P/11_.../x86_64-encoding`; 1 file, 103 lines.
- Purpose: Two fixed-width byte sequences for immediate port I/O (`encode_immediate_port_write`,
  `encode_immediate_port_read_u8`) with width constants re-exported from `target-x86_64` (`src/lib.rs:21-37`).
- Entry points: `encode_immediate_port_write` (`src/lib.rs:25`). Produces `[u8; 27]`.
- Production dependencies: `target-x86_64`. Dependents (production): 1 (`isa-x86_64`).
- Tests: 3 `#[test]` in `src/lib.rs`. Open work: none named.

### 2.5 `selected-form-encoding-to-resolved-layout` (stage 12)
- Path: `P/12_selected-form-encoding-to-resolved-layout`; 9 files, 4396 lines. No lib name override (package name
  is the crate name; consumers import it as `placement`).
- Purpose: Gives each encoded row a function-relative offset and a canonical block order, then relaxes x86
  conditional branches from near to short forms until no branch shrinks. It also lays out entry bridge and
  program entry call fields (`entry_layout.rs`, `program_entry_layout.rs`).
- Consumes `native_encoding::StagedOptimizedSelectedFormEncoding<Contracts>`. Produces
  `LayoutResolution<Contracts>` (`src/lib.rs:40`) holding a `ResolvedFunctionLayout<Contracts>`
  (`src/settled.rs`) and an optional baseline `machine_code::ResolvedMachineProgram` for replay.
- Entry points: `resolve_program` (`src/lib.rs:53`, called at `driver/compile_body.rs:219`);
  `entry_layout` and `program_entry_layout` public modules; `relaxation` public module (independent
  re-check of relaxation records); `StagedOptimizedX86BranchRelaxation` re-export.
- Production dependencies: `optimization-core`, `function-identity`, `diagnostics`, `target-aarch64`,
  `target-x86_64`, `physical-recipes`, `native-encoding`, `machine-code`, `post-allocation-machine`,
  `register-model`, `selected-instructions`, `target`, `semantic-vocabulary`, `encoding-admission`.
- Dependents (production): 4 (`psi-to-native` driver, stage 13, stage 14, `verification/published-objects`).
- Modules: `ordinary` (baseline layout, block order, AArch64 branch widening plan); `settled` (issues the
  layout and applies relaxation); `x86_branch_relaxation` (1670); `relaxation`; `entry_layout`;
  `program_entry_layout`; test-only `order_suite`, `validation_tests`.
- Tests: 36 `#[test]` (22 in `x86_branch_relaxation.rs`); dev-deps `isa-aarch64`, `isa-x86_64`.
- Open work: none named.

### 2.6 `function-layout` (stage 13)
- Path: `P/13_resolved-layout-to-placed-functions`; 8 files, 10207 lines; has its own `README.md`.
- Purpose: Takes the settled layout, assembles the fixed-frame function realization (with the whole-function
  exit contract), emits unplaced function fragments, inserts frame prologue/epilogue bytes and re-encodes
  displaced branches, then places all fragments into one text section and resolves internal calls.
- Consumes `placement::ResolvedFunctionLayout<Contracts>`. Produces `FunctionPlacement<Contracts>`
  (`src/text_placement.rs:79`) holding `StagedOptimizedFixedFrameTextSection` (`:147`).
- Entry points: `place_function_text` (`src/lib.rs:46`, called at `driver/compile_body.rs:246`);
  `validate_optimized_fixed_frame_text_section` (`src/text_placement.rs:95`);
  `code_layout::describe_native_code_layout` (`src/code_layout.rs:14`).
- Production dependencies: `selected-form-encoding-to-resolved-layout` (as `placement`), `target-x86_64`,
  `target-aarch64`, `frame-admission`, `wire`, `register-environment`, `register-model`, `register-homes`,
  `selected-instructions`, `optimization-core`, `target`, `calling-conventions`, `machine-code`,
  `semantic-vocabulary`, `diagnostics`, `terminal-psi`, `post-allocation-machine`, `target-vocabulary`.
- Dependents (production): 2 (`psi-to-native` driver, stage 14).
- Modules: `function_realization` (assembles and publishes realization, `:284`); `exit_contract` (2706);
  `fragment_emission` (1409); `frame_application` (2593); `relative_branches`; `text_placement` (1855);
  `code_layout` (native code layout queries).
- Tests: 31 `#[test]` (11 in `exit_contract.rs`); dev-deps `encoding-admission`, `isa-*`, `layout-plans`.
- Open work: `ADDRESS-EXPOSURE` in `TASKS.md` (covers `NATIVE-CODE-LAYOUT-QUERIES`), which concerns
  `code_layout`, by title only (the item text does not name this path).

### 2.7 `object-file` (stage 14)
- Path: `P/14_placed-functions-to-objects`; 19 files, 6184 lines. Package `object-file`, lib `object_file`.
- Purpose: Turns a placed text section into a relocation-free object (wire formats `OMGTRO` version 5 and
  `OMGOBJ` version 6, described in `src/lib.rs:11-27`), and defines the linkable `ObjectPlan` and
  `RelocationPlan` that image writers consume. Also places byte literals, code descriptors, import fields and
  private addresses in object sections (`body_object/`).
- Consumes `function_layout::FunctionPlacement<Contracts>`. Produces `PublishedFunctionObject`
  (`src/fragment_container.rs:143`), `ObjectPublication` (`:384`), `ObjectPlan` (`src/object_plan.rs:96`),
  `RelocationPlan` (`:369`).
- Entry points: `publish_function_object` (`src/fragment_container.rs:83`, called at
  `driver/compile_body.rs:268`); `validate_optimized_relocation_free_object_container` (`:167`);
  `place_foreign_call`, `resolve_instruction_sequence` (`src/foreign_calls.rs:93,76`); symbol-name functions
  in `src/names.rs` (`object_symbol_name:90`, `private_function_symbol_name:56`).
- Production dependencies: `selected-form-encoding-to-resolved-layout`, `function-layout`, `diagnostics`,
  `runtime-abi`, `target-vocabulary`, `installation-admission`, `physical-recipes`, `physical-program`,
  `wire`, `calling-conventions`, `layout-plans`, `machine-code`, `function-identity`, `optimization-core`,
  `selected-instructions`, `arena`, `semantic-vocabulary`, `terminal-psi`, `target`.
- Dependents (production): 7 (`psi-to-native` driver, `image`, `image-elf`, `image-emission`, `image-macho`,
  `image-pe`, `native-artifact`); dev: `build/publication`.
- Modules: `object_plan` (plans, symbols, relocations); `fragment_container`; `relocation_free_object/`
  (`mod.rs`, `publication.rs`, `text_section.rs`); `body_object/` (byte literals, code descriptors, imports,
  image residence, native code layouts, private addresses); `bridge_object`, `program_entry_object`,
  `foreign_calls`, `names`, `target_matrix` (five admitted architecture/format pairs, `src/lib.rs:37-39`).
- Tests: 29 `#[test]` (11 in `relocation_free_object/mod.rs`). No `tests/` directory.
- Open work: none named.

### 2.8 `image` (stage 15, format-neutral)
- Path: `P/15_objects-to-image/image`; 12 files, 5426 lines. Has a `test-support` feature.
- Purpose: Builds `FinalImage` from an `ObjectPlan` and `RelocationPlan`, applies relocations once a container
  writer chooses its virtual layout, checks text/data relocation envelopes, places executable and data region
  inventories, and defines the evidence digests the writers and `image-emission` share (`src/lib.rs:1-45`).
- Consumes `object_file::ObjectPlan`, `RelocationPlan`, text and data bytes (`FinalImageInput`,
  `src/builder.rs:151`). Produces `FinalImage` (`src/final_image.rs:1611`), `ExecutableImageOutput`
  (`src/output.rs:12`).
- Entry points: `build_final_image` (`src/builder.rs:160`); `apply_relocations` (`src/relocations.rs:17`);
  `place_executable_regions` / `place_data_regions` (`src/final_image.rs:1048,77`);
  `validate_final_text_relocation_envelope` and `validate_final_initialized_data_relocation_envelope`
  (`src/relocation_envelope.rs:86,19`); `emitted_direct_executable_output` (`src/output.rs:161`).
- Production dependencies: `machine-code`, `wire`, `calling-conventions`, `function-identity`, `arena`,
  `diagnostics`, `object-file`, `target`.
- Dependents (production): 5 (`image-elf`, `image-emission`, `image-macho`, `image-pe`, `native-artifact`).
- Modules: `builder`, `final_image` (1871, symbols, relocation table, region inventories), `relocations`,
  `relocation_envelope`, `output`, `import_code`, `test_support`.
- Tests: 33 `#[test]` (15 in `src`, 18 across `tests/initialized_data_envelope.rs`,
  `tests/relocation_rejections.rs`, `tests/text_relocation_envelope.rs`).
- Open work: none named.

### 2.9 `image-elf`
- Path: `P/15_objects-to-image/image-elf`; 11 files, 22414 lines.
- Purpose: Writes 64-bit ELF: static `ET_EXEC`, a 22-stage dynamic executable pipeline, and `ET_DYN` shared
  components for Linux x86-64 and AArch64. The 22 steps are spread across modules named by their step range
  in each file's header comment (read).
- Consumes `image::FinalImage`. Produces `ExecutableImageOutput`.
- Entry points: `emit_elf_executable` (`src/static_executable.rs:176`); `emit_elf_dynamic_executable`
  (`src/dynamic_executable.rs:614`); `emit_elf_shared_component` (`src/shared_component.rs:712`);
  `ValidatedElfDynamicExecutable` re-export (`src/lib.rs:132`).
- Production dependencies: `calling-conventions`, `machine-code`, `wire`, `arena`, `diagnostics`, `image`,
  `object-file`, `target`. Dependents (production): 1 (`image-emission`).
- Modules: `import_sections` (steps 1-4), `procedure_linkage` (5-7), `dynamic_table` (8-10),
  `section_headers` (11-15), `load_placement` (16-18), `file_assembly` (19-22), `dynamic_executable`
  (driver), `static_executable`, `shared_component`, `imports`.
- Tests: 137 `#[test]`, all inside `src` modules.
- Open work: none named.

### 2.10 `image-macho`
- Path: `P/15_objects-to-image/image-macho`; 5 files, 4788 lines.
- Purpose: Writes PIE, ad-hoc signed, dyld-linked Mach-O executables and `MH_DYLIB` shared components for
  macOS AArch64 and x86-64, binding only referenced imports (`src/lib.rs:1-33`).
- Entry points: `emit_macho_executable` (`src/lib.rs:673`), `emit_macho_executable_signed` (`:677`),
  `emit_macho_shared_component` (`src/shared_component.rs:303`), `code_signature_identifier`
  (`src/code_signature.rs:158`).
- Production dependencies: `machine-code`, `wire`, `arena`, `diagnostics`, `image`, `calling-conventions`,
  `object-file`, `target`, `target-x86_64`, external `sha2`. Dependents (production): 2 (`image-emission`,
  `native-artifact`); dev: `build/publication`.
- Modules: `dyld_linking` (2702), `file_layout`, `code_signature`, `shared_component`.
- Tests: 37 `#[test]` (11 in `lib.rs`, 19 in `dyld_linking.rs`). Open work: none named.

### 2.11 `image-pe`
- Path: `P/15_objects-to-image/image-pe`; 6 files, 2234 lines.
- Purpose: Writes PE32+ executables and DLL shared components for Windows x86-64, and a minimal Windows DLL of
  already encoded helper exports (`windows_provider.rs`, which "never chooses instructions", `:1-2`).
- Entry points: `emit_pe_x86_64_executable` (`src/lib.rs:174`), `emit_pe_shared_component`
  (`src/shared_component.rs:61`), `emit_windows_internet_provider` (`src/windows_provider.rs:3`).
- Production dependencies: `calling-conventions`, `arena`, `diagnostics`, `image`, `object-file`, `target`,
  `machine-code`. Dependents (production): 1 (`image-emission`). Windows-only dev-deps: `entry-planning`,
  `native-encoding`, `runtime-abi`.
- Modules: `headers`, `imports`, `sections`, `shared_component`, `windows_provider`.
- Tests: 22 `#[test]`. Open work: none named.

### 2.12 `image-emission`
- Path: `P/15_objects-to-image/image-emission`; 19 files, 34685 lines (21313 in `src/function_fragments.rs`,
  which holds many inline modules and tests). Has a `test-support` feature.
- Purpose: Binds entry points (physical, hosted, callable, foreign) to the body object, composes private
  function and component objects, relates object sections to instruction and stack evidence
  (`function_fragments`), emits the executable or shared image through the three writers or the dynamic ELF
  lane, and checks the written bytes (`src/lib.rs:1-5`).
- Consumes `object_file::PublishedFunctionObject` (through `prepare_object_for_image`) and encoded entries
  from stage 11. Produces sealed `ObjectArtifact` (`src/object_artifact.rs:23`) and `ExecutableImage`
  (`src/image.rs:1832`).
- Entry points: `prepare_object_for_image` (`src/function_fragments.rs:2769`);
  `compose_private_function_objects` (`src/object.rs:440`); `emit_component_shared_image`
  (`src/object.rs:242`); `bind_encoded_physical_entry`, `bind_private_function_entry`,
  `emit_native_foreign_adapter` (re-exports at `src/lib.rs:34-43`); `bind_hosted_program_entry`
  (`src/hosted_program.rs:12`); `emit_executable_image` (`src/image.rs:1631`) and `can_emit_executable_image`
  (`:1692`); `inspect_native_code_layouts` (re-export, `src/lib.rs:90`); `verify_entry_bridge_bytes`
  (`src/published_entry_bytes.rs:11`).
- Production dependencies (40, read from manifest): stage 14 `object-file`; `image`, `image-elf`,
  `image-macho`, `image-pe`; representations and verification crates (`machine-code`, `physical-program`,
  `physical-recipes`, `post-allocation-machine`, `selected-instructions`, `abstract-operations`,
  `optimization-unit`, `legalized-operations`, `verified-optimization-input`, `provider-admission`,
  `installation-admission`, `installation-conditions`, `program-entry-contracts`, `structural-storage-queries`,
  `native-execution-reach`); Psi crates (`terminal-psi`, `terminal-semantics`, `psi-publication-records`,
  `psi-entry-memory`, `layout-plans`, `language-core`); targets (`target`, `target-aarch64`, `target-x86_64`,
  `runtime-abi`, `register-model`, `calling-conventions`); `sha2`.
- Dependents (production): 4 (`psi-to-native` driver, `native-artifact`, `native-sponsor`,
  `verification/published-objects`); dev: `native-worker`.
- Modules: `function_fragments` (single large file plus `current_image_selections`, `image_data`,
  `internal_calls`, `validation`, `source/retained_source_validation`); `object` (inline
  `component_object`, `component_shared_image`, `private_object_composition`); `image` (inline `dynamic_elf`,
  `entry_effects`, `final_image_validation`, `image_output`); `entries` (4550, plus `hosted_entry_storage`);
  `object_artifact` (2877); `hosted_program`, `hosted_loader`, `callable_calls`, `companion_images`,
  `object_input`, `published_entry_bytes`, `retained_working_program` (the one place stage 02's working
  program is read).
- Tests: 121 `#[test]` (76 in `function_fragments.rs`, 18 in `entries.rs`). Dev-deps include `iced-x86`.
- Open work: `PIPELINE-ARCHITECTURE-ENFORCEMENT` in `TASKS.md` (covers `IMAGE-SELF-REPLAY-DELETED`); its text
  names no path here. `ADDRESS-EXPOSURE` (covers `NATIVE-CODE-LAYOUT-QUERIES`).

### 2.13 `native-artifact`
- Path: `P/15_objects-to-image/native-artifact`; 30 files, 16174 lines.
- Purpose: Source-free handoff from canonical Terminal Psi bytes to a finished native product: publishes
  fragment objects, builds `NativeArtifact` / `DynamicElfNativeArtifact`, derives physical evidence for each
  surviving operator or boundary occurrence from object and image bytes, and issues digest-bound publication
  receipts for executables and macOS `.app` bundles (`src/lib.rs:1-26`).
- Consumes `image_emission::ObjectArtifact`, `ExecutableImage`, selected provider plans, canonical Psi
  artifact. Produces `NativeArtifact` (`src/native_artifact.rs:432`), `FinishedNativeImage` (`:174`),
  `PublishedFragmentObject`, `NativeComponentProduct`, `ExecutablePublicationReceipt`,
  `NativePackagePublicationReceipt`, `TerminalRuntimeProvisions`.
- Entry points: `emit_fragment_object` (`src/fragment_object_publication.rs:171`, called at
  `driver/compile_body.rs:271`); `publish_fragment_object` (`:71`, called at
  `driver/image/object_emission.rs:260`); `ProgramImageInput::emit` (`src/native_artifact.rs:208`, called at
  `driver/mod.rs:149`); `NativeArtifact::validate` (`:744`); `publish_macos_application_package`
  (`src/package_publication.rs:280`); `describe_portable_component` (`src/component_description.rs:15`);
  `emit_runtime_adapter` (`src/runtime_provisions.rs:339`); `executable_container_digest`
  (`src/publication.rs:109`).
- Production dependencies (read): `image`, `image-emission`, `image-macho`, `object-file`, `terminal-psi`,
  `terminal-semantics`, `terminal-codec`, `machine-code`, `selected-instructions`, `optimization-core`,
  `operation-evidence`, `boundary-applications`, `verified-optimization-input`, `provider-admission` and its
  `authority` and `same-stack` children, `native-binding-admission`, `native-intrinsics`,
  `target-provider-admission`, `component-description`, `published-runtime-input`,
  `installation-conditions`, `native-execution-reach`, `selected-provider-plans` and `provider-plans`
  (both under `build/providers`), `psi-scalar-applications`, `psi-provider-selections`, `psi-entry-memory`,
  `platform-custody`, targets, `wire`, `diagnostics`, `sha2`, and others (about 45 path dependencies).
- Dependents (production): 3 (`build/publication`, `psi-to-native` driver, `interpreter/host/retained-image`).
- Modules: `native_artifact` (1298; pulls in `dynamic_elf`, `identity`, `mixed_structural_scalar`,
  `provider_execution`, test file via `#[path]`, `src/native_artifact.rs:25-35`); `fragment_object_publication`;
  `publication`; `package_publication`; `component_description`, `component_product`; `runtime_image`,
  `runtime_provisions`; `physical/` (`mod.rs`, `projection`, `operator_applications`, `replay`, `originals`,
  `fragment_publication`, and `derivation/` with `evidence`, `foreign_calls`, `hashing`, `internal_calls`,
  `hosted_builtins`, `provider_custody`, `provider_settlements`, `settlement_identity`, `fragment_custody`).
- Tests: 53 `#[test]` (14 in `physical/derivation/tests.rs`, 8 in `native_artifact_tests.rs`, 8 in
  `physical/projection.rs`). Dev-deps `operation-operands`, `optimization-core`.
- Open work: `RC-RELEASE-RECORD-AND-CLOSURE` (native matrix) in `TASKS.md`; `PIPELINE-ARCHITECTURE-ENFORCEMENT`
  (executable qualification on `linux_x86_64`). Neither names this crate.

## 3. How data flows through the area

1. `driver/compile_body.rs:193` calls `native_encoding::encode_bound_function_body` with the stage 10
   `BoundFunctionBody`. It chooses ISA encoders in `isa-x86_64` / `isa-aarch64` per instruction row and
   returns `StagedOptimizedSelectedFormEncoding`.
2. `compile_body.rs:219` calls `placement::resolve_program` (stage 12), which builds the baseline layout
   (`ordinary`), then relaxes x86 branches (`settled`, `x86_branch_relaxation`) and returns `LayoutResolution`.
3. `compile_body.rs:246` calls `function_layout::place_function_text` (stage 13): `function_realization`,
   `fragment_emission`, `frame_application`, `text_placement`; the result is `FunctionPlacement`.
4. `compile_body.rs:268` calls `object_file::publish_function_object` (stage 14), then
   `compile_body.rs:271` calls `native_artifact::emit_fragment_object`, which calls
   `image_emission::prepare_object_for_image` (`fragment_object_publication.rs:174`).
5. For an image request, `driver/image/object_emission.rs:260` calls `native_artifact::publish_fragment_object`,
   which composes private objects (`compose_private_function_objects`, `:77`) and binds hosted or physical
   entries (`bind_hosted_program_entry` `:80`, `bind_encoded_physical_entry` `:129`).
6. `driver/mod.rs:149` calls `ProgramImageInput::emit`; `native_artifact.rs:380` calls
   `image_emission::emit_executable_image`, which builds `FinalImage` with `image::build_final_image`, applies
   relocations, and calls `emit_elf_executable`, `emit_elf_dynamic_executable`, `emit_macho_executable[_signed]`
   or `emit_pe_x86_64_executable` by target.
7. `native-artifact` seals the result as `NativeArtifact`, derives physical evidence, and later issues the
   publication receipts (`publication.rs`, `package_publication.rs`).

Cargo shape follows this order: 11 -> 12 -> 13 and 12 -> 14 (`P/14.../Cargo.toml` depends on both 12 and 13);
14 -> `image` -> `image-elf`/`image-macho`/`image-pe` -> `image-emission` -> `native-artifact`.

## 4. Where this area's map differs from the documents

- `P/13.../README.md:3-5` says `src/lib.rs` "lists the four stages ... the three evidence modules ... and the
  x86-64 byte recipes". `src/lib.rs:1-3,5-12` is one doc line and a module list with no such listing.
- `P/13.../README.md` links files that are not in `P/13.../src/`: `src/frame_layout.rs`,
  `src/frame_protocol.rs`, `src/emission_facts.rs`, `src/private_callback.rs`, `src/native_entry_bridge.rs`
  (grepped links against `ls src`). A `native_entry_bridge.rs` and a `frame_protocol.rs` exist in stage 11
  (`P/11.../src/`), not stage 13.
- `P/13.../README.md` links `../images/image-emission/src/function_fragments.rs`; the directory is
  `../15_objects-to-image/image-emission/` (`ls P` shows no `images`).
- `P/13.../README.md:44` links `../../../../pipeline.md#verification-placement`, which resolves to
  `source-rust/subsystems/pipeline.md` (absent). `AGENTS.md` (omega-rust-v2) points at
  `_recycling/source-rust/pipeline.md`, which exists.
- `P/12.../src/ordinary.rs:3` says it lays out functions "in stage 08"; the crate is stage 12.
- `P/11.../x86_64-encoding/src/lib.rs:15-17` says "backend machine-emission consumers" also use its byte
  sequences. Only `isa-x86_64` depends on it (dependents script).
- `P/15.../native-artifact/src/lib.rs:20-21` names crates `machine_emission` and `object_file`; no crate named
  `machine_emission` exists (grepped). The stage 13 `FunctionRelativeOptimizationUnavailableData.machine_emission`
  field (`P/13.../src/function_realization.rs:97`) is a field name only.
- `psi-to-native/README.md:30` says "The Cargo boundary check records which production packages can read each
  stage". `tools/xtask/src/boundary/native_stages.rs:25-30` lists scoped exceptions; it allows stage 14 to read
  stage 12 ("Fixed-width entry layouts already contain the final call and relocation fields"), and the stage 14
  manifest also reads stage 13. The exception list shows only `(stages[14], stages[12])`, so stage 14 reading
  stage 13 (`function-layout`) is covered by the "successor" rule only if stage 14 counts as stage 13's
  successor (inferred from `except_owners: stages[index..index+2]` at `native_stages.rs:66`).
- Module headers in `image-emission/src/function_fragments.rs` and `image.rs` describe files as modules, but
  the crate keeps most of its modules inline inside those two files (`image.rs:19,291,488,1518`;
  `function_fragments.rs:8,750,1516,...`), so the file tree under `src/` does not match the module tree.

## 5. What I did not read, and what I am unsure about

Not read in detail (skimmed headers and signatures only): the bodies of `isa-x86_64/src/selected_form_encoding.rs`
(21917 lines) and `isa-aarch64/src/selected_form_encoding.rs` (17625), `image-emission/src/function_fragments.rs`
(21313), `entries.rs`, `object_artifact.rs`, `exit_contract.rs`, `frame_application.rs`, the ELF step modules,
`dyld_linking.rs`, and `native-artifact/src/physical/derivation/*`. The wire-format claims for `OMGTRO`/`OMGOBJ`
come from the header comment in `P/14.../src/lib.rs`, not from decoding code. No test, build or program was run,
so test counts are `#[test]` occurrences only (they omit macro-generated tests such as `order_suite!` in stage 12
and any `isa-conformance` tests that run through `psi-to-native/tests/isa-conformance`).

Unsure:
- Whether stage 14's dependency on stage 13 is allowed by the boundary rule (see section 4, last-but-one bullet).
- The dependents counts come from path dependencies only; a crate reaching these through a renamed or
  workspace-inherited dependency without `path =` would be missed (the 11-15 crates are all path dependencies
  in the manifests read).
- "Dependents: `psi-to-native` driver" is the root `psi-to-native/Cargo.toml` package; I did not read its package
  name.
- The line counts for `native-encoding` in `crates.tsv` (3806) exclude the three nested ISA crates, which have
  their own rows.
- No `TASKS.md` or `OWNER_CODE_CONCERNS.md` entry names a path in this area; the entries listed under
  "Open work" were matched by subject only.
