# Tasks — Filesystem (`std::fs`) + Windows-native thread

> **LOOP LIVE (Windows thread, since 2026-07-06).** Zach's redirect: Windows/x86_64
> is the primary TESTED target ("focus on us"); macOS/aarch64 canaries stay green
> structurally but runtime confirmation needs a Mac. This file is the source of
> truth; finished-work narrative lives in git history + the canaries that pin it.
>
> **WORKING RULES.** Consult `wiki/language_guide/*` before language features;
> ZII / arena / `Handle` / `HandleSpan`; full human-word op names (C symbols only
> in per-target binding tables); every fix ships a canary that RUNS and asserts.
> Gates: canary_suite (678+ green on Windows), samples_compile (grep the whole
> tail for FAILED — never judge by a piped grep's exit code), omega-interpreter
> coverage + differential drift guard (`run_canary_list` — run IMMEDIATELY after
> every rebase), instr-sel/reloc/calling-conv crate tests. Push every iteration:
> fetch → rebase → re-verify → `git push origin HEAD:main`.

## North star

A serious, ergonomic `std::fs` with Rust parity: portable `Filesystem` wrapper
(result enums / `File`) over a per-OS raw `FilesystemHost` seam (= Rust `std::fs`
over `std::sys`). Raw ops return syscall ints; the wrapper builds results in
Omega. Interpreter = full-parity reference oracle for everything.

## Current state

**Interpreter:** full Rust-parity fs (all ops + wrapper); virtual fs/GUI/keyboard
are the differential oracle. Value-position `match` desugars to tag arithmetic;
`Value::Enum` carries `type_symbol` (ordinals resolve type-locally).

**Native raw seam:**
- **darwin/aarch64** — complete (54 canaries; create→flock breadth incl. variadic
  `open_create`, deref-result `___error`, stack-marshalled mode). Runtime
  re-confirmation pending on a real Mac. 2026-07-07: the week's value-call
  selection changes (deferral, pairing, hoist) cross-compile cleanly to
  macos_arm64 (five key canaries probe-verified via temp target blocks).
- **windows/x86_64** — LIVE through msvcrt rows in `WINDOWS_IMPORT_ROWS` riding
  the general Win64 import-call encoder (data-address + byte-length args,
  `dereferences_result` for `_errno`). Verified RUNNING:
  `filesystem/windows_raw_roundtrip_exit` (create/write/read/verify/remove/ENOENT)
  and `filesystem/windows_raw_breadth_exit` (sync/_commit, seek/_lseeki64,
  dup, chmod, rename, mkdir, rmdir — 2026-07-07). O_BINARY (0x8000) is REQUIRED
  for binary reads (msvcrt text mode); interp ignores unknown flag bits, so it is
  differential-safe.
- Ops with NO msvcrt equivalent (pread/pwrite, *at, link/symlink/readlink,
  read_dir, flock, chown, futimens, realpath) + the STAT family keep the clean
  "no native lowering" error on windows — future Win32-call work.
- **linux** — structural only (surface + interp are target-agnostic; adding a
  target is table work).

**Native ergonomic wrapper:** scalar/tag AND payload results work on windows_x64
end-to-end. SECOND WAVE (2026-07-07, `filesystem/windows_wrapper_breadth_exit`):
WRAPPER rename (the two-path import call now resolves each path PER ARGUMENT
through the alias chain -- param-forwarded literals had no encodable sequence),
append (flag word 9 is darwin==msvcrt portable), read_all, remove. Still fenced
on windows: copy/exists/remove_dir_all (set_len/read_metadata/read_dir rows) and
create_dir_all's DEEP walk (runtime SUBSLICE paths need a NUL-terminated
scratch copy). ATTEMPTED 2026-07-07 (reverted clean, findings recorded): the
plan is an Omega-side rework, no encoder work -- copy the prefix into a
`mkall_scratch: [u8; 256]` field, NUL it, pass `mkall_scratch[0..i]` (a
LITERAL-START subslice of a fixed array = the already-supported
open_at/unlink_at name idiom; the interp sees the same exact bytes, perfect
agreement). TWO blockers hit: (1) the byte-copy walk's `decreases (j, i)`
proof rejects nested-state recursion -- mkall_walk's own comment records
"recurse in the ENTRY [is] the only shape the decreases proof accepts", so
the copy body must move into the ENTRY, guarded via `requires` clauses on
the machine params (spelling to be pulled from the language guide) with
call-site proofs (j=0 and j+1 under j<i). (2) PUZZLE while there: why does
`value_call_arm_effect_blockers` NOT fire on the existing
`self.walk_rc = self.mkall_step(..)` value call, whose `mkall_mk` arm state
does a HOST CALL? Either the fence has a reachability/keying hole (check
target_key vs machine symbols for attached wrapper machines) RESOLVED same
day: the fence never ran for these probes because the pipeline ABORTS at
host-call planning (windows mkdir encoding; on the macos_arm64 cross-compile
a separate pre-existing 'AArch64 value-returning host call has no result
storage operand' gap) BEFORE emission planning. On a real Mac -- where the
pipeline reaches emission planning -- the fence WILL fire on mkall_step's
host-call arm, and per the doctrine that is CORRECT: the arm effects run
per-arm today, invisibly, because redundant best-effort mkdirs are
EEXIST-harmless. LANDED 2026-07-07: the mkall restructure is IN
(entry-recursion copy walk `mkall_copy` + statement-position byte writer
`mkall_put` with per-state guard chains + `mkall_issue` -- effects in
entries/own-dispatch, pre-fixing the macOS fence collision; `requires` was
abandoned after probing: the contract prover consumes neither cross-state
guard facts nor +1 arithmetic, only same-shape threaded facts). The scratch
prefix reaches the seam through the NEW trusted plain-name variant
`create_dir_name` (D-at trust class -- the prefix is no_nul by construction;
same mkdir row on both targets; interp arm shared with create_dir). Suite
690/0, interp fs coverage green (semantic equivalence oracle'd).
⚠️ DIAGNOSIS CORRECTED: windows deep create_dir_all is STILL a clean error,
but the failing operand is the RESULT, not the path -- probed
`MKD: result=false path=true second=true` on BOTH mkdir sites:
`first_scalar_argument_operand` cannot resolve the `let rc` result place in
these deep sibling machines (pre-existing; the original wrapper hit the
same). Next windows-seam item = that result-place resolution. ARM-TARGET
RESULT PLACE FIXED 2026-07-07b: `branch_transition_target_key`
(omega-runtime-storage body.rs) now resolves SELF/sibling Nested targets
(it only knew contained-object receivers, so `-> self.e1()` callees were
never storage-walked in the inlining case). The prior-day revert was
re-examined and the "destabilization" attributed to the recorded parallel
temp-dir race: the suspect canary's emitted code is BYTE-IDENTICAL under
the fix and three consecutive full-suite runs are clean. Pinned by
`calls/runtime_arm_target_host_result_exit` (discriminating: ENOENT errno
2 through the arm target, not ZII). LESSON: a varying-count intermittent
suite failure + small-sample baseline is NOT evidence of a regression --
diff the emitted code and re-run N times before reverting. REMAINING
frontiers for deep create_dir_all (both clean errors, parked in
canaries/pending/calls/arm_target_host_result_place): the DECREASES-walk
flavor of the same result place, and mkall_walk's arm-guard lowering at
inline depth.
Flag for the macOS confirmation session: mkall behavior now matches the
fence discipline. First
wave detail — `filesystem/windows_wrapper_results_exit` runs write_all→Ok,
create_dir×2→AlreadyExists, open→Ok{File} destructured and USED, close, remove→Ok,
remove missing→NotFound. Discipline established: host results captured into
FIELDS in the machine entry (`self.file_fd = file.fd`; a `let` folds back to the
member); terminal host calls must be let-bound (compiler fences the bare shape
with a clean diagnostic — `fail/host/terminal_host_call_value`).

**Value-call dispatch-position matrix (2026-07-07 audit).** Same-callee
value calls at MULTIPLE sites per state now deliver per-site on BOTH emission
paths (a consequence of the deferral contiguity work below) — the historical
shared-result-slot fence was verified obsolete by discriminating probes and
REMOVED (`shared_value_call_slot_blockers.rs` deleted; shapes pinned by pass
canaries `calls/runtime_value_call_same_callee_sites_exit` +
`calls/runtime_value_call_shared_slot_straight_line_exit`). Matrix status:
(a) a scalar value-call vs an INTEGER literal as a GUARD SUBJECT **WORKS**
since 2026-07-07 (was silently always-true): the syntax lowering hoists the
call into a `let` temp SHARED across arms via the match-subject memo
(per-arm temps re-ran the callee once per attempted arm — the
effectful-subject tripwire caught the first attempt at tally 43 vs 41),
typed from the callee's DECLARED return; inferred-return callees get a
clear annotate-or-bind diagnostic. Unhoisted shapes (Call-vs-Call etc.)
still hit the emission backstop (`collect_unlowered_guard_blockers`:
NeedsRuntimeExpression + a real operator = blocker — the only gate
schedule-success programs pass through; operator-None fallthrough edges
stay accepted). Pass canary `calls/runtime_value_call_guard_subject_exit`
(designed-false + true + NotEqual legs); backstop fail canary
`calls/guard_call_vs_call_rejected`.
(b) TRANSITION-ARGUMENT value calls are FIXED (2026-07-07 deep fix — the
one-day stopgap is retired): TransitionArgument leaf captures defer past the
callee's spliced body ops; leaf expansions pair with their own call op by
(role, call_ordinal) — a LOAD-BEARING tightening for all value-call shapes;
delivery pairs the Nth Call-typed argument with the Nth call record by rank
(name-verified so builtin `.unwrap()` args don't consume a rank). Pass
canaries `calls/runtime_value_call_transition_args_exit` +
`..._straight_line_exit` pin call+literal / same-callee / different-callee /
guard-free shapes.

**Contained-machine same-type aliasing FENCED (2026-07-07).** Method dispatch
resolves the receiver region by TYPE (first matching field), so
`self.b.increment()` with `a: Counter; b: Counter` silently mutated `a`. New
emission-planning blocker (`contained_receiver_blockers.rs`) mirrors the
by-type walk and errors exactly when the receiver field's offset differs from
the walk's first match — first-instance calls, single instances, and direct
field access stay accepted (zero corpus impact). The state-call plan now
carries `receiver_name` (symbol handles cross arenas vs layout). Fail canary
`calls/contained_same_type_receiver_rejected`. Deep fix = thread the receiver
offset through dispatch storage resolution (focused session, still open).

**Value-call deferral (the machinery under wrapper results)** — five ordering
faces closed, each pinned by a canary:
1. Callee-entry HostCall store before the inline guard (`run/value_call_entry_host_state_payload`).
2. Arm straight-line locals defer WITH the leaf (same canary; exit 72 leg).
3. Callee-entry FIELD-WRITE (Mutation op) defers + fires like LocalStorage/HostCall
   (`calls/runtime_value_call_entry_field_write_exit`).
4. Same-callee-twice splice conflation — scans respect the contiguous run (same canary).
5. NESTED value call in the callee entry (`self.flag = self.helper.check(1)`) —
   the nested callee splices a THIRD source_key, so the splice boundary is the
   CALLER's next own op, not the first foreign key
   (`calls/runtime_value_call_nested_entry_call_exit`, 2026-07-07).
RECIPE: every `RuntimeDispatchBodyOperationKind` that can carry a callee store
needs deferral coverage, and every defer/fire scan must respect the splice
boundary. Kinds audited 2026-07-07: HostCall/LocalStorage/Mutation covered;
StateCall* are the calls themselves; StateCallResult/Other carry no store.

**Enum methods on bare `self` WORK (2026-07-07).** `transition self {
Signal::Green -> .. }` inside an enum-attached machine, called as
`self.s.go_value()`: native gained a guard-compare-only TAG place for the
bare-self subject (tag at offset 0 per DataShape::Enum) with the CALLEE's
machine threaded down the guard-conjunct chain (`callee_key` from the
expansion's `branch_key` -- the caller's key resolved `self` to the wrong
attached data); the interpreter gained ENUM receivers in
`machine_for_instance_state` (Struct-only before: enum method calls
silently returned ZII -- exit 0). Canary
`calls/runtime_enum_self_method_exit` (differential, 3 discriminating
cases + designed-false leg).

**Value-call arm bodies (2026-07-07).** Two more traced-broken shapes now
DELIVER (closed by the deferral machinery; pinned by
`calls/runtime_value_call_dispatch_results_exit`): dispatch STRUCT result
straight to a FIELD, and free-machine runtime-branch calls bound to lets.
NEW FENCE: EFFECTFUL arm bodies in a value call ran for EVERY arm (×2 for
machine-owned; ×1-per-arm for &mut-param mutations, probed count=11) while
the result stayed correct — MachineOwned/ParameterOrAlias mutations and host
calls in a non-entry state of a value-called machine are now a clean
move-to-the-entry error (`value_call_arm_effect_blockers.rs`; fail canaries
`calls/value_call_effectful_arm_rejected` +
`calls/value_call_param_effect_arm_rejected`). Caught account_ledger live
(query_count bumped per arm: 6 for 2 queries natively, invisible to the
exit-only differential — its bump moved to the entry). Pure arm bodies (the
fs wrapper's decode `let`s) are unaffected. Real fix = guard arm-body
straight-line expansions per arm (dispatch-specialization territory).

**Leaf-path writes:** `branches/mutation.rs` is a PARALLEL decomposition to
`writes/mod.rs` — keep arms in sync (convert arm + `case_variant` tagging added;
`calls/runtime_value_call_struct_payload_cast_field_exit`,
`calls/runtime_value_call_shared_payload_name_exit`). LATENT (noted, unchased):
the leaf path skips unnamed-common-field ZERO-writes on mixed-shape case
construction while call-result frame slots are REUSED — a later call could read
stale common bytes; did not reproduce with ordering fixed.

**GUI (Windows showcase thread):** `samples/gui/image_viewer` (BMP decode in pure
Omega, StretchDIBits, flip between 3 assets) verified live. Close paths fixed
across all GUI samples: pump intercepts WM_CLOSE/WM_NCLBUTTONDOWN+HTCLOSE/
SC_CLOSE (the #32770 dialog proc swallows them); key polling gated on the new
0-arg `Gui.foreground_window` op (GetAsyncKeyState is GLOBAL — the
Ctrl+Shift+Esc mystery). darwin parity via `MacosGui::foreground_window`.
STILL OPEN: title-bar context-menu Close SENDS (not posts) WM_SYSCOMMAND —
invisible to a pump; real fix = outbound WndProc entry stubs (extern brief §12.4).

## Open work

1. [ ] **macOS runtime confirmation + promotion** (needs a Mac): run
   `canaries/run/filesystem/wrapper_metadata_repro` (expect PASS, len 5), promote
   result-asserting wrapper canaries into `native_filesystem_canaries`.
2. [ ] **Portable per-OS VALUES design — talk to Zach before building.** One
   portable wrapper, per-OS value tables: open-flag words (darwin O_CREAT 0x200 =
   msvcrt O_TRUNC → the windows `open_create` lowering is REMOVED as a data-loss
   fence; `create_new`/`open_with` would silently truncate) and stat-record
   offsets (wrapper's byte-decode hardcodes darwin `struct stat`; windows
   `_stat64`/linux `statx` differ) — blocks windows-native `metadata_path`.
3. [ ] **build.omg asset copying — DESIGN Q for Zach** (declarative `Build` asset
   list the compiler copies at emit? build.omg must describe, never do).
   Interim: exes fall back to `../imgN.bmp`.
4. [ ] Windows ops without msvcrt equivalents → Win32 calls (stat family first,
   after #2's design).
5. [ ] Title-bar context-menu Close → outbound WndProc entry stubs (§12.4).
6. [ ] linux binding tables (structural → tested) when a target is available.

## Design decisions (ratified; user reviews later)

- **D1** Human-word API (`create`/`open`/`read`/`write`/`close`/`remove`/`metadata`);
  C abbreviations only in binding tables.
- **D2** Two layers: portable `Filesystem` over raw `FilesystemHost`.
- **D3** Raw ops return ints; wrapper builds `File`/result enums in Omega.
- **D4** `create` → `_creat`/`creat` (register mode).
- **D5/D6** Raw-seam breadth COMPLETE; wrapper is the focus.
- **D7** Receiver-typed value-call resolution fixed (validation + interp).
- **D8** Flag math is branch-free bitwise (exact-arith rejects `*`/`+` on casts).
- **D9** Deref-result host calls: `dereferences_result()` → one deref after the
  call; LOCKSTEP widths/relocation-walker/encoder (+N at all three sites keyed on
  the same predicate; same discipline as `restores_stack()`).
- **D10** Machine-to-machine self value-calls work; wrappers rely on it.
- **D11** Runtime-length subslice write `write(fd, buf[0..n])` fixed.
- **D-oracle** RUN canaries run interpreter-vs-native; unsupported constructs go
  to a skip bucket.
- Misc (in force): `remove_dir_all` one `fuel=4096` budget; `*at` names trusted
  relative; `create_dir_all` intermediates best-effort; raw boundary in its own
  std module; `read_dir` single 512-byte fill per call.

## Observations (not fs, flagged for Zach)

- samples_compile on Windows hosts has exactly 4 PRE-EXISTING failures
  (A/B-verified 2026-07-06, re-confirmed 2026-07-07): `cli__systems__file_journal`
  (uses `read_metadata` — the stat family is deliberately fenced on windows until
  open-work #2) and `stdin_checksum`/`stdin_rot1`/`stdin_upper` (other
  workstream's WIP frontend errors). Judge regressions by failure-SET diff
  against these names, never raw counts.
- macOS-host runs previously showed ~85 pre-existing differential-skip failures +
  a broad aarch64 `b.ne` alignment bug in samples_compile (task chip spawned) —
  NOT this thread's work.

## Coordination

A parallel agent advances origin/main (std::time lately); files have stayed
disjoint — fetch/rebase each iteration, run the differential drift guard
immediately after every rebase, work around collisions.
