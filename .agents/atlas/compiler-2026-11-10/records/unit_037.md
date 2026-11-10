# Unit 37 notes

Checkout: `<checkout>/` (main at `457adf4a11f`). All paths below are relative to `omega-rust/omega/packages/source-acquisition/src/` unless they start with `omega-rust/`. The crate is `package_source` in other crates' imports (read in `use package_source::...` lines found by grep).

## Files covered (131 rows of unit_037.tsv, all read in full)

custody/tests/{repository_integrity,snapshots,traversal,windows_ancestry,windows_siblings}.rs; custody/tree.rs; error.rs; git/cache/{configuration,creation,custody,identity,invalidation,mod,repository,retry,snapshots}.rs; git/executable/{budget,executor,mod,selection,validation}.rs; git/git_command.rs; git/git_command/{capture,policy,reconciliation}.rs; git/git_command/tests/{bounded,configuration,executable,mod}.rs; git/mod.rs; git/objects/{authentication,graph,identity,inspection,mod,object_rows,projection,tests,tree}.rs; git/objects/batch/{availability,custody,mod,protocol,transfer}.rs; git/objects/batch/availability/{protocol,tests}.rs; git/request.rs; git/resolution/{acquisition,exact_revision,issuance,materialization,mod,network,pinned_source,recorded_objects,repository,selection,tree_session,workspace_member}.rs; git/resolution/tests/{fetch,mod,resolution}.rs; git/resolution/tree_session/inventory.rs; git/resolution/workspace_member/{declarations,recorded}.rs; git/snapshot.rs; git/snapshot_metadata.rs; git/tests/{exact_revision,exact_workspace,mod,request,root_pin,snapshot,tree_session,validation,workspace}.rs; git/tests/exact_revision/{failures,fixtures,retry}.rs; git/tests/exact_workspace/{fixtures,planner,submodules}.rs; git/workspace/{mod,workspace_declaration}.rs; identity/{digest,error,git,local,locator,mod,resolution,tests}.rs; lib.rs; limits.rs; local/{mod,observation,operations,recovery,resolution_observations,snapshot,staging}.rs; local/recovery/{tests,timings}.rs; local/snapshot/{address,materialize}.rs; local/tests/{capture,limits,mod,observation,path_policy,snapshots,staging}.rs; observations/{mod,resolved,storage}.rs; snapshot/{construction,metadata,mod,permissions,publication}.rs; source_resolver.rs; storage.rs; test_support.rs; tree/capture/{mod,traversal,traversal_observations}.rs; tree/{filesystem,identity,mod,resolved_tree}.rs; tree/filesystem/read_tests.rs.

Not in this unit but referenced (not read by me): `custody/{lock,platform,publication,mod}.rs`, `custody/tests/locks.rs` and its `mod.rs`, `optimization_core::digest_encoder`, `bounded_process`, `resolver_execution`, `platform_custody::record_file`, `source::` crate (CapturedSourceTree).

---

# Crate-level shape (lib.rs, limits.rs, error.rs)

### lib.rs (76 lines)
- Purpose: declares modules and the public re-exports; two little-endian decode helpers.
- Shape: `#![forbid(unsafe_code)]` (lib.rs:29); `#![cfg_attr(windows, feature(windows_by_handle))]` (30); `pub use` of `SourceResolveError`, `GitSourceRequest`, `GitWorkspace*` planner types, identity types, `LocalSourceLimits`, `resolve_local_source`, `ResolvedLocalSnapshot/Source`, `GitAcquisitionPin/ResolvedGitSource`, `PrimaryGitChoices`, `SourceResolver` (45-60). `take_u64` (65-69), `take_framed_bytes` (71-76) read an LE u64 length frame.
- Same job elsewhere: the writer is `identity/digest.rs:4 append_framed_bytes` (LE u64 length + bytes). Users of the pair (grep `append_framed_bytes|fn take_framed_bytes|fn take_u64` over omega-rust): snapshot/metadata.rs, git/snapshot_metadata.rs, git/cache/identity.rs, identity/digest.rs, lib.rs, i.e. all inside this crate.
- Unused/one-caller: n/a. Fail-open: none seen.

### limits.rs (88 lines)
- Purpose: every compiler-owned ceiling and every versioned policy/format tag as `pub(crate) const`, plus `LocalSourceLimits`.
- Shape: policy tags `GIT_CACHE_POLICY = b"omega-git-cache-v28"` (17), `GIT_SNAPSHOT_POLICY ...-v4` (23), `LOCAL_SNAPSHOT_POLICY ...-v2` (27), custody policy v2 and legacy v1 (28-30), observation schema 3 (31-33). Canonical configs `GIT_CONFIG_SHA1`/`GIT_CONFIG_SHA256` byte strings (36-38). Limits: `CACHE_CUSTODY_ENTRY_LIMIT` 65_536 (39), `SOURCE_ENTRY_ABSOLUTE_LIMIT` 65_536 (40), `GIT_TREE_GRAPH_ENTRY_LIMIT` 262_144 (41), `SOURCE_BYTE_ABSOLUTE_LIMIT` 512 MiB (42), `SOURCE_DEPTH_ABSOLUTE_LIMIT` 256 (43), `CACHE_CUSTODY_DEPTH_LIMIT` = 260 (44), locator 4 KiB / revision 1 KiB (45-46), timeouts: command 120 s (50), whole resolution 600 s (55), cleanup 2 s (57), lock 120 s (58), launches 64 (56). `LocalSourceLimits{max_entries,max_bytes,max_depth}` default 4096/256 MiB/64 (62-77). `compiler_bounded` clamps each field with `min` (79-87).
- Decides: pure clamps; no judgement.
- Stated limits: limits.rs:7-9 "Omega's own workspace needs 163,219 graph entries / 23 MiB of listing" (documents why 262,144).
- `compiler_bounded` users outside this crate: grepped `recovery_owned_bytes|matches_lineage|from_canonical_path|hash_canonical|compiler_bounded` excluding source-acquisition: 29 hits across 16 files; compiler_bounded itself is applied inside this crate at local/operations.rs:45,56,161; local/staging.rs:85; git/resolution/acquisition.rs:334; workspace_member/recorded.rs:47; workspace_member.rs:60; tree_session.rs:90. Not applied at `resolve_git_workspace_member_from_pin_in_lanes` (workspace_member.rs:68-94) which passes caller `limits` unchanged (see fail-open note F9).

### error.rs (361 lines)
- Purpose: one closed error enum for the crate and constructor helpers.
- Shape: `SourceResolveError` 31 variants (error.rs:10-125) incl. `Git{operation,status,stderr}`, `GitOutputOverflow`, `GitTimedOut`, `GitResolution{CommandLimit,TimedOut,CapturedOutputLimit}`, `GitCleanupFailed`, `GitSubmodulesUnsupported{edges: Vec<GitSubmoduleEdge>}`, `GitObjectInvalid`, `GitExactRevisionUnavailable`, `GitTreeInvalid{path: Vec<u8>}`, `GitCacheInvalid`, `LocalSnapshotInvalid`, `LocalSourceChanged`, `SourceSnapshotContentMismatch`. `GitSubmoduleEdge{path, commit: Option<String>}` (127-131). Display match 133-318. Helpers `cache_invalid` (322), `local_snapshot_invalid` (329), `git_tree_invalid` (339), `too_many_files` (349), `too_many_bytes` (353), `local_source_changed` (357).
- Inputs/outputs: constructors only.
- How it decides: variant is the key; callers match on variants (e.g. `may_preserve_cache` git/cache/retry.rs:65). Tests also match on `message` strings (e.g. git/objects/tests.rs:138, git/tests/root_pin.rs:93) and `error.to_string().contains(...)` (source_resolver.rs tests 226,258).
- Stated limits: error.rs:1-3 and 243 "submodule edges that must become explicit package edges before they are supported".
- Same job: `GitCacheInvalid` vs `LocalSnapshotInvalid` are the same failure with two variants chosen by `CacheCustodyKind` (custody/tree.rs:489-498).
- Fail-open: none seen.

---

# custody/

### custody/tree.rs (498 lines)
- Purpose: verifies a cache directory tree: ancestry from the filesystem root, then root directory, then every entry under entry/byte/depth ceilings; reads bounded records.
- Shape: `CacheCustodyKind{Git,LocalSnapshot}` (41-45); `CacheCustodyMeasurement{entry_count,logical_bytes,maximum_depth}` (47-52); `read_bounded_cache_record` (54) and `_from_open_directory` (72) wrap `platform_custody::record_file::RecordFileRoot::read`; `verify_cache_custody_root` (132-148); `verify_retained_cache_custody_root` (150-166, single no-follow open + identity compare); Windows-only `verify_retained_cache_child_custody_root` (168-187); `classify_retained_cache_directory` (189); `verify_retained_cache_directory` (205); `verify_cache_ancestry` per OS (unix 229-268, windows 270-291, other 293-296); `git_cache_custody_byte_limit` = `max_bytes*3 + 64 MiB`, capped 1 GiB (298-304); `local_cache_custody_byte_limit` = `max_bytes + 64 MiB` capped 512 MiB (306-311); `verify_cache_custody_from_open_root` (334-448, explicit stack walk); `cache_custody_has_capacity` (450-454); `open_cache_custody_directory` (456-487); `cache_custody_invalid` (489).
- Inputs: `Path`, retained `cap_std::fs::Dir`, `LocalSourceLimits`. Output: `Ok(())`/measurement or `SourceResolveError` of kind chosen by `kind`. Calls `custody::platform::*` (not read) for owner/mode/ACL/Windows-identity checks (tree.rs:20-27, 145-147, 361-399).
- How it decides: file type from `symlink_metadata` (`is_symlink() || !is_dir()` rejects, 138, 194); unix ancestry: owner uid must be effective uid or 0 (245) and mode `& 0o022 != 0 && & 0o1000 == 0` rejects (253); per-node owner/mode via platform fn (361,374); regular files and symlinks add `metadata.len()` to `logical_bytes` and reject over `byte_limit` (401-413); directory depth over `CACHE_CUSTODY_DEPTH_LIMIT` rejects (418-429). Identity compare between classification and no-follow open (214, 479).
- Cases by shape: entry kinds are file, symlink, dir; anything else errors "unsupported filesystem entry kind" (431-436).
- Same job elsewhere: second tree walker with different rules at git/cache/custody.rs:72-148 (bans symlinks, checks link count). Directory "classify, open no-follow, compare identity" repeated at custody/tree.rs:189-227, 456-487; git/cache/custody.rs:28-49; configuration.rs:23-41; invalidation.rs:52-70; snapshot/publication.rs:62-74; git/cache/snapshots.rs:37-54; git/snapshot.rs:227-245; local/snapshot.rs:170-180; local/recovery.rs:84-94. Grep `is_symlink\(\)\s*\|\|\s*!` over this crate's src: 16 hits in 11 files (custody/lock.rs, custody/publication.rs included, not read).
- Unused: `verify_local_cache_root_custody` (127-130) is `#[cfg(test)]`; `verify_git_cache_root_custody` used at git/resolution/issuance.rs:31,36 and tests.
- Fail-open: F1 (non-unix non-windows `verify_cache_ancestry` returns `Ok(())`, 293-296; traced: `storage.rs:165-170` makes `current_user_cache_base` an error on such hosts so production sessions cannot be created; test-only `for_hardened_base` could).
  Also F2 note: symlinks are admitted by this walker (only counted/ACL-checked, 391-413); stated by the git-repository walker being separate.

