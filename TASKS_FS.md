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
- **darwin/aarch64** — complete AND runtime-CONFIRMED on a real Mac
  (2026-07-08c): native_filesystem_canaries 83/0 (create→flock breadth incl.
  variadic `open_create`, deref-result `___error`, stack-marshalled mode, the
  full wrapper incl. metadata payload). 2026-07-07: the week's value-call
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
(coverage.rs too) after any std-wrapper or interp change. UNFENCED 2026-07-08
(all off the fenced list): `exists`/`try_exists` (stat-rc only, side effect of
the stat migration), `set_len` (msvcrt `_chsize_s`), and `copy` (set_len wired +
its chmod mode arg moved to the `perm_mode` field so it stops eliding into a
computed host-call arg). Canaries: `windows_wrapper_{exists,set_len,copy}_exit`.
O_BINARY SWEEP DONE (2026-07-08e): EVERY wrapper content open/create now
carries `FilesystemHost::O_BINARY` (32768 on windows, 0 on posix -- Rust I/O
is byte-exact; msvcrt TEXT mode was CRLF-translating and ^Z-truncating):
`open`/`read_all` read flags, `create`/`write_all`/`copy`-dst migrated from
raw `create` (`_creat` = always TEXT) to `open_create` with composed
O_WRONLY|O_CREATE|O_TRUNC|O_BINARY, `create_new`/`open_with` OR it in, and
`append` now COMPOSES `O_WRONLY | (1 << O_APPEND_BIT) | O_BINARY` (the old
literal `9` was a latent LINUX bug -- O_APPEND is bit 10 there, not 3).
Directory opens (read_dir family) stay flag-0 (windows-fenced; no content).
Creation mode unified to Rust's 438/0o666 (umask trims; nothing asserted the
old 420). TWO deeper findings shipped with it:
(1) ⚠️ wrapper `open_create` NEVER COMPILED on darwin/arm64 (windows-verified
only; masked in the differential by the arithmetic-regression early-panic --
see Observations): the arm64 variadic encoder demands a compile-time-IMMEDIATE
mode and the `create_mode` FIELD resolves to a runtime place. Fix: mode is a
LITERAL `438` at every open_create call site (win64 takes either; the field
idiom is only for COMPOSED words like open_flags). This UNBLOCKED
create_new/open_with/copy/create/write_all on darwin native -- verified
native+interp 70 here.
(2) backend: the write arm resolves a fixed-array FIELD forwarded through a
value-call param (`fs.write_all(path, self.bin_src)`) via a new LAST-RESORT
alias-resolved fixed-array probe (`alias_resolved_fixed_array_length_at`,
operands.rs) -- kept last so the descriptor route (copy's `&mut buffer`)
always wins with its proven address.
`windows_wrapper_copy_exit` gained a BINARY leg (CR/LF/^Z bytes copied
byte-exactly; read-back via the RAW seam) and its text-era caveat is gone.
PARAM-NAME SHADOWING FIXED (2026-07-08f, was pending/host/
wrapper_read_buffer_decoy): the "wrong buffer" was NAME-COLLISION, not
kind-match -- renaming the decoy field fixed it, which pinned the root: the
direct callee-scope operand resolution falls through to MACHINE-OWNED NAME
matching, so a caller field named like a forwarded wrapper param captured the
operand (a caller `buffer: [u8;64]` swallowed the read -> native 73; a caller
`count: usize` ZII 0 made the read request 0 bytes -> native 72; interp 70
both). FIX (operands.rs): the ALIAS REWRITE (the forwarded param's semantic
truth) now precedes direct resolution in BOTH `address_argument_operand_at`
and `scalar_argument_operand_at` -- including the forwarded-LITERAL immediate
probe, which also sat behind the direct fallback (the count shadow only
cleared once the literal probe moved up). Both alias probes return None
unless an alias actually rewrote, so non-forwarded args are untouched.
Promoted as `filesystem/wrapper_param_shadow_exit` (differential; BOTH decoy
fields declared, binary roundtrip must land in the SPELLED buffer/count
byte-exactly); pending/host/ is empty again. The copy canary's raw read-back
note still stands as written history (the raw seam was never affected). Still fenced on windows: `read_dir` + everything that walks it
(`remove_dir_all`), and create_dir_all's DEEP walk (runtime SUBSLICE paths need a
NUL-terminated scratch copy). ATTEMPTED 2026-07-07 (reverted clean, findings recorded): the
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
PARAM/LOCAL receivers FENCED TOO (2026-07-08d, closes the windows
`let x = meta.is_file()` mis-delivery note): the fence's old "lenient
skip-if-unresolved" for non-field receivers assumed they resolve by other
routes — probed FALSE on darwin native (matrix + decoy discrimination):
`meta.is_file()` on a state-PARAM receiver reads the first same-typed FIELD's
storage when one exists (a decoy field with a directory mode flips the
answer — silent wrong-receiver) and ZII/garbage when none does; binding kind
(local vs field) is irrelevant. The direct-receiver arm now BLOCKS a receiver
that is not a field, with two precise skips: `self` (D10 machine-to-machine
self calls dispatch on the caller's own region) and the STATIC spelling
(`Worker::run(pair)` / `Duration::from_secs(n)` carry the TYPE name in
receiver position; receiverless callees read no receiver storage — their
by-value params deliver via leaf expansion, runtime-pinned by
`calls/runtime_attached_machine_struct_arg_exit`; without this skip the fence
falsely caught 8 green canaries, incl. the time thread's constructors).
Suite failure-set verified byte-identical to pre-fence. Fail canary
`fail/calls/param_receiver_method_rejected`; the prescribed idiom
(param → same-typed field, call through the field) is differential-pinned by
`filesystem/field_receiver_method_exit`. Same deep fix as above.

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

1. [x] **macOS runtime confirmation + promotion — ✅ DONE (2026-07-08c, real
   macOS/aarch64 session).** `wrapper_metadata_repro` ran PASS (meta.len == 5)
   and is PROMOTED to `canaries/pass/filesystem/native_wrapper_metadata` +
   `native_wrapper_metadata_passes` in native_filesystem_canaries
   (canaries/run/filesystem/ is now empty). The FULL macOS gate is GREEN for
   the first time: 83/0 -- the 6 GUI/input failures (the fence-flag predicted
   below) were fixed by making the darwin GUI std machines fence-conformant:
   `MacosGui::msg_peek` arms are now PURE terminal returns (the `self.r32 = 1`
   arm mutation moved to direct returns) and `MacosInput::key_state` split into
   a PURE `map_keycode` value machine + effects (map value-call, CG query) in
   the ENTRY -- the unmapped arm discards the harmless stray keycode-0 query.
   This ALSO fixed 2 pre-existing suite failures on macOS
   (`runtime_gui_window_lifecycle_exit`, `runtime_user32_key_state_exit`),
   A/B-verified failure-set diff (101 -> 99, strict subset; the 99 are the
   known pre-existing macOS-host set). Bonus confirmation: a nested SELF value
   call in a value-called callee's ENTRY (`self.keycode =
   self.map_keycode(vk)` inside value-called key_state) WORKS on darwin
   native -- the self-receiver sibling of deferral face 5.
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
   STD TARGETS FILES LANDED (2026-07-08g, the settled file shape):
   `std/targets/<target>.omg` (target def) + `std/targets/<target>/
   filesystem.provides.omg` (the rows) exist for all four targets; the four
   inline provides blocks left filesystem_host.omg (241 -> 131 lines), which
   now imports the four target defs so `use ...::filesystem_host;` stays a
   single self-contained entry point (the compiler's target filter keeps
   non-selected rows inert, unchanged). Compiler side: `source_path_candidates`
   (frontend.rs) gained the `<name>.provides.omg` compound-suffix candidate
   (plain `<name>.omg` wins when both exist). Provides blocks resolve their
   trait by NAME globally -- no import cycle back to filesystem_host needed.
   build.omg's target selection (#3) now has real files to pick; time_host
   has no provides rows yet (nothing to migrate there).
   NEXT: layout maps (stat normalization) -- superseded in practice by the
   shipped ST_*_OFF value rows; revisit only if a declarative map earns its
   keep -- WINDOWS_IMPORT_ROWS migration (authored import rows in the
   provides files replacing the Rust table; NEEDS A WINDOWS SESSION to
   runtime-verify), the let-local dispatch face, interp story for authored
   bindings.
   ⚠️ LET-LOCAL FACE WIDENED (2026-07-08g): `Filesystem::open_with` (Rust
   OpenOptions) turned out NATIVELY UNCOMPILABLE in value-call position --
   invisible until its FIRST caller (the new coverage canary; unreachable
   machine bodies are never lowered, and the raw-seam OpenOptions canaries
   hand-compute ints). A MachineOwned write in a value-called machine plans
   only TRIVIAL values: compound-over-locals, RMW-over-local, and even
   RMW-over-PARAM-MEMBER all refuse with the loud "needs runtime storage
   write lowering" (never a miscompile; copy's complex perm_mode write
   lowers because its value reads only FIELDS). The interpreter runs the
   full six-leg OpenOptions matrix to 70. Parked:
   `canaries/pending/host/wrapper_open_with_matrix` (write+create / read /
   truncate / append / create_new-exists / read-absent); promote to
   pass/filesystem + differential when the write-value lowering lands --
   the same face also still blocks host-call args reading locals under
   duplicated dispatch (rung-2 note). LESSON (coverage doctrine): zero-caller
   std machines are UNVERIFIED code -- "compiles as part of std" means
   nothing; every wrapper method needs at least one calling canary. The
   audit list of remaining zero-caller methods -- LOCK FAMILY + metadata(File)
   COVERED 2026-07-08h (`wrapper_lock_metadata_exit`, native 70 + interp 70;
   macos-gated in native_filesystem_canaries like native_flock -- flock has no
   msvcrt row, so it stays out of the differential RUN_CANARIES, which a
   windows host also runs). FOUND + FIXED by that canary:
   `try_lock`/`try_lock_shared` had an errno() host call in their contended
   ARM -- the effectful-arm fence refused every value-position caller
   (zero-caller code again); both now capture errno into a new `lock_errno`
   field in the ENTRY (the try_exists idiom; stale-but-unread on the acquired
   path). STILL zero-caller: set_times, set_owner family, symlink_metadata.
   DIR-WALK FAMILY: NOT darwin-runnable after all -- probed 2026-07-08h,
   INTERP-ONLY on every native target via THREE distinct honest fences:
   (1) create_dir_all's interior value-calls the re-entrant `mkall_copy`
   (fires even for a `_ =` discard caller); (2) read_dir_count/nth (so
   is_empty/stats too) do host READS in loop ARMS -- the try_lock
   entry-capture idiom cannot apply to a loop of reads; (3) remove_dir_all's
   `rda` is a genuine recursive CYCLE (depth into a DIFFERENT dirfd +
   sibling drain) that specialization refuses. Common unlock =
   CALL-WITH-RETURN (already named as the fence-lifting feature); (3) also
   needs an rda entry-recursion restructure (mkall precedent, but TWO
   recursion sites). Parked with the full six-leg matrix (interp 70):
   `canaries/pending/host/dir_walk_wrappers_native`; promote macos-gated
   when the unlocks land. ⚠️ This RAISES call-with-return's priority: it is
   now the single blocker for the whole native dir-walk wrapper surface on
   EVERY target (not just windows) -- flag for prioritization.
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
- **D12** Per-OS values = DATA in `.omg` provides rows + a `cfg!(target_os)`
  interpreter Rust mirror, differential-guarded. Duplication is interim debt
  that self-hosting dissolves (the Omega interpreter will read the same rows).
- **D13** Per-OS decode = variation-as-DATA at the edge (provides) + ONE generic
  central decoder producing a neutral `Metadata`. Strict "portable core never
  touches a raw OS byte" (a per-target-CODE normalize behind the seam) is
  deferred — a new mechanism, marginal purity.
- **D14** (⚖️ LEANING, not ratified) `Metadata` absent/unknown fields = `0`
  (ZII-clean); OS quirks like unix `uid 0 == root` remap at the SEAM to keep
  `0 = none` pristine. No `-1` sentinel (breaks ZII), no `Option`. A cased-data
  field (`Owner{None|Root|Id}`) is the clearer-but-heavier alternative, deferred.
- **D15** A `provides` target name is valid iff compiler-defined OR build-defined,
  else error (needs freestanding labels `uefi_x64`/`demo_target` registered first).
- **D-oracle** RUN canaries run interpreter-vs-native; unsupported constructs go
  to a skip bucket.
- Misc (in force): `remove_dir_all` one `fuel=4096` budget; `*at` names trusted
  relative; `create_dir_all` intermediates best-effort; raw boundary in its own
  std module; `read_dir` single 512-byte fill per call.

## Windows metadata / stat migration -- ✅ DONE (2026-07-08)

NATIVE WINDOWS METADATA WORKS. The decode reads per-target `_stat64` offsets;
canary `filesystem/windows_wrapper_metadata_exit` (RUN + differential 70);
`cli__systems__file_journal` now compiles+runs on windows (4 pre-existing sample
fails -> 3). The blocker was NOT the offsets -- it was the value-machine
computed-index miscompile ([[value-machine-computed-index-miscompile]]), now
fixed (pure-const index fold). Follow-ups: `read_symlink_metadata` stays fenced
(msvcrt has no lstat); the `let x = meta.is_file()` mis-delivery is EXPLAINED
+ FENCED 2026-07-08d (param-receiver method dispatch — see the aliasing-fence
paragraph; the metadata canary's inline `(meta.mode & 61440) == 32768` form
and the new field-receiver idiom are the two sanctioned shapes). How it
landed, for reference:
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
- ~~BLOCKED on [[value-machine-computed-index-miscompile]]~~ **RESOLVED**
  (verified 2026-07-08): the pure-const binary index fold landed (parallel
  thread); the pending canary was PROMOTED to
  `canaries/pass/backend/value_machine_const_index_self_array_exit` (+
  `..._self_array_local_index_exit`), both exit 99 in the differential
  RUN_CANARIES. `pending/backend/` no longer exists. The whole stat migration
  is DONE (header above); this bullet is retained only as the resolved history.
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
SELF-value-call literal arg -- ✅ FIXED 2026-07-08 (USED-result form). ROOT
CAUSE (traced): a path LITERAL passed to a SELF value call
(`self.probe("lit")` -> `self.raw.open(path)`) reaches the callee host call
as its `path` param ALIASED to the caller's literal, but the value-call ALIAS
BINDING resolution (`resolve_runtime_alias_binding_handle`) resolves a SELF
call to the CALLEE's source_key while a CONTAINED call resolves to the
CALLER's; the static-string collectors always key the literal's data object to
the CALLER's statement, so `aliased_literal_data_object`'s (resolved_key,
bytes) lookup MISSED for the SELF case (contained matched). Manifested as the
`open` arm producing NO operands -> "no encodable call sequence" (x86_64) /
"AArch64 value-returning host call has no result storage operand" (the latter
was a downstream SYMPTOM of the missing path operand, not a separate aarch64
bug). FIX: `aliased_literal_data_object`
([operands.rs:1416](compiler/omega-rs/backend/omega-instruction-selection/src/selection/host_operations/operands.rs))
now falls back to a BYTES-ONLY match when the state-keyed lookup misses --
every data object with identical bytes is the same read-only C string, so any
match is correctness-equivalent (worst case a missed dedup, never a wrong
pointer). The state-keyed match is still TRIED FIRST (contained receivers
unchanged). Pinned by RUN canary
`filesystem/self_value_call_literal_path_exit` (differential, native + interp
both exit 70: create with a literal, then reopen the SAME literal THROUGH a
self value call -> open must find the file). Wired into differential
RUN_CANARIES + a windows-gated suite test.
DISCARDED-result form -- ✅ RESOLVED + PROMOTED 2026-07-08b (the "collection
gap" half of the old diagnosis was WRONG): `_ = self.doit("lit")` lowers
through the STATEMENT-call path (real argument materialization), NOT the
value-call splice, so the literal is delivered normally -- probed on darwin
with the pre-fix operands.rs (A/B, rebuild-verified): masked, unmasked-write,
and errno-discriminated shapes all pass BOTH engines even WITHOUT the
bytes-only fallback. The historical windows "no encodable call sequence" on
this shape was fixed by the intervening local-slot/dispatch work. The pending
canary is DELETED (pending/host/ is empty); promoted as RUN canary
`filesystem/discarded_self_call_literal_errno_exit` (differential; the
discarded self-call opens an ABSENT path, then errno must be ENOENT 2 -- a
dropped call reads ZII 0, a garbled path a different errno), wired into
RUN_CANARIES + a windows-gated suite test next to its used-result twin.
LESSON (canary craft): a discarded-call repro that "compiles OK" proves
nothing about the VALUE-call path -- `_ =` and `let x =`-then-use take
different lowering routes; pin both shapes separately.

## Portable-values / Metadata design questions — RESOLVED (chat 2026-07-08)

The forks this section used to flag are decided (the flag + stat migrations
already shipped -- create_new/open_with/metadata are unfenced). Recorded as
D12–D15 below. One (Q3) is an explicit LEANING, not fully settled.

**Q1 — single source of truth for per-OS values (RESOLVED, D12).** Per-OS
numbers (flag bit positions, stat offsets) live as DATA in the `.omg`
provides rows; the interpreter carries a `cfg!(target_os)` Rust MIRROR
(`host_open_flags` / `host_stat_offsets`), differential-canary-guarded
against drift. The duplication is accepted INTERIM debt: the interpreter is
the language runtime implemented in Rust *until self-hosting*, at which point
it reads the same provides rows the compiler does and the mirror dissolves
for free. Not a fork — keep the mirror, canary-guarded.

**Q2 — where the per-OS decode lives / "push complexity to the edges"
(RESOLVED, D13; deeper option noted).** The per-OS VARIATION (layout offsets,
and value quirks like unix root-0) is at the EDGE as provides DATA + seam-side
normalization; the portable core runs ONE generic, data-driven decoder and
sees a clean neutral `Metadata`. The stricter reading — portable code never
touches a raw OS byte at all — would move the decoder itself behind the seam,
which needs per-target CODE (Omega has per-target DATA via provides, not
per-target code); that is a NEW mechanism, more machinery for marginal purity,
DEFERRED. Current shape (variation-as-data at the edge + one uniform central
decoder) is the accepted realization.

**Q3 — Metadata fields an OS lacks (LEANING, not fully settled; D14).** `0` =
"none / unknown" is the ZII-clean default — a zeroed `Metadata` is a valid
"we don't know" record. Windows reporting `0` for uid/gid/ino/blocks is
CORRECT (it has no such concept), not a hack. Real values stay 1:1 EXCEPT an
OS quirk like unix `uid 0 == root`, which the SEAM remaps to a distinct Omega
"root" value on the way IN, so our `0 = none` is never polluted — an edge-side
translation (ties to Q2), only real once a unix native target lands. NO
widening to a `-1` sentinel (breaks ZII, which is much desired), NO `Option`;
don't bend the clean abstraction for OS quirks. ⚖️ NOT fully settled: a more
advanced CASED-DATA field (`Owner { None | Root | Id(n) }`) gives explicit
clarity but is more baggage on an identifier and is functionally ~ `Option<>`;
we LEAN the plain-`0`-sentinel for now and revisit if it bites.

**Q4 — silent typo in provides target names (RESOLVED → engineering, D15).**
Rule: a `provides` target name is valid iff it is COMPILER-defined (the closed
arch/format target set) OR BUILD-defined (build.omg); else = error. Closes the
silent-void footgun (`windows_x86` typo). Engineering, not a fork — the one
real gotcha: freestanding provides labels (`uefi_x64`, `demo_target`) are
currently DECORATIVE (`build_freestanding_abi_plan` ignores the label), so they
must be REGISTERED as real target names BEFORE the check switches on, else it
wrongly rejects them. See Open-work #4.

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
- ⚠️ NATIVE MISCOMPILE on main (observed 2026-07-08, present on clean HEAD
  77d39fbfb with all fs changes stashed — the parallel sum-payload-range /
  const-fold thread's territory, NOT this thread's): the differential test
  `-p omega-interpreter --test differential` fails two with native≠interp:
  `dual_accumulator` sample (interp 70 vs native 71) and
  `arithmetic/runtime_cast_in_guard_exit` (native 71 vs suite-expected 70).
  Since the suite pins native as correct-by-definition, native 71 is a real
  regression. Flagged for the arithmetic thread — likely the folded-constant
  domain work leaking a sign/width into a cast-in-guard.
  ⚠️⚠️ SECOND-ORDER COST (found 2026-07-08e): the supported-canaries test
  PANICS AT THE FIRST MISMATCH, so everything alphabetically after
  `arithmetic/runtime_cast_in_guard_exit` in RUN_CANARIES is UNVERIFIED while
  the regression stands — it masked "wrapper open_create never compiled on
  darwin arm64" for days. Fix the regression (or make the differential
  collect-all-failures instead of panic-at-first) with priority. (6 GUI/input samples in
  native_filesystem_canaries also fail, but those are the KNOWN effectful-arm
  fence on `MacosInput::key_state`/`MacosGui::msg_peek` value calls, per the
  fence doctrine above — expected, not a regression.)

## Coordination

A parallel agent advances origin/main (std::time lately); files have stayed
disjoint — fetch/rebase each iteration, run the differential drift guard
immediately after every rebase, work around collisions.
