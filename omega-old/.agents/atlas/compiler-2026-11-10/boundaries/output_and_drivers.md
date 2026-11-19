# Output side and drivers: facts for the end-to-end design

Checkout `worker-23` at `a336a9f17c0` (atlas citations are at `457adf4a11f`; lines re-located by function name).
Nothing built or run. Paths below are relative to `omega-rust/omega/`.

Marking: **[opened]** = I read the code at the cited lines in this checkout. **[record]** = reported
from an atlas digest or record, not reopened by me (records are right about 82% of tested claims;
"unused" and "same job" claims are the least reliable). Readers' "suspected", "not traced" kept.

Short forms: `EM/` = `compilation/backend/images/image-emission/src/`, `ELF/` = `.../images/image-elf/src/`,
`MO/` = `.../images/image-macho/src/`, `PE/` = `.../images/image-pe/src/`, `NA/` = `.../artifacts/native-artifact/src/`,
`ME/` = `.../backend/machine-emission/src/`, `NC/` = `compilation/native-compiler/src/`, `ff` = `EM/function_fragments.rs`,
`CLI/` = `application/cli/src/`, `ORCH/` = `build-system/orchestration/src/`.

---

## The single most consequential fact

The default `omega` compile never runs the full native replay. `NativeArtifact::validate`
(`NA/native_artifact.rs:521-584`) is the one routine that replays fragment/target/selected evidence
(`ff` `replay::validate`, which nests the machine-emission and object-file stage validators,
`ff:24426-24432`), re-runs `validate_terminal_image`, replays provider executions and recomputes identity.
The default emit path seals without calling it (`NA/native_artifact.rs:194-219`, `:471-478`, `:508-519`)
[opened]. Its only production caller is `inspect_native_code_layouts` (`NA/native_artifact.rs:616-622`),
reached only when the Terminal runtime is handed a native image (`interpreter/terminal-runtime/src/execution.rs:269-270`,
`.../execution/native_code_layouts.rs:16-37`), which the CLI does only for `omega run --both` on the host
(`CLI/execution/compilation.rs:108`, `CLI/execution/mod.rs:214-216`) [opened]. If that validation fails,
the error is `MissingProvision { capability: "native-code-layout" }` (`native_code_layouts.rs:35-37`),
`comparison_unavailable()` returns true for every `MissingProvision` (`terminal-runtime/src/execution.rs:101-103`),
the comparison becomes `Declined` (`CLI/execution/mod.rs:544-545`) and the process exits with the native
exit code (`CLI/commands/run.rs:110-113`) [opened]. Dynamic-ELF products are never given to the runtime as
a native image at all (`compilation/products/src/compilation_products.rs:459-485`: `retained_native_artifact()`
is `None` for `RetainedDynamicElfArtifact`) [opened], and `DynamicElfNativeArtifact::validate` runs only from
`from_replayed_parts` (`NA/native_artifact/dynamic_elf.rs:86-92`), which has no production caller
(grep [opened]; digest area_02 item 2 agrees).

The specified `--validator` / `--every-validator` options (`wiki/spec/build/command_line.md:17`, `:104-125`)
have no implementation: zero hits for either spelling in `omega-rust/**/*.rs` [opened by grep].
The `--verify` flag that does exist withholds the lock's accepted checkpoint (`CLI/arguments/compile.rs:18`,
`ORCH/lib.rs:138`) and selects no stage validator [opened].

---

## Part 1. Output side, boundary by boundary

Each entry: handed over; receiver trusts / re-checks; check and when it runs; independence; per target.

### B1. Resolved layout -> machine emission (frame, fragments, frame application, text placement)
- Handed over: `ValidatedOptimizedTargetOperations` plus encoding and resolved-layout programs, assembled by
  `machine_emission::assemble_fixed_frame_realization` (`NC/pipeline.rs:196-205`) into
  `StagedOptimizedFunctionFragmentEmissionSource` (`NC/pipeline.rs:205`) [opened].
- Receiver: `stage_optimized_function_fragment_emission` (`ME/fragment_emission.rs:59-70`),
  `stage_function_fragment_frame_application`, `stage_optimized_fixed_frame_text_section`
  (`ME/text_placement.rs:52-63`) compute and attach a receipt; they do not call their validators [opened].
