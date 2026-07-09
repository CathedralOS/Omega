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
append (flag word 9 is darwin==msvcrt portable), read_all, remove.
CREATE_NEW / OPEN_WITH UNFENCED 2026-07-08 (the portable-values payoff): the
wrapper composes open flags from per-target `FilesystemHost` provides VALUES
(O_CREATE/O_EXCL/O_TRUNC/O_APPEND in filesystem_host.omg), so windows emits
msvcrt bits (O_CREAT 0x100, O_EXCL 0x400) instead of darwin's (O_CREAT 0x200
== msvcrt O_TRUNC = the silent-truncation hazard). The windows `open_create`
lowering (`_open`, 3-arg) is RE-ENABLED. open_with's per-target single-bit
flags use `(bool as i32) << O_XXX_BIT` -- the flags are provided as per-target
BIT POSITIONS (not values), so the shift of a small positive AGREES between the
native i32 backend and the i64 interpreter (a sign-bit mask `((b<<31)>>31)&VALUE`
worked natively but the interp computes it in i64 where `1<<31` stays positive
-- a real native-vs-interp `>>` width divergence, sidestepped here). The
composed word lands in a new `open_flags` FIELD (host-call scalar-arg idiom). Pinned by
filesystem/windows_wrapper_create_new_exit -- now DIFFERENTIAL (2026-07-08):
the interpreter decodes the HOST's flag BIT POSITIONS via `host_open_flags`
(evaluator.rs, cfg!(target_os)-selected, mirroring the filesystem_host.omg
provides bits). No target threading needed -- the differential oracle compiles
for host() and runs on the host, so the host layout matches the substituted
program; the differential canary is the drift guard against the Rust mirror
diverging from the .omg source. ⚠️ LESSON: the flag migration first broke the
omega-interpreter COVERAGE tests (create_new/open_options), missed because that
iteration only ran `--test differential`; run the FULL `-p omega-interpreter`
(coverage.rs too) after any std-wrapper or interp change. Still fenced on windows:
copy/exists/remove_dir_all (set_len/read_metadata/read_dir rows) and
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
diff the emitted code and re-run N times before reverting.
BOTH REMAINING FRONTIERS RESOLVED 2026-07-07d (pending/calls/ is empty),
and the probing mapped the wall PRECISELY:
⚠️ a VALUE call that reaches -- through the SPLICED continuum
(self/sibling/free receivers; contained receivers dispatch for real) --
a RE-ENTRANT machine (transition back to its own ENTRY, `SelfTarget`
included) whose looped body carries EFFECTS (any outgoing call, host
call, MachineOwned/param mutation, or sibling Nested arm) is silently
wrong: spliced body ops run at most ONCE, not per iteration
(`self.r = self.walk("a/b/c",0,0)` with a `self.bump(..)` entry call
delivered count 0 natively vs interp 2 -- as DIRECT sibling callee, as
STATEMENT call inside a value-called machine, and as ARM target). Two
shapes stay GREEN and are canary-pinned: PURE loop-carried recursion in
value position (`calls/runtime_loop_accumulator_exit`,
dual-accumulator) and CONTAINED-receiver walks (`self.r.sum(..)` = a
real dispatch; fence skips the contained target itself but still walks
its spliced interior). A retracted same-day promotion is the cautionary
tale: the decreases-walk repro "delivered" only because its expected
result was 0 == ZII -- beware result-0 canaries proving delivery.
LANDED as one package: the arm-guard lowering pieces
(leaf-binding-resolved guards + static summary, literal `.len` fold,
ordered static comparisons in guards.rs/leaf.rs) TOGETHER with
`reentrant_value_call_blockers.rs` (emission planning): from every
value-position call's target, walk spliced-route edges transitively --
state-call records (NO reachable filter: arm-reached machines keep
interiors marked unreachable) PLUS Nested transition-target edges
(`Nested.state_symbol` names the target's ENTRY-STATE symbol, resolved
across machines like `branch_transition_target_key`) -- and reject on
the first re-entrant + effectful machine. One deduped diagnostic per
call site. Statement-position calls to the same walks keep working.
Fail canaries pin all three faces:
`calls/inline_recursive_walk_rejected` (arm),
`calls/value_call_direct_recursive_walk_rejected` (direct),
`calls/value_call_statement_recursive_walk_rejected` (transitive);
the folds' positive shape -- two call sites hitting OPPOSITE arms of a
callee guarded by `path.len > 3` over substituted literals -- is pinned
by `calls/runtime_value_call_literal_len_arm_guard_exit` (differential).
CONSEQUENCE: deep create_dir_all on windows now hits the honest fence
(`fs.create_dir_all(..)` is contained, but its interior splices the
effectful re-entrant `mkall_walk`); the remaining route is dispatch
specialization / call-with-return -- the feature that also lifts the
effectful-arm fence and the match all-arms caveat. ⚠️ macOS session:
darwin wrapper canaries that VALUE-call remove_dir_all/create_dir_all
may now hit this fence -- that is the fence working; restructure to
statement calls or park behind call-with-return.
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
2. [ ] **Portable per-OS VALUES — SETTLED (Zach, chat 2026-07-07), now
   engineering.** The Rust split, mapped onto our settled Binding-sum provides
   tables: (a) per-target `provides` files carry named VALUES (flag words) next
   to the binding rows — the portable wrapper asks for `Filesystem.O_CREATE`,
   the target's table supplies the number (fixes: darwin O_CREAT 0x200 = msvcrt
   O_TRUNC, why windows `open_create` is a data-loss fence today); (b) stat
   decode leaves portable code entirely — the provides row carries a declarative
   LAYOUT MAP (field -> offset/width) the SEAM applies, normalizing into ONE
   Omega-defined `Metadata` record ("raw ops return ints" becomes "raw ops
   return ints or a defined record"). Interp reads the same tables for virtual
   semantics. File shape: `std/targets/<target>.omg` (target def) +
   `std/targets/<target>/<subsystem>.provides.omg`. Unblocks windows
   `metadata_path`, `create_new`/`open_with`, file_journal.
   RUNG 1 LANDED (hosted provides consumption): authored rows MERGE into the
   hosted plan when their target resolves to the COMPILE target (additive;
   colliding with a built-in binding = loud error; `demo_target` and other
   targets' rows stay inert — the filter is LOAD-BEARING, the old code
   ignored provider targets entirely). Import LIBRARIES now ride the binding
   end-to-end (SymbolPlan.import_library -> FinalImageImport.library ->
   PeImportThunk.library; the PE catalog lookup is the empty-string fallback;
   unit test pins binding-beats-catalog). HostCall.has_result records whether
   argument[0] is the prepended RESULT place (the two result collectors set
   it) — selection's Unknown arm marshals result + declared args on that
   signal; a VOID authored import call is fenced at planning (it would
   misread its first arg as the result place). Fixed in passing: the
   unknown-key loud-collision check was DEAD (compared capability_name() to
   "Unknown" but it renders "<unknown>"); fail canary
   capabilities/host_provides_two_unknown_rejected pins the live merge on
   every host (local_unchecked = host target). RUNG 2 LANDED (2026-07-08):
   authored imports work END TO END on hosted windows under the FIELD
   discipline -- pass canary capabilities/windows_provides_import_exit
   (`beep -> DllImport("msvcrt.dll","abs")`, abs(-42) -> 42,
   windows-gated, NATIVE-ONLY: no interpreter provider for authored
   bindings yet -- open question, relates to the build.omg real-fs
   provider). The rung-1 "result-place frame slot" diagnosis was WRONG in
   an instructive way: the miss is NOT provides-specific -- a host-call
   operand that is a FRAME LOCAL fails whenever the statement is selected
   under a duplicate dispatch context (slots are dispatch-keyed;
   fields/literals live in the machine region and always resolve -- the
   raw-canary FIELD discipline was load-bearing all along). Catalog-wide
   face CLOSED 2026-07-08 (diagnosis was WRONG -- not dispatch-keyed
   duplication; the dispatch-body STORAGE builder simply had NO HostCall
   arm, so a host-call result bound to a LOCAL in a dispatching state
   never got a frame slot; fields dodged it via the machine region,
   straight-line states via their own locals scan). Fix: HostCall arm in
   omega-runtime-storage/body.rs allocates the result-local slot
   (idempotent via append_local_slot's exists-guard). Pinned by
   filesystem/runtime_local_host_result_dispatch_exit (differential,
   open-of-absent -> fd < 0 -> 70). RUNG V1 LANDED
   (2026-07-08): per-target VALUE rows parse and ride the row stream --
   `O_CREATE -> 32768` (integer-led RHS, zero new grammar per the extern
   brief; snapshot kind "value"). They are constants, NOT call bindings:
   the ABI merge skips them BEFORE the operation-key checks (two value
   rows must not trip the unknown-key collision -- pinned by the extended
   host_provides_binding_forms canary, both inert-demo_target and
   host-merged flavors probed). Whitespace-run defect fixed in the three
   merge error messages (a python single-backslash heredoc landmine --
   scripts writing Rust string continuations need `\`).
   RUNG V2 LANDED (2026-07-08): the row IS the declaration -- no new
   const surface needed. `Trait::NAME` paths substitute the SELECTED
   target's Value row in a pre-resolution desugar pass
   (pipeline/provides_values.rs; const-v0 discipline -- each use becomes
   the literal, no downstream stage grows a concept, interp sees the
   same substituted program so the differential holds by construction;
   the pass is wired into BOTH pipeline entries, compile AND
   compile_to_checked -- forgetting the second made the interp oracle
   read ZII, caught by the new canary's interp leg). Loud edges: const
   vs row collision, two same-target rows with different values,
   wrong-target reference (targeted error naming the declaring target).
   Canaries: capabilities/runtime_provides_value_exit (differential RUN,
   63+7 -> 70 via local_unchecked=host) +
   fail/capabilities/provides_value_wrong_target_rejected.
   FOUND+CLOSED (2026-07-08): an unresolved two-segment path in value
   position (`Nowhere::NOPE`, or a bogus case `Signal::Blue`) read ZII 0
   in BOTH runtimes -- the multi-segment sibling of the closed bare-name
   existence check. Now a clean validation error (omega-validation
   calls.rs: a two-segment Name whose head AND leaf symbols both stay
   unresolved names nothing; a real qualified case resolves before this
   stage). Fail canary expressions/undeclared_two_segment_path_rejected
   + differential pass twin expressions/runtime_qualified_case_value_exit.
   NEXT: layout maps (stat normalization), std targets files +
   WINDOWS_IMPORT_ROWS migration, wrapper flag-word migration
   (unfences create_new/open_with on windows), the let-local dispatch
   face, interp story for authored bindings.
   ⚠️ BRIEF DRIFT noted (not touched -- freestanding thread's call): the
   extern brief revised `VtableSlot(index)` to `VtableField(field)`
   (field model, decided 2026-07-04) AFTER the parse landed; the
   implemented sum still spells VtableSlot.
3. [ ] **build.omg = CODE with granted capabilities — SETTLED (Zach, chat
   2026-07-07), now engineering.** No declarative asset list; build.omg runs
   INTERPRETED with a granted, scoped `Filesystem` capability (read: source
   tree; write: build dir) and copies assets itself — "this is the whole point
   of making the build system code." The old "describe, never do" framing is
   RETIRED: capability grants ARE the audit surface. Engineering rung: a
   REAL-fs interpreter provider (today's is virtual-only) + grant plumbing +
   path scoping. Native fences are irrelevant here (interp is
   reference-complete — deep create_dir_all works interpreted). Also settled:
   build.omg is the home for define-LIKE constant statements (immutable
   bindings; never C++ define/undefine mutability) and for TARGET SELECTION —
   it picks a std target file (or `host`), possibly composes a custom one from
   existing mechanisms. NOTED (Zach): the accepted-target set is still
   ultimately closed by the compiler — architectures, object formats, binding
   mechanisms need codegen; build.omg composes OS personalities from that
   closed set, it cannot mint new architectures. Interim until the rung lands:
   exes fall back to `../imgN.bmp`.
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

## Windows metadata / stat migration -- SCOPED, needs a decode refactor first (2026-07-08)

Investigated deeply, then REVERTED (too big for one clean pass). Findings so
the next attempt is mechanical:
- msvcrt `_stat64` WIRES and works: an import row (`Filesystem stat ->
  msvcrt.dll _stat64`) + `read_metadata` lowering unfences the raw stat op;
  `exists`/`try_exists` (which use only the stat RC, not the decode) then pass
  natively on windows (probe exit 70). BUT wiring stat also makes the DECODING
  methods compile with wrong offsets -- can't ship the wiring alone.
- The stat DECODE is per-target by OFFSET *and* WIDTH: windows `_stat64`
  (size@24, mode@6, mtime@40, atime@32, ctime=creation@48, nlink@8, dev@0,
  rdev@16, sizeof 56) has 2-byte ino/uid/gid where darwin has 8/4-byte, and
  lacks blocks/blksize/change-time. Per-target ST_*_OFF provides values, with
  the windows-absent fields at DISTINCT SYNTHETIC TAIL offsets (>=64, past
  `_stat64`'s 0..55) -- NOT a shared zero-region: the hermetic interpreter models
  each as a distinct non-zero value (ino=1000000, uid=501, changed=1000000050,
  blocks=8, blksize=4096) which would collide at one offset. A real native
  `_stat64` leaves the tail zero, so native windows reports 0 for those (honest).
- CORRECTION: `stat_buf[FilesystemHost::ST_SIZE_OFF + k]` COMPILES but on NATIVE
  windows READS OFFSET 0 (the earlier "compiles+runs" only checked compilation).
  The raw `_stat64` seam is fine (a raw read + inline `buf[24]` gives the real
  size); the miscompile is a VALUE-MACHINE codegen bug -- `self.stat_buf[<non-
  literal index>]` inside the `decode_metadata` value machine resolves against a
  wrong base. See [[value-machine-computed-index-miscompile]] +
  `canaries/pending/backend/value_machine_computed_index_self_array/`.
- The interpreter's `write_fs_stat` must fill the HOST's layout too (a
  cfg!(target_os) `host_stat_offsets` mirror) so the differential (compiles for
  host()) agrees. This was prototyped, verified green on the interpreter (all
  metadata coverage), and reverted with Phase A.
- ~~BLOCKER that forced the revert: the decode is DUPLICATED across~~
  ~~metadata_path + metadata(file, via fstat) + symlink_metadata~~
  **PREREQ DONE (2026-07-08):** the three per-caller decode bodies (was ~240
  duplicated `stat_buf[N]` lines) now collapse to ONE `Filesystem::decode_metadata(&mut self) -> Metadata`
  value machine at [filesystem.omg:637](omega/language/std/filesystem.omg). Each
  caller fills `stat_buf` via its raw op then `let m: Metadata = self.decode_metadata();
  MetadataResult::Ok { meta: m }`. `is_symlink` is computed from the mode's
  S_IFLNK bits (40960) in the shared machine -- correct for all three since
  stat/fstat follow symlinks (mode never a link) while lstat's can be. Verified
  behavior-preserving: interpreter runs the decode (coverage 68/0 + differential
  11/0), canary_suite 711/0, samples_compile the 4 documented pre-existing
  windows fails only. THE SINGLE DECODE SITE is now the migration target.
- BLOCKED on [[value-machine-computed-index-miscompile]]: the whole migration
  needs `stat_buf[ST_*_OFF + k]` (a non-literal index) inside `decode_metadata`,
  which native-miscompiles today. FIX THAT codegen bug FIRST (it is a general
  silent miscompile, not fs-specific -- receiver-base threading for indexed reads
  in value machines).
- THEN re-land (all prototyped+verified this session, reverted as commit
  72b1b112a's revert): (1) per-target ST_*_OFF provides + interp
  `host_stat_offsets` cfg mirror with DISTINCT synthetic tail offsets for
  windows; (2) `("Filesystem","stat","msvcrt.dll","_stat64")` + `_fstat64` import
  rows + `read_metadata`/`read_file_metadata` lowerings in windows.rs
  (`read_symlink_metadata` stays fenced -- msvcrt has no lstat); (3) a native
  windows metadata canary asserting size/is_file/!is_dir. The refactor (literal
  darwin offsets) is the CORRECT shipped state until then.

## Wrapper dark-method coverage + a parked backend gap (2026-07-08)

Audited the 55 Filesystem wrapper methods for canary coverage; wrote
`filesystem/windows_wrapper_dark_methods_exit` (differential) exercising
create/sync/try_clone(dup)/set_permissions -- all previously untested.
FOUND + FIXED: `set_permissions`/`set_file_permissions` passed `perms.mode`
(a member of a by-value struct param) DIRECTLY to chmod/fchmod, which fails
to resolve under some dispatch contexts -- exactly the hazard the wrapper's
`file_fd` scratch field already documents for `file.fd`. They now capture
into a new `perm_mode` field in the entry first (the established idiom).
PARKED backend gap (pending/host/self_value_call_literal_arg) -- ROOT CAUSE
re-diagnosed 2026-07-08 (two earlier guesses were wrong): a path LITERAL
passed to a SELF value call (`self.doit("lit")` -> `self.raw.open(path)`)
gets NO data object that the callee's host call can find. The value-call
ALIAS BINDING resolution keys the param literal to the CALLEE's source_key
for a SELF call but to the CALLER's for a CONTAINED call, while the
static-string collectors key the literal to the caller -- so the SELF-call
lookup by (callee_key, bytes) misses. Compounded by discarded/unused
results (`_ = f("lit")`) lowering to a LocalData whose local isn't in
state_storage.locals, so the collector never visits its call args. A
data-planning-only fix (collect unrequired call-initializer literals) was
attempted and REVERTED -- insufficient without fixing the key agreement.
Real fix is backend value-call arg handling; the std wrapper (contained
receivers, used results) is unaffected, so this is latent, not blocking.

## ⚠️ Portable-values FRONTIER IS DESIGN-GATED (mapped 2026-07-08, needs Zach)

The provides/portable-values ladder reached the point where the remaining
rungs are DESIGN decisions, not mechanical work. Two forks block the
headline payoff (unfencing windows create_new/open_with):

**Flag-migration plan (facts, fully scouted).** The wrapper hardcodes
darwin flag words (O_CREAT=512, O_EXCL=2048, O_TRUNC=1024, O_APPEND=8,
access=0x3). To migrate to `FilesystemHost::O_CREATE` per-target refs
(V2 substitution already works) needs THREE coordinated pieces:
(1) a bundled per-target flag provides module + injection — the
`substitute_native_gui_provider` pattern (stages.rs) injects a bundled
std module gated on a boundary trait, but it is NATIVE-ONLY (the
interpreter's compile_to_checked keeps abstract traits), so flags need
injection on BOTH paths; (2) wrapper literal->ref migration; (3)
interpreter target-aware flag decode — the decode is in evaluator.rs
`virtual_open_flags` + the `open_create` arm, hardcoding darwin bits
(0x200/0x400/0x8/0x3, EXCL 2048); it MUST match whatever numerology the
wrapper emits. ⚠️ LANDMINE: on a Windows host `host()` == `windows_x64`
(Coff/X86_64/8/8), so compile-target-None already resolves to windows —
migrating the wrapper without the interpreter flip breaks the
windows_wrapper interp-oracle canaries immediately. The darwin O_CREAT
0x200 == msvcrt O_TRUNC 0x200 collision means the interpreter CANNOT
decode semantically from bits alone; it needs the target.
🔷 DESIGN FORK (Zach): where do per-OS values live as the SINGLE source
of truth -- a Rust const table (interpreter + native both read it) or the
`.omg` provides rows (settled design says .omg, but then the interpreter
must READ provides tables for flag decode, more machinery)? This is the
crux; picking wrong duplicates the values (ZII "clear data-to-data"
concern). NOT settled -> not built.

**Silent typo hazard in provides target names (found 2026-07-08).** A
`provides` block naming an UNKNOWN target (`windows_x86` typo) silently
voids the whole block: an unreferenced value row = ZERO errors; a binding
= a misleading "no native lowering" at the call, not "unknown target". A
referenced value row IS caught (the V2 wrong-target error). 🔷 The clean
fix ("unknown provides target = error") needs a canonical VALID-TARGET-
NAME set, which is itself unsettled: `uefi_x64` (freestanding, 3 corpus
uses) and `demo_target` (placeholder, host_provides_binding_forms) are
NOT in `from_omega_target_name`, and freestanding provides labels are
DECORATIVE (build_freestanding_abi_plan takes ALL rows, no target
filter) -- plus freestanding-vs-hosted isn't known at the pre-resolution
point where names are processed. So this touches freestanding target
naming ([[first-boot-ladder]]/[[extern-binding-sum]] lane). Parked, not
fenced unilaterally.

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
