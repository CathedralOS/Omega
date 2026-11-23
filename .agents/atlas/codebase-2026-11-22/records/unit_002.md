# Record: unit_002 (areas 01-cli-foundation and 06-source-driver-build)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d`. Paths are relative to it. All line numbers come from the Read tool. "Read" means I read the lines; "grepped" means a Grep run (the command is named); "inferred" means not traced. Nothing was built or run. The unit has 77 rows and 21,786 lines. 18,607 lines were read in full. 3,179 lines were not read (listed under Coverage).

Order below: foundation crates (arena, bignum, bounded-process, diagnostics, filesystem, record-storage, source-text, wire), then the Build evaluation crates (accounting, activation, admission, authority).

---------------------------------------------------------------------------------------------------
## FOUNDATION

### source-rust/foundation/arena/src/paged.rs (3,073 lines; read 1-2200, not read 2201-3073)

1. Purpose. Holds the multi-page and concurrent arena types behind the `Handle` surface. A header comment (1-13) lists five of them. Lines 2201-3073 are the `mod tests` block (starts at 2177) and were not read.
2. Shape.
   - `ArenaPage<T>` (25): enum `Owned(Arena)` / `Shared{arena: Arc<Arena>, copy}`. `DerefMut` (49) detaches a shared page by calling `copy` when `Arc::strong_count > 1`. `share` (75) freezes a non-empty owned page.
   - `PagedArenaSet<T: Default>` (186): `first_page`, `additional_pages: Vec<(u32, ArenaPage)>`, `page_positions` (one-based index by generation minus `page_base`), `current_generation`, `active_count`, `dummy: T`, `revision: RevisionStamp`. The handle's generation selects the page. Methods: `begin_page` (382), `adopt` (304), `take_additional_pages` (319), `take_pages_from` (360), `relocate_single_page` (234), `share_pages` (250), `rewind_appended` (562), `span_since` (580), `frontier` (624), `get` (487), `get_mut` (498), `insert_page` (706). `PagedArenaFrontier` (153) is a `(generation, next_arena_index)` pair with `contains` (160).
   - `ConcurrentPagedArena` (847), `ConcurrentArenaPage` (938), `PublishedArenaPage` (1034), `SealedPagedArena` (1058), `PageDirectory` (1189, 256 pages per segment, line 845). `reserve_page_with_capacity` (900) takes an index with a CAS `try_update`. `publish` (989) sets a `OnceLock` once. `seal` (918) refuses if reserved pages != published pages (`IncompletePagePublication`, 1282).
   - `GenerationalPagedArena` (1368): `PAGE_SIZE=16` (1366), lock-free `FreeStack` (1289) with a 32-bit tag in the head word, slot metadata `AtomicU64` packing state and generation (1694), `ArcSwapOption` pages.
   - `OwnedInterner<T>` (1704) is a `Mutex<T>` wrapper. `OwnedTextInterner` (1755) has `prefixes` plus a `local` `TextInternerStorage` (1768) with 25 geometric text pages (`FIRST_TEXT_PAGE=256`, 1775) and 24 index generations (1778), open addressing, tag from `text_tag` (2001, a custom multiply-rotate hash).
3. Inputs/outputs. Takes `Arena<T>`, `Handle`, `HandleSpan` from the sibling files. Produces handles and spans. `Record` impls write/read pages and enforce generation order and `active_count` equality (770-843, 2110-2175).
4. How it decides. Every lookup is keyed on a handle's `(arena_index, generation)`. `PagedArenaSet::get` returns `&dummy` for handle zero and calls `refuse_unresolved` for any other unresolved handle (487-495, stated in the doc comment at 483-484). `SealedPagedArena::get` does the same (1086-1094). The text interner is keyed on string content (`probe`, 1961, compares `interner.text(slot) == Some(text)`).
5. Cases by shape.
   - `relocate_single_page` refuses unless `additional_pages` is empty: "only a single-page arena set can move generation" (240).
   - `GenerationalPagedArena::new` requires `max_capacity >= 16` (1377-1380).
   - `begin_page` panics when the generation is not above the first one (387-390).
   - `rewind_appended` panics if the generation differs from the frontier's (563).
6. Same job elsewhere. Four copy-on-write checkpoint tables do overlapping work: `PagedArenaSet` (paged.rs:186, `share_pages` 250), `RetainedArena` (retained.rs:50), `RootedArena` (rooted.rs:60), `RetainedRows` (retained_rows.rs:14). Each lets a fork share old rows and append privately. Read, not traced for consumers.
7. Unused/one-caller code.
   - `GenerationalPagedArena`, `FreeStack`, `SlotRef`: `Grep "GenerationalPagedArena"` over `source-rust` finds 2 files only (arena `lib.rs` and `paged.rs`), so 0 users outside the arena crate. `Grep` over `tools/` found 0.
   - `reclaim_empty_pages` (1582) is a body returning `0`. It has no real reclaim.
   - `ConcurrentPagedArena`/`SealedPagedArena`: `Grep "GenerationalPagedArena|SealedPagedArena|ConcurrentPagedArena"` count 4 in arena `lib.rs`, 36 in `paged.rs`, 2 in `source-to-psi/pipeline/01_tokens-to-syntax-trees/parser.rs`, 2 in `source-to-psi/representations/syntax-trees/files/pages.rs`. So two external files use the concurrent types.
8. Stated limits. "the loser" none. `reclaim_empty_pages` returning 0 has no comment. Comment 1810-1813 says a fork's prefix length is captured at fork time.
9. Fail-open risks.
   - `GenerationalPagedArena::get` (1489-1515) returns the dummy slot for a stale, unknown or invalid handle instead of refusing. No comment at the site. Traced: no caller outside the arena crate (7 above), so no program reaches it today.
   - `PartialEq` for `OwnedInterner` returns `true` unconditionally (1729-1733) and for `OwnedTextInterner` (2036-2040). Two arenas whose interners hold different texts compare equal if their other fields match. No comment. Open (not traced to which derived `PartialEq` structs hold them).
   - `OwnedInterner::with` recovers a poisoned mutex with `into_inner` (1745). Read; no comment.

### source-rust/foundation/arena/src/retained.rs (1,030 lines; read in full)

