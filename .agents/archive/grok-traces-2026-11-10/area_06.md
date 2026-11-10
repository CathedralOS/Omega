# Area 6 open accepts, traced

Citations were checked in the checkout at `8febfc42316`, which contains `457adf4a11f`. `git diff --stat 457adf4a11f HEAD` for `omega-rust/omega/interpreter/terminal-runtime` and `omega-rust/omega/packages` printed no files, so the cited lines still sit where the digest says. Each line below was read unless the sentence says it was inferred.

| # | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| 1 | `TR/clock.rs:405-422` | CLOSED | A poll is accepted only when the count's low 32 bits are zero, and that argument is a scalar, not storage. |
| 2 | `TR/filesystem.rs:437-439` | REACHABLE | The owning service is skipped, so a non-filesystem owner is admitted. |
| 3 | `BD/dependencies/mod.rs:162` | CLOSED | The only production caller then rejects every dependency-named call expression. |
| 4 | `BO/captured_source/mod.rs:68-75` | CLOSED | Every caller builds the entry with `file` or `symlink`, which hash the bytes. |
| 5 | `BO/staged_output_tree.rs:136-176` | REACHABLE | A self-consistent staged tree with a missing parent or an escaping symlink is decoded. |
| 6 | `PC/build_execution/build_admission.rs:78-81` | CLOSED | The reach plan inserts a row for every machine in the same program. |
| 7 | `PC/build_execution/build_record_codec.rs:655,417` | REACHABLE | A cache hit keeps the decoded request and observation without a new admission. |
| 8 | `PC/build_execution/filesystem_scope.rs:91-120` | REACHABLE | A path that cannot be canonicalized is compared by spelling. |
| 9 | `PC/build_execution/filesystem_scope.rs:630-632` | UNDECIDED | `Ok(None)` is reachable when the call list is empty; whether that list can hide real filesystem reach was not read. |
| 10 | `PC/package_compilation.rs:370-374` | REACHABLE | An empty handoff map skips the missing-handoff check. |
| 11 | `PC/package_compilation/provider_selections.rs:39-41` | UNDECIDED | The alias forces the seed to be kept; the admission of that anchor was not read. |
| 12 | `PC/source/source_imports/entry_contract_seed.rs:77` | REACHABLE | An ambiguous or unmatched `ProgramEntry` supplier is skipped with no diagnostic. |
| 13 | `PC/source/source_snapshot.rs:352` | REACHABLE | A required member outside the root is skipped on the non-retained capture path. |
| 14 | `PC/reuse/review_cache_digests.rs:142-144` | CLOSED | `file` looks up the same map `entries` just yielded, so a file row cannot be missing. |
| 15 | `PC/source/toolchain_library.rs:123-126` | REACHABLE | The compiler-input identity hashes only `.omg` files, while bundled import lookup also accepts `.omega`. |
| 16 | `PM/resolution/compiler_input.rs:196-198` | UNDECIDED | A key with no graph node stays in the reachable set; the closure invariant that would forbid that key was not read. |
| 17 | `PM/resolution/graph/accepted.rs:60-66` | UNDECIDED | The catalog search does not filter on purpose; whether ordinals collide across purposes was not read. |
| 18 | `PM/resolution/graph/mod.rs:60-66` and `PM/resolution/source/custody.rs:101` | CLOSED | Every production caller builds a same-family pair before the release-only assert. |
| 19 | `PM/resolution/graph/subject/catalog.rs:31-35` | CLOSED | The only observation constructor rejects unequal selection and graph lengths. |
| 20 | `PM/review/candidate/review_cache.rs:197-200` | REACHABLE | The trailer is an unkeyed SHA-256, and the decoded handoff is not recomputed. |
| 21 | `PM/review/candidate/error.rs:184` and `PM/review/compare/assembly.rs:73-75` | UNDECIDED | The name slice cannot panic; the occurrence slice was not tied to the roster length. |
| 22 | `RX/resolver_execution/path_custody.rs:77-82` | REACHABLE | A root that cannot be canonicalized is tested only as a spelling prefix. |
| 23 | `SA/git/cache/custody.rs:187-193` | REACHABLE | Off Unix a multiply-linked Git cache file is accepted. |
| 24 | `SA/git/cache/retry.rs:41` | UNDECIDED | Any existing `source.identity` node counts as complete; the later repository open was not read. |
| 25 | `SA/identity/resolution.rs:77-87` | REACHABLE | Lineage agreement is only the family, so two different Git repositories agree. |