### custody/tests/repository_integrity.rs (199)
Tests: provisional cache dir cleanup, symlink-in-entry invalidation not followed (cfg unix), config substitution rejected without calling Git (`git_cache_rejects_repository_config_substitution_without_asking_git`, 51-70 asserts `GitCacheInvalid` and that `source.identity` is removed), replaced repository / objects path rejected, `objects/info` replaced by file rejected (`NotADirectory` must not prove absence, 114-128), symlinks in `config`/`FETCH_HEAD`/`HEAD`/object file rejected (unix), hard link to `config` rejected (unix, 165-180), `0o777` cache dir rejected (unix). Windows `ignore` notes on replacement tests. No production logic.

### custody/tests/snapshots.rs (176)
Tests that `PendingMaterializedSnapshot` writes/cleanup stay bound to the opened stage handle after the parent path is replaced, that `open_or_create_snapshots` creates mode 0700 (unix, 74-81), that publication rejects a replaced stage name (118-142), that a nested directory swapped for a symlink makes `write_snapshot_file_from_open_root` fail `LocalSnapshotInvalid` (146-176, unix).

### custody/tests/traversal.rs (116)
Tests `open_cache_custody_directory` does not follow a directory replaced by a symlink (both kinds asserted by the match, only `Git` invoked at 14-16; `LocalSnapshot` variant of the helper is never called, 19-54), rejects different concrete directory (58-86, expects message containing "changed between classification"), and that an externally writable external-local lane (0o777) is rejected with `LocalSnapshotInvalid` (90-116).

### custody/tests/windows_ancestry.rs (131), windows_siblings.rs (84)
Windows-flavoured tests of `retain_verified_windows_cache_ancestor`, `verify_retained_cache_custody_root`, `verify_cache_custody_root` (junction parent via `cmd /C mklink /J`, 108-131) and `verify_retained_cache_child_custody_root` (not-direct-child, different retained identity, parent identity). Neither file has a file-level `#[cfg(windows)]`; they are presumably gated by `custody/tests/mod.rs` (not read) — inferred. `windows_siblings.rs` imports a `#[cfg(windows)]` function (tree.rs:168) so it can only compile on Windows.

---

# git/executable/

### budget.rs (4)
`pub(crate) use bounded_process::BoundedCaptureBudget as GitCapturedOutputBudget;` One alias. Comment says "Shared captured-output byte accounting ... across all child commands within one Git resolution". Users in this crate outside budget.rs (grep `GitCapturedOutputBudget|BoundedCaptureBudget`): executor.rs, git_command/capture.rs, batch/availability/tests.rs (3 files); definition is in `omega/foundation/bounded-process`.

### executor.rs (225)
- Purpose: one `GitExecutor` per Git resolution holding transport, start time, launch counter and output budget; hands each child a deadline.
- Shape: `GitCommandDeadline{timeout,resolution_timeout}` (26-54) — `new` takes `GIT_COMMAND_TIMEOUT.min(remaining)` and remembers the whole-resolution timeout only when `remaining <= GIT_COMMAND_TIMEOUT` (33-38); `project_error` turns `GitTimedOut` into `GitResolutionTimedOut` in that case (44-53). `GitExecutor` fields (56-65) with `launches: Cell<usize>`. `selected` (68-82), test-only openers (84-143, `cfg(all(test, unix))`), `from_primary_git` (145-169) calls `primary_git.verify_outside(roots)` and `ResolverExecutionBackend::open`, `begin_launch` (171-184), `remaining_time` (190-199), `impl CacheLockBudget` (202-210), `git_resolution_captured_output_ceiling` = `max_bytes + 64 MiB` capped 576 MiB (212-217), `test_system_git_executor` (219-225, `cfg(test)`).
- How it decides: launch count `>= maximum_launches` -> `GitResolutionCommandLimit` (174-178); elapsed `>= timeout` -> `GitResolutionTimedOut` (192-196). Keyed on counters/time.
- Same job: budget checking duplicated as `verify_budget` called around every command (git_command.rs:68, batch/transfer.rs:110,154, availability.rs:49,116, acquisition.rs:307).
- Fail-open: none seen.

### mod.rs (7), validation.rs (49)
mod.rs lists submodules (`selection` is `pub mod`, others `pub(crate)`). validation.rs `verify_git_executable_launchability` (8-42): `metadata().is_file()`; unix: any exec bit (`mode & 0o111 == 0` rejects, 24); windows: file name must case-insensitively equal `git.exe` (`is_direct_windows_git_executable`, 44-49, `cfg(any(windows,test))`). Keyed on file name spelling on Windows. Tests at selection.rs:344-351 and git_command/tests/executable.rs:18-58 pin that mode bits like 0o720/0o4700 are accepted ("does not invent mode trust", test name).

### selection.rs (352)
- Purpose: freeze one canonical Git executable path before any package input is read.
- Shape: `PrimaryGitSelection{path}` (17-20), `capture` (23-29) errors `GitExecutableUnavailable` when none; `capture_optional` (31-45) snapshots `PATH` only if no explicit path; `verify_outside` (51-63); `capture_primary_git_optional` (80-109); `freeze_explicit_primary_git` (111-141); `ExcludedRoots{paths: Vec<ExcludedRoot{path,canonical: Option}>}` (160-198); `resolver_package_controlled_roots` (216-228) prepends `current_dir()`. Tests 230-352.
- How it decides: explicit path must be absolute (115), not inside an excluded root lexically or canonically (121,133), launchable. Automatic: iterate `split_paths(PATH snapshot)`, skip non-absolute entries (93-95), skip candidates whose path or canonical path is in an excluded root (97,103), skip non-launchable (103). `ExcludedRoots::contains` is `starts_with` on the stored path or its canonical form (186-194).
- Unused/one-caller: `PrimaryGitSelection::capture` is `pub`; grep `PrimaryGitSelection::capture\(|capture_optional\(` in omega-rust: outside this crate 0 hits; inside: executor.rs:134 (`cfg(all(test,unix))`) and :223 (`cfg(test)`); production uses `capture_optional` once (source_resolver.rs:121). `capture_primary_git` (66-78) and `capture_primary_git_from_snapshot` (203-214) are test-only.
- Fail-open: F3 `ExcludedRoot.canonical` is `path.canonicalize().ok()` at construction (179); a root that does not exist yet is compared only lexically. Traced: a candidate that canonicalizes cannot be inside a nonexistent directory at that moment; only a race. Treat as low.

---

# git/git_command*

### git_command.rs (124)
- Purpose: the only function that launches Git (`run_git_output`, 44-69) plus three wrappers.
- Shape: `GitCommandCapture{operation,input,stdout_limit}` with default (28-42). `run_git_output` builds the command with `sealed_git_command`, calls `executor.begin_launch()`, runs `run_command_capture` with `GIT_STDERR_LIMIT`, maps timeout via `deadline.project_error`, and reconciles with `executor.verify_budget()`. `run_git` (71), `run_git_stdout` (84, `String::from_utf8_lossy`), `run_git_bytes_stdout` (98) turn non-zero exit into `SourceResolveError::Git{operation:"command",...}` (118-122).
- "Sealed" claim check: grep `Command::new|sealed_git_command\(|run_git_output\(` over src excluding tests: only `git_command.rs:55,108`, `policy.rs:20`, `transfer.rs:135`, `availability.rs:98` plus test_support.rs. So all production launches go through `run_git_output`.
- Same job: non-zero-exit to `Git{..}` is written three times: git_command.rs:118, transfer.rs:146-152 (operation `"cat-file --batch"`), availability/protocol.rs:20-26 (operation `OPERATION`). The `"command"` label at git_command.rs:119 means the `operation` argument of `GitCommandCapture` is not used for that error.
- Fail-open: none seen (non-zero exit is an error).

### git_command/capture.rs (135)
`run_command_capture` (52-70) builds `BoundedCaptureLimits` and calls `bounded_process::run_bounded_process`; `project_error` (72-131) maps each of 12 `BoundedProcessRunError` variants onto `SourceResolveError` (Spawn/Wait/WorkerSpawn/StreamCapture/InputTransfer/WorkersEndedEarly -> `Git{operation: "<op> <what>", status: None}`; OutputOverflow -> `GitOutputOverflow`; AggregateOutputOverflow -> `GitResolutionCapturedOutputLimit`; TimedOut -> `GitTimedOut`; Cleanup -> `GitCleanupFailed`; Finalize/InvalidLimits -> `GitExecutionBoundaryInvalid`). `duration_millis` (133). Two `cfg(all(test,unix))` wrappers (14-50).

### git_command/policy.rs (142)
`sealed_git_command` (20-122): requires an absolute, existing directory working dir (25-40); `executor.execution_backend.prepare(phase, dir)`; sets env `LANG=C`, `LC_ALL=C`, `GIT_ALLOW_PROTOCOL` (from transport), `GIT_ATTR_NOSYSTEM`, `GIT_LFS_SKIP_SMUDGE`, `GIT_NO_LAZY_FETCH`, `GIT_PROTOCOL_FROM_USER=0`, `GIT_TERMINAL_PROMPT=0`; args `--no-replace-objects`, `core.hooksPath=<null device>`, `protocol.allow=never`, per-protocol `allow` values, `http.followRedirects=false`, `core.fsmonitor=false`, `core.autocrlf=false`, `gc.auto=0`, `maintenance.auto=false`, `fetch.fsckObjects=true`, `transfer.fsckObjects=true`, `fetch.recurseSubmodules=false`, `submodule.recurse=false`, lfs filter overrides. `protocol.file.allow` is `"never"` unless `cfg(any(test, feature="test-fixtures"))` (66-77). Windows adds `core.longpaths=true` (124-127).
- Decides on transport: `permits` (request.rs:81-87) returns "always" for both https and ssh when the executor transport is Https or Ssh (so an Https-tagged resolution still allows ssh and vice versa; configuration.rs test lines 70-71 pin `https` and `ssh` both `always` for `Https`).
- Stated limit: policy.rs:8 `https:ssh` "or test `file`".

### git_command/reconciliation.rs (28)
`reconcile_git_command_result` (7-16): `GitCleanupFailed` outranks everything, then a budget error, then the command result. `reconcile_git_cache_operation_result` (18-28): `namespace_result?` first, then `Some(Err)` invalidation/cleanup error, then the operation result. Call sites (grep `reconcile_git_cache_operation_result\(` over omega-rust, 14 hits in 7 files): the third argument is `None` at creation.rs (4), repository.rs (2: macro 75, 231), acquisition.rs:221-225, snapshot.rs:75; it is `Some(..)` at acquisition.rs:185-189 and 291-298 (invalidation result), transfer.rs:67 and availability.rs:78 (request-file cleanup); custody/tests/locks.rs has 2 (unread). So the `invalidation_result` slot carries two different meanings (cache invalidation, temp-file cleanup).