- Check: `validate_optimized_function_fragment_emission` (`ME/fragment_emission.rs:72-83`) and
  `validate_optimized_fixed_frame_text_section` (`ME/text_placement.rs:65-79`) exist; their only production
  callers are each other in a chain ending at `object_file::validate_optimized_relocation_free_object_container`
  (`object/object-file/src/fragment_container.rs:53-58`), called only from `ff:24430` inside the replay [opened].
  So: never in default compilation; runs under `omega run --both` (host, direct image only) and test harnesses.
- Independence: build-then-replay; the records say the stage validators re-derive with the stage's own rules
  ("dominant pattern ... build-then-replay", area_03 "What this area does") [record].
- Per target: `TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1` is the only policy passed
  (`NC/pipeline.rs:161`) [opened]; return bytes are literals per ISA (`ME/exit_contract.rs:2723, 2741`) [record].

### B2. Text section -> relocation-free object (OMGTRO) -> `ObjectArtifact`
- Handed over: `StagedOptimizedFixedFrameTextSection` -> `stage_optimized_relocation_free_object_container`
  -> `image_emission::build_function_fragment_object_artifact` (`NA/fragment_object_publication.rs:165-187`) [opened].
- Receiver builds `ObjectArtifact` (functions, symbols, relocations, foreign-call custody, settlements;
  `ff:3592-3848`) and retains the source for later replay (`fragment_replay: Some(...)`, `ff:3827`) [opened].
  It does not run `replay::validate` or `source::admit` [opened: no call in `ff:3592-3848`].
- Check: `ff` `replay::validate` / `source::admit` (`ff:24426-24432`) — never in default compilation (see above).
  Records: `source::admit` has arms that return `true` with no check for atomics, constants and the
  arithmetic/comparison/cast family (`ff:18745`, `:18864-18865`, `:18995-19007`; digest-checked) [record].
- Independence: replay against the producer's retained inputs; same project, different module [record].

### B3. `ObjectArtifact` + private callback objects + ProgramEntry binding -> published object
- Handed over: `FragmentObjectProjection`, `PrivateFunctionObject`s, `ValidatedNativeProgramEntrySettlement`
  (`NC/realization/object_emission.rs:124-137`, `NA/fragment_object_publication.rs:63-163`) [opened].
- Receiver re-checks: private composition (`EM/object.rs:449-468`), access-view custody against private entry RAM
  (`fragment_object_publication.rs:71-97`), UEFI adapter is the emitted root (`:98-110`), hosted receiver binding
  (`:131-153`, `EM/entries.rs:2903-2952`), then `validate_emitted_receiver_binding` (`:189-250`) — default
  compilation, same-process equality of retained values [opened].
- Physical evidence: `physical_evidence_scope` is always `NativePhysicalEvidenceScope::Unavailable`
  (`fragment_object_publication.rs:161`), and `derive_physical_evidence` returns `Unavailable` at once for that
  scope (`NA/physical/derivation/evidence.rs:78-80`) [opened]. So no physical-evidence derivation happens in any
  production path.
- Per target: physical entry only for `UefiX64` slot owner (`fragment_object_publication.rs:98`;
  `NC/realization/object_emission.rs:94-95`) and only x86-64 COFF (`EM/entries.rs:5133-5146`) [opened].
  Hosted receivers exist for macos_arm64, macos_x64, linux_x64, linux_arm64, windows_x64
  (`EM/entries.rs:2960-3010`) [opened]. Private entry RAM allowed for linux_x64, linux_arm64, macos_arm64,
  and windows_x64 only when the slot owner is `TargetProfile::WindowsX64` (`fragment_object_publication.rs:83-91`) [opened].

### B4. `ObjectArtifact` -> `FinalImage` (format-neutral image, relocations applied)
- Handed over: `ObjectPlan`, `RelocationPlan`, text and data bytes via `image::build_final_image`
  (`EM/image.rs:1833-1840`; dynamic `:164-171`) [opened].
