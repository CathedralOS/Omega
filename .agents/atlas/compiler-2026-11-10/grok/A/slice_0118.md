# slice_0118

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0905 | omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/clock.rs:404-433 | REACHABLE | Admitted Linux poll with count zero sleeps without validating the pointer operand at read or admission. |
| A0906 | omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/filesystem.rs:427-461 | UNDECIDED | Owning-service skip and HostNamespace `supplied` are read; no traced acceptance path for a program that should be rejected. |
| A0907 | omega-rust/omega/packages/build-declarations/src/dependencies/mod.rs:156-194 | CLOSED | Package-manager projection rejects dependency calls not among top-level projected rows after `project_dependency_rows`. |
| A0908 | omega-rust/omega/packages/build-output/src/captured_source/mod.rs:64-83,159 | CLOSED | Production capture paths build entries only via digest-correct constructors before `from_capture_rows`. |
| A0909 | omega-rust/omega/packages/build-output/src/staged_output_tree.rs:130-176 | REACHABLE | `from_retained_parts` and production materialize skip parent and symlink-escape checks that exist only under `#[cfg(test)]`. |
| A0910 | omega-rust/omega/packages/package-compilation/src/build_execution/build_admission.rs:78-138 | UNDECIDED | `for_machine` missing yields Pure mode; not shown whether the selected build machine can lack a reach row while needing filesystem fencing. |
| A0911 | omega-rust/omega/packages/package-compilation/src/build_execution/build_record_codec.rs:417-722 | REACHABLE | Cache decode rebuilds observation summary and restricted requests without re-running build admission; payload integrity is content hash only. |
| A0912 | omega-rust/omega/packages/package-compilation/src/build_execution/filesystem_scope.rs:85-124 | REACHABLE | `overlap_key` returns the unfolded absolute path when `ParentDir` cannot pop, so overlap compares spelling not resolved location. |

## A0905

Digest row cites `TR/clock.rs:405-409`; lines match `omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/clock.rs:404-433` at this checkout (read).

Opened `ClockImport::handle_effect_result` (clock.rs:352-436, read). The `ClockOperation::Poll` arm binds the first scalar argument with `_` at clock.rs:406-408 (read) and never uses it. When the second operand has no bits in the low 32 bits (`*count & u32::MAX == 0` at clock.rs:422) and the timeout is non-negative, execution calls `wait` at clock.rs:433 (read), which only sleeps (clock.rs:282-292, read).

Opened `admit` in clock/admission.rs:19-160 (read). `ClockImplementation::Poll` admission checks scalar parameter types and order (admission.rs:133-141) but does not require pointer custody or validate the pointer operand (read). Module header states descriptor readiness is unsupported (clock.rs:17-18, read).

Search `handle_effect_result` under `omega-rust/omega/interpreter/terminal-runtime`: 7 files, 8 matches (count mode; read). Poll handling is only in clock.rs; `handle_effect_with_buffers` dispatches to `handle_effect_result` at clock.rs:343 when storage buffers are empty (read).

**Input that reaches the branch:** A checked program whose admitted Linux `poll` foreign call is invoked with any first integer operand, second operand zero, and a finite non-negative timeout passes operand shape checks and is executed as a pure sleep with no pointer validation.

**Verdict:** REACHABLE — smallest case: admitted poll with zero count and an arbitrary pointr value is accepted and treated as sleep only.

## A0906

Lines at filesystem.rs:427-461 match digest `TR/filesystem.rs:437-439` (relocated by path prefix; read).

Opened `FilesystemContext::admit_services` (filesystem.rs:427-461, read). The loop skips `declaration.owning_service` at filesystem.rs:437-439 (read). Additional services must appear in `program.module().services` with one of six fixed identity spellings (filesystem.rs:446-455, read).

Opened `FilesystemContext::new` (filesystem.rs:167-175, read): `supplied` is true exactly when delegation is `TerminalFilesystemDelegation::HostNamespace` (filesystem.rs:169, read).

