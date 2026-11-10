slice_0118

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0905 | omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/clock.rs:404-433 | CLOSED | nfds=0 is an intentional modeled sleep; nonzero count without descriptor custody is rejected before the pointer is used. |
| A0906 | omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/filesystem.rs:427-461 | CLOSED | Skipping owning_service is deliberate; every other declared service in the reach lists is still checked before host filesystem calls run. |
| A0907 | omega-rust/omega/packages/build-declarations/src/dependencies/mod.rs:156-193 | CLOSED | build-declarations only lists entry-statement rows; package-manager always runs reject_unprojected_dependency_syntax on the editable path. |
| A0908 | omega-rust/omega/packages/build-output/src/captured_source/mod.rs:64-83,159 | CLOSED | Production capture builds entries through file()/symlink(), and narrowing uses full PartialEq including digest. |
| A0909 | omega-rust/omega/packages/build-output/src/staged_output_tree.rs:130-176; omega-rust/omega/packages/package-compilation/src/build_execution/build_record_codec.rs:202-233 | REACHABLE | Codec recovery accepts trees that omit parents or carry non-canonical symlinks if the commitment matches; live capture validates elsewhere. |
| A0910 | omega-rust/omega/packages/package-compilation/src/build_execution/build_admission.rs:78-81,137-145 | CLOSED | Empty transitive reach yields Pure by design; unexpected boundary services still fail at admit_build_machine:114-120. |
| A0911 | omega-rust/omega/packages/package-compilation/src/build_execution/build_record_codec.rs:655-722; omega-rust/omega/packages/package-manager/src/review/candidate/review_cache/record_codec.rs:848-900 | CLOSED | Cache records are domain-digested and cross-checked on decode; stale restricted requests are trusted cache content, not unsigned input. |
| A0912 | omega-rust/omega/packages/package-compilation/src/build_execution/filesystem_scope.rs:85-106 | CLOSED | ParentDir pop failure returning the absolute path is documented overlap policy for non-resolvable paths, not silent acceptance of bad builds. |

### A0905

1. Read `ClockImport::handle_effect_result` in full (`clock.rs:352-436`, read), including the `ClockOperation::Poll` arm at 404-433 (read).
2. Read `ClockImport::handle_effect_with_buffers` (`clock.rs:312-347`, read), which calls `handle_effect_result` for Poll when no storage buffers (read).
3. Search: `rg 'handle_effect_result' omega-rust/omega/interpreter` → 8 files (read hits); Poll path is only in `clock.rs` (read). Upstream binding: `rg 'ClockImport::admit' omega-rust` → `host_imports.rs:521` (read); read `ClockImport::admit` (`clock.rs:181-233`, read) and `admission::validate` Poll shape (`clock/admission.rs:133-141`, read).
4. Input on the accepting branch: admitted Poll call with three scalars; first integer bound to `_` (407); `count` and `milliseconds` extracted (410-417). Branch taken when `*count & u32::MAX == 0` and `*milliseconds >= 0` (422-426 pass) → `wait(...)` (433).
5. **Verdict: CLOSED.** Check at `clock.rs:422-425` rejects any nonzero nfds without descriptor custody; nfds=0 matches the module header’s modeled sleep (`clock.rs:17-18`, read). **Unsure:** nothing material for this entry.

### A0906

1. Read `FilesystemContext::admit_services` in full (`filesystem.rs:427-461`, read).
2. Search: `rg 'admit_services' omega-rust/omega/interpreter/terminal-runtime` → 3 hits (`filesystem.rs:383,427`; `linux.rs:59`; `windows.rs:93`) (read linux `authority` at 51-60, read).
3. Read Darwin `select` (`filesystem.rs:366-424`, read): `namespace_admission = self.admit_services(...)` stored on the call; Io bindings skip failing at provision (384-387, read) but `DarwinFilesystemCall::arguments` still does `self.namespace_admission.map_err(rejection)?` at 636 (read).
4. Input: `HostNamespace` delegation sets `supplied: true` (`filesystem.rs:169`, read); loop iterates `fixed_service_reach` ∪ `published_service_ceiling`, skips `declaration.owning_service` (437-438, read); every other service must appear in `program.module().services` with one of six toolchain identity strings (440-456, read).
5. **Verdict: CLOSED.** Owning service is intentionally not cross-checked against the module table; ancillary services are still enforced before argument dispatch (636). **Unsure:** none.

### A0907

1. Read `project_dependency_rows` in full (`BD/dependencies/mod.rs:156-194`, read).
2. Search: `rg '\bproject_dependency_rows\b' omega-rust` → 5 matches in 3 files (1 production call in `PM/.../calls.rs:31`, definition/tests in `mod.rs`, re-export in `lib.rs`) (read).
3. Read `project_direct_dependencies` (`calls.rs:20-47`, read) and `project_editable_dependencies` (`PM/.../projection/mod.rs:42-63`, read); read `reject_unprojected_dependency_syntax` (`read/policy.rs:55-190`, read in full).
4. Input reaching accepting branch in BD: only dependency-shaped calls in the build entry’s flat statement list become rows; nested/helper calls hit `continue` at `mod.rs:163-164` and produce no row.
5. **Verdict: CLOSED.** Package-manager editable projection always calls `reject_unprojected_dependency_syntax` immediately after row projection (`mod.rs:54-60`), which errors on any unlisted dependency call in states or expressions (`policy.rs:183-186`). **Unsure:** none for the syntax route; `from_executed_requests` not traced.