### git_command/tests/{bounded,configuration,executable,mod}.rs (50,82,168,18)
Tests: stdout/stderr overflow with `sh` (unix; macOS accepts `GitCleanupFailed` as alternative, bounded.rs:29-30), cleanup failure outranks budget expiry (38-50), exact env map and required `-c` args for an `Https` executor (configuration.rs), launchability modes, 1-launch budget, 30 ms timeout, cumulative-output 12-byte ceiling (executable.rs, unix only). macOS branches appear in 3 assertions. mod.rs re-exports with `use`.

---

# git/cache/

### mod.rs (31), identity.rs (33), configuration.rs (77)
mod.rs documents (13-22) "invalidation removes `GIT_CACHE_METADATA`" and "`discard_incomplete_entry` ... removes the entire entry directory". identity.rs: `git_cache_identity(url, rev, transport)` = SHA-256 over `GIT_CACHE_POLICY`, url, rev, `execution_transport.cache_tag()` (9-20); `git_cache_metadata` = policy bytes + 3 LE-framed fields (22-33). Keyed on locator_identity (canonical), requested revision spelling, transport tag. configuration.rs: `replace_canonical_git_control_file` (17-47) classifies the repository dir, opens no-follow, compares identity, then `..._from_open_repository` (49-77) calls `RecordFileRoot::replace_existing("config", bytes, max = GIT_CONFIG_SHA256.len())`.

### creation.rs (241)
- Purpose: build a new bare Git cache entry.
- Shape: `create_git_cache_entry` (44-178) with 12 parameters; `discover_git_object_format` (180-203) runs `git ls-remote --symref -- <url> HEAD <rev>` unless `rev` is already an object id (186-188) or a `known_format` was passed (63-66); `parse_git_remote_object_format` (205-241).
- Flow: `PendingCacheEntry::create` (custody/publication.rs, unread) -> empty template dir -> discover format -> `git init --quiet --bare [--object-format=sha256] --template empty-template repository` -> replace `config` with canonical bytes -> remove template -> write `source.identity` with `create_new`, mode 0600 and exact-mode re-check (122-158) -> `VerifiedGitRepository::open` -> drop handle -> `pending.publish`. Each step wrapped in `reconcile_git_cache_operation_result(..., verify_ambient_path_identity(cache_dir), None)`.
- Decides: object format from the OID length (via `git_object_algorithm`), mixed SHA-1/SHA-256 rows rejected (227-232), no row rejected (235-240). Rows starting `ref: ` skipped (211).
- Cases by shape: only two object formats.
- Fail-open: none seen.

### custody.rs (git/cache, 239)
`open_retained_git_directory` (28-49), `verify_retained_git_directory_identity` (51-70), `verify_git_repository_tree_from_open_root` (72-148, stack walk over the bare repo; rejects symlinks (106-111), unsupported kinds (131-135), depth and entry ceilings), `verify_retained_git_regular_file` (150-169), `verify_git_regular_file_link_count` (unix: `nlink != 1` rejects, 171-185; **non-unix: `Ok(())`**, 187-193), `reject_retained_git_path` (195-239): walks `components` with no-follow opens, returns `Ok(())` when a parent is `NotFound` (209) or leaf `NotFound` (232), errors on any other I/O error or existing leaf. Used for `objects/info/alternates`, `objects/info/http-alternates`, `commondir` (repository.rs:187-197).
- Fail-open: F2 (non-unix link-count check absent; header comment says "`nlink != 1` on Unix"; open for Windows unless `verify_windows_open_cache_regular_file_identity`, tree.rs:384, covers it — not read); F4 `reject_retained_git_path` `NotFound` => `Ok` is stated by the function's purpose ("proves ... absent") and pinned by test custody/tests/repository_integrity.rs:114-128 (non-NotFound errors reject): traced.

### invalidation.rs (93)
`invalidate_git_cache_entry_from_open_parent` (46-77): classify entry dir, open no-follow, compare identity, `remove_file(source.identity)` (NotFound ignored), `synchronize_cache_parent` (unix fsync; non-unix `Ok`, 87-93). `invalidate_git_cache_entry_from_retained_parent` is `cfg(all(test, unix))` (27-44). Production caller: git/resolution/acquisition.rs:179,285.

### repository.rs (298)
- Purpose: `VerifiedGitRepository` holds capability handles and `Metadata` identities of cache parent, entry, `repository`, `objects`.
- Shape: struct (48-60, all fields `pub(crate)`); `cache_git!` macro (62-78) defining `run_git`, `run_git_stdout`, `run_git_bytes_stdout` that wrap `verify_identity()` before and after; `open` (81-125); `verify_identity` (131-160); `verify_current` (162-200) = identity, `verify_cache_custody_from_open_root` over the whole entry, bounded read of `source.identity` and exact byte compare with `git_cache_metadata(...)`, `verify_git_repository_tree_from_open_root`, three `reject_retained_git_path` calls, `read_canonical_config`, identity again; `read_canonical_config` accepts exactly `GIT_CONFIG_SHA1` or `GIT_CONFIG_SHA256` (202-218); `restore_canonical_config` (220-232); `open_or_create_snapshots` (247-297).
- Same job: the repository subtree is walked twice per `verify_current` (custody walk at 167 covers the entry including `repository/`; second walk at 186). `verify_current` is called many times per resolution (inspect_verified_git_cache_entry at repository.rs:48-56,81,129,132 in resolution/, recorded_objects.rs:28,54).
- Fail-open: none seen. Note: `read_canonical_config` accepts either algorithm's config without tying it to the object format (recorded_objects.rs:29-38 does tie it for recorded revisions only).

### retry.rs (75)
`discard_incomplete_entry` (27-63): if `source.identity` exists (any file type, `Ok(_)` at 41) returns `Ok(false)`; if missing, custody-verifies the whole entry, re-checks identity, `make_open_tree_owner_writable`, `remove_open_dir_all`, fsync parent, returns `Ok(true)`. `may_preserve_cache` (65-75): `matches!` on 6 variants (`Git`, `GitTimedOut`, `GitOutputOverflow`, `GitResolutionTimedOut`, `GitResolutionCommandLimit`, `GitResolutionCapturedOutputLimit`) => true. Everything else (including `GitCleanupFailed`, `GitObjectInvalid`, `GitTreeInvalid`, `TooManyBytes`, `Io`) leads to invalidation (acquisition.rs:285-299) unless re-open succeeds... see acquisition.
- Fail-open: F5 `discard_incomplete_entry` treats any non-NotFound `symlink_metadata` success (including a symlink or directory named `source.identity`) as "complete" and does not remove the entry (41); traced: the next `VerifiedGitRepository::open` runs `read_bounded_cache_record_from_open_directory` (tree.rs:72) through `RecordFileRoot` (not read) which I infer rejects non-regular files; open for the exact behavior.

### snapshots.rs (55)
`RetainedGitSnapshots{path,entry,directory,identity}`, `verify_identity`, `publication_exists` (37-54; non-dir or symlink -> `GitCacheInvalid`). Same job as git/snapshot.rs:227-245 `snapshot_publication_exists` (identical match; one takes `kind`) — two copies.

---

# git/objects/

### identity.rs (114)
Object-ID spelling and hashing. `verify_git_object_identity` (10-29), `git_object_identity` (31-57: SHA-1 through `sha1_checked` with collision detection -> error "known SHA-1 collision attack" 61-66; SHA-256 through `sha2`), `git_object_algorithm` (70-79: `is_object_id` then length 40 => Sha1 else Sha256), `decode_git_object_id` (81-100), `is_object_id` = length 40 or 64 and all hex digits including uppercase (102-104), `git_object_invalid` (106). Hex digits are decoded via `crate::identity::hex_value`.
- Decides: algorithm keyed on string length. Uppercase hex is accepted by `is_object_id` but the computed id is lowercase so `verify_git_object_identity` compares `!= expected` (22) and rejects uppercase expected ids (fail-closed).

### authentication.rs (299)
- Purpose: in-process proof that listed trees/blobs hash to the declared ids.
- Shape: `verify_exact_git_revision` (44-55); `authenticate_git_commit` (57-72, runs `git cat-file commit <oid>` through `VerifiedGitRepository::run_git_bytes_stdout`); `authenticate_git_commit_payload` (74-107: first line must be `tree <oid>` equal to reported tree and same algorithm); `authenticate_git_tree` (109-115) = graph + payloads; `authenticate_git_tree_graph` (117-134) rebuilds nested `AuthenticatedGitDirectory{entries: BTreeMap<Vec<u8>, AuthenticatedGitTreeNode>}` from flat entries, hashes bottom-up, compares to the expected root; `authenticate_git_tree_payloads` (136-158) hashes each File/Symlink blob (Tree/Gitlink skipped, 143); `git_tree_entry_order` (275-299) sorts a tree entry as if suffixed with `/`.
- Cases by shape: fixed mode strings `160000`, `40000`, `100644`, `100755`, `120000` (184-208, 263); `insert_authenticated_git_components` errors on duplicate path (209-214), missing parent (218-223), path through a blob (224-232).
- Fail-open: F6 `verify_exact_git_revision` returns `Ok(())` when `requested_rev` is not an object id (48): a selector like `HEAD`/branch accepts whatever commit Git's `rev-parse` returns; authentication only proves the commit/tree bytes match their own ids (traced via acquisition/repository.rs:83-131). Stated by comment in git/mod.rs:13-24 ("authenticate Git objects in-process" — integrity, not ref choice).

### object_rows.rs (91), graph.rs (42)
`GitTreeEntry{relative_bytes, relative_path, oid, size, kind}`, `GitTreeEntryKind{Tree,Gitlink,File{executable,bytes},Symlink{target_bytes}}`, `GitBlobBytes{batch: Arc<Vec<u8>>, start, end}` (zero-copy slices of one batch response). `validate_blob_entry` (63-73) rejects `Gitlink` with `GitSubmodulesUnsupported{commit: Some(oid)}`; `validate_source_entry` (75-90) also rejects any path component equal ignoring ASCII case to `.gitmodules` with `commit: None` (keyed on spelling). graph.rs: `AuthenticatedGitTreeGraph::authenticate` (17-26) calls `authenticate_git_tree_graph`; `entry(exact_path)` binary search on `relative_bytes` (36-41), correct only if entries are sorted (tree.rs:181 sorts them).