Review-evidence entries in the same section were not opened. They are listed under "Entries not reached".

## 1. Poll pointer

`ClockImport::handle_effect_result` (`clock.rs:352-436`, read) binds the first poll integer to `_` and rejects the call when `count & u32::MAX != 0` or the timeout is negative (`clock.rs:422-426`, read). `admission::validate` admits poll only as three scalars, the first a wide integer, with an empty structural-parameter list (`clock/admission.rs:133-140`, read). `handle_effect_with_buffers` sends a storage-free poll into that function (`clock.rs:312-343`, read). Search `ClockOperation::Poll` in `omega-rust`: 1 hit, the match arm. The assignment `ClockImplementation::Poll => ClockOperation::Poll` was read at `clock/admission.rs:150`.

A nonzero count, which is the case that would name descriptors, is rejected before the pointer is ignored. The ignored value is never borrowed storage. Digest wording that a nonzero pointer and a zero count sleep is what the code does; that is the zero-count case, and it does not consult the integer.

## 2. Filesystem owning service

`admit_services` (`filesystem.rs:427-462`, read) skips a reached service equal to `owning_service` and requires every other reached service's identity text to be one of six `toolchain::Filesystem…` strings. The local `supplied` is that allowlist. It is not true for any host-namespace delegation. The digest is wrong on that point. `FilesystemContext.supplied` is the host-namespace flag (`filesystem.rs:167-169`, read) and is required earlier (`filesystem.rs:377-381`, `filesystem/linux.rs:56-58`, `filesystem/windows.rs:92-95`, all read).

Search `admit_services` in `omega-rust`: 3 definition/call sites, all read: Darwin `filesystem.rs:383`, Linux `filesystem/linux.rs:59` (both `LinuxFilesystemWrite::admit` and `LinuxFilesystemImport::admit`, the second read at `linux.rs:321`), and Windows `filesystem/windows.rs:93`. `native_provider.rs:186-207` (read) skips `service_reach::admit_additional_services` when a filesystem call is already present. `service_reach.rs` (read through line 85) also filters out the owning service and does not require the filesystem allowlist.

Smallest program: a boundary machine bound to a known filesystem entry, with host filesystem delegation, whose owning service is not one of those six identities, and whose other reached services are among those six or absent. `admit_services` returns success.

## 3. Nested dependency calls

`project_dependency_rows` (`dependencies/mod.rs:156-194`, read) keeps only direct calls in the build entry state. Search `project_dependency_rows` in `omega-rust`: the definition, its test, the `lib.rs` re-export, and one production call at `PM/.../read/projection/calls.rs:31` (read). That function is called only from `project_editable_dependencies` (`projection/mod.rs:54`, read), which then calls `reject_unprojected_dependency_syntax` (`projection/mod.rs:55-60`, read) before `Ok`.

The statement walk (`policy.rs:158-189`, read) does not recurse into `nested_machines`. The expression walk does (`policy.rs:120-151`, read). Statement calls are parsed as `ExpressionNode::Call` and then wrapped (`statements_and_assembly.rs:2364-2366` and `expression_handle_to_statement_call` at `2512-2531`, read). Argument handles are the same expression handles (`bodies_and_transitions.rs:1357-1378`, read). A dependency-named call whose arguments are not the accepted row's handles returns `UnsupportedDependencyShape`. `Item` has no separate function body (`declarations.rs:611-625`, read). The only `StatementNode::Call` inserted without an expression is the asm intrinsic helper (`statements_and_assembly.rs:1583-1589`, read), and those names are not dependency names.

## 4. Captured digest

`matches_row_kind` (`captured_source/mod.rs:64-83`, read) compares kind, executable bit, and length, not `digest`. The variant fields are public. `file` and `symlink` (`:40-55`, read) set `digest` from `content_identity` of those bytes. Search `CapturedSourceEntry::` in `omega-rust`: every construction is `Directory`, `file(…)`, or `symlink(…)`. The production ones are `source_snapshot.rs:147-152` and `capture_physical_source_row` (`source_snapshot.rs:723-751`, read; the symlink arm continues at `:755` and uses the same `..` ignore of a constructor-built entry). `from_capture_rows` callers are that snapshot module, its tests, and a test in `filesystem_scope/runtime.rs:831`. No caller builds a struct literal with a foreign digest.

