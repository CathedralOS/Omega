# Record: unit 002, gap 001 (second pass)

Unit file: `units/gap_001.tsv`. Areas: `01-cli-foundation`, `06-source-driver-build`.
Every range in this unit lies inside a `#[cfg(test)]` module or a test-only file. No range contains
production code. The `#[cfg(test)] mod tests` openings are at `arena/src/paged.rs:2177`,
`arena/src/rooted.rs:854`, `bignum/src/lib.rs:1292`, `record-storage/src/lib.rs:629` (grepped), so each
range below is the tail of that module. Items 4 to 9 are answered once at the end for the whole unit
where they are the same for every file ("none seen" or "tests only").

## source-rust/foundation/arena/src/paged.rs 2201-3073 (tail of `mod tests`)

1. Purpose: tests for the paged arena family: `PagedArenaSet` (sparse pages keyed by generation),
   `ConcurrentPagedArena` / sealed arena, `GenerationalPagedArena`, and `OwnedTextInterner`.
2. Shape (read): tests, no types. Fork/share cases at 2201-2217 (child shares parent page pointers
   until written, copy-on-write: `child.span_mut` at 2208 leaves `parent` unchanged). Worker hand-back
   `workers_hand_back_only_pages_from_their_split_generation` 2220 uses `take_pages_from(split, restored)` and
   `adopt_additional_pages`. Sparse directory test `late_page_directory_spans_only_additional_pages` 2262
   asserts `page_positions.len()` of 1, 3, 10_003. `revision()` tests 2344-2440 (every mutation and
   transfer route changes the revision; `clone_from` copies it). Record codec round-trip of sparse pages
   2496 and refusal of five malformed forms 2522-2550 (duplicate generation, unordered pages, page
   whose generation disagrees, wrong `active_count`, `current_generation = 0`). Concurrent arena tests
   2553-2762: publication keeps the worker `Vec` allocation (2553), 8 workers publish disjoint pages
   (2571), a 1,000,000-page bound allocates only the top-level directory of `div_ceil(256)` segments
   (2612-2632), sealed rewrite (2648), later epoch via `begin_concurrent_extension` (2664),
   `into_pages` order (2693), dropped reservation refuses `seal` with `reserved_pages 1, published_pages 0`
   (2711), handle zero resolves to a default only on read and the default is constructed lazily, counted
   by a static `DEFAULTS` (2736-2762), header size independent of `T` (2765). Three `should_panic`
   tests 2776-2804 pin the panic text "arena get received an unresolved handle" /
   "arena get_mut received...". `GenerationalPagedArena` tests 2807-2885 (16-slot page growth, `new(64)`
   starts with capacity 16, full arena of 16 gives 15 handles and an invalid overflow handle,
   `reclaim_empty_pages()` returns 0 "until quiescence exists"). `OwnedInterner` / `OwnedTextInterner`
   tests 2888-3072: clone keeps keys; 8 threads mint each key once; forks share prefix pages and
   number independently, hidden ancestor keys do not mask fork keys (2944-2993); decoder charges
   allocation and refuses duplicate text and invalid UTF-8 (2996-3035); capacity refusal leaves storage
   unchanged via `catch_unwind` (3038-3046); parallel interning in a fork (3049-3072).
3. Inputs and outputs: asserts only; reads `record_storage::{encode, decode, RecordLimits}` (2505-2507,
   2528-2548, 2928-2931, 2985-2991) and private fields (`page_positions`, `additional_pages`,
   `active_count`, `current_generation`, `first.local.indexes`, `first.prefixes[0].storage`).
4. How it decides (what the tests pin): validity of a handle is keyed on (arena_index, generation)
   pair, e.g. 2276, 2490-2492, 2730; interner keys are keyed on text equality and slot number.
5. Cases by shape: none. Magic sizes used: 90_000/80_000 generations (2264-2272), 4_000 (2468), 1_000_000
   pages (2613), 16/32/64 slot arenas (2808-2876), `FIRST_TEXT_PAGE * 3` (2946).
6. Same job elsewhere: `RootedArena` tests in `rooted.rs:1154-1190` check the same record codec
   round-trip and `revision` idea against `Arena`; suspected (not read) that earlier parts of
   `paged.rs` tests repeat the retained-page codec cases.
7. Unused code: not applicable (tests).
8. Stated limits: comment test name `page_reclamation_is_disabled_until_quiescence_exists` 2875 (reclaim
   returns 0 even when all 20 slots are freed, 2883).