### tree.rs (335)
- Purpose: parse NUL-delimited `git ls-tree --full-tree -r -t -l -z` and validate paths.
- Shape: `parse_git_tree_entries` (31, WholeTree), `parse_git_tree_graph_entries` (44, SelectedOnly with `max_entries = GIT_TREE_GRAPH_ENTRY_LIMIT`, `max_bytes = u64::MAX`, depth 256), `parse_git_tree_entries_with_policy` (66-205), `parse_git_blob_size` (207), `git_directory_paths` (214), `validate_git_path` (222-250), `validate_portable_git_component` (252-287), `git_path_from_bytes` (unix raw bytes 289-295; non-unix requires UTF-8 and swaps `/` for the host separator 297-304), `validate_git_symlink_target` (306-335).
- Cases by shape (fixed list): "unsupported Git tree entry" for anything but `(160000,commit,-)`, `(040000,tree,-)`, `(100644|100755|120000,blob,size)` (119-143). Header must have exactly 4 whitespace fields (90).
- Decides keyed on spelling: WholeTree scope rejects any gitlink (101-107) and any `.gitmodules` component case-insensitively (109-118) by collecting all edges then returning `GitSubmodulesUnsupported` (176-180); SelectedOnly scope skips both (stated by the module comment 7-9 and the `inert_gitlink_graph...` test, tests.rs:616-664). Path rules: rejects empty/leading/trailing `/`, backslash, `.`/`..`/empty component, component equal to `.git` ignoring case (237), control chars and `< > : " | ? *`, trailing `.` or space, Windows device stems `CON PRN AUX NUL COM1-9 LPT1-9 CONIN$ CONOUT$` (256-286). Parent edges must exist and be trees (182-203).
- Same job elsewhere: a Windows reserved-name list also appears in `omega-rust/omega/packages/build-output/src/portable_paths/mod.rs` (grep `CONIN`: 2 files; the other not read; suspected duplicate). Other source-path validators in this unit: `SourceRelativePath::parse` (identity/local.rs:47), `validate_repository_path` (identity/locator.rs:165), `excluded_from_capture` (tree/capture/traversal.rs:284). `.git` is matched case-insensitively here (237, 325) but exact-case in traversal.rs:290,322.
- Fail-open: F7 SelectedOnly mode does not reject `.gitmodules` or gitlinks (by design, stated). Reaching consumers: `open_catalog_inventory` filters gitlinks and does not call `validate_source_entry` (inspection.rs:43-47), so a catalog capture contains `.gitmodules` blobs (test tree_session.rs:107-120 asserts this).
- Unused: `git_directory_paths` used at git/snapshot.rs:386 (1 prod call).

### batch/mod.rs (16), batch/protocol.rs (156), batch/transfer.rs (155), batch/custody.rs (243)
- Purpose: one `git cat-file --batch` launch for all blobs, fed from a private request file.
- `git_batch_request_bytes` (protocol.rs:13-20) `oid\n` per blob/symlink entry; `git_batch_output_limit` (22-56) sums `<oid> blob <digits>\n<size>\n` per entry and fails `TooManyBytes` over `limits.max_bytes` (35); `assign_git_batch_output` (67-156) re-parses the response entry by entry requiring the exact header `"<oid> blob <size>\n"`, exact `size` bytes and a `\n`, rejects trailing bytes, validates symlink targets with `validate_git_symlink_target` (111-113), then stores `GitBlobBytes` slices over one `Arc<Vec<u8>>`.
- transfer.rs `read_git_blobs_batch` (39-68), plan 105-114 (`None` when no blobs, so no launch), execute 128-155. Test-only `read_git_blobs_batch_from_path` (70-103) is a second request-file mechanism using plain `std::fs::OpenOptions`.
- custody.rs `PendingGitBatchRequest` (31-38): file `.omega-cat-file-batch.<pid>.<seq>` created `create_new`, no-follow, mode 0600 inside the retained cache parent (41-103, up to 128 attempts); `verify_current` (117-131) and `verify_git_batch_request_identity` (160-194) compare name, opened file and retained identity, owner/mode, exact 0600 (unix), macOS ACL; `remove` (133-157); `Drop` removes only if identity still matches (196-231).
- Same job: the create-write-verify-run-verify-remove-reconcile sequence is written twice: transfer.rs:49-67 and availability.rs:51-79 (suspected from reading both; identical shape). Staging-name allocation loop with 128 attempts and `STAGING_SEQUENCE` repeats at snapshot/publication.rs:56-87.
- Fail-open: none seen. `unreachable!("only blobs can receive payload bytes")` at protocol.rs:137 (guarded by `carries_blob` filter).

### batch/availability.rs (118), availability/protocol.rs (70), availability/tests.rs (260)
`probe_exact_git_object` (43-79) runs `git cat-file --batch-check` with the oid in a request file; `ExactGitObjectAvailability{Present{kind,size}, Missing}`; `ExactGitObjectKind{Commit,Tree,Blob,Tag}`. `protocol::response` (12-70) is a strict parser: success status, empty stderr, one line ending in `\n`, `<oid> missing` or `<oid> <commit|tree|blob|tag> <canonical decimal u64>`. `validate_requested_oid` requires a full lowercase id (81-90). "Absence is only accepted from a clean zero-exit `missing` line" (availability.rs:4-6). Tests (availability/tests.rs): every truncation of a valid line errors (63-67), 14 malformed lines, offline probe uses exactly one launch (165-200), launch/timeout/output limits are errors not absence (202-240), detached repository custody rejected before launch (242-260, ignored on Windows).
- Unused: `Present.size` is never read in production (`recorded_objects.rs:72` matches `{kind, ..}`; sizes read only in tests); `ExactGitObjectKind::Tag` is produced by the parser but production only compares kinds to `Commit`/`Tree`.
- Fail-open: none seen; `Missing` requires the exact line.

### inspection.rs (150), projection.rs (367)
- inspection.rs: `AuthenticatedGitTreeInspection{graph}` (27-79) with `open_catalog_inventory` (33-49), `open_regular_files` (55-67), `project` (69-78); `inspect_git_tree` (81-93, whole-tree: `list_tree` -> `parse_git_tree_entries` -> `authenticate_git_tree_graph` -> `read_git_blobs_batch`; blobs are not hashed here), `inspect_git_tree_graph` (107-118, SelectedOnly graph), `list_tree` (133-150), `validate_tree_oid` (120-131).
- projection.rs: `GitTreeProjectionRequest{declaration_paths, member_tree_path}`; `GitTreeProjectionPlan::from_graph` (113-151) selects declarations and the member subtree, `open_and_authenticate` (153-201) batch-reads only selected payload rows, `authenticate_git_tree_payloads` then member `authenticate_git_tree`; `select_regular_files` (204-231) is the same selector with byte/entry budgets used by `open_regular_files`; `project_member` (280-348) strips the member prefix after `validate_source_entry` on each original entry, applies depth/entry limits, then `authenticate_git_tree_graph` on the projected rows (346). `validate_request_paths` (233-252): unique declaration paths, `validate_git_path` with depth up to 256.
- Same job / repeated work: whole-source graph is authenticated twice (inspection.rs:90 and `preflight_git_snapshot` -> `authenticate_git_tree` at git/snapshot.rs:251). Member subtree: `authenticate_git_tree_graph` (projection.rs:346), `authenticate_git_tree` (190), and again in `preflight_git_snapshot` (snapshot.rs:251) so member payloads are hashed at 165, 190 and 251.
- `.expect(...)` at projection.rs:135,181,221 depend on the selected paths coming from the same graph.
- Fail-open: none seen. Empty `member_tree_path` selects the whole repository (285-287): traced to `select_regular_files` using `Vec::new()` at 209 and `materialize_member` requiring a real path.

### objects/mod.rs (24), objects/tests.rs (777)
mod.rs re-exports. tests.rs: batch launch counts (25-114), request custody (120-193), batch parser (196-259), selective projection (279-470), graph forgery (473-502), authentication (505-558), SHA-1 collision vector (561-580; the hex vector from `sha1-checked` is embedded), preflight before staging (583-613), inert gitlink graph (616-664), `.GiTmOdUlEs` member prefix rejected (667-681), metadata budget does not widen selected-source budget (683-746), portable paths (749-777). Windows `ignore` on one test (116-119).

---

# git/request.rs (317)
- Purpose: validated `GitSourceRequest` (locator, canonical identity, revision, lineage, transport).
- Shape: struct (21-29) fields `requested_locator`, `fetch_locator` (set to the raw locator, 142), `locator_identity` (canonical, 139), `requested_revision`, `lineage`, `execution_transport`. `GitExecutionTransport{Https, Ssh, File}` — `File` exists only under `cfg(any(test, feature = "test-fixtures"))` (35-36); `GitTransportProfile{Https,Ssh,TestFile}` is always compiled and public (39-44). `new` (117-148), `validate_git_revision` (239-263), `canonical_git_locator` (265-317), `for_local_test_repository` (178-201, gated).
- Decides: locator length, `trim() == locator`, parsed by `SourceLineage::git_with_transport`; transport from locator form (https -> Https; ssh url and scp -> Ssh, 57-62). Revision: non-empty, <= 1024 bytes, no leading `-`/`/`/`.`, no trailing `/`/`.`, no `..`, `//`, `@{`, no component starting `.` or ending `.lock`, bytes only alnum and `/ . _ -` (248-261). Canonical locator rebuilt from lineage; GitHub lowercased `https://github.com/<owner>/<repo>.git`.
- Same job: `cache_tag` (64-71) and `allowed_protocol` (73-79) and `profile` (89-96) are three matches over the same enum.
- Unused: grep shows `test-fixtures` feature defined at `omega-rust/omega/packages/source-acquisition/Cargo.toml:9` and no other Cargo.toml mentions it (grep `test-fixtures` over `**/Cargo.toml`: 1 hit), so `GitExecutionTransport::File` and the file-protocol allowance exist only in this crate's own `cfg(test)`.
- Fail-open: F8 abbreviated hex strings (e.g. `abc123`) pass `validate_git_revision` as selectors (alnum) and are not `is_object_id`, so they are treated as ordinary selectors (traced: acquisition.rs:240-241 and `verify_exact_git_revision` no-op). They resolve however Git resolves them; no exactness is enforced. Also `fetch_locator` keeps the caller's spelling while the cache key uses the canonical form, so two spellings share one cache entry and the fetch uses whichever spelling created it (inferred from request.rs:141-142 + acquisition.rs:136-139,215).
- Test coverage: git/tests/request.rs (121) checks case folding, HTTPS vs SSH lineage equality but distinct cache identity, 16 bad ports, 6 insecure forms, length and refspec shapes.

---

# git/resolution/

