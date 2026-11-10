# slice_0119

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0913 | `omega-rust/omega/packages/package-compilation/src/build_execution/filesystem_scope.rs:630-632` | CLOSED | `Ok(None)` without a sponsor is only reached when no declared required outputs and no completed output obligations; Terminal filesystem execution and obligation settlement block the accepting branch otherwise. |
| A0914 | `omega-rust/omega/packages/package-compilation/src/package_compilation.rs:370` | REACHABLE | `from_parts` can build inputs with an empty handoff map and skip `with_complete_dependency_build_handoffs`, while early source assembly uses that path before package-manager review attaches bundles. |
| A0915 | `omega-rust/omega/packages/package-compilation/src/package_compilation/provider_selections.rs:39-41` | REACHABLE | Under package-sliced sources, a native declaration with a dependency alias is treated as reached even when its source path is not in the selected file set. |
| A0916 | `omega-rust/omega/packages/package-compilation/src/source/source_imports/entry_contract_seed.rs:77` | REACHABLE | A target whose `ProgramEntry` contract resolves to `Unavailable` is skipped with no diagnostic, so assembly can proceed without that contract source. |
| A0917 | `omega-rust/omega/packages/package-compilation/src/source/source_snapshot.rs:352` | REACHABLE | `capture_scoped_source_input` silently skips a required member that cannot be relativized to the capture root instead of rejecting the capture. |
| A0918 | `omega-rust/omega/packages/package-compilation/src/reuse/review_cache_digests.rs:142` | REACHABLE | Review-cache digest hashes directory/file metadata for an entry but omits file bytes when `input.file(...)` is missing, so divergent retained content can share a digest. |
| A0919 | `omega-rust/omega/packages/package-compilation/src/source/toolchain_library.rs:126` | REACHABLE | Toolchain library identity digests only `.omg` paths while import resolution also admits `.omega` (`paths.rs:146-153`). |
| A0920 | `omega-rust/omega/packages/package-manager/src/resolution/compiler_input.rs:196-198` | CLOSED | A reachable key without a graph node adds no dependency edges and does not admit extra package bindings beyond filtered custodies. |

## A0913

Digest: area_06, `PC/build_execution/filesystem_scope.rs:630-632`.

**Cited site (read).** `staged_output_tree` (`omega-rust/omega/packages/package-compilation/src/build_execution/filesystem_scope.rs:623-634`): when `filesystem_reachable` is true and `self.sponsor` is `None`, returns `Ok(None)` instead of capturing the staging tree.

**Enclosing function (read).** Full `staged_output_tree` and callers into `verify_required_outputs` (`:568-598`).

**Callers (search).** `rg '\.staged_output_tree\(' omega-rust`: 8 hits; the only call that passes `filesystem_reachable` is `CapturedBuildObservations::settle` at `omega-rust/omega/packages/package-compilation/src/build_execution.rs:137` (read). Production settlement also runs from `omega-rust/omega/build-system/build-settings/src/activation/preparation.rs:168-169` (read).

**Upward trace (read).** `settle` (`build_execution.rs:131-163`) always runs `scope.verify_required_outputs(staged_output_tree.as_ref(), machine_name)` (`:143`). With `staged_output_tree == None` and a non-empty `required_outputs`, `verify_required_outputs` errors (`filesystem_scope.rs:576-579`). It then runs `output_custody::settle_build_output_obligations` with the same optional tree (`build_execution.rs:144-148`; `output_custody.rs:68-78` errors when a completed obligation has no sealed tree entry). Any build that executes filesystem calls through Terminal Psi must pass `begin_filesystem_runtime`, which returns an error when the sponsor does not own the private staging root (`filesystem_scope/runtime.rs:75-81`, read). Package-aware preparation auto-provisions a private sponsor when snapshot/retained custody applies (`filesystem_scope/preparation.rs:94-122`, read).

**Input that reaches the branch.** A filesystem-reachable machine with no sponsor, no declared snapshot `required_outputs`, and no completed build output obligations—typically a build that never issued staged-output custody.

**Verdict: CLOSED.** The accepting `Ok(None)` branch is harmless relative to staged-output claims because paths that assert output custody hit `verify_required_outputs` or `settle_build_output_obligations`, and live Terminal filesystem use requires a sponsor.

## A0914

Digest: area_06, `PC/package_compilation.rs:370`.

**Cited site (read).** `PackageCompilationInputs::from_parts` (`package_compilation.rs:347-378`): `with_complete_dependency_build_handoffs` runs only when `!dependency_build_handoffs.is_empty()` (`:370-374`).