9. Fail-open: none seen.

## source-rust/foundation/arena/src/rooted.rs 1001-1385 (tail of `mod tests`)

1. Purpose: tests for `RootedArena`, a copy-on-write arena whose records live in `Arc`'d batches and whose
   handles go through `Arc`'d association pages.
2. Shape (read): `Tracked { copies: Arc<AtomicUsize>, value }` counts record clones (used at 1001,
   1082, 1134). Tests: invalid span mutation refuses before any copy (1001-1076: `for_each_span_mut` on an
   oversized span panics with 0 callbacks and 0 copies; `get_span_member_mut` panics for out-of-range
   position, `usize::MAX`, wrong generation, empty span; one retired slot inside the span also makes the
   whole span invalid, 1062-1070); fork and `project` copy only the written record (1079-1116);
   aliasing within one arena protects both versions (1119-1127); reserved batches survive append and
   replace (1130-1151); codec and retirement order equal the contiguous `Arena` (1154-1190);
   views across association pages and `append_to_span` (1193-1219); NaN is not equal to its clone,
   because sharing does not shortcut `PartialEq` (1222-1226); retired partial pages fit the charged
   decode allocation, with an explicit byte tally compared to `decoded_association_allocation`
   (1229-1286); retirement stores each replacement default and decode never calls `Default`
   (1289-1316); retiring an exclusive record drops its payload (1319-1332); over-aligned (4096) zero-sized
   type charged before sharing (1335-1365); validated views survive fork retirement and slot reuse
   (1368-1384).
3. Inputs and outputs: asserts only; uses `Arena`, `HandleSpan`, `record_storage`, `shared_allocation_bytes`,
   `decoded_association_allocation` (1239, 1342), private fields `associations`, `batches`.
4. How it decides: span validity keyed on every slot's (index, generation) not just the first (1062-1064,
   1208-1212).
5. Cases by shape: constants `ASSOCIATIONS_PER_PAGE` (1238), 65-slot capacity (1132, 1284), 300/400 rows.
6. Same job elsewhere: allocation-charging tests here (1229-1286, 1335-1365) repeat the same budget
   assertion style as `record-storage/src/lib.rs:759-855` and `paged.rs:2996-3035`.
7. Unused code: not applicable.
8. Stated limits: comment 1240-1241 "Rooted storage moves that free list, not copies it."
9. Fail-open: none seen.

## source-rust/foundation/bignum/src/lib.rs 1295-1708 (tail of `mod tests`, then line 1708)

1. Purpose: tests for `BigInt`, `BigRational` and `ExactFloat`. The last line, `pub mod float_bits;`
   (1708), is production: it declares a public module after the test module.
2. Shape (read): `big(i128)` helper 1296. Tests: zero is signless (1301); canonical-word codec rejects
   alternate zero and high-zero encodings with the string `"integer magnitude is not canonical"`
   (1321-1337); ring ops against `i128` on a 14-value grid, including `bitwise` And/Or/Xor, `div_rem`,
   `shr_bits` (1340-1392); division by zero is `None` (1395); gcd (1413); decimal and radix parse
   round-trip (1423-1467); exact narrowing `to_i64`/`to_u64` (1470); `BigRational` exact integer
   conversion and display (1482-1548); decimal arithmetic exact until rounding (1551); binary32 rounding
   ties-to-even (1566); zero, subnormal, overflow to infinity (1579-1612); `ExactFloat` division produces
   NaN/infinity at landing (1615-1632); 2,000-case pseudo-random grid comparing `to_f32`/`to_f64` bits with
   Rust's `str::parse` (1635-1659); dyadic divisors at limb edges against `div_rem_magnitudes` and a hand
   Euclid loop (1662-1705).
3. Inputs and outputs: asserts only; uses `div_rem_magnitudes`, `BigInt { negative, magnitude }.normalized()`.
4. How it decides: equality is by canonical sign plus magnitude words; float rounding judged against the
   host Rust parser (1648-1657).
5. Cases by shape: limb-edge sizes `[0,1,31,63,64,65,127,128,1024,1074]` (1663); shift amounts
   `[0,1,63,64,65,100,127]` (1378).
6. Same job elsewhere: not seen.
7. Unused code: `pub mod float_bits;` is declared, but the file `float_bits` was not in my unit; not examined.
8. Stated limits: none.
9. Fail-open: none seen.

## source-rust/foundation/record-storage/src/lib.rs 701-1034 (tail of `mod tests`)