Search `admit_services` in `omega-rust`: 4 matches in 3 files — filesystem.rs (definition and call site), linux.rs:59, windows.rs:93 (read). Callers pass the result as `namespace_admission`; for non-`DescriptorEffect::Io` paths, failure rejects admission at filesystem.rs:383-386 (read).

**Verdict:** UNDECIDED — digest records no concrete wrong program; I did not trace whether skipping the owning service or blanket `HostNamespace` `supplied` lets a program that should fail `admit_services` still run filesystem effects.

## A0907

Opened `project_dependency_rows` (build-declarations/.../dependencies/mod.rs:156-194, read). It iterates only top-level statements of the build entry state (mod.rs:161-162, read); nested calls are invisible here (read).

Search `project_dependency_rows` in `omega-rust`: 5 matches — 3 in mod.rs (definition + tests), 1 in lib.rs re-export, 1 production call in `PM/.../projection/calls.rs:31` (read).

Opened `project_direct_dependencies` (calls.rs:20-47, read) then `extract_scoped_requests_from_source` → `project_editable_dependencies` (projection/mod.rs:42-63, read). After `project_direct_dependencies`, `reject_unprojected_dependency_syntax` runs (mod.rs:55-60, read).

Opened `reject_unprojected_dependency_syntax` and `reject_dependency_statements` (policy.rs:55-189, read). Any dependency-named call in a machine state, trait default body, conformance member, or capability state whose statement handle is not in `accepted_statements` returns `DependencyProjectionError::UnsupportedDependencyShape` (policy.rs:183-186, read).

**Closing check:** policy.rs:183-186 for nested states; mod.rs:55-60 ensures it runs on every editable dependency read path.

**Verdict:** CLOSED — a build.omg with a dependency call only inside a nested state is rejected on the package-manager syntax route even though `project_dependency_rows` omits it.

## A0908

Opened `matches_row_kind` (captured_source/mod.rs:64-83, read): compares directory/file/symlink kind and byte lengths only; `digest` fields are not compared (read). `from_capture_rows` uses `matches_row_kind` at mod.rs:159 (read).

Enum fields are public (mod.rs:25-36, read). Constructors `CapturedSourceEntry::file` and `symlink` set `digest` from `content_identity` (mod.rs:40-54, read).

Search `from_capture_rows` in `omega-rust`: 7 matches — 3 in source_snapshot.rs (production), 1 in filesystem_scope/runtime.rs, 1 definition, 2 tests (read). Production callers build entries via `CapturedSourceEntry::file` / `symlink` in `retained_source_input` (source_snapshot.rs:147-152, read) or reuse entries from an existing `CapturedBuildSourceInput` (source_snapshot.rs:187-191, read).

**Verdict:** CLOSED — compiler source capture paths do not construct file or symlink variants with mismatched digests before `from_capture_rows`; the gap is only on manual enum construction, which production callers do not use.

## A0909

Opened `BuildStagedOutputTree::from_retained_parts` (staged_output_tree.rs:130-177, read). Validates canonical relative paths, duplicate paths, and commitment equality (staged_output_tree.rs:136-175, read); does not require parent directories or confine symlink targets (read).

Search `BuildStagedOutputTree::from_retained_parts` in `omega-rust/omega/packages/build-output`: 2 matches (definition and `materialize_into` path is separate) (read). Non-test production caller: `PC/build_execution/build_record_codec.rs:233` via `recover_staged_output_tree` (build_record_codec.rs:202-233, read).

Opened `materialize_retained_tree` (materialization/mod.rs:28-71, read): creates entries in order without calling `validate_retained_tree` (read). Parent-directory and symlink-canonical checks live in `validate_retained_tree` at materialization/mod.rs:89-147 marked `#[cfg(test)]` (read; matches area_06.md:308-309). Production `create_materialized_symlink` (materialization/mod.rs:316-327, read) symlinks the raw target spelling without escape validation.

**Verdict:** REACHABLE — a staged-output tree decoded through `recover_staged_output_tree` with a matching commitment can contain a symlink whose target spelling escapes the output root and still be accepted by `from_retained_parts` and materialized without the test-only validators.