### mod.rs (99), network.rs (29), pinned_source.rs (30), exact_revision.rs (43), materialization.rs (37)
mod.rs: lifecycle doc (1-26), `pub use` of session/pin/exact/workspace functions, `resolve_git_source_in_lane` (69-82, `GitRevisionSelection::Ordinary(None)`), test-only `resolve_git_source` (85-99). network.rs: `bounded_git_fetch_arguments` -> `fetch --quiet --depth=1 --no-tags --no-recurse-submodules --filter=blob:limit=<max_bytes+1> -- <locator> <rev>` (7-29). pinned_source.rs: `resolve_git_source_from_pin_in_lane` (16-30). exact_revision.rs: `GitExactRevisionAcquisition{Offline,AllowFetch}`, `resolve_git_source_at_revision_in_lane` builds a `RecordedGitRevision` (35). materialization.rs: `GitMaterializedSource<Evidence>` and `materialize_whole_git_source` = `inspect_git_tree` + `resolve_git_snapshot`.
- External callers (grep, excluding this crate): `resolve_git_source_in_lane` 1 (package-manager/.../inspect_source/execution.rs:29); `resolve_git_source_at_revision_in_lane` and `GitWorkspaceSession` in package-manager (git/recorded.rs:13,61; workspace_acquisition.rs:241,251); `resolve_git_source_from_pin_in_lane` 0 external hits in the combined grep except package-manager `acquisition.rs` (3 hits for the group of names); I did not isolate it.

### selection.rs (131)
`GitRevisionSelection<'a>{Ordinary(Option<&GitAcquisitionPin>), Recorded(&RecordedGitRevision)}` (57-60). `RecordedGitRevision{commit,tree,algorithm,acquisition}` with `new` (26-46: same algorithm for commit and tree; `verify_exact_git_revision(request rev, commit)`). Methods: `validate_request` (63-82, only checks a pin matches the request), `expected_commit/tree` (84-96), `permits_new_acquisition` (98-105: Ordinary with no pin, or Recorded+AllowFetch), `known_object_algorithm` (107-112), `offline_unavailable` (114-123), `fetch_revision` (125-130: recorded commit id, or the authored selector). This enum is the single place that decides whether transport is allowed (comment at git/resolution/mod.rs:16-26); users are acquisition.rs, repository.rs, tree_session.rs, workspace_member*.rs (6 files).

### acquisition.rs (349)
- Purpose: open or create the cache entry under a lock, verify it, run inspection, then issue.
- Shape: `resolve_git_source_from_retained_cache` (44-105, generic over `Evidence`, `PlannerError`), `with_git_repository_from_retained_cache` (107-321), `resolve_whole_git_source_from_retained_lanes` (323-342), `verify_retained_lane_custody` (344-349).
- Decides (acquisition.rs:107-321): entry exists? (`retained_cache_directory_exists`, 144); if exists and selection permits new acquisition, discard an incomplete entry (153-165); if exists, open `VerifiedGitRepository` and on any failure invalidate then return the reconciled failure (167-195); if absent: pinned `Ordinary(Some)` -> `GitCacheInvalid("pinned Git acquisition cache entry is absent")` (197-203), offline recorded -> `offline_unavailable` (204-206), else create (207-226). Fetch decision passed to `inspect_verified_git_cache_entry`: `matches!(Ordinary(None)) && (!cache_entry_existed || !is_object_id(requested_rev))` (240-241). After failure: preserve the cache if `may_preserve_cache(&error)` and a fresh `VerifiedGitRepository::open` succeeds (261-275); preserve for `Recorded` + `GitExactRevisionUnavailable` (276-284); otherwise invalidate (285-299).
- Same job elsewhere: error reconciliation matching `GitWorkspaceProjectionError::{Source,Planner}` is repeated at 254-305 and 308-320.
- Fail-open: none seen. `unreachable!()` at 295 (`namespace_result` only built as `Source`).

### repository.rs (resolution/, 134)
`inspect_verified_git_cache_entry` (28-134, `#[allow(clippy::too_many_arguments)]`, 12 args). Opens the repository; fetch decision by selection (58-69): `Recorded` -> `recorded_revision_needs_fetch`; `Ordinary(None)` with an exact id and no forced fetch -> probe object, fetch only if `Missing`; `Ordinary(_)` -> caller's flag. Fetch: save canonical config, run `fetch` args, then **always** restore config (`restore_canonical_config`, 71-79) except a `GitCleanupFailed` returns early (75-77). Then `rev-parse --verify <rev>^{commit}` where `<rev>` is expected commit, `FETCH_HEAD` or the requested rev (83-97), `verify_exact_git_revision`, compare to pin/recorded commit (100-109), `rev-parse <commit>^{tree}` (110-117), compare tree (119-128), `authenticate_git_commit`, then the caller's `inspect` closure.
- Fail-open: none seen; every mismatch returns `GitObjectInvalid`.

### recorded_objects.rs (78)
`recorded_revision_needs_fetch` (22-62): verify repo, config bytes must equal the config for the recorded algorithm, probe commit; if present authenticate commit against recorded tree (so a wrong expected tree fails closed, not by fetching); probe tree; both present -> `Ok(false)`; else Offline -> `GitExactRevisionUnavailable`, AllowFetch -> `Ok(true)`. `present` (64-78): wrong kind -> `GitObjectInvalid`.

### issuance.rs (127)
`finalize_git_resolution` (22-64): re-verify lock identity, cache root custody, entry custody (`verify_git_cache_custody`, walk), `verify_pending_git_snapshot` (re-captures the published snapshot and compares to `pending.local`, 83-100), then again lock/root/entry custody, `validate_pending_git_request` (compares request fields with `pending`, 66-81), `validate_pending_git_source_custody` (102-127), builds `GitRetainedStorageCustody` and checks `validate_git_retained_storage_custody` (observations/storage.rs:62-75).
- Judgement: all comparisons are on equality of strings/values copied earlier in the same call chain (acquisition.rs:79-93 builds `pending` from `request`, then 98 validates it against the same `request`); in production these cannot differ; tests construct divergent values (resolution/tests/resolution.rs:52-85). Self-check, not an independent check. Similar self-check: `validate_git_retained_storage_custody` re-derives `entry_limit`, `byte_limit`, `depth_limit` from the same constants/limits used a few lines earlier (storage.rs:46-75) and only adds `entry_count > 0` and `<=` limits.
- Fail-open: none seen.

### tree_session.rs (270), tree_session/inventory.rs (153)
- Purpose: staged workspace discovery so the build system can read the root catalog, request member declarations, and later materialize one member without re-authenticating the whole graph.
- Shape: `GitWorkspaceSession{request, commit, tree, inspection, root, limits, declaration_limits}` (30-39) with `open` (48-62), `open_at_revision` (64-81), `acquire` (83-125; limits are `compiler_bounded` at 90), `acquisition_pin` (127-133), `open_members` (151-188, re-opens the cache with `Recorded(Offline)` built from the stored commit/tree, 159), `recorded_revision` (190-209); `GitWorkspaceCatalog{session, members}` (41-45) with `materialize` (220-262: selected path must be a member of the roster (225-234), else `GitTreeInvalid`).
- inventory.rs: `capture_inventory` (31-152): re-opens with `Recorded(Offline)`, `catalog_limits` = 262,144 entries / 512 MiB / depth 256 (39-43), routes under `workspace_members` lane child `catalog-<namespace>/<commit>` (49-52) only as an identity, `open_catalog_inventory`, filters entries whose path has an ancestor excluded by `excluded_from_capture(LocalPackage, ...)` (66-81), builds `SourceIdentityHasher`, builds `CapturedSourceTree`, canonicalizes every symlink inside the tree (128-134).
- Same job: duplicate of the planner-based flow `project_git_workspace_member` (workspace_member.rs:134-191): both run `inspect_git_tree_graph` -> `declarations::open_root` -> `declarations::open_members` -> `materialize_member`. The planner flow has no production caller outside tests (see workspace_member.rs entry). The identity loop in inventory.rs:85-105 repeats `SourceIdentityHasher` dispatch also at git/snapshot.rs:262-278, local/staging.rs:152-164, tree/capture/mod.rs:109-125 (grep `SourceIdentityHasher::new`: exactly these 4 sites).
- Fail-open: F10 `unreachable!("authenticated source projection rejects gitlinks")` (inventory.rs:103,121) is justified by the Gitlink filter in `open_catalog_inventory` (inspection.rs:43), not by a rejecting projection as the message says (traced).

### workspace_member.rs (242), workspace_member/declarations.rs (144), workspace_member/recorded.rs (51)
- workspace_member.rs: planner-driven flow. `resolve_git_workspace_member_from_pin_in_lanes` (68-94, `pub`), `resolve_git_workspace_member_at_selection` (97-132), `project_git_workspace_member` (134-191): discover members, open declarations, call `planner.select_member`, check the selection is in the discovered set (171-178), `materialize_member` (193-242): `inspection.project(...)` -> `into_member`, `GitWorkspaceProjectionCustody::new`, scope the lane as `member_lane.retain_child(<cache entry name>)` (222-226), `publish_git_member_snapshot`.
- declarations.rs: `open_root` (13-43) reads exactly `build.omg` at the repo root; `open_members` (45-115) reads `<member>/build.omg` for each member under member-count and byte budgets (total, per-declaration), rejects duplicate member paths (69-74), uses `.expect("validated workspace member paths are UTF-8")` (104); `validate_workspace_declaration_limits` (117-134).
- recorded.rs: `resolve_git_workspace_member_at_revision_in_lanes` (22-51) applies `compiler_bounded` (47).
- Cases by shape: the declaration file name is the hard-coded `build.omg` (declarations.rs:21,75).
- Unused/one-caller: grep over omega-rust for `GitWorkspaceProjectionPlanner|resolve_git_workspace_member_from_pin_in_lanes|resolve_git_workspace_member_at_revision_in_lanes|GitWorkspaceProjectionResult|GitWorkspaceSelection` finds only source-acquisition (src, tests, README.md) and package-manager files that match only by longer names (`GitWorkspaceSelectionEvidence/Plan/Error`); package-manager exact-name grep for the three functions and the planner trait returned no matches. So the pub planner API (`GitWorkspaceProjectionPlanner`, `GitWorkspaceSelection`, `GitWorkspaceProjectionResult`, the two `*_in_lanes` functions) has zero callers outside this crate; `resolve_git_workspace_member` is `cfg(test)` (workspace_member.rs:39-40; used at git/tests/workspace.rs:82). Package-manager uses `GitWorkspaceSession` instead (workspace_acquisition.rs:241,251; git/recorded.rs:61).
- Fail-open: F9 `resolve_git_workspace_member_from_pin_in_lanes` passes `limits` to `resolve_git_source_from_retained_cache` unbounded (84-93), whereas the sibling `..._at_revision_in_lanes` calls `limits.compiler_bounded()` (recorded.rs:47) and the whole-source path does at acquisition.rs:334. Traced: no external caller (grep above) so only tests/pub API reach it; a caller passing `max_bytes > 512 MiB` would exceed `SOURCE_BYTE_ABSOLUTE_LIMIT` on that function only. Open for the effect on cache-custody ceilings (`git_cache_custody_byte_limit` caps at 1 GiB regardless).

### resolution/tests/{mod,fetch,resolution}.rs (21,101,167)
fetch.rs: with a file transport and `uploadpack.allowFilter`, a 4096-byte blob above `max_bytes=1024` is not fetched and the resolution fails `GitTreeInvalid`; asserts `source.identity` stays and `snapshots/` is absent. resolution.rs: exact commit resolve, pending-vs-request mismatch rejections, snapshot drift, remote object-format parser (3 invalid shapes), tree prefix ordering (`name/` vs `name.ext`). mod.rs imports.