**Enclosing function (read).** Full `from_parts`; `with_complete_dependency_build_handoffs` in `package_compilation/build_handoffs.rs:286-328` (read) enforces `MissingBuildHandoff` when the map is populated but incomplete.

**Callers (search).** `rg 'PackageCompilationInputs::from_parts'`: 3 production Rust call sites—`source/source_assembly.rs:387` (read), `package_compilation.rs:387` (`with_compilation_purpose`, read), `omega-rust/omega/compilation/products/src/compile_request.rs:448` (read).

**Upward trace (read).** `prepare_from_sources` builds `PackageCompilationInputs::from_parts(sources, Default::default())` (`source_assembly.rs:386-394`), so the empty-map skip is on the early source-assembly path before handoffs exist. Package-manager review compilation closes the gap by calling `with_complete_dependency_build_handoffs` with assembled bundles (`PM/review/candidate/passes/occurrences.rs:468-469`, read).

**Input that reaches the branch.** A workspace whose dependency graph requires Product/Build handoffs, but inputs are constructed through `from_parts` with `PackageCompilationTargetInputs::default()` (empty `dependency_build_handoffs`) during `prepare_from_sources`.

**Verdict: REACHABLE.** Source assembly can proceed under inputs that never ran the missing-handoff completeness check, even though a later review pass enforces handoffs when inputs are rebuilt there.

## A0915

Digest: area_06, `PC/package_compilation/provider_selections.rs:39-41`.

**Cited site (read).** In `product_selection_source_seeds` (`provider_selections.rs:20-62`), for base-stratum native declarations:

```39:45:omega-rust/omega/packages/package-compilation/src/package_compilation/provider_selections.rs
 let reached = !package_sliced
 || declaration.dependency_alias.is_some()
 || loaded_sources.is_none_or(|sources| {
 sources
 .selected_files()
 .any(|file| file.path == anchor.source().path)
 });
```

**Enclosing function (read).** Full `product_selection_source_seeds`.

**Callers (search).** `rg 'product_selection_source_seeds' omega-rust/omega/packages/package-compilation`: definition plus `source/source_assembly.rs:514` (read).

**Upward trace (read).** `source_assembly.rs:510-528` seeds any returned path not already selected, driving further Product import phases.

**Input that reaches the branch.** Package-sliced `SourceMap` (`package_sliced == true`) and a retained native product declaration whose physical source path is not among `selected_files`, but `declaration.dependency_alias` is `Some(_)`.

**Verdict: REACHABLE.** The compiler can load a dependency-aliased provider source seed that the slice did not include, bypassing the “path must appear in selected_files” gate.

## A0916

Digest: area_06, `PC/source/source_imports/entry_contract_seed.rs:77`.

**Cited site (read).** `entry_contract_seeds_from_graphs` (`entry_contract_seed.rs:44-85`): `EntryContractSource::Unavailable => continue` (`:77`) skips seeding; `:72-74` similarly `continue`s when a package root is missing from graphs (read).

**Enclosing function (read).** Full `entry_contract_seeds_from_graphs`; `EntryContractSource::select_from_graphs` (`:107-139`, read) returns `Unavailable` on ambiguous suppliers without an accepted binding.

**Callers (search).** `entry_contract_seeds` from `source_assembly.rs:497` and `product_queries.rs:410` (read paths via grep).

**Upward trace (read).** `source_assembly.rs:496-508` adds seeds to imports without a separate diagnostic for profiles that were skipped.

**Input that reaches the branch.** A multi-target catalog profile whose `ProgramEntry` physical contract has multiple supplying packages (or none) and no accepted semantic binding, so `select_from_graphs` yields `Unavailable`.

**Verdict: REACHABLE.** Assembly continues without seeding that target’s entry-contract source and without reporting why.

## A0917

Digest: area_06, `PC/source/source_snapshot.rs:352`.

**Cited site (read).** `capture_scoped_source_input` (`source_snapshot.rs:335-367`): for each `required_members` path, if `relative` cannot be computed, `continue` (`:352`) instead of erroring.

**Enclosing function (read).** Full `capture_scoped_source_input`; contrast `require_source_members` (`:288-301`) which errors on omission when relative paths are valid.

**Callers (search).** `rg 'capture_scoped_source_input'`: `filesystem_scope/preparation.rs:184` (read), `lib.rs:81` re-export.

**Upward trace (read).** In `prepare_filesystem_scope`, the non-retained `BuildSnapshotCapture::Scoped` branch passes absolute selected file paths and `root_path` (`preparation.rs:184-191`), read). The retained branch instead canonicalizes and errors when a member lies outside the root (`:164-175`, read).