### A0908

1. Read `CapturedSourceEntry`, `matches_row_kind`, and `from_capture_rows` (`BO/captured_source/mod.rs:25-83,135-194`, read in full for those functions).
2. Search: `rg 'CapturedSourceEntry::' omega-rust/omega` → production construction via `file`/`symlink`/`Directory` in `source_snapshot.rs:147-152`; `from_capture_rows` at `mod.rs:135`, `source_snapshot.rs:260,326,361` (read constructors 40-54).
3. Read `is_subset_of` (`mod.rs:208-211`, read): compares full `CapturedSourceEntry` values, including `digest` fields.
4. Input on accepting branch: iterator entry whose kind/extent matches canonical metadata row via `matches_row_kind` (159) even though that helper ignores digest (64-83).
5. **Verdict: CLOSED.** All read production paths build entries through `CapturedSourceEntry::file` / `::symlink`, which set digest from bytes (`mod.rs:40-54`, read); mismatched digest would fail `is_subset_of` against the authentic complete capture (`mod.rs:208-211`). **Unsure:** whether any non-read wire decoder instantiates `CapturedSourceEntry` literals with wrong digest (not searched beyond production grep).

### A0909

1. Read `BuildStagedOutputTree::from_retained_parts` (`BO/staged_output_tree.rs:130-180`, read in full).
2. Read `recover_staged_output_tree` caller (`PC/build_execution/build_record_codec.rs:202-233`, read).
3. Read `materialize_retained_tree` (`BO/materialization/mod.rs:28-71`, read): does not call `validate_retained_tree` (validate is `#[cfg(test)]` at 75+, read). Read contrast path `tree_from_entries.rs:195-199` which calls `validate_retained_tree` on live capture (read).
4. Input: decoded `(relative_path, kind)` pairs whose sorted commitment equals `expected_commitment`; no parent-directory or symlink-canonical checks in `from_retained_parts`.
5. **Verdict: REACHABLE.** A persisted observation whose staged-output subtree lists a nested file without a directory row (or a symlink whose target is not canonical under `validate_retained_tree` rules) can be rebuilt via `recover_staged_output_tree` → `from_retained_parts` if the attacker or bug supplies a matching commitment; live capture rejects that shape in `tree_from_entries`. **Unsure:** whether every downstream consumer materializes without `select_files` ancestor checks (`completed_outputs.rs:33-43` read only).

### A0910

1. Read `admit_build_machine` in full (`PC/build_execution/build_admission.rs:50-156`, read).
2. Read caller `admit_build_program` (`build-settings/.../program.rs:98-107`, read) supplying `validation::infer_service_reaches`.
3. Input: `service_reach_plan.for_machine(machine.symbol)` returns `None` → `transitive` defaults to empty slice (`78-81`, read) → `filesystem_reachable` false → `BuildMachineExecutionMode::Pure` (`137-145`, read).
4. **Verdict: CLOSED.** Digest item 6 in area_06 marks this as intentional (read); machines that incorrectly reach boundary services still fail when `transitive_names` is nonempty (`114-120`, read). **Unsure:** whether `infer_service_reaches` can omit a machine that truly needs a plan row (inference body not read in full).

### A0911

1. Read `recover_restricted_build_request` and `recover_observation_summary` endings (`build_record_codec.rs:417-500,655-722`, read portions).
2. Read `decode_record` (`PM/.../review_cache/record_codec.rs:848-900`, read) and `into_review_and_facts` (`review_cache.rs:95-129`, read).
3. Read cache key binding (`review_cache/key.rs:26-73`, read): key hashes package, purpose, target, entry, inputs, snapshot—not the restricted-request payload fields themselves.
4. Input on accepting branch: malformed or stale restricted requests and observation fields survive decode when framed bytes satisfy structural invariants and record digest (`record_codec.rs:855-858`, read).
5. **Verdict: CLOSED.** Trust boundary is record `content_identity` over the payload plus `into_review_and_facts` identity/lineage/resolution match (`review_cache.rs:100-106`, read) and handoff/observation digest tie (`record_codec.rs:889-899`, read); corrupt records become cache misses (`lookup` removes bad files at `review_cache.rs:173-175`, read). Re-admission on hit is intentionally skipped for carried-forward reviews. **Unsure:** none for acceptance of unsigned bytes.

### A0912

1. Read `overlap_key` in full (`PC/build_execution/filesystem_scope.rs:85-106`, read) and module comment on write-root fencing (`filesystem_scope.rs:12`, read).
2. Read admission uses at `468-481` and `550-555` (read).
3. Input on accepting branch: path whose `ParentDir` cannot pop without emptying `folded` → returns `absolute` unchanged (99-101); `current_dir()` failure leaves relative path (89-91, read).
4. **Verdict: CLOSED.** Fallback is explicit policy for paths that cannot be canonicalized while keeping dangling symlink spellings visible; admission still compares keys consistently at establish/verify sites (`470-481`, `550-555`, read). **Unsure:** none.

### What I did not read

- `validation::infer_service_reaches` implementation (only caller at `build-settings/admission/program.rs:99`, read).
- `from_executed_requests` dependency path (area_06 Disagreements §6).
- Full `terminal-interpreter` dispatch stack above `TerminalEffectHandler::handle_effect_with_buffers` (inferred only via `host_imports.rs:521`).
- Windows-specific filesystem `admit_services` callee bodies beyond signatures (read linux only).

SLICE COMPLETE