- Receiver: `apply_relocations` is called inside each writer (ELF static `ELF/static_executable.rs:216`,
  PE `PE/lib.rs:205`; records add `ELF/file_assembly.rs:2504`, `ELF/shared_component.rs:307`, `MO/lib.rs:783`) [opened/record].
- Records: `apply_relocations` does not read `byte_width` when patching; disagreement caught only by the envelope
  validators (`IM/relocations.rs:57-65`, traced) [record]. `final_image_symbol_address` adds an offset without a
  section-size check (`IM/lib.rs:221`, open) [record].

### B5. `FinalImage` -> container bytes (ELF static, ELF dynamic, Mach-O, PE)
- ELF static (Linux, no imports): `emit_elf_executable` (`ELF/static_executable.rs:169-258`) writes header,
  two program headers, text, data; returns `final_text_bytes: image.memory.text.clone()` (`:242`) — the same
  buffer it copied into `bytes`. No re-read inside image-elf [opened]. Imports present -> refuses unless the
  dynamic writer is selected (`:184-211`; `EM/image.rs:1756-1783`) [opened].
- ELF dynamic (Linux with normalized imports and an interpreter plan): 22 stages (`ELF/dynamic_executable.rs:614-663`,
  record). Each stage runs `plan_candidate` -> `validate_candidate` (`ELF/dynamic_executable.rs:282-437`), so stage
  validators do run in default compilation [opened]. At assembly the validator calls the same `expected_fragments`
  that `derive_contents` used (`ELF/file_assembly.rs:432` vs `:468`) — the producer's own builder [opened].
  Final admission `admit_elf_dynamic_executable` only derives the output (`ELF/file_assembly.rs:184-211`);
  the verifier that compares output with the assembled file and checks ELF magic, `validate_executable_output`,
  is `#[cfg(test)]` (`:352-391`) [opened]. **Record confirmed: ELF final-byte admission is test-only** (dynamic
  ELF). Static ELF has no final-byte admission at all.
- Mach-O (macOS): `emit_macho_executable_signed` -> `emit_macho_container_contents`, which after writing calls
  `validate_macho_container_mapping` on the bytes, `validate_macho_loader_fixups_for_segment`, and for dylibs
  `validate_export_trie` (`MO/lib.rs` near `:876-897`) [opened]. The mapping validator decodes header and load
  commands and compares file text and data with `final_text`/`final_data` (`MO/dyld_linking.rs:1992-2247`,
  `compare_file` at `:2166`, `:2247`) [opened]. Independence: hand-written decoder in the writer's own crate.
- PE (Windows and UEFI): `emit_pe_x86_64` (`PE/lib.rs:178-360`) installs/patches thunks, applies relocations,
  checks thunk footprints on the in-memory image before assembly (`:206`), writes headers and sections, and
  returns `final_text_bytes: image.memory.text.clone()` (`:344`). Nothing in image-pe re-reads `bytes` [opened].
  **Record "PE output is never re-read" is partly refuted**: image-emission's entry check decodes the written PE
  (see B6). Records: unchecked `as u32`/`as u16` casts (`PE/headers.rs:42, :52`, `PE/sections.rs:51...`), empty
  library binds to `KERNEL32.dll` (`PE/imports.rs:78-88`, digest-checked), UEFI subsystem rejection only on the
  Foreign-locator branch (`PE/imports.rs:95`, open) [record].