1. Purpose: tests of the `Record` encode/decode codec with limits.
2. Shape (read): `Shared { first: Arc<Payload>, second: Arc<Payload>, #[record(skip)] derived }` 702; tests:
   shared `Arc` identity preserved and skipped field rebuilt empty (711); every truncation, trailing byte,
   and foreign variant tag `u32::MAX` refuses (729-744); allocation abuse and unpublished shared references
   (747-757); shared allocation budgets include alignment and zero-sized values via
   `shared_allocation_bytes` (760-820); a layout budget one byte too small refuses before the value is read,
   counted by a static `READS` (823-855); `Link` depth limit for both encode and decode with `depth: 3`
   (857-870); fixed-width sequence bytes are little-endian and share array identity, NaN payload bits
   survive (873-918); size, element and bool-value refusals (921-949); generation domains
   `remap_generations`, `generation`, `define_generation` keep semantic handles and separate equal old
   tags (952-994); a failed `define_generation` registers nothing (997-1033).
3. Inputs and outputs: asserts only; uses `RecordReader::new`, `RecordLimits { allocation, elements, depth }`.
4. How it decides: `define_generation` is keyed on (domain type, old tag) and the "source" value compared
   with `&"source"` versus `&"changed"` / `&17_u32` is rejected (981-990).
5. Cases by shape: fixed-width element types `u32`, `bool`, `i16`, `f64`, `u8` only in the sequence tests.
6. Same job elsewhere: allocation-budget tests duplicate in style `rooted.rs:1335-1365` (read).
7. Unused code: not applicable.
8. Stated limits: none.
9. Fail-open: none seen.

## source-rust/foundation/bounded-process/src/bounded_process_tests.rs 1-178 (whole file)

1. Purpose: Unix-only (`#![cfg(unix)]`, line 6) integration tests for `run_bounded_process`; included via
   `#[path]` at `bounded_process.rs:420` (grepped).
2. Shape (read): helpers `process_limits()` 17 (30 s CPU, 1 GiB, 64 MiB, 64, 8, 512 MiB, 768 MiB),
   `capture_limits` 29, `shell` 43 (canonicalizes `/bin/sh`). Tests: duplex transfer 54; each stream
   ceiling and the shared `BoundedCaptureBudget` reject (68-115); timeout kills the process group, a
   descendant that would write a marker after 0.75 s must not (118-143); completed parent closes pipes held
   by a backgrounded `sleep 10` in under 1 s (146-157); overflow from an already-exited child reports
   `OutputOverflow` (160-178).
3. Inputs and outputs: asserts only.
4. How it decides: error variant matched structurally (`OutputOverflow { stream, limit }`,
   `AggregateOutputOverflow { ceiling: 7, attempted: 8 }`, `TimedOut`).
5. Cases by shape: macOS tolerance, `cfg!(target_os = "macos") && matches!(&error, Cleanup(_))` accepted
   in place of the expected overflow (85-89, 108-109): on macOS a `Cleanup` error also passes these two tests.
6. Same job elsewhere: process-group cleanup marker pattern is repeated at
   `process_child/tests.rs:147-171, 195-225`.
7. Unused code: not applicable.
8. Stated limits: comment 85 name `fail_closed_macos_cleanup`.
9. Fail-open: tests only. The macOS alternative at 85-89 and 108-109 means that on macOS these overflow
   tests pass when the run fails with `Cleanup` instead of an overflow error. Classification: stated by
   the variable name, no comment.

## source-rust/foundation/bounded-process/src/process_child/tests.rs 1-273 (whole file)

1. Purpose: unit tests for `BoundedProcessChild` and `ToolCommandChild`, mounted at `process_child.rs:323`
   (grepped). The file has no `cfg(unix)` gate itself; it spawns `/usr/bin/true`, `/bin/cat`, `/bin/sh`,
   so it relies on the parent's gating (not confirmed; I did not read `process_child.rs`).
2. Shape (read): `intersect_limit(Rlimit, u64)` never loosens an inherited limit (14-57: current 64 /
   max 1024 with 256 gives 64/256; `None`/`None` gives 256/256); `preparation_rejects_inconsistent_resource_limits`
   zero `cpu_seconds` is `InvalidInput` (83); implicit stdio is rejected with message containing
   "explicitly null or piped" (96-107); completion requires `terminate` before `finish`, and an
   unterminated finished process cannot `finish` (110-144); drop kills descendants (147-171);
   `terminate` of a group with an exited primary succeeds and still kills descendants (175-225); tool
   file streams keep the primary exit code 23 and close descendants (239-273).