## 5. Staged output parts

`from_retained_parts` (`staged_output_tree.rs:130-180`, read) checks a canonical path, duplicate paths, and commitment equality. `commitment_for_retained_entries` (`:466-512`, read) hashes path, kind, file bytes, and the symlink target spelling. It does not require a parent directory entry or a target that stays inside the tree. Search `BuildStagedOutputTree::from_retained_parts` in `omega-rust`: the definition and one non-test caller, `recover_staged_output_tree` (`build_record_codec.rs:202-233`, read), which `recover_observation_summary` stores on the summary (`:494-515`, read).

`validate_retained_tree` (`materialization/mod.rs:74-160`, read) does check missing parents and symlink targets, and it is `#[cfg(test)]`. Production `materialize_retained_tree` (`:28-72`, read) creates the symlink from the raw target (`:315-328`, read). The only `materialize_into` caller is `completed_outputs.rs:106`, after `select_files` (`completed_outputs.rs:33-36`, read), which requires selected paths to be non-executable files and their ancestors to be directories (`staged_output_tree.rs:234-291`, read) and drops symlinks. That later selection does not run on the decode itself.

Smallest record: a build observation whose staged-output commitment matches entries that omit a parent directory, or whose symlink target is absolute or climbs with `..`. Decode returns the tree.

## 6. Missing reach row

`admit_build_machine` (`build_admission.rs:78-81`, read) uses `unwrap_or(&[])` when `for_machine` returns `None` (`flow-effects/src/lib.rs:516-522`, read). The only production caller is `build-settings/.../admission/program.rs:98-107` (read): the plan comes from `validation::infer_service_reaches(typed, &operational_plan)` and the machine symbol is taken from that same `typed` program. `infer_service_reaches_uncached` pushes one work row per `program.machines()` (`effect_inference.rs:2342-2360`, read) with `symbol: machine.symbol` (`:2820-2821`, read) and appends each of those machines onto `plan.root_machines` (`:2527-2682`, read). The `continue` at `:2529` runs only when the symbol is absent from that index. A machine of this program therefore has a row, and the empty fallback is not taken. An actually empty reach list is a row that was computed, not a missing row.

## 7. Decoded request and observation

`recover_restricted_build_request` (`build_record_codec.rs:655-709`, read) and `recover_observation_summary` (`:417-517`, read) rebuild values with `from_retained` and structural tag checks. They do not call build admission. The only caller is `review_cache/record_codec.rs:880-885` (read). `decode_record` checks an unkeyed content hash (`:848-858`, read) and that the handoff's observation digest equals the summary identity (`:889-899`, read). `into_review_and_facts` (`review_cache.rs:95-128`, read) checks package identity, name, lineage, and resolution, recovers policy bytes, and copies the request and observation through.

`compiled_review_key_with_identity` (`review_cache/key.rs:48-73`, read) binds package, purpose, target, profile, entry path, compiler identity, inputs digest, and snapshot digest. It does not bind the request or observation bytes. The lookup is `passes/occurrences.rs:557-569` (read).

Smallest record: a compiled-review cache file whose name is the live key, whose trailer is the SHA-256 of its payload, whose lineage and resolution match the live package, and whose restricted-build request grants a path the live admission would refuse. The cached review is reused.

## 8. Overlap key

`overlap_key` (`filesystem_scope.rs:85-125`, read) returns the unfolded absolute path when a `..` cannot be popped (`:99-101`), and uses the lexical base when `canonicalize` fails (`:120`). Call sites in this file, all read: `for_root` `:159`, `resolved_write_fence` `:128`, `admit_write_roots` `:470` and `:477`, and `ensure_established_write_root` `:550`. `verify_materialized_write_root` (`:451-465`, read) canonicalizes the build directory and fails if that call fails, then asks whether `resolved_write_fence(source_root)` starts with that path. If the source root itself cannot be canonicalized, the fence is the spelling from `overlap_key`.