---

# git/snapshot.rs (443), git/snapshot_metadata.rs (66)
- Purpose: materialize an authenticated entry list into `tree-<oid>/source` with metadata, or verify an existing publication.
- Shape: `resolve_git_snapshot` (53-76: preflight, `open_or_create_snapshots`, existence, `resolve_git_snapshot_in_collection`); `publish_git_member_snapshot` (78-115: takes a `CacheEntryLock::acquire_local_from_parent` `tree-<oid>.lock` in the workspace-members lane, kind `LocalSnapshot`); `resolve_git_snapshot_in_collection` (117-225): if publication exists, drop blob payloads and `verify_git_snapshot`; else stage, write dirs/files/symlinks, drop payloads, `capture_exact_materialized`, compare to `expected` (file_count, byte_count, content_identity), write `snapshot.identity`, `make_open_snapshot_read_only`, recapture and compare again (211-220), drop handle, `pending.publish`, `verify_git_snapshot`; `preflight_git_snapshot` (247-254) = `authenticate_git_tree` + destination containment + identity; `verify_git_snapshot` (334-376); `verify_captured_git_snapshot_shape` (378-443).
- snapshot_metadata.rs: `GitSnapshotMetadata{tree,file_count,byte_count,content_identity}`, serialization `GIT_SNAPSHOT_POLICY` + LE frames, strict parser (30-66).
- Decides: keyed on `tree-<tree oid>` directory name (129, 62). Reuse is always followed by a full re-hash of the published tree (`capture_local_source_from_open_root` at 358). `entry.validate_source_entry()` is called per entry at 158, 263, 384.
- Same job: `snapshot_publication_exists` (227-245) vs git/cache/snapshots.rs:37-54 (copy); `PendingMaterializedSnapshot::create_from_open_parent` use here vs local/snapshot/materialize.rs:62 (shared). Hash-and-verify both before publish and after (two captures at 192 and 211, plus verify at 224) — three full captures per new publication.
- Windows inference (not run): `set_open_snapshot_file_mode` non-unix ignores `executable` (snapshot/construction.rs:167-176) and `capability_metadata_is_executable` returns false off Unix (tree/filesystem.rs:223-226), while the expected identity includes `executable` from the Git mode (snapshot.rs:269-271); a tree with a `100755` blob would therefore mismatch at snapshot.rs:193-200 on Windows. The only test that covers modes is `cfg(unix)` (git/tests/snapshot.rs:140-199). Marked inferred.
- Fail-open: none seen.

---

# git/workspace/{mod,workspace_declaration}.rs (10, 219)
Coordination types: `GitWorkspaceDeclarationLimits{maximum_members, maximum_declaration_bytes, maximum_total_declaration_bytes}` (15-46), `GitWorkspaceDeclaration{member_path: Option<SourceRelativePath>, repository_path, object_id, bytes}` (48-54; `root()` and `member()` constructors are `pub(crate)`), `GitWorkspaceProjectionCustody{root_declaration, member_declarations, selected_member_path, selected_member_tree}` (56-94), trait `GitWorkspaceProjectionPlanner{discover_members, select_member}` (132-146), `GitWorkspaceSelection`, `GitWorkspaceProjectionResult`, `GitWorkspaceProjectionError{Source,Planner}` (195-199). No decisions in code. Callers of the planner trait: none outside the crate (see workspace_member.rs entry).

---

# git/tests/ (all test code)
- mod.rs (47): `fixture_storage` builds a `SourceResolver` with the system Git as explicit.
- request.rs (121): see request.rs entry.
- exact_revision.rs (111) + exact_revision/{failures 63, fixtures 125, retry 98}: SHA-256 cold fetch without discovering the authored selector (17-40), offline misses preserve a healthy cache (43-88), warm recovery enforces current limits (`max_bytes: 0` -> `TooManyBytes{limit:0}`, 90-111), wrong tree/algorithm rejects before fetch, authored full commit must equal the persisted commit, transport failures retry without manual cache edits, symlinked entry not removed (retry.rs:87-98, unix).
- exact_workspace.rs (96) + {fixtures 157, planner 56, submodules 72}: historical declarations survive branch drift offline; planner selection outside discovered members rejected with exact `GitTreeInvalid` message; warm declaration limits re-applied (`GitWorkspaceDeclarationLimits::new(8,8,8)` -> `TooManyBytes{limit:8}`) before the planner runs; `.gitmodules` or gitlink inside the selected member rejected, root-level inert edges not (submodules.rs:53-72).
- root_pin.rs (165): pin vs different revision/locator rejected before transport (error message pinned), absent cache with a pin errors without recreating, corrupt cache or changed snapshot with a pin errors `GitCacheInvalid` and removes metadata (153-164).
- snapshot.rs (327): member snapshot reuse rejects tamper (unix), replaced workspace lane rejected (Windows ignored), modes/symlink spelling preserved (unix), reuse rejects content with forged matching metadata (unix, 203-245), batch failure precedes staging, `.gitmodules` rejects and prior snapshot is untouched.
- tree_session.rs (306): catalog inventory keeps helpers and excludes `omega.lock`/`build/` (17-79), gitlinks stay inert in catalog but selected member refuses them (82-133), session pinned across HEAD movement, open at recorded revision offline, missing/duplicate/excess member requests refused.
- validation.rs (105): native separators, 12 unsafe paths rejected, Windows-ambiguous symlink targets rejected, directory entries count against `max_entries`.
- workspace.rs (128): planner-driven member projection never materializes unrelated payloads.

---

# identity/

### mod.rs (121), digest.rs (15)
`domain_digest!` macro (28-56) generates 32-byte digest newtypes with `derive`, `parse_hex`, `to_hex`, `digest`; instantiated for `WorkspaceLineageIdentity`, `ExternalSourceContext`, `SourceContentDigest` (local.rs:153-155). `hash_field` (73-76: BE u64 length + bytes into a `sha2::Sha256`), `hash_optional_field` (78-86), `decode_hex` (88-99), `hex_value` (101-108, accepts uppercase), `encode_hex` (110-118, lowercase). digest.rs: `append_framed_bytes` (LE u64 length + bytes), `format_sha256` and `format_hex` — both only call `super::encode_hex` (9-15), i.e. two names for the same function.
- Same job elsewhere: three length-framing schemes in this crate: LE `append_framed_bytes` (digest.rs:4), BE `hash_field` (mod.rs:73), `DomainDigestEncoder::bytes` (used as `hasher.bytes` e.g. mod.rs:36, tree/identity.rs:38; implementation in `optimization_core`, not read). Also local/observation.rs:60 defines a second `hash_field` that is a forwarder to `hasher.bytes` (grep `fn hash_field`: omega/packages has 4 definitions: build-output/staged_output_tree.rs:515, package-manager/declarations/identity.rs:97, identity/mod.rs:73, local/observation.rs:60; only the last two are in this unit). `encode_hex` also exists at `omega-rust/omega/packages/package-manager/src/resolution/graph/subject/encoding/framing.rs:15` (grep `fn (encode_hex|decode_hex|hex_value|format_hex)`; not read, suspected duplicate).