3. Inputs and outputs: asserts only; helper `cleanup_probe_tree` 227 returns (root, marker, `/bin/sh`).
4. How it decides: status keyed on exit code (`Some(23)`).
5. Cases by shape: fixed host paths `/usr/bin/true`, `/bin/cat`, `/bin/sh`.
6. Same job elsewhere: marker-file survival check also in `bounded_process_tests.rs:118-143`.
7. Unused code: not applicable.
8. Stated limits: header 1-4 lists "macOS zombie-primary `EPERM` handling in `terminate`", but the test
   at 175 only asserts `terminate()` returns Ok after exit; the EPERM itself is not produced here.
9. Fail-open: none seen.

## source-rust/foundation/bounded-process/src/process_child/windows/tests/{lifecycle,resource_limits,support,worker}.rs

Group: Windows Job Object tests (same kind of thing, four files). All read in full.

- `lifecycle.rs` 1-37: two tests spawn `cmd.exe` through `WindowsJobChild::spawn_with_config`; normal exit
  reaches active-process zero (12); `kill()` terminates the whole job and the status is not success
  (27-37).
- `resource_limits.rs` 1-113: four tests serialized by `JOB_LIMIT_TEST_LOCK`: active-process limit
  (`fanout`), per-process memory (`touch`), aggregate memory (`aggregate-memory`), aggregate CPU (`spin`,
  `spin-until-terminated`). Each runs a below-limit control then an over-limit case and checks
  `JobLimitEvent::{ActiveProcess, ProcessMemory, AggregateMemory, AggregateCpu}` in `child.limit_events`.
- `support.rs` 1-72: constants `WORKER_MODE`, `WORKER_VALUE`, `WORKER_TEST`, `MIB`, `JOB_LIMIT_TEST_LOCK`;
  `wait_bounded` 14 (10 ms poll, panics at deadline); `test_limits` 29 builds `WindowsJobLimits {
  active_processes, process_memory_bytes, aggregate_memory_bytes, aggregate_cpu_ticks }` with ticks =
  nanoseconds / 100 (33-34); `worker_command` 46 re-invokes the test executable with
  `--exact process_child::windows::tests::worker::job_limit_worker --test-threads=1`; `run_worker` 59.
- `worker.rs` 1-107: `job_limit_worker` 36 returns immediately when `OMEGA_BOUNDED_PROCESS_WINDOWS_JOB_WORKER`
  is unset (37-39), so ordinary test runs pass it as a no-op; otherwise dispatches on mode string
  `hold`, `fanout`, `touch`, `aggregate-memory`, `spin`, `spin-until-terminated`, and panics on any
  other mode (105).
- How it decides: modes selected by string compare on the environment value (`worker.rs:41`).
- Cases by shape: `parse_worker_values(value, expected)` accepts a fixed number of comma fields per mode
  (2, 2, 3).
- Stated limit: comment `resource_limits.rs:5-8` says `spin-until-terminated` is used so only the OS CPU
  limit can end the worker.
- Fail-open: `job_limit_worker` early `return` at `worker.rs:38` when the env var is missing is intended
  (stated by the design: ordinary run is a no-op); a missing worker env leaves this a passing test.

## source-rust/subsystems/compiler/build/evaluation/accounting/tests/accounts.rs 1-243 (whole file)