Smallest case: a source root whose canonical form cannot be computed, and a build directory that is the same tree under a different spelling. The spelling comparison does not see the overlap, and admission continues.

## 9. Staged tree with no sponsor

`staged_output_tree` (`filesystem_scope.rs:623-634`, read) returns `Ok(None)` when the filesystem is reachable and `sponsor` is `None`. The production settle is `build-settings/.../activation/preparation.rs:168-169` (read). `verify_required_outputs` (`filesystem_scope.rs:568-578`, read) errors on `None` only when required outputs are non-empty. `begin_filesystem_runtime` (`runtime.rs:75-81`, read) errors when the sponsor does not own the build directory. Its only production caller is `TerminalBuildFilesystemHost::new` (`host/filesystem.rs:77-81`, read), and only when `calls` is non-empty. `TerminalBuildHost::new` always constructs that host (`activation.rs:157-163`, read) before a successful activation can settle.

So `Ok(None)` with a reachable filesystem happens when the produced call list is empty and no sponsor was installed. `prepare_filesystem_scope` (`preparation.rs:90-123`, read) creates a sponsor only in snapshot mode. A standalone scope can keep `sponsor: None`. What was not read: `calls_for`, and whether a machine whose reach plan says the filesystem is reachable can still have an empty call list. That reading would decide whether the empty list is a real filesystem build with its calls dropped, or a machine that never had filesystem calls.

## 10. Empty handoff map

`from_parts` (`package_compilation.rs:347-378`, read) calls `with_complete_dependency_build_handoffs` only when the map is non-empty. That function (`build_handoffs.rs:286-328`, read) reports `MissingBuildHandoff` for every required dependency instance that has no bundle. Search `PackageCompilationInputs::from_parts` in `omega-rust`: `source_assembly.rs:387` (read), which passes `Default::default()`, and `compile_request.rs:448` (read). `CompileRequest` starts with an empty target (`compile_request.rs:246`, read). `with_package_inputs` (`:348-352`, read) stores the target half of an already built value, so a value that skipped the check is joined again with the same empty map.

Smallest case: package source inputs that already name dependency instances, assembled with an empty handoff map. The missing-handoff error is not raised.

## 11. Alias keeps the seed

`product_selection_source_seeds` (`provider_selections.rs:20-49`, read) pushes a base-stratum anchor path when the package is not sliced, or the declaration has `dependency_alias`, or the path is among the selected files. The alias arm does not look at the slice. The one caller found is `source_assembly.rs:514` (search `product_selection_source_seeds`: that definition and that call). What was not read: `admit_retained_product_declaration`, which produces the anchor, and the code that sets `dependency_alias`. Those would decide whether the alias can name a file the slice had already excluded, or only a file that belongs to the dependency handoff.

## 12. Unavailable entry contract

`entry_contract_seeds_from_graphs` (`entry_contract_seed.rs:50-85`, read) hits `continue` on `EntryContractSource::Unavailable`. `select_from_graphs` (`:107-139`, read) returns `Unavailable` when there are several suppliers and the accepted package is missing or not among them, and when an accepted package is set but no supplier file exists. The module comment (`:1-10`, read) says several suppliers require an accepted binding. The function returns `Ok(seeds)` with that target omitted. Callers `source_assembly.rs:143` and `:497` (search hits, the call lines were not read past the search) only iterate the returned seeds. No diagnostic is built in the function that was read.

Smallest program: a product compile for a target whose `ProgramEntry` contract package is supplied by two packages and has no accepted binding that picks one of them. That target's contract source is left out, and seeding still succeeds.

## 13. Required member outside the root

`capture_scoped_source_input` (`source_snapshot.rs:335-367`, read) `continue`s when a required member is neither under the canonical root nor under the source root (`:345-352`). The non-retained caller is `filesystem_scope/preparation.rs:184-190` (read), which passes every selected file path plus the root path and does not pre-check containment. The retained caller at `:164-176` (read) does error when `strip_prefix` fails. That check is not on the path that reaches line 352.

Smallest case: a scoped capture with no retained complete input, and a selected source file whose path is outside the source root. The capture succeeds and that file is not required.

## 14. Missing file in the review digest