1. Purpose. `RetainedArena<T>`: an absolute-slot table whose forks share published pages and append in a private page. Handles keep their slot and generation across forks (1-4).
2. Shape. `RetainedPage{start, rows: Arc<Arena>}` (12), `RetainedDirectory{pages, page_by_slot}` (29), `RetainedArena{directory: Arc, appended: Arc<Arena>, start, revision, rewrite_revision, append_only, retired}` (50). Entry points: `append` (198), `insert_many` (209), `finish_append_page` (103), `get`/`try_get` (232/238), `span` (250), `get_mut` (425), `free` (436), `append_to_span` (478), `retire_all_retain_capacity` (372), `reset_retain_capacity` (401), `partition_point` (318), `append_epoch` (182). `Record` (512) and `PartialEq` (588).
3. Inputs/outputs. Wraps `Arena`; produces absolute handles by adding `start`.
4. How it decides. Page lookup is keyed on slot (`page_by_slot[slot]`, 132, 169). Validity is the page's own generation and occupancy check. `Record::read` rejects pages that are empty, overlap, or whose `start` is not the running sum (535-547).
5. Cases by shape. "Issued spans always fit in one page" (47). `append_to_span` copies a span that lies in a retained page (480-481) rather than extending it.
6. Same job elsewhere. See paged.rs item 6. `RetainedRows` (retained_rows.rs:14) is built on `RootedArena`, not on this type.
7. Usage. `Grep "reclaim_empty_pages|RetainedRows|\bRetainedArena\b"` outside `arena/` gives 37 occurrences in 10 files, all under `source-to-psi/representations/` (symbol-table, typed-trees). No dead code seen.
8. Stated limits. "Retired slots are not reused" (45).
9. Fail-open risks. `get` on an invalid (zero) handle returns the append page's dummy (239-241) and refuses any other unresolved handle (243). Stated: the comment on `refuse_unresolved` use is in paged.rs; here no comment. Traced as fail-closed for non-zero handles. `has_dense_generation`/`pages_fit` return booleans and fail-closed. None else seen.

### source-rust/foundation/arena/src/retained_rows.rs (457 lines; read in full)

1. Purpose. `RetainedRows<T>` is a `Vec`-like ordered roster of immutable records. Forks share the roster `Arc<Vec<Row>>` and the records in a `RootedArena<Option<T>>` (1-4).
2. Shape. `RetainedRows{records, order}` (14), `push` (72), `insert` (76), `retain` (116), `truncate` (142), `get_mut` (162, `T: Clone`), `RetainedRowsIter` (236). `Record` writes the old `Vec<T>` bytes (274-286); `read` charges allocation up front (289-332).
3. How it decides. `retain` evaluates the predicate over all rows first and only then publishes the new roster and frees removed records (116-140). The comment at 127-128 gives the reason (a panicking predicate leaves the owner unchanged).
4. Same job elsewhere. Overlaps with `RootedArena::project` (rooted.rs:494), which builds filtered rosters over shared records.
5. Usage. Used by `RetainedRows` consumers: part of the 37 occurrences in item 7 of retained.rs (the grep pattern included `RetainedRows`).
6. Fail-open. None seen. `row()` panics with `expect` if the roster names an empty record (51).

### source-rust/foundation/arena/src/rooted.rs (1,385 lines; read 1-1000, not read 1001-1385)

1. Purpose. `RootedArena<T>`: "declaration roots and child spans name logical rows, not record allocations" (1-3). Association pages of 128 `RecordAddress{batch,row,exclusive}` entries point into shared `Arc<Vec<T>>` batches.
2. Shape. `RootedArena` (60) with `associations: Vec<Arc<Arena<RecordAddress>>>`, `batches`, `dummy`, `free_slots`, `fresh`, `explicit_slots`, `rewrite_revision`. `RootRows` (588) is a borrowed view over a span (`Retained` arena+span or `Slice`). Methods: `append` (429), `insert` (434), `set` (464), `project` (494), `append_to_span` (510), `free` (525), `get_mut` (569), `span` (295), `iter_from` (335). `from_storage_parts` (120), `decoded_record_metadata_bytes` (116).
3. How it decides. `address` (225) takes a fast path when `fresh` (generation equals `fresh_generation` and `slot < length`) and otherwise asks the page. `get_mut` copies a record into a private batch when `!address.exclusive` or the batch is shared (573-581).
4. Cases by shape. `ASSOCIATIONS_PER_PAGE=128` (17). `RootRows::as_array::<N>` returns `None` for any other length (669).
5. Fail-open. `get` for handle zero returns `&self.dummy` (287-289); other unresolved handles refuse (291). `RootRows::slice` panics on bad bounds. None else seen in the read part. Lines 1001-1385 (tests) not read.

### source-rust/foundation/arena/tests/handle_spans.rs (35 lines; read in full)
Four tests on `HandleSpan::extend_contiguous` and `push_contiguous`: adjacency, gap refusal, generation refusal, count overflow. Panic messages are matched by text ("handle span push must be contiguous").

### source-rust/foundation/bignum/src/float_bits.rs (25 lines; read in full)
`binary32_from_bits` and `binary32_to_bits` move binary32 bits through `f64` and keep NaN sign and payload (3-25). `binary32_to_bits` forces payload `0x400000` when the payload bits come out zero (21-23). Callers: not grepped. `float_bits` is not declared in `lib.rs` (I saw no `mod float_bits` in lines 1-1294); inferred that `Cargo.toml` or an out-of-file path wires it. Open.

### source-rust/foundation/bignum/src/lib.rs (1,708 lines; read 1-1294, not read 1295-1708)