**Input that reaches the branch.** Scoped build snapshot capture where a listed required source member path does not canonicalize/strip to a path under `canonical_root` (for example, a path outside the capture root).

**Verdict: REACHABLE.** A required member can be dropped from verification silently on this caller path, so the scoped inventory check does not fail closed.

## A0918

Digest: area_06, `PC/reuse/review_cache_digests.rs:142`.

**Cited site (read).** `hash_captured_input` (`review_cache_digests.rs:127-151`): for file entries, metadata is always digested; file bytes are digested only inside `if let Some(file) = input.file(entry.relative_path())` (`:142-144`, read).

**Enclosing function (read).** Full `hash_captured_input`; used by `review_cache_inputs_digest` / snapshot digest functions in the same file (`:29`, `:70`, read).

**Callers (search).** `rg 'hash_captured_input'`: only `review_cache_digests.rs` (read).

**Upward trace (read).** Review-cache digests fingerprint compilation inputs for reuse; missing `input.file` leaves the content-identity hash out while the entry row still contributes structural fields (`:131-145`).

**Input that reaches the branch.** Two `CapturedBuildSourceInput` values that list the same relative file entry in the index, but one retains bytes and one does not (`input.file` returns `None`).

**Verdict: REACHABLE.** Divergent on-disk content can yield the same review-cache digest when retained bytes are absent.

## A0919

Digest: area_06, `PC/source/toolchain_library.rs:126` and `source_imports/paths.rs:151-153`.

**Cited site (read).** `library_digest` (`toolchain_library.rs:110-149`) collects only paths whose extension is `omg` (`:123-126`). `source_path_candidates` (`source_imports/paths.rs:139-154`, read) lists both `.omg` and `.omega` (and `mod.*` variants).

**Enclosing function (read).** Full `library_digest` and `collect`; `identity()` calls it at `:106-108`.

**Callers (search).** `rg 'toolchain_library::identity|library_digest' omega-rust/omega/packages/package-compilation`: internal to `toolchain_library.rs` (read grep listing).

**Upward trace (read).** Toolchain identity is content-addressed over the collected file list; import resolution can load modules via `.omega` candidates independent of that list.

**Input that reaches the branch.** Toolchain library tree differing only in `.omega` sources (no `.omg` change) while importers resolve those `.omega` paths.

**Verdict: REACHABLE.** Reported toolchain library identity can stay unchanged while importable `.omega` content changes.

## A0920

Digest: area_06, `PM/resolution/compiler_input.rs:196-198`.

**Cited site (read).** `reachable_package_keys` (`compiler_input.rs:186-208`): after `reachable.insert(package.clone())`, `let Some(node) = closure.graph().package(&package) else { continue; }` (`:196-198`, read).

**Enclosing function (read).** Full `reachable_package_keys`; `PackageCompilationScope::new` stores the set (`:32-40`, read) and `compilation_inputs_reusing` filters custodies and dependency edges by it (`:55-120`, read).

**Callers (search).** `rg 'reachable_package_keys'`: definition and single use at `compiler_input.rs:39` (count 2, read).

**Upward trace (read).** Keys enter `pending` only from `node.dependencies()` when a node exists; a missing node stops expansion but leaves the key in `reachable`. Package bindings still come from `closure.custodies()` filtered by `reachable.contains` (`:62-66`); dependency rows come from existing graph packages (`:98-119`), not from orphan keys alone.

**Input that reaches the branch.** BFS reaches a `PackageKey` present on a dependency edge whose graph node is absent (inconsistent closure), so the key is marked reachable without traversing further edges.

**Verdict: CLOSED.** Orphan reachable keys do not admit extra packages or dependency bindings; they only shrink traversal. Root missing from the graph is already rejected in `compilation_inputs_reusing` (`:136-138`, read).

## What I did not read

- Full construction of `ResolvedPackageSourceClosure` invariants (whether a dependency target can lack a graph node in valid inputs).
- `omega-rust/omega/compilation/products/src/compile_request.rs` policy path that fills `package_target_inputs` (beyond the `from_parts` call at `:448`).
- `PM/resolution/graph/**` beyond `compiler_input.rs` and the cited review pass.
- Test-only modules under `filesystem_scope/runtime/*_tests.rs` beyond cited production paths.
- Commit `457adf4a11f` tree; line numbers are from the current checkout (re-located by symbol where read).

SLICE COMPLETE
