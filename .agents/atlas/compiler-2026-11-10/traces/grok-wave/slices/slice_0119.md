slice_0119

| entry id | file:line | verdict | one-line reason |
| -------- | --------- | ------- | ---------------- |
| A0913 | `PC/build_execution/filesystem_scope.rs:630-632`; `filesystem_scope/runtime.rs:75-81` | CLOSED | Terminal runtime and settle-time custody checks block reachable `Ok(None)` when outputs matter; sponsor auto-provisioned on package snapshot builds. |
| A0914 | `PC/package_compilation.rs:370` | CLOSED | Empty handoff map skip is for intermediate `from_parts`; review and orchestration always call `with_complete_dependency_build_handoffs`. |
| A0915 | `PC/package_compilation/provider_selections.rs:39-41` | CLOSED | `dependency_alias` branch is deliberate dependency-owned seed admission behind `admit_retained_product_declaration`. |
| A0916 | `PC/source/source_imports/entry_contract_seed.rs:77` | CLOSED | `Unavailable` skips optional seeding; executable production rejects missing ProgramEntry later. |
| A0917 | `PC/source/source_snapshot.rs:352`; `filesystem_scope/preparation.rs:184` | REACHABLE | Required member outside canonical root is skipped (no error) on live `capture_scoped_source_input`. |
| A0918 | `PC/reuse/review_cache_digests.rs:142` | CLOSED | Valid `CapturedBuildSourceInput` cannot have file index rows without retained bytes (`from_capture_rows`). |
| A0919 | `PC/source/toolchain_library.rs:126`; `source_imports/paths.rs:151-153` | REACHABLE | Toolchain identity digest omits `.omega` while imports admit `.omega` paths. |
| A0920 | `PM/resolution/compiler_input.rs:196-198` | CLOSED | Resolved closure validates every dependency target has a graph node before this runs. |

### A0913

1. Read `BuildMachineFilesystemScope::staged_output_tree` in full (`filesystem_scope.rs:623-634`, read).
2. Search `rg 'staged_output_tree\(filesystem' omega-rust` → **1 hit** (`build_execution.rs:137`, read). Broader `rg 'staged_output_tree\(' omega-rust` → **13 file hits** (mostly `Option` accessors and codecs; read call site only).
3. Read caller `CapturedBuildObservations::settle` (`build_execution.rs:131-164`, read) and `BuildFilesystemRuntime::begin_filesystem_runtime` (`filesystem_scope/runtime.rs:68-97`, read). Read `prepare_filesystem_scope` sponsor provisioning (`filesystem_scope/preparation.rs:90-123`, read) and `activation/preparation.rs` `complete` (`build-settings/.../preparation.rs:157-173`, read).
4. Input on accepting branch: `filesystem_reachable == true`, `self.sponsor == None`, call from `settle` after a build observation session.
5. **Verdict: CLOSED.** `runtime.rs:75-81` (read) refuses Terminal build runtime without a sponsor owning the output root, so reachable filesystem builds with output effects cannot start on the Terminal path. Package builds that retain custody auto-enter snapshot mode and get a private sponsor (`preparation.rs:94-122`, read). When `staged_output_tree` is `Ok(None)` with reachable FS, `verify_required_outputs` (`filesystem_scope.rs:576-579`), `output_custody::settle_build_output_obligations` (`output_custody.rs:75-78`), and `select_generated_sources` (`output_custody.rs:107-110`) error if required outputs or generated handoffs need custody. Entry’s cited `runtime.rs:75-81` matches at read; line numbers align with commit note.

### A0914

1. Read `PackageCompilationInputs::from_parts` in full (`package_compilation.rs:347-378`, read) and `with_complete_dependency_build_handoffs` (`build_handoffs.rs:286-328`, read).
2. Search `rg 'PackageCompilationInputs::from_parts' omega-rust` → **3 hits** (`package_compilation.rs`, `source_assembly.rs:387`, `compile_request.rs:448`; all read at call sites).
3. Read upward: review compile path (`occurrences.rs:413-475`, read) calls `with_complete_dependency_build_handoffs` directly; orchestration (`build-system/orchestration/.../build_execution.rs:400`, inferred from grep, not full file); `source_assembly.rs:387` uses `Default::default()` target during early source prep (read).
4. Input on accepting branch: `dependency_build_handoffs` map empty when passed into `from_parts`.
5. **Verdict: CLOSED.** The empty-map guard at `370` only skips validation when there are no handoffs to check; review acceptance always supplies complete bundles via `occurrences.rs:468-475` (read). `compile_request.rs` runs `with_complete` when the map is non-empty (`370-373`). Early `prepare_from_sources` is not compilation acceptance.

### A0915

1. Read `product_selection_source_seeds` in full (`provider_selections.rs:20-63`, read) and `admit_retained_product_declaration` (`provider_selections.rs:126-135`, read).
2. Search `rg 'product_selection_source_seeds' omega-rust` → **2 hits** (`provider_selections.rs`, `source_assembly.rs:514`; read caller loop).
3. Callers require admitted handoff bundles before seeding (read `admit_provider_activation` at `provider_selections.rs:84-123`).
4. Input: `loaded_sources` present with package-sliced files, `declaration.dependency_alias.is_some()`, Base stratum native declaration, anchor path not among selected files.
5. **Verdict: CLOSED.** Entry description matches read at `39-41`. Keeping the seed without a local `selected_files` hit is gated on `dependency_alias` for dependency-sourced declarations; `admit_retained_product_declaration` still re-validates source (`126-135`).