1. Purpose: source-free tests of `BuildEvaluationSponsor` counters and leases (header 1-2: "no test here claims
   that an interpreter charges its actual storage sites").
2. Shape (read): helpers `limits(work, cells, bytes)` 11 (calls `BuildEvaluationSponsorLimits::new(work, 8,
   2, 2, cells, bytes, cells, bytes)`), `terminal()` 15 (= `EvaluationMeasurementIdentity::current()`),
   `other_schedule()` 19, `sponsor()` 27. Tests: a non-current measurement is refused with no charge (32);
   a rejected combined `reserve_site(work, cells, bytes)` changes neither work nor storage (52); two
   invocations race for one allowance, exactly one wins, loser's peaks stay 0 (72); `Arc`-aliased lease
   releases once (103); `grow`/`shrink` refusal keeps held storage (117); `absorb` transfers cells and bytes
   without recharging, `truncate_cells` (134); `absorb` across sibling invocations returns
   `LeaseAccountMismatch` and moves nothing (157); equal limits do not give `owns_usage` authority (178);
   result custody persists across retries without refund (192); host charges `charge_build_log_bytes`,
   `charge_filesystem_operation_attempt` refuse atomically (208); unsupported storage schedule is not
   treated as schedule one (222); u64 overflow preserves both accounts (238).
3. Inputs and outputs: asserts only.
4. How it decides: usage ownership keyed on sponsor identity not on limit values (178-186); measurement
   keyed on `EvaluationMeasurementIdentity::TerminalPsi { work_schedule: 1, storage_schedule: 2 }`
   (19-24).
5. Cases by shape: only one supported measurement, `current()`; others refused (32-48, 222-236).
6. Same job elsewhere: `cached_usage.rs:84-99` repeats the overflow-preserves-accounts check on the replay
   path.
7. Unused code: not applicable.
8. Stated limits: header comment 1-2 (quoted above).
9. Fail-open: none seen.

## source-rust/subsystems/compiler/build/evaluation/accounting/tests/cached_usage.rs 1-150 (whole file)

1. Purpose: tests for `BuildEvaluationSponsor::charge_pure_usage`, replaying a recorded cost (header 1:
   "Recorded-cost replay tests; public Build execution is covered by build_charge").
2. Shape (read): `limits` 9 (six axes), `measured()` 17 records a usage with work 7, 3 cells, 6 bytes, log 2,
   result custody 2 cells 4 bytes. Tests: replay spends the current account and refreshes peaks while
   leaving live storage alone (30-56); each of six refused axes leaves every counter unchanged (58-79);
   invalid records (`step_schedule_marker = 0`), effectful usage (`filesystem_operation_attempts = 1`) and
   other measurements cannot spend (81-98); replay counts current live reservations (100-113); overflow does not
   change other axes and the error string contains "overflowed" (115-130); two concurrent replays cannot
   both spend one allowance of 7 (132-150).
3. Inputs and outputs: asserts only.
4. How it decides: pure replay accepted only when record validates, has no filesystem effects, and
   measurement is current (81-98).
5. Cases by shape: error type is `String` (`unwrap_err().contains("overflowed")` 127), a text match.
6. Same job elsewhere: see accounts.rs 6.
7. Unused code: not applicable.
8. Stated limits: header.
9. Fail-open: none seen.

## Items 7 and 9 across the unit

Item 7: no `pub` item is defined in the ranges read, except `pub mod float_bits;` (`bignum/src/lib.rs:1708`);
that module is outside this unit and its callers were not counted. No "single/canonical" comment appears in
these ranges.
Item 9: no production fail-open found. Test-only tolerance: macOS `Cleanup` accepted in
`bounded_process_tests.rs:85-89, 108-109`; no-op worker when env unset `worker.rs:37-39`.

- **Coverage:** read in full: paged.rs 2201-3073 (873), rooted.rs 1001-1385 (385), bignum 1295-1708 (414),
  record-storage 701-1034 (334), bounded_process_tests.rs 1-178 (178), process_child/tests.rs 1-273 (273),
  windows/tests/lifecycle 1-37, resource_limits 1-113, support 1-72, worker 1-107 (329 together),
  accounting/tests/accounts.rs 1-243 (243), cached_usage.rs 1-150 (150). Total 3,181 lines. Skimmed: none.
  Not read: none. (Lines 1-1294 of bignum etc. belong to the first reader.)
- **Five things a maintainer most needs to know:**
  1. Everything in this unit is test code; the only production line is `pub mod float_bits;` at `bignum/src/lib.rs:1708`.
  2. Windows Job tests re-run the test binary as a worker selected by environment variables; `job_limit_worker` is a silent no-op without them (`worker.rs:37-39`, `support.rs:46-56`).
  3. On macOS, two overflow tests also pass on a `Cleanup` error (`bounded_process_tests.rs:85-89, 108-109`).
  4. Arena tests pin that `GenerationalPagedArena::reclaim_empty_pages` returns 0 (`paged.rs:2875-2885`) and that handle zero resolves to a lazily built default (`paged.rs:2736-2762`).
  5. Accounting tests refuse any measurement but `current()` and prove refusals leave every counter unchanged (`accounts.rs:32-48,52-70`; `cached_usage.rs:58-79`).
- **Unsure about:** whether `process_child/tests.rs` is cfg-gated by its parent (not read); whether
  `float_bits` is used elsewhere; the earlier halves of the tests were not read so "same job elsewhere"
  duplicates inside `paged.rs` are suspected only.