### B6. Container output -> `validate_terminal_image` (image-emission, default compilation)
- Runs on every direct and dynamic-ELF emission: `EM/image.rs:1851-1858` and `:207-216` [opened].
- What it reads (`EM/image.rs:833-1079`) [opened]:
  - Mach-O only: `validate_macho_loader_mapping(&output.bytes, ...)`, `validate_macho_object_fixups`,
    `validate_macho_import_binding_pairing` (`:842-864`). **Record "Mach-O alone re-reads the finished
    container" confirmed** for full-container decoding.
  - All formats: counts, layout vs inventories, region inventories against `final_text_bytes` /
    `final_data_bytes`, relocation envelope (`validate_final_text_relocation_envelope`, `:1054-1058`), data
    relocation envelope, implementation tables, code layouts, byte literals, callback relocations. These read the
    writer-reported fields, which for ELF static and PE are clones of the writer's input buffer (above), not
    bytes read back from the file.
  - Entry: with an entry shim, the file bytes are decoded at the entry: hosted receiver (`EM/entries.rs:3200-3305`):
    macOS `main_points_to` (LC_MAIN), Windows `pe_entry_points_to` (MZ, `PE\0\0`, PE32+ magic, AddressOfEntryPoint,
    `.text` section header, shim bytes at the raw entry offset; `EM/entries.rs:4008-4093`), Linux a local
    `elf_entry_points_to` that reads only bytes 24..32, `e_entry` (`EM/entries.rs:3308-3319`). Free-Unit entry form:
    ELF `e_entry` + executable PT_LOAD + shim bytes (`EM/entries.rs:4095-4160`), PE and Mach-O as above
    (`EM/entries.rs:3817-3878`). Receiver shim instructions are re-decoded per platform
    (`instructions::validate_windows_x86_64` / `validate_aarch64` / `validate_x86_64`, `EM/entries.rs:3276-3305`)
    from `final_text_bytes`.
- Independence: entry decoders are hand-written second copies in image-emission, separate from the writer crates;
  the rest compares the writer's reported fields with re-derived expectations (same project, same plan).
- Records: `ProvisionedUnit` shim bytes are not checked inside `validate_terminal_image`; emit paths do not run
  `validate_unit_shim`, only the replay path does (`EM/image.rs:1019`, traced; confirmed by my reading of
  `:1017-1019` and `:1941-1952`) [opened].

### B7. Emitted image -> `NativeArtifact` / `DynamicElfNativeArtifact` (seal)
- `NativeArtifact::from_emitted_parts` checks image vs object foreign calls, derives physical evidence (always
  `Unavailable`), seals identity (`NA/native_artifact.rs:471-519`). No `validate()` [opened].
- `NativeArtifact::validate` exists (`:521-584`): runs under `--both` host only (see top). Records: it refuses any
  module containing a nearest-FMA operation (`NA/native_artifact.rs:1076-1090`, digest-checked) [record].

### B8. Native artifact -> publication -> file on disk
- `PreparedExecutablePublication::new` (`NA/publication.rs:277-297`) checks the file name is one component and
  that `compiler_text_validation` and `compiler_function_validation` are present, then hashes the container
  [opened]. `compiler_function_validation` is a summary of counts and fingerprints (`ff:4123-4153`), not a check
  [opened].
- Files are staged, read back and byte-compared, renamed, and read back again (`foundation/platform-custody/src/file_publication.rs:27-95`)
  [opened]. This checks the disk round trip only, not format.
- Windows: if the image contains the bytes `omega_host.dll` anywhere, a synthesized provider DLL is written beside it
  (`compilation/products/src/publication.rs:219-245`; substring test `runtime-abi/src/windows_provider.rs:14`, record) [opened/record].
- macOS GUI: `.app` bundle publication (`publication.rs:88-126`) [opened].

### Calling conventions, ISA crates, plans, register environment (records unless marked)
- Call-plan validation is structural (coverage, disjointness, alignment, architecture); it never compares registers
  with what the policy would assign (`CC/plans/call_plan_evaluation.rs:167-310`, open) [record].
- One `Aapcs64` policy for Linux arm64 and macOS arm64 (`CC/plans/vocabulary.rs:488-489`) [record]; "Microsoft x64
  aggregate classification is not normalized yet" (`call_plan_evaluation.rs:463`) [record].
- Register environment is built and validated per `NativeTarget` and cached (`register-environment/src/lib.rs:66`);
  called on every emission (`NC/pipeline.rs:103-104`) [opened call; record for contents]. Catalog validation
  compares with a rebuild from the same builder (`isa-x86_64/src/register_model.rs:1383-1410`) [record].
- ISA encoders are used by default emission; the selected-form validators (A-SFE:14890, 14925; X-SFE:23899, 23943)
  re-check streams [record]; whether they run outside the replay chain I did not trace.
- x86 `encoded_effects` second `match` has `_ =>` declaring no effects (`isa-x86_64/src/machine_effects.rs:1395-1403`,
  digest-checked) [record].

