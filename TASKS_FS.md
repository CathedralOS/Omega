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
   OPEN_WITH UNBLOCKED (2026-07-08j; was "LET-LOCAL FACE WIDENED"):
   the debug trace (OMEGA_DEBUG_MUTATION_SELECTION=1) showed the failing
   write VALUE is fully CONSTANT after alias substitution -- the splice
   rewrites `options.write` through the caller's OpenOptions struct LITERAL,
   leaving `(true as i32)` casts and `Member(StructLiteral, ..)` reads
   (absent field = ZII 0) under pure bitwise arithmetic. Fix: a scoped fold
   at mutation-write selection (`fold_substituted_constant_integer`,
   writes/mutation.rs) collapses such trees to ONE constant store per call
   site -- restricted to the sign-safe operator class (`| & ^ <<`, never
   `>> / %` per the const-fold signedness trap) and bool-source casts (no
   truncation possible), so the fold cannot disagree with the interp's i64
   evaluation. Suite failure-set verified unchanged. The pending canary is
   PROMOTED to `filesystem/wrapper_open_with_exit` (differential + a
   windows-gated suite test; native 70 + interp 70 -- the full six-leg
   OpenOptions matrix). pending/host/ holds only dir_walk_wrappers_native.
   STILL OPEN (narrower): a write value with RUNTIME operands (true locals /
   non-literal options) still refuses loudly -- that remainder is the real
   let-local face, same bucket as host-call args reading locals under
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
   path). ZERO-CALLER SWEEP COMPLETE (2026-07-08i):
   set_times + set_owner/set_owner_no_follow/set_file_owner + symlink_metadata
   covered by `wrapper_times_owner_lstat_exit` (native 70 + interp 70,
   macos-gated -- chown/futimens/lstat have no msvcrt rows): set_times
   round-trips through metadata(File); ownership uses uid/gid -1 (the
   unprivileged leave-unchanged no-op); symlink_metadata probes lstat on a
   regular file (is_symlink false + len). Every user-facing wrapper method
   now has either a calling canary or a pending canary with a fence
   diagnosis (open_with; dir-walk family). Internal helpers (decode_metadata,
   last_error, mkall_*, rda, read_dir_entry_fd) are covered through their
   callers.
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
   **RUNG 1 DONE (2026-07-09): real-fs interpreter provider, opt-in.**
   `omega-interpreter` now has `interpret_with_options(checked, stdin,
   InterpretOptions { filesystem })` with `FilesystemAccess::{Virtual
   (default), RealUnscoped}` (lib.rs); real mode serves the whole fs-op
   family against the REAL disk via `evaluator_real_fs.rs` (a `#[path]`
   child module of the evaluator, so it reuses the private argument/buffer/
   stat helpers): `std::fs::File`s behind the same synthetic-fd table shape
   (fds from 3), errno from `io::Error::raw_os_error()`, open flags decoded
   by the existing `host_open_flags` mirror, real `Metadata` laid out via
   the shared `write_fs_stat` writer (mode|size|mtime at host stat offsets),
   op-name parity ONE-FOR-ONE with the virtual dispatcher. Core serve set:
   create/open/open_create(+unix create-mode)/read/write/seek/close/
   duplicate/set_len/sync/sync_data/remove/create_dir(_name)/remove_dir/
   rename/read_metadata/read_symlink_metadata/read_file_metadata/errno.
   Next-slice ops (at-family, read_at/write_at, locks, chown, perms, times,
   links, read_dir, canonicalize) return -1/ENOTSUP — loud, never silently
   wrong. Acceptance: `omega-interpreter/tests/real_fs.rs` runs ONE
   build-shaped staging program both ways — real mode must materialize the
   asset on disk byte-exact, hermetic default must exit identically with
   NOTHING on disk. Every existing gate untouched (default is `Virtual`;
   `interpret()` delegates unchanged).
   **RUNG 2 DONE (2026-07-09h): path grants.**
   `FilesystemAccess::RealScoped(FsGrants { read_roots, write_roots })`:
   every path-taking op authorizes BEFORE touching the OS — reads must
   land under a read or write root (write grant implies read-back;
   stage-then-verify is the normal build shape), writes/creates/removes/
   BOTH rename ends under a write root; refusal is -1/EACCES (same shape
   as an OS permission denial, so the wrapper error surface gains no new
   cases). Roots canonicalize at construction; op paths canonicalize for
   the check (a not-yet-existing leaf rides its canonicalized parent), so
   `..` traversal and root-escaping symlinks resolve to their real target
   and refuse — and the op then RUNS on the resolved path (authorized
   location == operated-on location). Fd ops need no re-check (fds only
   enter the table through an authorized open). Acceptance: the
   four-quadrant probe in tests/real_fs.rs (read-root read, write-root
   stage+read-back, EACCES create under read-only root, EACCES read
   outside all roots) + on-disk world assertions (artifact real, denied
   files absent).
   **RUNG 3 DONE (2026-07-09i): read_dir + positioned I/O in real mode.**
   `read_dir` mirrors the virtual dispatcher's contract exactly (first
   call packs `.`/`..` + immediate children as darwin dirent records via
   the now-SHARED `pack_dirent_records` packer, position out-param marks
   end) with names from a real `std::fs::read_dir` of the fd's opened
   path — the fd table now carries the RESOLVED path per descriptor
   (RealFd { file, path }; std has no fd-based dirent read) — children
   sorted for determinism (native getdirentries order is fs-defined, no
   program may rely on it), ENOTDIR/EBADF mapped. `read_at`/`write_at`
   ride portable pread/pwrite emulation (seek, op, restore cursor).
   Acceptance: the WRAPPER's `read_dir_count` (open dirfd → read_dir →
   dirent decode → skip dot entries) counts a seeded real directory
   under a read-only grant — the full stack, wrapper decode over real
   records.
   **RUNG 4 INTERPRETER SIDE DONE (2026-07-09j): the granted build
   entry.** Discovery finding first: the compiler-side build.omg entry
   ALREADY EXISTS — `omega-compiler/src/pipeline/build_config.rs`
   auto-includes build.omg next to main.omg, finds the free
   `machine build(b: &mut Build)`, purity-gates it (decision 12 transitive
   effect surface must be EMPTY — the retired "describe, never do"
   framing, enforced in code), evaluates with a ZII Build
   (subsystem/freestanding today) and extracts the config. So rung 4 =
   RELAXING that gate to admit granted fs, not building an entry from
   scratch. Landed interpreter half:
   `evaluate_build_machine_with_filesystem(program, machine, args,
   options)` — the augmenting-machine runner parameterized by policy
   (`allow_filesystem`): fs ops allowed and served per options
   (virtual/real/scoped), every OTHER host boundary rejects via a new
   `non_fs_host_boundary_touched` backstop flag (split at the Filesystem
   receiver-trait branch + the value-fallback sites); FULL step budget
   (staging is real work; the pure entry keeps its const-eval fuel cap).
   Pure entry delegates unchanged (build_time 2/2). Acceptance in
   tests/real_fs.rs: a `Stager::build(&mut self, b: &mut Build)` machine
   stages a real asset AND augments the Build (read back: staged=1,
   target_index=7, asset bytes on disk; hermetic default run = same
   augmentation, no disk); a Console-touching build machine rejects with
   the policy-naming error.
   REMAINING RUNGS: (a) further next-slice ops as build programs need
   them (at-family, locks, perms, times, links, canonicalize — all
   -1/ENOTSUP today); (b) the COMPILER-side gate relaxation in
   build_config.rs — DESIGN-GATED, questions for Zach below.
   **DESIGN QUESTIONS (build.omg fs grants — for Zach):**
   (1) CAPABILITY INJECTION: how does the free `machine build(b: &mut
   Build)` GET its Filesystem? Options: a field on Build (`b.fs`), a
   second parameter (`machine build(b: &mut Build, fs: &mut
   Filesystem)`), or machine-owned data. This is user-facing surface —
   every build.omg spells it.
   (2) GRANT DERIVATION: what are the default roots the compiler passes?
   Read = the package dir (main.omg's directory)? Write = which output
   dir (the artifact/-o dir)? May build.omg request EXTRA roots (assets
   outside the tree), and does that need a CLI acknowledgment
   (--allow-read=...)?
   (3) STATIC GATE SHAPE: build_config.rs currently requires an EMPTY
   transitive effect surface. Relax to "⊆ {filesystem}" unconditionally,
   or only when some explicit opt-in exists (in build.omg or on the CLI)?
   (4) CONSOLE FOR BUILD LOGGING: the granted entry currently rejects
   Console strictly (only Filesystem is granted). Build scripts want
   print-logging — grant Console (stdout is captured by the interpreter
   anyway), or keep strict? Strict is the reversible choice, so that is
   what ships until decided.
   (5) DOC DEBT — DRAFTED (2026-07-10b): build_and_package_model.md now
   carries a second addendum ("SETTLED 2026-07-07: granted filesystem;
   'describe, never do' retired") — the grant model, what stays true
   (Build-as-plan, no toolchain circularity, the version-invariance
   nuance now conditioned on fs state), scoping enforcement, engineering
   state, and questions 1–4 above restated as the brief's open items.
   §1/§3 annotated with pointers. Review welcome — it is a draft of
   YOUR decision, edit at will.
4. [ ] Windows ops without msvcrt equivalents → Win32 calls (stat family first,
   after #2's design).
5. [ ] Title-bar context-menu Close → outbound WndProc entry stubs (§12.4).
6. [ ] linux binding tables (structural → tested) when a target is available.
7. [ ] **CALL-WITH-RETURN — SCOPED (2026-07-08k), ready for a focused
   session.** The reframing discovery: the feature ALREADY EXISTS for
   statement-dispatched calls. The state-graph runtime-flow builder
   (omega-state-graph/src/runtime_flow/builder.rs) clones callees per
   CallContext, SEGMENTS the calling state at each call (`segment_index`;
   the call returns into the next segment of the same state, sharing its
   frame), threads a `continuation` per clone, and — for VALUE-position
   statement calls (`let n = f(..)`) — stamps `CallResultReturn
   { call_source_key, statement_index }` on the clone's TERMINAL edge so
   the terminal value writes back to the caller's call-result slot. The
   dispatch-loop plan (omega-runtime-dispatch-loop) already carries
   `continuation`/`call_result` per edge, and emission consumes them
   (runtime-storage call_result_slot_by_ordinal etc.). Pinned by
   `calls/runtime_looping_value_return_exit` -- which is ALREADY a
   CONTAINED-receiver value call (`let n = self.m.count(s, 0)`): a callee
   that LOOPS (self-transition) dispatches with result delivery TODAY (the
   "value-return-in-dispatch keystone"). So a route CHOOSER already exists:
   looping callees → dispatch, straight-line callees → splice.
   THE GAP: the chooser sends non-looping callees to the SPLICE route
   (runtime dispatch bodies) even when their shape trips the splice fences
   (effectful arms, re-entrant interiors, all-arms). The feature work is
   WIDENING THE CHOOSER, not new machinery.
   MECHANICAL PLAN: (a) routing policy -- extend the existing
   looping-callee test with the fence predicates (effectful-arm callees,
   re-entrant spliced interiors → dispatch); keep the proven splice for
   pure/simple callees (code-size: clones duplicate dispatch cases per
   call site, splice stays inline).
   ⚠️ CHOOSER INTEL (2026-07-08l, corrects "no new analysis needed"): the
   chooser is `dispatch_state_call_edges`
   (omega-backend-pipeline/src/builder.rs) and its comment records that
   NAIVE widening was ALREADY TRIED: "dispatching NON-looping value calls
   broadly regresses ~13 canaries -- the inline-branching value path
   handles shapes (binary operands, reference/slice-element results,
   aliases, multi-arm) the dispatch RETURN-WRITE does not yet serve"
   (memory [[inline-branching-value-runtime-guard]]). So targeted routing
   of FENCE-REFUSED shapes is safe (error → attempt, no green canary can
   regress), but those shapes may then hit the same return-write gaps as
   NEW loud errors -- the return-write shape coverage is the real second
   half of the feature.
   INVESTIGATION ITEM 1 — LANDED 2026-07-08l, ⚠️ RETRACTED 2026-07-08n:
   the fence exemption for dispatch-routed calls was UNSOUND. The
   counterexample (one probe later): an effectful re-entrant SELF-value
   call (`self.total = self.count(4, 0)`, per-entry `self.hits` bump,
   SelfTarget re-entry) ROUTES to dispatch (state_call_target_loops sees
   the SelfTarget back-edge) -- but the dispatch RETURN-WRITE does not
   serve the shape, so with the exemption it COMPILED and silently
   misdelivered (native 71 vs interp 70). "Routed to dispatch" does NOT
   imply "the return-write serves this shape" (the recorded ~13-canary
   regression class was the warning). Both fences refuse again; the
   helper (`dispatch_route`, with its OMEGA_DEBUG_DISPATCH_ROUTE
   instrumentation) stays as DORMANT infrastructure. Counterexample
   parked as the acceptance canary:
   `pending/host/dispatched_effectful_reentrant_value` (interp 70).
   CORRECTED SEQUENCE for #7 — STEPS 1+2 COMPLETE (2026-07-09f):
   the fence exemption for dispatch-routed calls is RE-ADDED and SOUND.
   What made it sound (the retraction's missing piece): a new
   emission-planning check (`collect_call_result_return_blockers`)
   guarantees every dispatched terminal carrying a CallResultReturn has a
   SELECTED return-write -- unserved shapes (float terminals, unresolvable
   values) refuse LOUDLY instead of silently ZII. So a dispatched value
   call either delivers correctly or fails to compile. The retraction
   counterexample now passes 70/70 and is PROMOTED as the acceptance
   canary `calls/runtime_dispatched_effectful_reentrant_exit`
   (differential + suite; both the looped RESULT and the per-entry EFFECT
   COUNT deliver). The effectful-entry "shape gap" turned out to BE the
   already-fixed field-read terminal. Fence fail canaries still fire
   (non-looping callees never dispatch). DIR-WALK STATUS: the pending
   matrix now fails ONLY on rda's genuine call recursion -- the read_dir
   and mkall legs cleared. Remaining for the full unlock: the recursion
   story (rda depth+drain restructure or tail-call-to-loop; mkall_copy's
   entry self-call is the same class), and the alias/slice-element result
   shapes are still unprobed (now loud-if-unserved rather than silent).
   STEP 1 FIRST TWO SHAPES CLOSED (2026-07-08o) -- and both were LIVE
   UNFENCED silent-wrongs (pure looping callees dispatch today with no
   fence, so these shipped broken, worse than the fenced effectful class):
   (B) result bound to a FIELD (`self.total = self.count(..)`): fields
   have no frame result slot, so the return-write silently SKIPPED and the
   field stayed ZII -- fixed by resolving the caller statement's
   Assignment target to its MACHINE-region place when no slot exists
   (edges.rs `assignment_target_machine_place`; conservative: machine
   region only, frame targets stay the slot path's job);
   (C) TERMINAL returning a FIELD read (`-> self.base`): the return-write
   copy hardcoded source_region RuntimeFrame, reading the frame at a
   machine offset -- fixed by using the resolved place's REGION.
   Both pinned differential:
   `calls/runtime_dispatch_result_field_binding_exit` +
   `calls/runtime_dispatch_result_field_terminal_exit` (native+interp 70;
   RUN_CANARIES + suite tests).
   THREE MORE SHAPES CLOSED/PINNED (2026-07-09a): (D) BINARY terminals
   (`-> acc + 100`) fell through SILENTLY (live unfenced, native 71) --
   new dedicated path in edges.rs computes into the result place
   (WriteRuntimeStorageBinary; arithmetic/bitwise subset via a local
   operator map, typed-place signedness + resolved domain, floats bail);
   (E) MULTI-ARM terminals (place arm + binary arm at two sites taking
   opposite arms, field-bound) -- the binary arm was the same fallthrough
   (native 72), now green; (F) GUARD-SUBJECT binding probed GREEN and
   pinned. Canaries (differential + suite):
   `calls/runtime_dispatch_result_{binary_terminal,multi_arm,guard_subject}_exit`.
   ROUND 3 (2026-07-09b): (G) SATURATING terminal arithmetic -- the NATIVE
   side is CORRECT (the binary path rides the resolved domain: i8 127+50
   saturates to 127) but the INTERPRETER diverges (computes wide, no
   clamp through recursion-param carry; parked
   `pending/host/interp_saturating_param_carry`, flagged for the
   interp/arithmetic thread in Observations). [RESOLVED 2026-07-09c by the
   arithmetic thread: interp now applies Saturating/Trapping at binary NODES
   (expression_scalar_type witness); the repro is PROMOTED to
   pass/arithmetic/runtime_saturating_param_carry_exit, differential 70/70.
   See TASKS.md "EXPRESSION-DOMAIN SESSION 2026-07-09c".] (H) TRANSITION-ARG binding
   (`true -> check(self.count(..))`) is another LIVE silent-wrong (native
   71): widening the return-write slot lookup to any role
   (`state_call_result_slot_any_role`, kept -- documented and needed) was
   insufficient alone; the ordering theory was WRONG; the traced
   root (2026-07-09c, two instrumented sides): the clone terminal carried
   NO CallResultReturn at all -- `statement_call_is_value` sniffed
   Assignment/Expression OPERATIONS and a transition-embedded call has no
   operation at its statement. FIXED: `RuntimeStateCallEdge.is_value` is
   now set AUTHORITATIVELY from the plan's role (!= Statement) in the
   router and the builder heuristic is deleted. The write side now RUNS --
   and exposed the SECOND gap, still open: call-result slots are
   DUPLICATED PER DISPATCH CONTEXT and the writer picks first-match
   (dispatch 1, offset 0) while the reader resolves under the caller
   segment's context (dispatch 4, offset 4) -- different slots, result
   still ZII. `state_call_result_slot_for_dispatch` (new, dispatch-keyed)
   keys on `edge.TARGET_dispatch_index` (2026-07-09d -- the clone-terminal
   RETURN edge ENTERS the caller's next segment; continuation is None
   there), and the full selection chain traced correct --
   and the backend_report DIFF against the let-bound twin found the real
   residual (2026-07-09e): the direct spelling materialized NO ARGUMENTS
   for the dispatched callee (the let-bound build writes 3/0/0 into the
   clone's params in case #1; the direct build writes nothing -- the clone
   looped on ZII and returned 0). `statement_call_arguments` (runtime-flow
   builder) sniffed OPERATIONS like statement_call_is_value before it;
   it now DESCENDS into transition TARGET-ARGUMENT / guard expressions.
   SHAPE CLOSED: `pending/host/dispatch_result_transition_arg` PROMOTED to
   `calls/runtime_dispatch_result_transition_arg_exit` (differential 70/70
   + suite test); pending/host is now EMPTY — dispatch_slice_element_terminal
   PROMOTED 2026-07-09k2 to `calls/runtime_dispatch_slice_element_terminal_exit`
   (differential 70/70 + suite test): the serve picks the indexed-copy kind
   BY TARGET REGION (frame slot → CopyRuntimeFrameIndexedToRuntimeFrame,
   machine place → ...ToRuntimeStorage — the mutation path's exact pairing,
   writes/storage_copy.rs), so NO region-parametric encoder was needed;
   the 07-09l crash was the machine-region kind emitted against a frame
   slot. Blocker served-kind list extended with both indexed kinds.
   (The interp_saturating_param_carry repro was RESOLVED + PROMOTED by the
   arithmetic thread, cd42934d5). All three connected gaps are documented in the
   canary header (role-stamped CallResultReturn; return-target dispatch
   slot keying; transition-expression argument descent). Parked
   `pending/host/dispatch_result_transition_arg` (native 71/interp 70);
   also recorded: `CallResultReturn` carries no call ORDINAL (two
   dispatched calls in one statement cannot be disambiguated -- plan-shape
   gap). Safety: suite A/B zero new failures with the partial landed (the
   newly-stamped terminals write first-match slots; no green canary
   reads those paths yet).
   MATRIX COMPLETE (2026-07-10a): the ALIAS-READ terminal (`-> acc`,
   acc: &mut i32 -- the pointer-bits-as-result risk) was the last
   unprobed shape and it already serves correctly (place resolution is
   deref-aware); pinned by calls/runtime_dispatch_result_alias_read_exit
   (differential 70/70 + suite test). Slice-element results SERVED
   2026-07-09k2 (see above). Every return-write shape now either SERVES
   (param/field-binding/field-read/binary/multi-arm/guard-subject/
   transition-arg/enum-case/machine-array-slice-arg/slice-element/
   alias-read) or LOUD-BAILS (float terminals, documented). Remaining
   adjacent (not return-write): transition-arg ordering (above),
   effectful entries (the entry-op splice/dispatch interaction). LESSON: an exemption from a
   silent-wrong fence needs a RUNTIME differential proof per shape, not a
   routing-evidence proof -- compile-time evidence says the route was
   taken, not that the route is correct.
   INVESTIGATION ITEM 1b — RESOLVED (2026-07-08m), `required` was NOT the
   blocker: the router/plan staging is now a monotone FIXPOINT
   (backend-pipeline builder.rs -- dispatch edges and the state-call plan
   feed each other; one round baked the seed flow's under-approximated
   `required` into the edge set; the loop iterates until the edge set
   stabilizes; today's corpus converges in ONE round = zero behavior
   change, pure hardening for the widening work). The REAL wall, found via
   the new env-gated route-miss instrumentation
   (OMEGA_DEBUG_DISPATCH_ROUTE=1 in dispatch_route.rs): `mkall_copy`
   recurses via a SELF-CALL in its entry -- the only shape the decreases
   PROVER accepts ("the contract prover consumes neither cross-state guard
   facts nor +1 arithmetic") -- and RECURSION can take NEITHER route:
   the clone DFS is finite (the same specialization-refuses-cycles class
   as rda) and the splice runs effects once. IDENTIFIED UNLOCK:
   TAIL-CALL-TO-LOOP conversion -- an entry self-call in tail position is
   semantically a loop (re-bind params + back-edge to the entry dispatch
   case); that converts mkall_copy and rda's sibling-drain (`rda_more` is
   tail) but NOT rda's depth recursion (`rda_recurse` continues after the
   call -- needs the .omg work-stack restructure or real frames).
   ALTERNATIVE: prover work so a self-TRANSITION loop can carry the
   decreases proof (then mkall_copy rewrites as a plain loop, no backend
   change). Both are bounded; the prover route touches the checker (the
   arithmetic thread's area -- coordinate), the tail-call route is
   backend-local (this thread's area). TAIL-CALL STEP 1 DONE (2026-07-10c):
   the pure accumulator canary is PARKED at
   pending/calls/tail_self_call_accumulator with the full diagnosis and
   the mapped transform plan. Findings: the PROVER ACCEPTS the recursive
   spelling (decreases checks fine) -- only the runtime-flow builder's
   cloning-DFS cycle check refuses, so this is purely a lowering gap; the
   rejection-site edge is { same machine handle, is_value, segment 0 of
   1 } but RuntimeStateCallEdge carries NO RECEIVER IDENTITY, and the
   receiver check is load-bearing (`self.other.sum(..)` on a contained
   same-machine field must keep rejecting -- wrong instance otherwise).
   Plan: thread is_self_receiver into the edge (backend-pipeline builder
   has receiver_path), rewrite same-machine+self-receiver+tail calls at
   the rejection site as the entry transition visit_transition already
   emits (entry_continuation/entry_arguments(context) already model
   re-entering the clone's own entry -- the no-guard-matched fall-through
   uses exactly that), keep the error otherwise. OMEGA_DEBUG_TAILCALL
   prints the edge at the site. NEW DEV TOOL: `omega-run` bin
   (omega-compiler) -- compile+run a .omg natively, `--both` adds interp
   agreement; the probe workflow's missing one-shot harness.
   INVESTIGATION ITEM 2: enumerate the dispatch return-write's missing
   shapes against the ~13-canary regression list (binary operands,
   slice-element results, aliases, multi-arm) -- each is its own bounded
   sub-task once routing sends real consumers through. (b) receiver storage: dispatched clones are
   keyed by StateKey; the CONTAINED receiver's region/offset must thread
   into the clone's dispatch -- this INTERSECTS the known deep
   receiver-storage-through-dispatch fix (contained same-type aliasing /
   param receivers); doing both in one session is the natural cut.
   (c) argument delivery: the builder's `target_arguments` materialization
   already handles statement-call args; receiver-call args ride the same
   path once routed. (d) the fences then RELAX to routing (fence sites
   become route-to-dispatch sites; fail canaries flip to run canaries --
   the dir-walk pending matrix is the acceptance test, minus (e)).
   (e) NOT covered: rda's genuine recursion (the builder's static
   recursion check rejects it) -- rda needs the entry-recursion loop
   restructure separately (mkall precedent, two recursion sites).
   NO DESIGN FORK IDENTIFIED: routing policy has a natural answer
   (fences → routes), cost tradeoff is implementation judgment. Estimated
   as one focused session for (a)+(c)+(d) with (b) as the risk item.

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

- RETURN-WRITE MATRIX row request (2026-07-10, from the arithmetic thread):
  the collect-all pass_canaries_compile umbrella (this tick) unmasked two
  termination canaries your 2026-07-09 return-write fence (936fc62c1)
  loudly regressed -- invisible for a day behind the umbrella's old
  efi-first panic. Shape: a dispatched value call whose terminal is a
  PARAM-STRUCT-FIELD read (`false -> card.power`, `card: Card` by value).
  Both parked at pending/termination/custom_ranking_* with PROMOTE notes;
  the fence is correct (unserved = ZII result), the ask is the matrix row.


- ARITHMETIC-THREAD note (2026-07-09i, from fixing collateral): the new
  operand-domain fence (cd42934d5) broke `samples/gui/windowed_calculator`
  on main (digit_append's fused `(current*10 + digit) % 10000`); fixed
  forward in this lane by stepping through Saturating FIELDS
  (append_shift/append_sum). Two follow-ups for that thread: (1) the
  fence diagnostic says "store into a `Saturating`-typed LOCAL or field"
  but a let-LOCAL does NOT satisfy it (lets substitute into their uses;
  probed 2026-07-09i) — either make let-landing count or drop "local"
  from the message; (2) the diagnostic names no state/statement, which
  made locating the site needle-in-haystack in a 1000-line sample —
  thread the source attribution the other emission blockers carry.
  [RESOLVED 2026-07-09d by the arithmetic thread: the fence is RETIRED --
  operand-position Saturating/Trapping Add/Sub/Mul now lower correctly on
  both ISAs (write-path clamp/trap sequences reused at the operand
  evaluator's registers), so both follow-ups are moot and the let-
  substitution behavior is now harmless (the fused op carries its domain).
  digit_append is reverted to the natural fused spelling; the append_*
  staging fields are gone. See TASKS.md "OPERAND-POSITION DOMAIN LOWERING
  2026-07-09d".]
- samples_compile on Windows hosts has exactly 4 PRE-EXISTING failures
  (A/B-verified 2026-07-06, re-confirmed 2026-07-07): `cli__systems__file_journal`
  (uses `read_metadata` — the stat family is deliberately fenced on windows until
  open-work #2) and `stdin_checksum`/`stdin_rot1`/`stdin_upper` (other
  workstream's WIP frontend errors). Judge regressions by failure-SET diff
  against these names, never raw counts.
- macOS-host runs previously showed ~85 pre-existing differential-skip failures +
  a broad aarch64 `b.ne` alignment bug in samples_compile (task chip spawned) —
  NOT this thread's work.
- ✅ RESOLVED (by the arithmetic thread, cd42934d5 2026-07-09): the
  interpreter Saturating param-carry gap this lane parked
  (`pending/host/interp_saturating_param_carry`) — interp now applies
  Saturating/Trapping at binary operation nodes; the repro was promoted to
  `pass/arithmetic/runtime_saturating_param_carry_exit` (differential
  70/70). The remaining OPERAND-position native lowering gap is theirs,
  parked at `pending/arithmetic/runtime_saturating_expression_domain_exit`
  with its own promote criteria.
- ✅ RESOLVED (verified 2026-07-09g): the 2026-07-08 native arithmetic
  regression (`dual_accumulator` interp 70 vs native 71;
  `arithmetic/runtime_cast_in_guard_exit` native 71) is FIXED on main — the
  differential now runs PAST both (dual_accumulator sample test green;
  supported-canaries progresses beyond RUN_CANARIES line 33) . Credit the
  arithmetic/const-fold thread.
- ✅ RESOLVED (2026-07-10d, by the parallel thread's 2b3441b8c — the repeat
  collect-all ask, delivered): the differential umbrella no longer
  panics at the first native-blocked canary — compile failures print as a
  native-blocked bucket (the canary suite owns that signal) and the
  mismatch assert runs over everything runnable. VERIFIED on this host:
  `-p omega-interpreter --test differential` is 12/12 GREEN with
  tick_count riding the bucket; the masking class that hid "open_create
  never compiled on darwin arm64" is structurally closed. The tick_count
  aarch64 native lowering itself remains the time lane's (its
  canary_suite test still fails on macOS — known, theirs).

## Coordination

A parallel agent advances origin/main (std::time lately); files have stayed
disjoint — fetch/rebase each iteration, run the differential drift guard
immediately after every rebase, work around collisions.