### git.rs (249), locator.rs (205), local.rs (177), resolution.rs (176), error.rs (77)
- git.rs: `SourceLineage{GitHub,GitLab,Git,Workspace,ExternalLocal}` (25-32). `git_with_transport` (47-58) dispatches on `parsed.host == "github.com"` / `"gitlab.com"` (50-55) — keyed on host spelling after lowercase in locator.rs:151. GitHub: no port, no https user, ssh user must be `git`, exactly `owner/repo`, `.git` stripped, owner/repo validated and lowercased (109-144). GitLab: same rules, path case preserved, at least two components (161-186). Generic: stores parsed fields (203-211). `hash_canonical` (69-99) uses tags `github`, `gitlab`, `git`, `workspace`, `external-local`. `git_recovery_owned_bytes` (36-41).
- locator.rs: `ParsedGitLocator::parse` (17-33): rejects `?` and `#`; schemes only `https` and `ssh` (case-insensitive), else `UnsupportedGitProtocol`; scp-like `user@host:path` (75-94); `parse_url` rejects userinfo for https, requires user for ssh, user must be alnum `-` `_`, no `:`; ports parsed canonically (no leading zero, nonzero u16, 101-114); host labels alnum/`-` lowercased (136-152); path components alnum `.` `-` `_`, no `%`, no `\`, no `.`/`..` (165-188).
- local.rs: `SourceRelativePath::parse` (47-62): bytes alnum `-` `_` `.`, no empty/`.`/`..` components; `.git`, `CON` etc. are accepted by this type. `ExternalLocalLineage::canonicalize` (87-115) uses `std::fs::canonicalize`, requires UTF-8; `from_canonical_path` (118-142) validates lexically without touching the filesystem. `WorkspaceLineageIdentity::from_root_source` (162-170) rejects `Workspace` roots.
- resolution.rs: `ImmutableSourceResolution{Git{commit,tree,content},Workspace{content},ExternalLocal{content}}` (24-36); `git` requires equal algorithms and derives `content` from the tree id only (48-58, 90-100, test at identity/tests.rs:193-206 asserts two different commits with one tree give equal content). `matches_lineage` (77-87) compares only the family (Git/Workspace/ExternalLocal) — grep shows it is used at package-manager/.../custody.rs:101, graph/mod.rs:53,64, graph/subject/validation/source.rs:90. `GitCommitId/GitTreeId::parse_hex` accept uppercase and store bytes (`to_hex` lowercases).
- error.rs: 16 variant `IdentityError`.
- Fail-open: F11 `matches_lineage` only checks family, so a `Git` resolution paired with a *different* Git lineage passes this predicate; what else binds them is in callers (package-manager, not read). Open.
- Unused/one-caller check (grep outside this crate): `recovery_owned_bytes` family used at package-manager/.../graph/subject/validation/source.rs:19 and text/source.rs:66,238,248; `from_canonical_path`, `hash_canonical` have hits within the 29-hit combined count; I did not isolate them.

### identity/tests.rs (230)
Normalization tests: GitHub/GitLab spellings, credentials/ports/bad namespaces rejected (10 bad locators), path case rules (GitLab preserved), generic rejections (8), `SourceRelativePath` bad forms incl. non-ASCII, workspace lineage, external local lineage, recovered path separators, content digest from tree only, ids typed/canonical (uppercase accepted and lowered at 217-222).

---

# local/

### mod.rs (42), operations.rs (185)
operations.rs: `resolve_local_source` (41-47, pub, applies `compiler_bounded`, LocalPackage policy); `capture_local_source_input` (49-84) turns a `CapturedLocalTree` into `source::CapturedSourceTree` in memory (symlink target bytes -> `PathBuf`, 87-106); `verify_package_source_snapshot` / `capture_verified_package_source_snapshot` (108-154: `verify_open_snapshot_tree_modes` then exact capture, compare `SourceContentDigest::derive(content_identity)` to `expected`); `resolve_local_source_snapshot_in_lane` (156-172); test-only `resolve_local_source_snapshot` (174-185).
- External callers (grep, outside this crate): `capture_local_source_input` 6 (compiler tests/examples, package-manager acquisition.rs:439,489, package-compilation lib.rs:97 and frontier.rs:207); `resolve_local_source` 2 (package-manager publication/checked.rs:128, inspect_source/execution.rs:15). `verify_package_source_snapshot`, `capture_verified_package_source_snapshot`, `recover_cached_local_source_in_lane`, `resolve_local_source_snapshot_in_lane`, `stage_local_source_replacement_in_lane` appear in package-manager (review/candidate/custody.rs:2, review/audit/snapshot.rs:3, resolution/source/local/recovery.rs:3, operations/stage_build_edit.rs:2, resolution/source/acquisition.rs:3) in the group grep (22 hits total in 13 files); per-name counts not isolated.
- mod.rs documents origin-bound addressing `(canonical_live_root, SourceContentDigest)`.

### observation.rs (93), resolution_observations.rs (109)
`issue_local_source_resolution_observation` hashes domain tag, schema 3, policies, requested / canonical / publication / snapshot roots (via per-OS `hash_path`: unix raw bytes, Windows length-framed UTF-16 units, 68-93), counts, content identity, limits, custody id, and two literals into a SHA-256 hex id. resolution_observations.rs: `CapturedLocalSource{requested_root, normalized: ResolvedSourceTree, tree: Arc<CapturedSourceTree>}` (derives `compiler_storage::Record`), `LocalSourceResolutionObservation{schema_version, identity, custody_identity}` (fields `pub(super)`), `ResolvedLocalSnapshot` built by `from_issued_parts`. The observation is issued only in `local/snapshot.rs:248-254` (grep'd by reading).

### snapshot.rs (365), snapshot/address.rs (74), snapshot/materialize.rs (154)
- snapshot.rs: `publish_local_snapshot_in_lane` (96-121), `publish_local_tree_in_lane` (123-146: topology check `cache_dir.starts_with(live_root) || live_root.starts_with(snapshots)` -> `LocalSnapshotCacheOverlapsSource` 296-310, `lane.retain_child("local-snapshots")`), `publish_local_snapshot_in_retained_collection` (148-212: lock `source-v2-<digest>.lock`, if publication dir exists `verify_local_snapshot` else materialize), `finalize_local_tree` (214-239: outer custody, `verify_requested_local_root` (canonicalize requested root equals captured root, 264-275), `verify_local_cache_custody`, `same_source_identity` on file_count/byte_count/content_identity (361-365), custody again), `verify_live_source_unchanged` (349-359: re-runs `resolve_local_source` and compares).
- address.rs: `LocalSnapshotAddress::new` = SHA-256(`LOCAL_SNAPSHOT_CUSTODY_POLICY`, raw OS bytes of canonical origin, content digest) -> `source-v2-<hex>` (14-40); legacy `source-<hex>` helpers (46-74); `is_legacy_snapshot_name` requires 64 lowercase-hex digits.
- materialize.rs: `materialize_pending_local_snapshot` (78-154): write all entries, recapture, compare, `verify_live_source_unchanged` before writing metadata (136), read-only, drop dir handle, publish, `verify_local_snapshot`.
- Same job: `publication_exists` match repeated at snapshot.rs:170-180 (4th copy, see custody/tree.rs entry). Test-only parallel path `publish_local_snapshot` (46-94) with `canonicalize_prospective_path` (312-347) duplicates `publish_local_tree_in_lane` for tests.
- Fail-open: none seen. `finalize_local_tree` compares the published snapshot with the proposed tree and not with the live tree at the end (live re-check is `verify_requested_local_root` only checks the path still canonicalizes to the same root, 225, 237).

### recovery.rs (245), recovery/timings.rs (105), recovery/tests.rs (240)
- recovery.rs: `recover_cached_local_source_in_lane` (37-50) -> `recover_bounded` (52-171): origin must be absolute with no `.`/`..` component (60-69); open `local-snapshots` no-follow, `NotFound` -> `Ok(None)` (72-76); direct `source-v2-<digest>` lookup (84-125, a hit consumes one entry from the budget so `maximum_entries == 0` fails, 96-101); on miss scan up to `maximum_entries` (default `CACHE_CUSTODY_ENTRY_LIMIT`) legacy `source-<digest>` entries (130-166), skipping non-matching names/types and unreadable routing identities (`let Ok(identity) = ... else continue`, 149-151); candidate is locked, custody-verified and `verify_local_snapshot` re-hashed (`verify_selected_snapshot` 185-224); `verify_collection` (226-242).
- timings.rs: counters enabled only when env `OMEGA_REVIEW_TIMINGS` is set; `Drop` prints one `eprintln!` line.
- Fail-open: F12 a corrupt or unreadable *legacy* candidate is skipped (149-151) instead of rejecting; result is `None` (cache miss), not acceptance. Traced.
- tests.rs: historical snapshot survives changed/deleted live source, absent collection creates nothing, byte-identical content does not cross origins, corrupted snapshot rejected without repair, lookup limit 0 and `max_bytes: 1` rejected, symlinked collection not followed (unix).

### staging.rs (170)
`stage_local_source_replacement_in_lane` (77-106) and `replace_file` (108-170): captures the live package, finds the entry by exact `relative_bytes`; entry must be a captured regular file (else `LocalSourceReplacementInvalid`, 125-128); `expected_sha256` compared to `content_identity(bytes)` (130-133, mismatch -> `LocalSourceChanged`); new total bytes must fit `max_bytes` (134-140); rehash with `SourceIdentityHasher` over all entries (151-165); publish through `publish_local_tree_in_lane`. `StagedLocalSnapshot` accessors. Excluded paths (`.git`, root `build`, root `omega.lock`) are not in `captured.entries`, so replacing them reports "invalid" (tests 193-222).
- Same job: the per-entry identity loop duplicates tree/capture/mod.rs:109-125.

### local/tests/{mod,capture,limits,observation,path_policy,snapshots,staging}.rs (28,89,56,106,109,183,257)
capture.rs (unix): no-follow of replaced file/dir/root leaf. limits.rs: entry limits count directories, exclusions free. observation.rs: observation id binds request spelling and limits (alternate request spelling `a/../root` shares snapshot root but different observation id, 11-71), mismatched requested root at issuance rejected with `LocalSourceChanged`. path_policy.rs: non-UTF-8 names distinguished (non-Apple unix), symlink into `build/` rejected, absolute symlink target rejected even inside root, Unix socket rejected as `UnsupportedFileType`. snapshots.rs: empty dirs kept, live mutation during publish rejected and stage removed, reuse rehashes and rejects tampering (unix), root `build` kept in exact capture. staging.rs: staged replacement and later landing of the edit yields the identical content identity and snapshot reuse, modes 0555/0444 and symlink preserved (unix), stale digest, invalid targets, byte budget counts the proposed tree.

---

# observations/

### mod.rs (5), resolved.rs (209), storage.rs (75)
resolved.rs: `ResolvedGitSource` (21-37, 14 fields all `pub(crate)`), `GitAcquisitionPin` (39-48; `matches_request` compares locator spelling, lineage, locator_identity, transport profile and revision, 149-162), `PendingResolvedGitSource` (173-188), `from_issued` (cfg(test)). Two constructors for `GitAcquisitionPin`: `authenticated_revision` (51-66, used by `GitWorkspaceSession::acquisition_pin`) and `ResolvedGitSource::acquisition_pin` (69-79). storage.rs: `GitRetainedStorageCustody` plus `git_retained_storage_custody` (46-60) and `validate_git_retained_storage_custody` (62-75, returns bool). Fail-open: none seen.

---

# snapshot/

### construction.rs (176), publication.rs (167), metadata.rs (131), mod.rs (30)
- construction.rs: `open_or_create_snapshot_directory` (19-50: per-component `create_dir` ignoring `AlreadyExists`, then `open_dir_nofollow`; non-`Normal` components rejected), `write_snapshot_file_from_open_root` (52-91: `create_new`, no-follow, 0600 then `sync_all`, then final mode 0444/0555 on unix; readonly attribute elsewhere), `create_snapshot_symlink_from_open_root` (unix `symlink_contents`, non-unix requires UTF-8 and `symlink_file`, 93-152).
- publication.rs: `PendingMaterializedSnapshot{root,parent,directory,stage_name,kind,published}` (27-34) with `create_from_open_parent` (49-93, 128 attempts, `STAGING_SEQUENCE`), `publish` (101-158: compare retained handle to the named stage, `publish_cache_directory_from_open_parent`, then on non-Windows make root read-only, `verify_open_snapshot_tree_modes`, compare identity), `Drop` removes unpublished stages (161-167).
- metadata.rs: `local_snapshot_metadata` and strict `parse_local_snapshot_metadata` (28-69), `verify_local_snapshot` (71-118: bounded metadata read (512 bytes), identity must equal the cache key, modes verified, exact recapture, counts and identity compared), `local_snapshot_content_identity` (120-131).
- Same job: `GitSnapshotMetadata` parser (git/snapshot_metadata.rs:30-66) and `LocalSnapshotMetadata` parser (metadata.rs:36-69) are the same framing with and without a tree field; `verify_git_snapshot` (git/snapshot.rs:334-376) and `verify_local_snapshot` (metadata.rs:71-118) are the same verification with different record types. Comment at mod.rs:5-9 "Both Git materialization (`git`) and local-source snapshotting (`local`) share this custody machinery": users of `snapshot::{construction,permissions,publication}` outside snapshot/: git/snapshot.rs and local/snapshot/materialize.rs (+ git/cache/retry.rs for `make_open_tree_owner_writable`, local/operations.rs for `verify_open_snapshot_tree_modes`): consistent with the claim.
- Fail-open: F13 `verify_capability_snapshot_directory_mode` is `Ok(())` off Unix (permissions.rs:113-120); `verify_capability_snapshot_file_mode` off Unix checks only `readonly` (140-153). Stated in the header (permissions.rs:4-8 "(`0555` on Unix)") but a Windows snapshot directory that is writable is not detected (traced by reading).

### permissions.rs (443)
`verify_open_snapshot_tree_modes` (31-93) recurses and checks directory mode 0555 and file mode in `{0444,0555}` (unix) via `same_capability_file_identity` between `symlink_metadata` and opened metadata; symlinks are accepted without inspection (84). `make_open_snapshot_read_only` (156-211) sets file mode from `capability_is_executable` (`mode & 0o111 != 0`) — preserves any exec bit as 0555. `make_open_snapshot_root_publishable` is a 0700 chmod on macOS only (240-260). `make_open_tree_owner_writable` (322-373) and `remove_unpublished` (432-443) ignore errors (`let _ =`). Test-only helpers `make_snapshot_read_only`, `make_tree_owner_writable` (285-430).
- Fail-open: error-ignoring cleanup (`let _ = ...`, 327, 347, 366, 439) leaves state that a later verification would catch; no acceptance.

---

# source_resolver.rs (266), storage.rs (192)
- storage.rs: `RetainedStorageRoot::create_beneath` (21-52): base must be absolute, `create_dir_all(base)`, canonicalize, open no-follow, then for each of `CathedralOS/Omega/source/v1` create a private dir (ignoring `AlreadyExists`) and retain it with `CacheCustodyKind::Git` (35-45; the root path is always verified as `Git` kind, 67-73). `RetainedStorageLane{path,directory,kind}` (76-81): `create`, `retain_child`, `verify_path_identity`, Windows `verify_direct_child_identity`. `current_user_cache_base` per OS (147-170): Windows `LOCALAPPDATA`, macOS `$HOME/Library/Caches`, unix `XDG_CACHE_HOME` else `$HOME/.cache`; other OS error. Must be absolute.
- source_resolver.rs: `SourceResolver{root, git_sources, workspace_members, external_local_sources, primary_git: Option<PrimaryGitSelection>}` (25-32); lanes `git-sources` (kind Git), `workspace-members` and `external-local-sources` (kind LocalSnapshot) (123-127); `primary_git()` errors `GitExecutableUnavailable` when absent (71-75); `verify_path_identity` (77-92); `PrimaryGitChoices{explicit_git, excluded_controlled_roots}` (34-38); `create_beneath` builds excluded roots = cwd + storage root + operator list (103-119). Tests (139-266): Windows sibling-walk tests, replacement of root or lane path rejected (non-Windows).
- Callers outside crate: `SourceResolver::for_current_user`/`PrimaryGitChoices` used by `omega/application/cli/src/commands/packages.rs` and package-manager (grep: 13 hits in 5 files).
- Fail-open: none seen.

---

# test_support.rs (289)
Test-only fixture helpers (`cfg(test)`, lib.rs:62-63): temp roots, `resolve_git_source_from_hardened_base`, `primed_git_cache`, `create_git_source_with_format` (shells out to the host `git`), `run_test_git`, `shell_command` (unix), `change_macos_acl`. Uses plain `Command::new("git")` (139,156,203) rather than the sealed path.

---

# tree/

### tree/mod.rs (35), resolved_tree.rs (28), identity.rs (60)
resolved_tree.rs: `ResolvedSourceTree{root, file_count, byte_count, content_identity}` (derives `compiler_storage::Record`), alias `ResolvedLocalSource` (15), `VerifiedPackageSourceEntry{relative_path: Vec<u8>, kind}`. `file_count` counts files and symlinks (comment at 10). identity.rs: `SourceIdentityHasher` (domain `omega-source-tree-v4\0`, entry count first) with `add_directory(path, mode)`, `add_file(path, executable, bytes)`, `add_symlink`, `finish() -> (byte_count, hex)`. Users: 4 construction sites (see tree_session entry). Keyed on sorted relative path bytes.

### tree/filesystem.rs (263), filesystem/read_tests.rs (168)
- Purpose: capability-relative open/read primitives and drift comparison.
- Shape: `open_canonical_source_root` (29-40), `open_absolute_directory_nofollow` (42-82: opens the anchor with ambient authority and walks each component with `open_dir_nofollow`; rejects `..` and relative paths), `open_captured_directory` (84-97), `read_capability_file_bounded` (99-115) and `read_opened_file_bounded` (117-160: bounded reads with `remaining`, length equality, metadata unchanged before/after), `require_unchanged_entry` (175-214: file type, dev/ino (unix) or volume serial + file index (Windows; any `None` compares as changed), length, permissions, mtime, unix ctime/ctime_nsec), `capability_metadata_is_executable` (unix `mode & 0o111`, non-unix `false`, 216-226), `raw_os_bytes` (241), `IoResultAt` trait (252-260).
- Decides: identity-based; drift is equality of those metadata fields. Non-unix has no change-time comparison (201-202).
- Fail-open: F14 non-unix `capability_metadata_is_executable` always `false` (223-226): on Windows an executable local file is captured as non-executable, which is consistent for local identity but differs from Git mode identity (see git/snapshot.rs Windows inference). Not an acceptance path.
- read_tests.rs: growth in the remaining budget -> `LocalSourceChanged`, budget exhaustion stays `TooManyBytes`, unix mode changes rejected; Windows-only incomplete-metadata test; non-unix prints a SKIP line (161-168).

### tree/capture/mod.rs (168), traversal.rs (623), traversal_observations.rs (47)
- Purpose: bounded, drift-checked capture of a directory into entries and an identity.
- Shape: `capture_local_source` (42-54: canonicalize, require dir, open), `capture_exact_materialized` (56-67), `capture_local_source_from_open_root` (69-142: compare opening root handle with a fresh open of the same path, `visit_directory`, `verify_captured_tree`, closing root compare, sort entries by `relative_bytes`, hash); traversal.rs: `visit_directory` (51-185), `close_captured_directory` (187-218), `verify_captured_tree` (220-259), `excluded_from_capture` (284-293), `read_and_validate_symlink_target` (295-345), `push_entry` (351-366), `canonical_relative_path_bytes` (368-377). `SourceTreePolicy{LocalPackage, ExactMaterialized}` (traversal_observations.rs:43-47).
- How it decides: policy `LocalPackage` excludes names `.git` (exact), and at the root `build` (exact) and `omega.lock` (ASCII case-insensitive) (284-293, 347-349); `ExactMaterialized` excludes nothing. Listing is capped at `remaining entries + 4` at the root and `+1` otherwise for exclusions (72-77). File kinds: symlink (target must be relative, must `canonicalize` inside the root, must not resolve into `.git` component, root `omega.lock`, or root `build` under `LocalPackage`, 304-343), dir, regular file; anything else `UnsupportedFileType` (169). Executable bit from `mode & 0o111`.
- Cases by shape: the "+4" allowance (74) is hard-coded for the three root exclusions.
- Same job: the closing drift sweep in `close_captured_directory` and `verify_captured_tree` re-lists every directory twice (per directory at close and again from the root handle).
- Tests: `close_tests` (380-512) and `tree_tests` (514-623) at the bottom of traversal.rs cover added member, in-place rewrite with restored mtime (unix), removed root member, replaced nested directory; mod.rs:145-168 root handle for a different directory rejected.
- Fail-open: none seen. `read_and_validate_symlink_target` does not apply the exclusion checks for `ExactMaterialized` by design (policy check at 319, 333).

---

# Coverage

- Read in full: all 131 files, 19,993 lines (sum of the line-count column of unit_037.tsv; computed by adding the 131 numbers by hand).
- Skimmed: 0 lines.
- Not read: 0 lines.
- Method: Read tool, whole file per call (no offset/limit needed).

# Fail-open index (cross-reference)
F1 tree.rs:293-296 (traced, non-reachable in production). F2 git/cache/custody.rs:187-193 (open). F3 selection.rs:179 (traced low). F4 custody.rs:209,232 (traced/stated). F5 retry.rs:41 (open on RecordFileRoot). F6 authentication.rs:48 (stated). F7 objects/tree.rs:100-118 (stated). F8 request.rs:248-261 (traced). F9 workspace_member.rs:84-93 (traced: no external callers). F10 inventory.rs:103,121 (traced). F11 identity/resolution.rs:77-87 (open). F12 recovery.rs:149-151 (traced, returns miss). F13 permissions.rs:113-153 (stated-ish). F14 filesystem.rs:223-226 (inferred).

# Five things a maintainer most needs to know about this unit

1. All Git launches go through one function, `run_git_output` (git_command.rs:44), after `sealed_git_command` (policy.rs:20); grep of `Command::new|sealed_git_command\(|run_git_output\(` outside tests shows only that wrapper plus `transfer.rs:135` and `availability.rs:98`. The sealing is policy-by-arguments: https and ssh protocols are both allowed for either transport (request.rs:81-87), `file` only in `cfg(test)`/`test-fixtures` and no other crate enables that feature.
2. Trust is layered but repeated: object bytes are authenticated in-process (authentication.rs), then repository custody is re-verified around every operation (`VerifiedGitRepository::verify_identity/verify_current`, repository.rs:131-200, which walks the repository twice), then published snapshots are fully re-hashed on every reuse (git/snapshot.rs:334-376, snapshot/metadata.rs:71-118). Several checks are repeated on the same data (whole graph authenticated at inspection.rs:90 and snapshot.rs:251; member payloads at projection.rs:165,190 and snapshot.rs:251).
3. Fetch/no-fetch and cache preserve/invalidate decisions are encoded in `GitRevisionSelection` (selection.rs:57-131) and one long function `with_git_repository_from_retained_cache` (acquisition.rs:107-321); `may_preserve_cache` (retry.rs:65) lists only 6 process-level error variants, everything else invalidates the entry by removing `source.identity` (invalidation.rs:46-77).
4. Two parallel workspace flows exist: planner-trait based (workspace_member.rs, pub API with zero callers outside the crate by grep) and session based (`GitWorkspaceSession`/`GitWorkspaceCatalog`, used by package-manager). Both end in `materialize_member` (workspace_member.rs:193). Whole-source and catalog capture use different listing scopes: `WholeTree` rejects gitlinks and `.gitmodules`, `SelectedOnly` keeps them inert until a member is selected (objects/tree.rs:100-118, 157-166).
5. Platform differences are real and mostly silent: off Unix the link-count check, directory-mode check, change-time check and executable bit are absent or constant (git/cache/custody.rs:187-193, permissions.rs:113-153, filesystem.rs:201-226); a Windows-only consequence for Git `100755` blobs is inferred at git/snapshot.rs:193-200 (not run). Many tests are `cfg(unix)` or Windows-`ignore`d.

# What I am unsure about

- Whether `custody/platform.rs` (`verify_windows_open_cache_regular_file_identity`, tree.rs:384) rejects hard links on Windows, which decides F2.
- Whether `RecordFileRoot::read` rejects a symlink or directory named `source.identity` (decides F5).
- Whether `custody/tests/mod.rs` gates the two `windows_*` test files with `cfg(windows)` (inferred, not read).
- Exact per-name external caller counts for `resolve_git_source_from_pin_in_lane`, `verify_package_source_snapshot`, `capture_verified_package_source_snapshot`, `recover_cached_local_source_in_lane`, `resolve_local_source_snapshot_in_lane`, `stage_local_source_replacement_in_lane`, `from_canonical_path`, `hash_canonical`: only grouped counts were taken.
- What else in package-manager binds a `Git` `ImmutableSourceResolution` to a specific lineage beyond `matches_lineage` (F11).
- The Windows `100755` mismatch at git/snapshot.rs:193-200 is reasoning from code, not an observed failure.
- Whether the duplicated request-file sequence in transfer.rs:49-67 and availability.rs:51-79 is meant to stay separate (both read; I did not diff them character by character).