### Record claims checked
| Record claim | Verdict | Where |
| --- | --- | --- |
| ELF final-byte admission is test-only | Confirmed (dynamic ELF); static ELF has none | `ELF/file_assembly.rs:184-211`, `:352-391` |
| PE output is never re-read | Partly: image-pe never re-reads; image-emission decodes PE headers, `.text` header and entry shim bytes from the file | `PE/lib.rs:178-360`; `EM/entries.rs:3238-3239`, `:4008-4093` |
| Mach-O alone re-reads the finished container | Confirmed for whole-container decode; ELF/PE get entry-only decodes | `MO/dyld_linking.rs:1992-2247`; `EM/image.rs:842-864` |
| Windows x64 and UEFI x64 are the same `NativeTarget` | Confirmed (fields: architecture, object_format, pointer size/alignment; no OS field) | `compilation/backend/target/src/target_profile.rs:695-711` |
| Program-entry binding only works for `uefi_x64` | Refuted as stated: hosted receivers bind ProgramEntry on the hosted targets; the *physical* entry binding is UEFI x86-64 PE only | `NA/fragment_object_publication.rs:98-154`; `EM/entries.rs:2960-3010`, `:5133-5146` |

---

## Part 2. Driving side: one `omega` compile

1. **CLI -> BuildRequest.** First token picks a subcommand, unknown first token starts a compile (`CLI/arguments/mod.rs:76-100`) [record].
   Zero `--target` -> `BuildTargets::Host`, one -> `Single`, two+ -> `Batch` (`CLI/commands/compile.rs:9-11`, `:108-113`) [opened].
2. **Target decided** by the CLI flag or the build host: `ORCH/build.rs:168-182` maps `Host` to
   `TargetProfile::host_if_supported()` (`compilation/backend/target/src/target_profile.rs:304-318`: windows host -> `WindowsX64`, never UEFI) [opened].
   `build.omg` does not choose the target in this path. Trusted: the CLI string; checked: name parses to a catalogued profile (`target_profile.rs:320-329`).
3. **Package preparation / lock.** `packages::begin_local_project_preparation` (`ORCH/build.rs:54-65`) reads `omega.lock`,
   recovers transactions, and selects the accepted lock section per requested target; a single-target request with no
   accepted section fails; no lock -> `accepted: None` and compilation continues "unlocked"
   (`packages/package-manager/src/operations/prepare_project.rs:229-265`) [opened]. Package **review** passes run only
   in `omega install` / `omega update` (`ORCH/package/package_commands.rs:1-57`) [opened]; an ordinary compile receives
   accepted evidence through `PackageCompilationInputs`, e.g. the accepted `ConsoleExitProcessI32` semantic binding
   handed to the source checker (`compilation/compiler/src/checked/checking/source_checking.rs:42-58`) [opened].
4. **`build.omg` evaluation.** `resolve_project_sources` (`ORCH/compile/build_execution/source_resolution.rs:152`) and
   `execute_returned_build` / `execute_build` (`ORCH/compile/build_execution.rs:123-181`): the compiler checks the Build
   program (`compiler.begin_package_build`), then `build_settings` runs it as a stepped activation (`advance(1024)`),
   answering product queries; a provider-readiness wait is refused (`:166-171`) [opened]. build-settings executes on
   the Terminal interpreter (`build-system/build-settings/src/activation.rs:55`, `:234`) [opened imports]. Records:
   filesystem fenced and attempts recorded (`packages/package-compilation/src/build_execution.rs:67-168`) [record].
5. **Front half.** `psi_compiler::CheckRequest { ... }.check()` (`source_checking.rs:86-95`) receives Build-derived
   provider selections, root ProgramEntry bindings and per-target layout scopes (`source_checking.rs:34-36`, `:170-255`) [opened].
   The Terminal program is produced once per cohort and shared (`compilation/compiler/src/terminal/terminal_artifact.rs:476-484`),
   then per application a `TerminalApplication` with provider proposals and the ProgramEntry receipt
   (`terminal_artifact.rs:432-466`) [opened] -> `RetainedTerminalArtifact` with a native realization proposal
   (`NC/application.rs:22-39`) [opened].