`hash_captured_input` (`review_cache_digests.rs:127-148`, read) hashes file-row length and the executable bit, and hashes content only inside `if let Some(file) = input.file(...)`. `entries` and `file` both read `self.entries` (`captured_source/mod.rs:214-232` and `:245-254`, read). A `File` row yielded by `entries` is the same map entry `file` looks up. The `None` arm does not run for a value `from_capture_rows` built. The digest's "missing retained file contributes nothing" does not occur for this type.

## 15. Toolchain `.omega` files

`library_digest` (`toolchain_library.rs:110-152`, read) records only files whose extension is `omg`. `compiler_input_identity` (`compilation/compiler/src/lib.rs:81-88`, read) mixes that digest into the accepted compiler-input identity. `resolve_source_path` (`source_imports/paths.rs:15-44`, read) roots `omega::language` imports at `bundled_omega_root`, which is `toolchain_library::root` (`paths.rs:119-121`, read), and tries `source_import_candidates`, which include `.omega` and `mod.omega` (`paths.rs:139-154`, read).

Smallest case: a bundled toolchain file whose name ends in `.omega` and which an `omega::language` import resolves. Changing that file does not change the compiler-input identity, and the import still succeeds.

## 16. Reachable key with no node

`reachable_package_keys` (`compiler_input.rs:186-208`, read) inserts a key, then `continue`s when `graph().package` is missing, so the key stays in the set and contributes no edges. The only caller is `PackageCompilationScope::new` (`:32-39`, read). `compilation_inputs_reusing` (`:62-65`, read) keeps custodies whose keys are in that set. What was not read: the constructor of `ResolvedPackageSourceClosure`, which would show whether a dependency target or the root can be in the custody list and absent from the graph. The digest's "I infer it cannot occur" was not confirmed.

## 17. First catalog match

`admit_locked_issued_source_request` (`accepted.rs:42-74`, read) takes the first graph package with the same lineage and navigation. If there is none, `find` (`:60-64`, read) takes the first catalog dependency with that lineage, navigation, and ordinal, and does not look at purpose. It then requires the live purpose to be `Build`, the observer resolution to match, and `request_matches` to succeed, and returns `Ok(())`. What was not read: `request_matches`, and the code that assigns `ordinal`. Those would decide whether two catalog rows that differ only by purpose can share an ordinal and let the first one's request text satisfy a Build check.

## 18. Release lineage assert

`from_validated_parts` (`graph/mod.rs:60-66`, read) and `PackageSourceCustody::from_resolved_parts` (`custody.rs:87-101`, read) check `matches_lineage` only with `debug_assert!`. `ResolvedSourceIdentity::new` (`graph/mod.rs:49-57`, read) checks it for real, but `matches_lineage` itself only compares family (entry 25).

Search `from_resolved_parts` in `omega-rust`: production calls at `acquisition.rs:558`, `git/recorded.rs:90`, and `local/recovery.rs:55`, plus tests. Those bodies were read. `capture_git` (`acquisition.rs:471-508`, read) and the workspace root (`workspace_acquisition.rs:263-276`, read) pair the lineage and the Git resolution of one acquired source. The member copy (`workspace_acquisition.rs:187-197`, read) copies the root lineage and the root resolution after `capture_git`. `recover_recorded_git_source` (`git/recorded.rs:39-48`, read) rejects a non-Git resolution and a request lineage that differs from the key before `from_resolved_parts`. `recover_cached_external_local_source` (`local/recovery.rs:29-30`, read) returns `Ok(None)` unless the key is `ExternalLocal`, then builds an external-local resolution. `from_validated_parts` callers (`reconcile/resolution.rs:245` and `catalog.rs:46`, both read) copy a key and resolution that already sit on such a custody or identity. A family mismatch is not passed by these paths. The digest is wrong that any unvalidated pair gets through: the callers that were found build the pair first. Two different Git repositories still agree, and that is entry 25, because the assert calls the family-only predicate.

## 19. Catalog zip

The zip is `catalog.rs:31-35` (read), with no length test at the loop. `WorkspaceMemberObservation::new` (`workspace_acquisition.rs:33-57`, read) returns an error when `demanded_sources().len() != dependencies.len()` and then stores those two vectors. Search `WorkspaceMemberObservation::new`: one production caller, `source_resolution/readiness.rs:661` (the call was not opened; the constructor was). `observations()` on selection evidence returns that slice or an empty slice (`selection.rs:17-22`, read). Unequal lengths do not reach the zip.