1. Purpose. Exact `BigInt`, `BigRational`, `ExactFloat` over little-endian `u64` limbs (3-9).
2. Shape. `BigInt{negative, magnitude}` (23). `from_canonical_words` (45) refuses a trailing zero limb or a negative zero. Arithmetic: `add` (267), `sub` (290), `mul` (294, schoolbook `mul_magnitudes` 1158), `div_rem` (305, powers of two go through `div_rem_power_of_two` 346; otherwise `div_rem_magnitudes` 1193, a bit-at-a-time long division, 1212-1225), `gcd` (379, Euclid with power-of-two shortcuts), `shl_bits` (62), `shr_bits` (182, arithmetic shift with floor correction), `bitwise` (213, two's complement words). `BigRational{numerator, denominator, negative_zero}` (558) with `to_ieee_bits` (731) and rounding via `rounded_scaled_quotient` (1066). `ExactFloat` enum `Finite/Infinity/NaN` (824), with `add_with_rounding` (854) deciding the sign of an exact zero (868-873).
3. Inputs/outputs. Strings parse through `from_decimal_str` (428) and `from_str_radix` (402). Hosts convert through `TryFrom` macros (488-511): `u32` and `usize` go through `to_u64` then `try_from`.
4. How it decides. `ExactFloat::from_decimal_str` matches the literal spellings `"NaN"`, `"inf"`, `"infinity"` and signed variants (833-835). Rounding is keyed on `IeeeRounding` (15). Subnormal handling keyed on `exponent < minimum_exponent` (754).
5. Cases by shape. `rounded_scaled_quotient` asserts the quotient "fits in one u64" with `expect` (1087). `from_decimal_str` for `BigRational` accepts `e`/`E` exponents parsed as `i64` and `+` only for rationals (573-580).
6. Same job elsewhere. `DecimalChunks` (1260) and `from_str_radix` (402) are two parse paths. `mul_magnitudes_by_u64` (1186) is a thin call to `mul_magnitudes`.
7. Usage. Not grepped for `pub` items.
8. Stated limits. None. Division is bit-serial (1212): quadratic cost for wide divisors. Read only; no comment.
9. Fail-open. `ExactFloat::from_decimal_str` returns `NaN` for any NaN spelling including signed (833); the sign is dropped (inferred from the match arm; `NaN` carries no sign). `BigRational::new` is `pub(crate)` and returns `None` on a zero denominator (704-707) (fail-closed). None else seen. Lines 1295-1708 (tests) not read.

### source-rust/foundation/bounded-process/src/bounded_process.rs (421 lines; read in full)
1. Purpose. Run loop for one child with piped stdin/stdout/stderr under a shared deadline (1-25).
2. Shape. `run_bounded_process` (61) builds `BoundedProcessChild::spawn` then calls `capture_child` (82). Stream workers (`spawn_stream_capture` 258, `capture_stream` 304) and an input worker (`spawn_input_transfer` 278). `terminate_before` (363) retries on `EPERM`. `fail_after_cleanup` (351), `gate_worker_spawn` (336).
3. How it decides. Time: `execution_deadline = deadline - min(cleanup_timeout, timeout/4)` (90-92). Overflow of `started + timeout` becomes `started`, so the run times out at once (90, `unwrap_or(started)`). Output: a stream over its limit gives `OutputOverflow`; over the shared `BoundedCaptureBudget` gives `AggregateOutputOverflow` (305-330).
4. Cases by shape. `validate_limits` refuses a zero timeout, cleanup or poll interval (248-256).
5. Same job elsewhere. `ToolCommandChild` (process_child.rs:65) reuses the same `BoundedProcessChild` owner without these limits.
6. Usage. `capture_bounded_child` is the `pub use` alias of `capture_child` (lib.rs:55). A grep over the primary checkout for `ToolCommandChild|capture_bounded_child|run_in_private_root` lists `application/src/execution/private_observation/channel_windows.rs` and `tools/xtask/src/execution.rs` outside this crate (grepped; I did not read them, so I cannot say which name each uses).
7. Stated limits. Comment 13-24 says a mid-exit macOS primary is retried until the cleanup deadline.
8. Fail-open. `Disconnected` with all three completions already set loops and sleeps (238-243): if worker results arrive after the status flag, the loop continues until the deadline (inferred, not traced). None else seen.

### source-rust/foundation/bounded-process/src/budget.rs (79 lines), capture_contract.rs (100), lib.rs (57) (read in full)
`BoundedCaptureBudget{ceiling, observed: Arc<AtomicU64>}` (budget.rs:12) charges with a CAS loop that refuses before incrementing (33-62). `capture_contract.rs` holds the input enum (`Null/Bytes/File`, 8), `BoundedCaptureLimits` (15), `BoundedProcessOutput` (42), `BoundedProcessRunError` (65, 12 variants; its `Display` prints the `Debug` form, 94-98). `lib.rs` wires modules and re-exports. `lib.rs:1-34` states the crate claims no filesystem, executable, credential or network sandbox.

### source-rust/foundation/bounded-process/src/preparation.rs (319 lines; read in full)
1. Purpose. `BoundedProcessPrepared` wraps a `Command` plus `BoundedProcessLimits` and refuses to spawn unless all three standard streams were set to null or piped (209-221).
2. Shape. `BoundedProcessPrepared::new` (96) validates limits and installs resource limits through `limits::configure_child_resource_limits`. `retain_descriptor` (144, unix) requires fd >= 3. `validate` (209) checks arguments (4,096), environment entries (1,024), total bytes (4 MiB) (21-23, 222-261).
3. How it decides. Fixed ceilings by count and bytes. `validate_limits` (282-297) requires `open_files >= 3` and `aggregate_memory_bytes >= process_memory_bytes`.
4. Same job elsewhere. Windows `WindowsJobLimits::from_limits` (windows/limits.rs:20) re-derives job limits from the same struct.
5. Fail-open. None seen. The `os_str_bytes` non-unix/non-windows branch uses lossy UTF-8 length (316-318), which can under-count bytes (inferred; only reached on a third platform).

### source-rust/foundation/bounded-process/src/private_root.rs (212 lines; read in full)
1. Purpose. Run a native test executable over an empty tmpfs root inside a new user and mount namespace (Linux only). Other hosts return `PrivateRootUnavailable` (27-35).
2. Shape. `run_in_private_root` (38). `Prepared::enter` (119) does `unshare`, writes `/proc/self/{setgroups,uid_map,gid_map}`, mounts tmpfs (`size=64m,mode=0755`), `chroot`, sets secure bits and `NO_NEW_PRIVS`, drops all 64 capabilities in a loop (155-157, return value ignored), `capset` to zero, then `execveat` from a held file descriptor.
3. Usage. `Grep "run_in_private_root"` with glob `*.rs` over the worktree: 3 files: `bounded-process/src/lib.rs`, `private_root.rs`, and `_recycling/source-rust/compiler/v1-build-orchestration/src/compile/test_execution.rs`. So no active caller outside this crate; the one caller is in the archive.
4. Stated limits. "A host that cannot create the namespace ... reports the native engine unavailable; it never falls back to the host's filesystem" (19-20).
5. Fail-open. The capability-drop loop at 155-157 ignores `prctl` errors (open; later `capset` result is checked at 185). Environment is empty (`envp` null, 74), so the child gets no environment.

### source-rust/foundation/bounded-process/src/process_child.rs (323 lines) and process_child/{completion,descriptors,limits,resource_usage}.rs (68, 102, 63, 151) (read in full)
1. Purpose. `BoundedProcessChild` (42) owns the spawned process group (unix via `command_group`, Windows via `WindowsJobChild`). `finish` (265) refuses unless `container_closed` and a status is latched. `Drop` kills and reaps (296-309). `ToolCommandChild` (65) spawns with no resource caps.
2. How it decides. `terminate` (191-217) treats errno 3 (`ESRCH`) as already absent and errno 1 (`EPERM`) as "group signal refused", retries once after a `try_wait`. On non-unix `native_container_already_absent` is `error.kind() == InvalidInput` (317-319). `limits.rs` intersects each rlimit with the inherited soft/hard value (49-63). `descriptors.rs` marks descriptors 3.. close-on-exec in one `close_range` syscall on Linux (21-26) or by listing `/dev/fd` elsewhere (85-102).
3. Stated limits. `resource_usage.rs:105`: "primary process only; excludes descendants" (Windows).
4. Same job elsewhere. `ProcessResourceUsage::unavailable` (24).
5. Fail-open. On Windows, a failed `TerminateJobObject` with `InvalidInput` is treated as the container being absent and the child as closed (process_child.rs:317-319). No comment at the site; open. `group_signal_refused` is always false on non-unix (222-228).

### source-rust/foundation/bounded-process/src/process_child/windows/{job,launch,lifecycle,limits,mod,native,termination}.rs (124, 118, 255, 37, 20, 47, 20 lines; read in full)
Purpose: Windows Job Object containment. `create_job` and `create_completion_port` (job.rs:28, 32); `configure_job_limits` (46) sets kill-on-close, die-on-unhandled-exception, active-process, per-process memory, job memory and job CPU limits; `configure_tool_job` (40) sets only kill-on-close. `assign_and_resume` (launch.rs:28) assigns the suspended child to the job and resumes every thread found by a `CreateToolhelp32Snapshot`, retrying on `ERROR_BAD_LENGTH` up to 8 times (26, 86-100). `WindowsJobChild::try_wait` (lifecycle.rs:138) returns a status only after the primary exited and the completion port posted `ACTIVE_PROCESS_ZERO` (4-9). A completion key that differs from the job handle is an error (179-184). Failed setup terminates and reaps the child (106-109). Fail-open: `resume_thread_fully` errors when a thread was not suspended (108-113). None seen.

### source-rust/foundation/bounded-process tests not read
`bounded_process_tests.rs` (178), `process_child/tests.rs` (273), `process_child/windows/tests/{lifecycle 37, resource_limits 113, support 72, worker 107}.rs`. `windows/tests/mod.rs` (6 lines) read: lists the four modules.

### source-rust/foundation/diagnostics/{diagnostic,message,provision,text}.rs (144, 122, 88, 165 lines; read in full)
1. Purpose. The error/warning/note record every stage returns, plus text rendering.
2. Shape. `Diagnostic` (diagnostic.rs:17) with `severity`, `message`, `stage: Option<DiagnosticStage>`, `build_context`, `exceeded_provision`, `source_span`, `resolved_location`. `DiagnosticStage` (29, 7 stages; `attribute` fills `stage` only when empty, 40-45). `ExceededProvision` (provision.rs:2) and `CoverageForm` (10). Display macros (`debug_display!`, `diagnostic_display!`, `string_diagnostic!`, `failed_display!`, message.rs:4-48). `write_one_diagnostic` (text.rs:21) escapes bytes and cuts at 4,096 (5, 32-42). `write_diagnostics` (99) and `write_location` (48).
3. How it decides. `ExceededProvision::read` accepts only the seven coverage resource names matched by string (provision.rs:21-31, 76-79) and rejects mismatching `limit`/`requested` (80-83). `write_location` branches on `build_context`, `stage`, `resolved_location.package`, `source_span` (49-96).
4. Cases by shape. The seven `CoverageForm` names are a fixed list (10-18). `ExceededProvision` has no `Record` derive; the manual impl round-trips only coverage provisions (56-88). Any other resource refuses on read.
5. Same job elsewhere. `write_one_diagnostic` renders "\n  error: msg" (text.rs:31) while `Display` for `Diagnostic` and `write_diagnostics` render "error: msg" (diagnostic.rs:139, text.rs:104): two renderers.
6. Usage. `Grep "write_bounded_diagnostics|bracketed_display_names|comma_join_display|diagnostic_display!|failed_display!|string_diagnostic!|debug_display!"` outside `foundation/diagnostics`: 101 occurrences in 65 files (count mode).
7. Fail-open. None seen.

### source-rust/foundation/filesystem/src/{lib,file_publication,lock_file,record_file,temporary_directory}.rs and record_file/{lock,stage}.rs (308, 124, 42, 424, 41, 80, 136 lines; read in full)
1. Purpose. Narrow host-filesystem pieces: macOS ACL inspection, `(volume, object)` identity, exact-byte staged publication, create-new lock files, capability-relative record files, scratch directories.
2. Shape. `filesystem_object_identity` (lib.rs:62): `(dev, ino)` or `(volume serial, file index)`. `extended_acl_has_allow_entry` (51): macOS only; elsewhere returns `Unsupported` (199-214). `publish_exact_file_bytes` (file_publication.rs:16), `stage_exact_bytes` (27), `install_staged_files` (73). `open_or_create_lock_file` (lock_file.rs:22). `RecordFileRoot` (record_file.rs:57) with `read_optional` (109), `write_new_in` (157, hard-link into place), `replace_existing` (191), `RootRecordRead::{replace,remove,verify_current}` (232-283), `try_lock` (lock.rs:27), stage helpers (stage.rs:53-136). `TemporaryDirectory::create` (41 lines) names `omega-<purpose>-<pid>-<seq>`.
3. How it decides. Record operations accept a single `Normal` path component only (`single_file_name`, record_file.rs:285-298). A read or write succeeds only if the open handle and the directory entry have the same regular-file identity (`verify_regular_identity`, 300-317). A symlink or non-regular file is `NotRegularFile`. Staging names are `.<name>.<pid>.tmp` (file_publication.rs:39) or `.omega-record-stage-<pid>-<seq>` (stage.rs:109), tried up to 256 times (24, 107).
4. Cases by shape. Windows identity relies on `cap_primitives::fs::_WindowsByHandle`; a `@Hack` comment explains why (record_file.rs:328-330).
5. Same job elsewhere. Two stage-then-install paths: `file_publication` (installs by `rename`, 98-101) and `record_file::stage` (`rename` or `hard_link`). They share no code. `RecordFileLock::try_lock` (lock.rs) uses `open_or_create_lock_file` (lock_file.rs).
6. Usage. `Grep "TemporaryDirectory|publish_exact_file_bytes|stage_exact_bytes|install_staged_files"` outside `filesystem/`: 178 occurrences in 66 files, mostly application tests.
7. Stated limits. "overwritten destinations are not restored" (file_publication.rs:8-9). Non-unix `install_staged` removes the destination, then renames (103-111), so it is not atomic.
8. Fail-open. `same_capability_file_identity` returns false on a third platform (record_file.rs:345-348, fail-closed). `TryLockError::WouldBlock` returns `Ok(None)` (lock.rs:57), which is the contention signal. `stage_exact_bytes` falls back to the name `"omega-output"` for a non-UTF-8 file name (file_publication.rs:35-38), so two such outputs in one directory would share a stage name (inferred, open).

### source-rust/foundation/record-storage/derive/src/lib.rs (168 lines; read in full)
`#[proc_macro_derive(Record, attributes(record))]` (32). Struct fields serialize in declaration order; enums write a positional `u32` tag (138, 143). `#[record(skip)]` fields are rebuilt from `Default` (89-91). `#[record(write_scope = path)]` wraps the writes (158-161). Every type parameter gets a `Record` bound (116-122). A `union` is refused (151-155). Unknown tag on read is an error: "unknown <Type> variant" (148). Variant identity is positional, so reordering enum variants changes the bytes (read, inferred consequence).

### source-rust/foundation/record-storage/src/lib.rs (1,034 lines; read 1-700, not read 701-1034) and values.rs (591; read in full)
1. Purpose. Bounded binary records. `Record` trait (37), `RecordWriter` (175), `RecordReader` (384), `RecordLimits` (139, defaults 64 MiB bytes, 512 MiB allocation, 4 Mi elements, depth 128, 146-155).
2. Shape. `encode` (49), `decode` (55), `decode_with_budget` (61), `encode_beside` (83) and `decode_beside` (126) for one externally supplied `Arc`, `encode_with` (115). `Renumbering` (201): writer-side map from minted interner numbers to canonical numbers; `renumbered` (283) refuses a slot the map does not cover. Reader: `begin_shared` / `finish_shared` / `shared` (485-526) require shared objects in publication order; `remap_generations`, `define_generation`, `generation` (528-607).
3. How it decides. Limits are charged before allocation (`sequence` 440-455, `allocate` 432). Shared `Arc` values are keyed by `(TypeId, pointer, length)` in a `HashMap` on write (346-371). A second value of the externally supplied type is refused (355-360). `values.rs` writes `HashMap`/`HashSet` ordered by each key's encoded bytes (403-416, 418-462) and `BTreeMap`/`BTreeSet` in key order unless renumbering is active (337-394). Decoding a map or set refuses duplicates (361-362, 388-390, 435-437, 456-458). `bool` accepts only 0 or 1 (58-62).
4. Cases by shape. Tuples up to arity 8 (322-329). Non-zero integer reads refuse zero (565).
5. Same job elsewhere. `wire::cursor::Cursor` (cursor.rs:17) is a second bounded byte reader with its own `counted` pre-flight.
6. Stated limits. "Decoding produces structural candidates only; ... must still bind the exact producer and input digests" (lib.rs:17-22).
7. Fail-open. `generation` returns the original number when no remapping domain exists for the type (592-598). Stated by design (no remap domain registered means identity). `finish_shared` consumes `identity` order only (498-513). None else seen. Lines 701-1034 (tests) not read.

### source-rust/foundation/source-text/src/lib.rs (175 lines; read in full)
`Span` (9), `SourceId` (21), `SourceSpan` (24), `SourcePosition::of` (42, counts characters, not bytes, by walking `char_indices` up to `byte_offset`), `SourceBytes` (64, `Arc<Vec<u8>>` + range; equality and hash by content, 131-143), `display_literal_bytes` (145: `\\`, `\"`, `\n`, `\r`, `\t`, printable ASCII kept, else `\xNN`). `SourcePosition::of` is O(n) per call.

### source-rust/foundation/wire/src/*.rs (codec_bytes 65, cursor 123, digest_encoder 104, error_display 28, fnv1a 82, lib 53, record_views 66, tagged_vocabulary 64 lines; read in full)
1. Purpose. Fixed-width helpers, a bounded `Cursor`, a domain-tagged SHA-256 encoder, FNV-1a, and macros.
2. Shape. `Cursor` (cursor.rs:17): `take`, `array`, fixed integer reads, `counted(width)` (refuses a count above `remaining()/width`, 74-80; it divides by `width`, so width 0 would panic: inferred), `length_prefixed` (87). `DomainDigestEncoder` (digest_encoder.rs:11): `raw` adds bytes unframed, `bytes`/`string` add a u64 length prefix. `content_identity` (90) is the unframed SHA-256 of the bytes. `Fnv1a` (fnv1a.rs:4). `push_count_u32` panics with `expect` when a count exceeds u32 (codec_bytes.rs:16-19). `decode_header` (47). Macros: `tagged_vocabulary!` builds a `#[repr(u8)]` enum plus an `ALL` array, `tag_from_all!` builds `from_tag` by scanning `ALL`, `record_views!`, `invalid_error_display!`, `fixed_error_display!`.
3. How it decides. `tag_from_all!`'s `from_tag` looks a byte up in `ALL` (tagged_vocabulary.rs:56-60); an unknown tag becomes the caller's `unknown` error.
4. Same job elsewhere. `Fnv1a`: `Grep "Fnv1a"` outside `wire/`: 82 occurrences in 30 files (count mode), mostly psi/native-target identity code. `build/providers/plans/digest_encoder.rs` exists next to `DomainDigestEncoder` (name from the crate map; suspected duplicate, not read).
5. Stated limits. `lib.rs:1-4`: "Representation owners choose their tags".
6. Fail-open. `Fnv1a::finish_or(fallback)` returns the fallback when the hash is 0 (73-75). This maps a zero hash to a fixed value; used to keep zero as a reserved marker (inferred from the name; callers not grepped).

---------------------------------------------------------------------------------------------------
## BUILD EVALUATION (source-rust/subsystems/compiler/build/evaluation/)

### accounting/contracts/usage.rs (386 lines; read in full)
1. Purpose. Immutable measurement, limit and usage records; no live sponsor or lock.
2. Shape. `EvaluationResource` (7), `EvaluationAccountingError` (16), `TerminalStorageScheduleIdentity` (58, `CURRENT = 1`), `EvaluationMeasurementIdentity::TerminalPsi{work_schedule, storage_schedule}` (75), `BuildEvaluationSponsorLimits` (117, `new` returns `None` if any ceiling is zero, 129-149), `BuildEvaluationUsage` (216, 22 fields), `BuildEvaluationUsageError` (243), `validate` (272).
3. How it decides. `is_supported(schema, schedule)` is true only for schema `8`, schedule marker `1` and storage marker `1` (99-110, hard-coded). `BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION = 7` (114). `validate` matches the nine optional sponsor fields: all `None` (unsponsored) requires zero session peaks (302-309); all `Some` checks ceilings; any mix is `IncompleteSponsorIdentity` (380-382).
4. Cases by shape. Nine arms of one tuple match: one arm per zero ceiling (310-333). A usage with two zero ceilings reports whichever arm matches first only if exactly the others are `Some` (inferred; all nine are `Some` in every zero arm, so the first zero ceiling in the field order wins).
5. Fail-open. None seen.

### accounting/accounting.rs (282), accounting/invocation.rs (450), accounting/work/work.rs (43) (read in full)
1. Purpose. `BuildEvaluationSponsor` holds aggregate counters behind a `Mutex`; `EvaluationInvocationAccount` charges one invocation under it; `EvaluationStorageLease` is an RAII reservation.
2. Shape. `BuildEvaluationSponsor::new` (accounting.rs:68, `expect`s the current measurement), `charge_pure_usage` (180), `charge_build_log_bytes` (148), `charge_filesystem_operation_attempt` (165). `begin_invocation` (invocation.rs:47), `charge_work` (105), `reserve_site` (175), `increase_site` (189), `charge_result_custody` (267), `snapshot` (306), `EvaluationStorageLease::{absorb, truncate_cells, grow, shrink}` (371-426). `EvaluationWorkPayment`, `sponsored_meter`, `unsponsored_meter` (work.rs:7-43).
3. How it decides. Locks sponsor before invocation (`consumption`, invocation.rs:73-93). Every charge computes both candidate totals with `checked_add`, tests all limits, then writes (e.g. 105-123); a refused charge changes neither counter. `sponsored_meter` refuses when the account's work schedule differs from `WorkSchedule::CURRENT` (work.rs:31-34). `charge_pure_usage` refuses usage with nonzero filesystem attempts (accounting.rs:190-192) and a measurement different from the sponsor's (187).
4. Same job elsewhere. Sponsor-level `charge_build_log_bytes` (accounting.rs:148) and `charge_filesystem_operation_attempt` (165) duplicate the per-invocation versions (invocation.rs:125, 143) with `String` errors. Callers of the sponsor-level pair: not grepped (suspected 0).
5. Usage. `Grep "charge_pure_usage|BuildEvaluationSponsor::new\(|owns_usage"` outside `accounting/`: 2 occurrences in 2 files (`build/inputs/build_execution/filesystem_scope/runtime.rs` and `runtime_live_source_tests.rs`).
6. Stated limits. None.
7. Fail-open. `release_storage` (invocation.rs:242-265) checks `shared >= cells` only with `debug_assert!`, then does `-=`. In a release build without overflow checks a release larger than the held amount would wrap the counter. No comment; open (callers hold only leases they reserved, inferred). `charge_build_log_bytes` on the invocation account checks only the aggregate ceiling (133), while `charge_work` also checks a per-invocation budget (114): the invocation log has no own ceiling (read).

### accounting tests not read
`accounting/tests/accounts.rs` (243), `accounting/tests/cached_usage.rs` (150). `work/tests/sponsorship.rs` (116) read: two tests. One shows a refused payment leaves sponsor and meter unchanged (45-82). The other shows the meter cannot raise an invocation ceiling (85-104).

### activation.rs (991 lines; read in full)
1. Purpose. `TerminalBuildHost` mints one root receiver, wires three host adapters (request, binding, filesystem) and starts at most one `TerminalBuildActivation` (a stateful coroutine). `ReturnedBuild` is the completed result.
2. Shape. `TerminalBuildHost::new` (212), `start` (301), `resume` (322), `instantiate` (405). `TerminalBuildActivation` (571): `advance` (810), `advance_diagnostic` (816), `answer_query` (607), `install_provider` (658), `dependency_prepared` (727), `carry` (774), `complete` (934). `ActivationState` {Running, AtPreparation(usize), Returned, Refused} (565). `TerminalBuildStep` (584): Returned, AtPreparation, NeedsWork, Provide, Prepare, Query. `OriginalOutput` (138). `CarriedStage` (544).
3. Inputs/outputs. Reads a `ProducedTerminalProgram`, the accepted source, provider installation, an `EvaluationInvocationAccount`, a `SelectedBuildMachine` and a filesystem scope. Outputs `ReturnedBuild` (83) holding history, requests, settings, selections, log, filesystem effects.
4. How it decides.
   - A mutating step sets `state = Refused` first and restores `Running` or `Returned` on success (616, 666, 737, 830, 882-885).
   - `AwaitingEffect` is resolved by which of two hosts reports a pause: only a provider pause or only a dependency pause is accepted; neither or both is refused (862-874).
   - `answer_query` checks that each dependency names an already issued `Product` request by ordinal (617-636). `install_provider` compares purpose, package and the dependency alias string against the backing path (672-699).
   - `TerminalBuildHost::new` calls `entry.unconditional_precondition()`: `Some(true)` proceeds, `Some(false)` and `None` refuse (235-249).
   - `instantiate` binds each unbound import by finding the boundary in the request, binding, filesystem or intrinsic call lists, else errors "has no admitted host operation or installed provider" (526-530).
   - `guest_diagnostic` finds a user source span by walking caller sites and skipping toolchain files (913-922).
5. Cases by shape. `resume` requires the stopped call stack's last frame to equal the old call (338-340).
6. Same job elsewhere. `project::declaration` (project.rs:18) re-derives the project role from the ledger.
7. Usage. `TerminalBuildHost` and `TerminalBuildActivation`: the crate map lists `project-build` as the caller (inferred; not grepped here).
8. Stated limits. "not supported yet: a build entry `requires` clause over its arguments" (243).
9. Fail-open. None seen. Every unknown (no pause, unknown import, no candidate) errors.

### activation/{preparation,project,protocol,provider_installation,source_join,source_result}.rs (519, 92, 295, 199, 246, 43 lines; read in full)
1. preparation.rs. `PreparedBuildExecution` (26) takes an `AdmittedBuildProgram`, admits providers, opens an account with a fixed 100,000 work units (`INVOCATION_WORK`, 72). Without a supplied sponsor it creates one with `u64::MAX` limits except work (74-90). `arm_preparations` (190) checks the stage's preparation paths against the machine. `resume` (296) and `host` (349) each issue the host once (`host_issued`). `complete` (384) refuses dependency requests not settled by a source join (390-399), refuses `artifact_only` with entries/providers/native bindings (405-407, 493-514), and requires at least one completed output for artifact-only (421-423).
2. project.rs. `declaration` (18) builds `BuildDeclaration` from the request ledger: one role among Package/Application/Member(s) (workspace); two roles, repeated member, repeated artifact-only, or artifact-only on a non-application role are errors (34-90). No role is an error (84).
3. protocol.rs. `check` (125) compares every earlier host call between the old and new program by action: bindings compare request, action, selecting machine, environment and operand shape after remapping source spans; queries compare occurrence, request and shape; others compare equal (166-272). Pairs with no declared action on both sides are accepted if the boundary is an intrinsic with the same operation or collected as a supply pair (259-270), and later each supply pair must match an installed provider's boundaries (281-293). Provider requirement identity is compared by text: `before.text(a) == after.text(b)` (177-186). `Sources::rejoin` maps an old source id to a new one by `rejoined_source_id`, else by a unique `continues_capture_of` match; two candidates give `None` (98-123). A span that cannot be rejoined sets `accounted = false` and the check fails (160-164, 273).
4. provider_installation.rs. `admit_request` (33) compares the backing path suffix with `provider.path()` and requirement/application text with the provider's strings (43-49). `prepare` (57) finds exactly one provider candidate whose declaration text and requirement identity text match (92-106) and refuses zero or two (99-105); completes scalar applications under the live account and builds `TerminalComponentBinding`s.
5. source_join.rs. `validate_dependency_sources` (109) walks issued requests in order against selections and acquisitions and refuses unrequested direct dependencies, a count mismatch, or a demanded source missing from the join (198-216). `validate_source_join` (220) compares the joined source inputs to the completion inputs.
6. source_result.rs. Thin wrapper over `ReturnedBuild`.
7. Cases by shape. protocol.rs:166 matches fixed pair shapes (Binding/Binding, Query/Query, Some/Some, None/None); everything else is "changed". `BuildRequest::Dependency{..}` is skipped when computing roles (53).
8. Fail-open. protocol.rs:258 `(Some(a), Some(b)) => a == b`: request and filesystem actions pass if equal; no source span remap is applied to them (read; open whether those carry spans). protocol.rs:259-270 accepts `(None, None)` pairs whose boundary is a non-intrinsic import by recording a supply pair; traced: lines 281-293 then require an installed provider covering both boundaries, so a program with such a pair and no provider is refused. Stated by the module header (1-2).

### admission/{mod,program,provider_execution,checking}.rs (6, 109, 152, 471 lines; read in full)
1. program.rs. `admit_build_program` (43): checks the Build entry symbol's span lies in the selected source (56-63), `admit_build_machine` (66), publishes, then calls `selections.admit(...)` with Build and Product package authorities (78-94). Missing source-selection companion is an error (73-77).
2. provider_execution.rs. `BuildProviderPlans::admit` (31) admits selected provider adapters and the installation, runs `admit_declaration_selections` (54-66), `admit_captured_execution` (67-71), computes demand and refuses if `demand.missing_providers` is non-empty (96-101), and admits numeric intrinsics for the demanded boundary identities (102-142). Row executions are passed as `vec![None; rows]` named `unreviewed` (121-126).
3. checking.rs. `settle_rejoined_build_slice` (25) refuses if `folded_under_providers` (34-38) or any read constant lacks a declaration or value (42-49), and returns an empty accounting when no constants were read (39-41). `compare_rejoined_constants` (66) rejoins each read constant by source path plus byte range and compares values after sorting and dedup (73-133). `prepare_admitted_build` (135) refuses combined anonymous source and package inputs (151-158). Six tests at 180-471.
4. How it decides. Constants are matched by `(source path, start, end)` then `rejoin_declaration` (99-106); values compared by equality of retained values (127).
5. Stated limits. "no product route yet: Build admission folded a constant under its provider plans..." (36); "no product route yet: the Build program reads constant ..." (44).
6. Usage. `Grep "admit_compiler_evaluation|admit_declaration_selections|settle_rejoined_build_slice"` over `source-rust`: 17 occurrences in 10 files (count mode).
7. Fail-open. `settle_rejoined_build_slice` returns `Ok(Default::default())` for an empty read list (39-41). Stated by test comment 206-207 ("leaves a later Product frontier nothing to settle"). Traced only in the test. provider_execution.rs:121-126 passes `None` row executions for every plan row: intrinsic plans are admitted without per-row review data (open).

### authority/consent/requests.rs (187 lines; read in full)
`RestrictedBuildOperation` (7), `RestrictedBuildGrantRoot` (13; `Other(u32)`), `RestrictedBuildGrant` (20), `RestrictedBuildBounds` (54), `RestrictedBuildRequest` (97). `from_retained` constructors are `#[doc(hidden)]`. `for_target` (151) only replaces the selected target profile. One test (162-186) shows `for_target` keeps grants and the execution profile.

### authority/filesystem/filesystem_authority.rs (181 lines) and grants.rs (518 lines; read in full)
1. Purpose. Filesystem operation vocabulary and grant/handle/metadata records for Build evidence.
2. Shape. The `filesystem_operations!` macro defines `FilesystemHostOperation` with 50 operations tagged 1-50 (filesystem_authority.rs:47-98), each with a canonical name, tag and result kind. `BuildOutputObligation` (122), `BuildOutputReceipt` (153), `MAX_BUILD_OUTPUT_OBLIGATIONS = 4_096` (106), `FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION = 20` (18). grants.rs: `FilesystemGrantRoot` (103), `FsGrants` (180), logical handle types, `FilesystemMetadataLayout` (313) with `new` validation (319-377) and `host()` (391).
3. How it decides. `filesystem_root_relative_path_is_canonical` (71) accepts a relative path only if it is within the byte limit, has no NUL, is UTF-8, does not start with `/`, has no `\\`, no `X:` prefix, and has no empty/`.`/`..` component. `FilesystemMetadataLayout::new` demands each of 14 fields exactly once, widths in {16,32,64} not above the semantic width, in bounds, with no overlap (323-372).
4. Cases by shape. `host()` has per-target tables for linux x86_64, linux aarch64, macOS and windows x86_64, and `panic!("unsupported host filesystem metadata layout")` for any other (392-462). Record size 128 for linux aarch64, else 144 (463-467).
5. Same job elsewhere. `filesystem_root_relative_path_is_canonical` (grants.rs:71) and the re-exported `canonical_filesystem_metadata_path_is_canonical` from `source_selection` (filesystem_authority.rs:13-15): suspected overlap, other site not read.
6. Fail-open. None seen. An unlisted host panics.

### authority/admission/admission.rs (865 lines; read in full)
1. Purpose. Admit a published declaration for compiler evaluation (`admit_compiler_evaluation`, 71) or admit the declarations a Build can select (`admit_declaration_selections`, 231).
2. Shape. `AdmittedCompilerEvaluation` (16) with `require_invocation` (34). `CapturedBuildSelections{entry, calls}` (308). `require_invocation_selections` (345), `selected_call_request_sources` (541), `build_calls` (413), `covered_recursive_edge` (813), `require_compiler_evaluation_floor` (840).
3. How it decides.
   - Evaluation is refused if the root has an authored `requires` (105-110) or fails `require_crash_free_evaluation` (111).
   - The floor refuses non-empty service reach, possible suspension or blocking, a closure violation (linear carrier or a non-terminating machine not selected by a requirement) or a selection violation (840-865; the message lists "empty service reach, no possible suspension or blocking, ordinary checked termination ..." at 862).
   - A recursive invocation edge (an edge whose target reaches its source) needs a recursive-component entry covering that exact call (187-212, 813-838).
   - Selection between declaration custodies goes through `require_declaration_selection(from, to, authority)` (297, 810). Unowned user to unowned user is allowed when a source consent object allows it and the artifact has an invocation scope (801-809). For `admit_declaration_selections` with no package, unowned-user to unowned-user or toolchain edges are accepted and recorded as `captured` entries or `requests` (281-295).
4. Cases by shape. `build_calls` handles a captured root, direct sites (by ordinal into the machine's module and boundary invocations), cleanup actions and machine-value targets; anything else errors ("neither a cleanup action nor a machine value", 518-523).
5. Same job elsewhere. `evaluation_declaration` (781) vs `declaration` (769): the first adds the invocation-refusal check. `provider_execution.rs:54` calls `admit_declaration_selections` for Build.
6. Usage. See checking.rs item 6 (17 occurrences in 10 files).
7. Stated limits. "assumed binding contracts are runtime-only and cannot supply compiler evaluation" (136). "constant invocation retains a pending target proof condition ..." (141).
8. Fail-open. The unowned-user acceptance at 281-295: stated at 305 ("The source companion must discharge each selection before execution"); traced to `provider_execution.rs:67-71` (`admit_captured_execution(artifact, pending.entry, &pending.calls)`). A test `declaration_admission_preserves_authority_without_a_pure_effect_floor` sits outside any `mod tests` (674-750) with a bare `#[test]` (read).

### activation sub-files not listed elsewhere
All files of the unit's activation directory were read; no unread parts.

---------------------------------------------------------------------------------------------------
## Same job elsewhere (cross-file summary)
- Four copy-on-write forkable tables: paged.rs:186, retained.rs:50, rooted.rs:60, retained_rows.rs:14.
- Two bounded byte readers: `RecordReader` (record-storage lib.rs:384) and `wire::Cursor` (cursor.rs:17).
- Two stage-and-install writers: file_publication.rs and record_file/stage.rs.
- Per-invocation and sponsor-level log/filesystem charge pairs: accounting.rs:148/165 and invocation.rs:125/143.
- Two root-relative path canonical checks (grants.rs:71 and the `source_selection` re-export, other site not read).
- Provider and requirement identity compared by text in protocol.rs:177-186 and provider_installation.rs:43-49, 92-98.

## Coverage
Read in full: 77 files minus the ranges below = 18,607 lines.
Skimmed: none.
Not read (3,179 lines):
- `source-rust/foundation/arena/src/paged.rs` lines 2201-3073 (873)
- `source-rust/foundation/arena/src/rooted.rs` lines 1001-1385 (385)
- `source-rust/foundation/bignum/src/lib.rs` lines 1295-1708 (414)
- `source-rust/foundation/bounded-process/src/bounded_process_tests.rs` 1-178 (178)
- `source-rust/foundation/bounded-process/src/process_child/tests.rs` 1-273 (273)
- `source-rust/foundation/bounded-process/src/process_child/windows/tests/lifecycle.rs` (37), `resource_limits.rs` (113), `support.rs` (72), `worker.rs` (107)
- `source-rust/foundation/record-storage/src/lib.rs` lines 701-1034 (334)
- `source-rust/subsystems/compiler/build/evaluation/accounting/tests/accounts.rs` (243) and `cached_usage.rs` (150)

## Five things a maintainer most needs to know about this unit
1. `GenerationalPagedArena` has no user outside the arena crate (grepped: only arena `lib.rs` and `paged.rs`), its `reclaim_empty_pages` returns 0 (paged.rs:1582), and its `get` returns a dummy for any stale handle (1489-1515). `run_in_private_root` (private_root.rs:38) has no active caller; its one caller is under `_recycling/`.
2. `OwnedInterner` and `OwnedTextInterner` implement `PartialEq` as always true (paged.rs:1729, 2036). Any struct that derives equality over them ignores interned content.
3. Build activation is a one-shot state machine: every mutating call sets `Refused` before fallible work, so a single failure poisons the activation (activation.rs:616, 666, 737, 830). Code switching after a dependency preparation is allowed only at the checked cut and is verified action by action in `protocol::check` (protocol.rs:125-295), with provider and requirement identity compared as text.
4. Admission relaxes the unowned-user selection rule for a captured Build (admission.rs:281-295) and relies on a later source-companion step (provider_execution.rs:67-71) to discharge each selection. Intrinsic plans are admitted with `None` row executions (provider_execution.rs:121-126).
5. Accounting is two-level (sponsor and invocation, locked in that order) and refuses atomically (invocation.rs:73-93, 105-123). The usage schema and measurement markers are hard-coded constants (usage.rs:99-110, 114); `release_storage` guards underflow only with `debug_assert!` (invocation.rs:255).

## What you are unsure about
- Whether `bignum/src/float_bits.rs` is compiled into the crate: no `mod float_bits` appears in lib.rs lines 1-1294, and I did not read `Cargo.toml`.
- Which of `ToolCommandChild`, `capture_bounded_child` the two external files (`channel_windows.rs`, `tools/xtask/src/execution.rs`) use; the grep that found them ran against the main checkout, not the worktree.
- Whether the always-true `PartialEq` on the two interners is relied on by any derived equality (not traced).
- Whether sponsor-level `charge_build_log_bytes` / `charge_filesystem_operation_attempt` (accounting.rs:148, 165) have callers (not grepped).
- Counts quoted for `Fnv1a`, `TemporaryDirectory`, diagnostics helpers and the `admit_*` functions are Grep count-mode occurrences per file, not call-site counts.