### A0916

1. Read `entry_contract_seeds_from_graphs` in full (`entry_contract_seed.rs:44-86`, read) and `EntryContractSource::select_from_graphs` (`107-140`, read).
2. Search `rg 'entry_contract_seeds_from_graphs' omega-rust` → **2 hits** (`entry_contract_seed.rs`, `product_queries.rs:410`; read).
3. Read terminal production gate (`terminal_artifact.rs:486-491`, read).
4. Input: `EntryContractSource::Unavailable` for a `TargetProfile` program-entry slot.
5. **Verdict: CLOSED.** `Unavailable => continue` at `77` skips seeding without diagnostic; ambiguous/missing suppliers become `Unavailable` at `136-138`. Executable Terminal production that needs a bound ProgramEntry fails at `terminal_artifact.rs:489-490` (read). Inspection/product-query paths that intentionally omit ProgramEntry are not mis-accepting executables.

### A0917

1. Read `capture_scoped_source_input` in full (`source_snapshot.rs:335-367`, read) and caller in `preparation.rs:184-191` (read; retained branch `170-175` read for contrast).
2. Search `rg 'capture_scoped_source_input' omega-rust` → **3 hits** (`source_snapshot.rs`, `lib.rs` re-export, `preparation.rs:184`).
3. Did not read full `retained_scoped_source_input` loop internals beyond `require_source_members` (`288-301`, read).
4. Input: `required_members` entry whose path does not strip to a relative path under `canonical_root` (`let Some(relative) = relative else { continue }` at `352`).
5. **Verdict: REACHABLE.** A required source path outside the canonical captured root is silently skipped instead of rejected; live scoped capture can complete without verifying that member. Retained scoped path in `preparation.rs` errors on outside-root members (`170-175`). Smallest failure shape: scoped build snapshot with a listed required file that lies outside the package canonical root—the capture omits the inclusion check and may proceed.

### A0918

1. Read `hash_captured_input` (`review_cache_digests.rs:127-151`, read) and `CapturedBuildSourceInput::from_capture_rows` (`build-output/.../captured_source/mod.rs:135-188`, read).
2. Search `rg 'hash_captured_input' omega-rust` → **1 definition + 1 call** in `review_cache_digests.rs` (read).
3. Did not read full review-cache hit validation path beyond digest construction.
4. Input: `CapturedSourceEntryKind::File` in `input.entries()` where `input.file(relative_path)` is `None` (defensive branch at `142-144`).
5. **Verdict: CLOSED.** `from_capture_rows` rejects any metadata file row lacking a retained entry (`176-187`, read), so production captures admitted into review-cache digests should not hit the `if let Some(file)` miss path; missing bytes cannot be silently hashed as present.

### A0919

1. Read `library_digest` / `collect` (`toolchain_library.rs:110-136`, read) and `source_path_candidates` (`source_imports/paths.rs:139-154`, read).
2. Search `rg 'library_digest|toolchain_library::' omega-rust/omega/packages/package-compilation` → used from `paths.rs:120` and `toolchain_library.rs` identity checks (read `90-102`).
3. Did not read every bundled std import path.
4. Input: toolchain tree where identity is checked via `library_digest`, but authored modules use `.omega` extensions listed by `source_path_candidates`.
5. **Verdict: REACHABLE.** Digest collection keeps only `.omg` (`126-127`, read) while imports try `.omega` candidates (`146-153`, read). A toolchain tree could match digest expectations on `.omg` content yet ship additional or altered `.omega` sources that participate in import resolution without changing the digest—integrity check would not reject it.

### A0920

1. Read `reachable_package_keys` (`compiler_input.rs:186-208`, read) and `PackageCompilationScope::compilation_inputs_reusing` (`55-159`, read).
2. Search `rg 'reachable_package_keys' omega-rust` → **1 hit** (definition only).
3. Read closure graph validation (`resolution/graph/mod.rs:206-215`, read) and closure finish (`resolution.rs:240-256`, read).
4. Input: package key popped from `pending`, inserted into `reachable`, then `closure.graph().package(&package)` is `None`.
5. **Verdict: CLOSED.** Valid `ResolvedPackageSourceClosure` rejects missing dependency targets at graph construction (`mod.rs:207-215`, read). `pending` only receives targets from existing node dependencies (`201-205`, read). Root missing-node case still lands in `reachable` but `compilation_inputs_reusing` errors (`136-139`, read). The `continue` at `196-198` is unreachable on accepted closures, not program acceptance.

### What I did not read

- Full `BuildMachineFilesystemScope` establishment and `ensure_write_roots` beyond snippets.
- Complete `output_custody.rs` and `build_record_codec.rs`.
- Full `ResolvedPackageSourceClosure` resolution state machine.
- Review-cache hit re-verification after digest compare.
- Entire `admit_retained_product_source` dependency-alias branch below line 159.
- Test modules in `filesystem_scope/runtime.rs` beyond cited admission gate.

SLICE COMPLETE