6. **Provider selection.** Records: per-target provider sets are re-derived from `provider_selections` +
   `default_provider_selections` in three places (`source_checking.rs:170-256`, `comp/terminal/product_code.rs:139-199`,
   `comp/checked/target_preparation.rs:97-154`); provider-selection chooses one plan per slot and grows demand to a
   fixed point (`ps/choice.rs:48`, `:80`; `ps/publication.rs:318`); settlement in `pp/provider_settlement.rs:160-359` [record].
   Native side: `proposal.prepare_native_providers` (`NC/compile.rs:90-101`), admission request (`:104-109`),
   `admit_prepared_native_realization` and `native_artifact::admit_executable_entry` (`NC/realization.rs:44-58`) [opened].
7. **Native compiler.** `ORCH/compile/native_completion.rs:113-131`: `native_compiler::prepare`, scalar completion via
   `build_settings::scalar_completion::complete` (`:115-127`), then `native_compiler::compile` [opened].
   `prepare` trusts the retained Terminal artifact and its proposal (`NC/request.rs:56-94`), checks installation
   assumptions against the artifact and target (`NC/preparation.rs:119-167`) and admits private entry RAM against target
   layouts (`:59-93`) [opened]. Omega 01-09 run in `NC/pipeline.rs:75-208` with no stage-output validator call [opened].

---

## Part 3. The oracle: `omega run --both`

- Reference engine: the Terminal interpreter via `terminal_runtime::execute`, on the same produced Terminal artifact and
  the native admission's runtime provisions (`CLI/execution/compilation.rs:94-131`) [opened]. Not the checked interpreter.
  (The corpus harness can choose the checked-tree interpreter with `OMEGA_CORPUS_CHECKED_INTERPRETER=1`,
  `compilation/compiler/tests/corpus_runner.rs:80-84` [opened].)
- Order: interpretation runs before native publication (`CLI/execution/mod.rs:214-242`) [opened].
- Compared: (1) exit status: Unit completion = 0, `ExternalCompletion.return_code`; matched against the native status,
  masked `& 255` on Unix, full 32 bits on Windows (`mod.rs:551-591`; `process_exit.rs:50-57`); (2) crash: only
  `CrashCause::Trap`, mapped to SIGTRAP (arm64), SIGILL (x86-64 Unix), `-1073741795` (Windows); UEFI and others none
  (`process_exit.rs:59-92`); (3) private entry RAM bytes, mismatch -> `RunError` (exit 200) (`mod.rs:275-281`) [opened].
- **Not compared: stdout and stderr.** The interpreter's streams are written to files only with `--keep`
  (`mod.rs:282-286`, `:349-378`); the native streams are forwarded (`run.rs:104`) [opened]. Case runs write both to
  files and compare exit only (`mod.rs:479-497`) [opened]. The corpus harness compares each engine's stdout with an
  authored `expected_stdout.txt` and records native-vs-interpreter exit agreement (`corpus_runner.rs:44-73`, `:904-912`) [opened].
- Skipped: `--both --target X` -> no interpretation (`mod.rs:215`) and `TargetOnly`; the native program is not run
  either and the command exits 0 (`mod.rs:243-244`; `run.rs:91-97`) [opened]. **Record confirmed.**
- Declined (native exit code kept, `run.rs:110-113`) [opened]: any `MissingProvision` or the four
  `UnsupportedCapability`-family errors (`terminal-runtime/src/execution.rs:101-116`); non-process entry result,
  sponsor exhaustion (default 10,000,000 work units, `mod.rs:87`), awaiting effect, pending host computation
  (`mod.rs:554-573`); a Trap with no host mapping or any non-Trap crash (`mod.rs:574-582`). Includes a failed
  `NativeArtifact::validate` (top of file). **Record confirmed** and extended.
- Divergence exit 201: exit mismatch, crash mismatch, or interpretation `Failed` (`run.rs:122-143`) [opened].
- What it covers: the exercised inputs and the exit status / trap / entry-RAM bytes of one run on the host target.
  It cannot see a defect that both engines share (same Terminal artifact, same provider admission and provisions) —
  `omega-rust/pipeline.md:124-129` says so.