## A0910

Opened `admit_build_machine` (build_admission.rs:50-155, read). `service_reach_plan.for_machine(machine.symbol)` at build_admission.rs:78-80; `None` yields `transitive = &[]` (read). Empty transitive makes `filesystem_reachable` false (build_admission.rs:122-133, read) and selects `BuildMachineExecutionMode::Pure` at build_admission.rs:137-138 (read).

Caller: `admit_build_program` in `build-settings/.../admission/program.rs:99-106` builds `service_reach_plan` via `validation::infer_service_reaches` then calls `admit_build_machine` (read).

Search `admit_build_machine` in `omega-rust`: 5 matches in 3 files (definition, re-export, program.rs call) (read).

Opened `ServiceReachInferencePlan::for_machine` (flow-effects/src/lib.rs:516-523, read): searches `self.machines()` which is the `root_machines` span (lib.rs:512-513, read). Inference appends one summary per `program.machines()` work item to `root_machines` (effect_inference.rs:2342-2354,2660-2682, read).

**Verdict:** UNDECIDED — I read that a missing `for_machine` row forces Pure admission, but did not prove or disprove that the selected build machine symbol can be absent from the plan while the program still requires filesystem execution fencing.

## A0911

Digest cites end of `build_record_codec` entry; opened `recover_observation_summary` (build_record_codec.rs:417-517, read) and `recover_restricted_build_request` (build_record_codec.rs:655-721, read). Both assemble structs via `from_retained` / `from_retained_parts` without calling `admit_build_machine` (read).

Search `recover_observation_summary` and `recover_restricted_build_request` in `omega-rust`: production use in `PM/.../review_cache/record_codec.rs:880-885` inside `decode_record` (read). `decode_record` checks payload `content_identity` at record_codec.rs:856-858 and handoff digest consistency at record_codec.rs:892-899 (read); it does not re-derive restricted grants from source (read).

Opened `compiled_review_key_with_identity` (review_cache/key.rs:48-73, read): cache key hashes package, purpose, targets, entry path, compiler identity, `review_cache_inputs_digest`, and `review_cache_snapshot_digest` — not the encoded restricted-request bytes (read). Stored records are keyed by filename derived from that hash (review_cache.rs:153-171, read).

Baseline validation for restricted requests only checks `selected_target_profile` against policy target (`RE/record/baseline/validation.rs:127-130`, read).

**Verdict:** REACHABLE — a on-disk compiled-review record with valid payload/trailer digests can carry decoded restricted build requests and observation fields that were never re-checked against live build admission.

## A0912

Opened `overlap_key` (filesystem_scope.rs:85-124, read). Builds `absolute`, folds `.` and `..` into `folded`; when `ParentDir` cannot pop, returns `absolute` unchanged at filesystem.rs:99-100 (read). Otherwise uses `canonicalize` with fallback to lexical `base` plus tail (filesystem_scope.rs:106-123, read).

Search `overlap_key` in `omega-rust`: 9 matches, all in `filesystem_scope.rs` and one use in `filesystem_scope/preparation.rs:121` (read). Used at admission to compare `source_root` and `build_dir` (filesystem_scope.rs:159,185,470,477,550, read).

Module comment at filesystem_scope.rs:12 states the intent is spelling-independent comparison when canonicalization works (read).

**Verdict:** REACHABLE — path spellings that fail `ParentDir` folding return the raw absolute buffer, so two spellings that would resolve to the same directory may not be treated as overlapping at the fence check.

## What I did not read

- `infer_service_reaches_uncached` fixpoint details beyond the machine loop that fills `root_machines` (effect_inference.rs:2404+, not opened).
- PM `from_executed_requests` dependency route (disagreement 6 in area_06.md names it as a separate path).
- Terminal-runtime filesystem execution paths beyond `admit_services` call sites for concrete accept/reject of namespace failures.
- `review_cache_inputs_digest` field coverage for restricted-request binding.

SLICE COMPLETE