## 20. Review-cache trailer

`store` (`review_cache.rs:180-201`, read) appends `content_identity(payload)`. `lookup` (`:158-171`, read) accepts `decode_record`. `decode_record` (`record_codec.rs:848-929`, read) requires that hash, magic, version, an exhausted cursor, and the handoff observation link from entry 7. It does not recompute the handoff from the build. `into_review_and_facts` (`review_cache.rs:107-127`, read) recovers policy with `PackagePolicyBaseline::recover_canonical` (that function was not read) and then keeps the decoded handoff. The key (entry 7) does not cover the handoff body.

Smallest record: a hand-built cache file under the live key, with a trailer equal to the SHA-256 of the payload, live lineage and resolution, a policy encoding `recover_canonical` accepts, and a handoff whose other fields were not produced by this build but whose observation digest equals the decoded summary. The review is accepted. Whether a malformed policy is rejected depends on `recover_canonical`, which was not read; a well-formed forged policy is in the record that this path accepts.

## 21. Name slice and occurrence index

`render_package_diagnostics` (`error.rs:173-188`, read) slices `name[..name.len().min(256)]`. `PackageName::parse` (`declarations/identity.rs:11-14`, read) requires `ProjectName::parse`, and `is_snake_case` (`syntax_projection/mod.rs:440-458`, read) allows only ASCII lowercase, digits, and underscores. The index is on a character boundary. This is not a panic. The digest is wrong for this line.

`packages` in `review/compare/assembly.rs` (`:54-76`, read) slices `accepted.occurrences()` and `reviews` with counts from `roster.purposes(key)`. What was not read: `PackageOccurrenceRoster::derive` and `occurrences()`, which would show whether those counts stay inside the slices. Until that is read, the index stays open.

## 22. Executable outside a root that will not canonicalize

`require_outside_roots` (`path_custody.rs:68-92`, read) rejects when the path starts with the root spelling or with `root.canonicalize().ok()`. A failed canonicalize leaves only the spelling test. Callers, all in `resolver_execution.rs` and read: `open` checks the authored path (`:40`) and the canonical executable (`:41-49`), and `prepare` checks the stored canonical executable against the working root (`:79`). The executable's own canonicalize failure is an error (`:41-46`). The hole is a root whose canonicalize fails.

Smallest case: a resolver executable whose canonical path lies inside a package-controlled root, while that root's path cannot be canonicalized and the executable's spelling does not start with the root's spelling. `open` returns success.

## 23. Link count off Unix

`verify_git_regular_file_link_count` (`git/cache/custody.rs:171-193`, read) rejects `nlink() != 1` on Unix and returns `Ok(())` off Unix, with no comment on that stub. It is called from the regular-file check at `:168` (read). Search `nlink` in `custody/platform.rs`: no hits. The file header (`platform.rs:5-6`, read) and `verify_capability_cache_node_owner_and_mode` (`:78-85`, read) excuse owner and mode checks off Unix. They do not read a link count.

Smallest case, on a non-Unix host: a file in the native Git cache with more than one hard link. The link-count check succeeds.

## 24. `source.identity` node kind

`discard_incomplete_entry` (`git/cache/retry.rs:27-44`, read) returns `Ok(false)` when `symlink_metadata(GIT_CACHE_METADATA)` succeeds for any node. `GIT_CACHE_METADATA` is `"source.identity"` (`limits.rs:18`, from the search; that line was read). The only caller is `git/resolution/acquisition.rs:153-168` (read). `Ok(false)` leaves `cache_entry_existed` true and falls through to `VerifiedGitRepository::open` (`:167-169`). That open was not read. It would decide whether a symlink or directory named `source.identity` is then rejected, or kept as a finished cache entry.

## 25. Lineage family only

`matches_lineage` (`identity/resolution.rs:77-87`, read) is true when both sides are Git (GitHub, GitLab, or generic Git), both workspace, or both external-local. `SourceLineage` (`identity/git.rs:26-65`, read) carries owner, repository, host, and path inside the Git family, and `hash_canonical` (`:68-87`, read) hashes those fields. `matches_lineage` does not.