---

## Answers

### (a) Output-side hand-offs taken on faith in default compilation
1. Resolved layout -> machine-emission stages -> text section -> relocation-free object: no stage validator runs
   (`ME/fragment_emission.rs:59-70`, `ME/text_placement.rs:52-63`; validators reached only via `ff:24430`) [opened].
2. Object container -> `ObjectArtifact`: `source::admit` / fragment replay not run (`ff:3592-3848`, `ff:24426-24432`) [opened].
3. Physical evidence: never derived (`NA/fragment_object_publication.rs:161`; `evidence.rs:78-80`) [opened].
4. ELF static and PE file bytes outside the entry header/shim: not re-read; `final_text_bytes` is the writer's own buffer
   (`ELF/static_executable.rs:242`, `PE/lib.rs:344`) [opened].
5. Dynamic-ELF final admission: derive-only (`ELF/file_assembly.rs:184-211`); stage replays use the producer's builder [opened].
6. `NativeArtifact` sealing: no `validate()` (`NA/native_artifact.rs:471-519`) [opened].
7. Publication: presence of validation fields and a digest, plus a disk round trip (`NA/publication.rs:277-297`;
   `file_publication.rs:27-95`) [opened].

### (b) Checked independently, per hosted target (default compile unless marked)
| Target | Container | Whole-file decode | Entry decode of file bytes | Full replay | Notes |
| --- | --- | --- | --- | --- | --- |
| `linux_x86_64` | ELF static, or dynamic with imports | none (dynamic: per-stage same-builder replay) | hosted receiver: `e_entry` only (`EM/entries.rs:3308-3319`); free Unit: `e_entry`+PT_LOAD+shim | `--both` only, static only | dynamic ELF never replayed outside tests |
| `linux_arm64` | same as above, EM 183 | same | same | same | same |
| `macos_arm64` | Mach-O, ad-hoc signed | yes, twice: in writer (`MO/lib.rs` ~876-897) and in image-emission (`EM/image.rs:842-864`); both are the writer crate's decoder | LC_MAIN (`EM/entries.rs:4162-...`) | `--both` only | one `Aapcs64` policy shared with Linux [record] |
| `windows_x86_64` | PE32+ | none | MZ/PE/optional magic, AddressOfEntryPoint, `.text` header, shim bytes (`EM/entries.rs:4008-4093`) | `--both` only | same `NativeTarget` as UEFI (`target_profile.rs:695-711`) |
"Independent" here means a second hand-written decoder in the same repository; no check uses a separate tool,
separate derivation from source, or the platform loader, other than actually executing the file.

### (c) What a wrong executable must get past to ship
- `omega <root>` (default): source checking and Omega 00 admission; then only `validate_terminal_image` — region and
  relocation-envelope consistency against the writer's reported bytes, an entry-point decode of the file, receiver shim
  instruction decode, and for Mach-O a full container decode — then publication presence checks and a disk byte round trip.
  A miscompiled instruction stream that is consistent with its own plan and relocations passes all of these.
- `omega run` (native only): the same, plus the program's own run; no comparison.
- `omega run --both` (host only, direct image only): additionally `NativeArtifact::validate`, whose failure is reported as
  `DECLINED` with the native exit code; then an exit-status/trap/entry-RAM comparison, not stdout.
- Corpus / release gates (harness only): authored exit and stdout oracles per fixture on each host
  (`corpus_runner.rs:44-73`; `wiki/drafts/reference/rust_compiler_completion.md:40`, `:66-79`).

---

## Could not establish
- Whether `validate_composition` (composed artifacts skip the full fragment replay, `ff:24407-24421`) re-validates the root.
- Whether the ISA selected-form validators run anywhere outside the replay chain.
- How the corpus harness classifies a `native-code-layout` `MissingProvision` (it also passes the native image,
  `corpus_runner.rs:951-975`).
- What an unlocked (`accepted: None`) project with dependencies is allowed to do at compile time.
- Whether a native validation failure under `--both` has ever occurred in recorded runs (nothing was run).
- Provider-planning/selection internals beyond the cited call sites are records only.