Search `matches_lineage(` in `omega-rust`: the definition, `ResolvedSourceIdentity::new` and `from_validated_parts` (`graph/mod.rs:53` and `:64`), `custody.rs:101`, and `validate_source_identity` (`subject/validation/source.rs:83-95`, read). The validator calls the same predicate and then returns success. `read_source` (`subject/text/source.rs:50-79`, read) builds the identity with `new` and reports a lineage disagreement only when `new` fails.

Smallest record: package-lock text whose key is one Git repository and whose resolution is a Git commit and tree of a different Git repository. `new` returns the identity, and the text reader accepts it.

## Counts

- REACHABLE: 12 (entries 2, 5, 7, 8, 10, 12, 13, 15, 20, 22, 23, 25)
- CLOSED: 7 (entries 1, 3, 4, 6, 14, 18, 19)
- UNDECIDED: 6 (entries 9, 11, 16, 17, 21, 24)

12 + 7 + 6 = 25 traced entries. 22 review-evidence bullets were not opened.

## Entries not reached

None of the review-evidence bullets under "Classed open" were opened. In digest order:

- `RE/capture/authority.rs:111`
- `RE/capture/api/data/evidence.rs:154-155,181-182`
- `RE/capture/api/slot_metadata.rs:51-56`
- `RE/capture/behavior/policy/receivers.rs:20-22`
- `RE/capture/callables/boundary_operators.rs:203-205` and `callables/boundary_requirements.rs:108-110`
- the missing-identity-text bullet (`boundary_operators.rs:141-143`, `boundary_requirements.rs:92-94`, `external_supply.rs:164,169`, `static_arguments.rs:319-320,400-401`, `binders.rs:68-69`, `semantics/types/identity.rs:60,130`)
- `RE/capture/callables/surface.rs:199-200,264`
- `calling/application/signature/instantiation.rs:227` and `semantics/conformances/policy_arguments.rs:190`
- `RE/capture/contracts/expressions/calls.rs:99-109`
- the zip-truncation bullet (`native_bindings.rs:21-26`, `runtime_permissions.rs:185-187`, `terminal_authority_permissions/declarations.rs:87-92`)
- `RE/capture/package_policy/semantic_dependencies.rs:143-145`
- `RE/capture/package_review.rs:53-63`
- `RE/capture/providers/application_realizations.rs:339-346`
- `RE/capture/providers/policy/rows.rs:111-113` with `providers/intrinsics.rs:264-265`
- `RE/capture/semantic_bindings.rs:120-122`
- `RE/capture/semantics/declarations/ownership.rs:145`
- `RE/encoding/encode/membership/framing.rs:73-75`
- `RE/record/package/provider_policy/binding_validation.rs:180`
- `RE/record/package/callable_policy/validation/signature_expressions.rs:125-148`
- `RE/record/package/provider_policy/validation.rs:30`
- `RE/record/package/callable_policy/validation/structural.rs:106-112`
- the unclassed bullet `RE/record/baseline/validation.rs:127-131` (the digest says unit 036 did not class it; it was not opened here either)

## What I did not read

- Every review-evidence file named in the list above, including their callers.
- `calls_for` on the produced build program (entry 9).
- `admit_retained_product_declaration` and the writer of `dependency_alias` (entry 11).
- `ResolvedPackageSourceClosure`'s constructor (entry 16).
- `request_matches` and catalog ordinal assignment (entry 17).
- `PackageOccurrenceRoster::derive` and lock `occurrences()` (entry 21).
- `VerifiedGitRepository::open` (entry 24).
- `PackagePolicyBaseline::recover_canonical` (entry 20); the accept of a well-formed payload does not depend on it, and a malformed policy might still be rejected there.
- `source_assembly.rs` past the call sites at lines 143, 387, and 497, and `readiness.rs` past the observation constructor call.
- The bodies of `dependency_source_instances` and `review_cache_inputs_digest`. Entry 10 uses the required-set check that was read; entry 7 uses the key fields that were read.
- Tests, except where a `#[cfg(test)]` gate was the point (`validate_retained_tree`).
- No file was built, compiled, or executed.
