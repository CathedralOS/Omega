# Tasks — Filesystem (`std::fs`) + Windows-native thread

> **CONVENTION (Zach, 2026-07-10):** this file is a CURRENT-STATE document,
> not history — open items, active investigation state, load-bearing
> doctrine, and pointers (dates + canary names) into git history. Finished
> arcs live in commit messages and the canaries that pin them. Condense
> continuously.
>
> **WORKING RULES.** Consult `wiki/language_guide/*` before language features;
> ZII / arena / `Handle` / `HandleSpan`; full human-word op names (C symbols
> only in per-target binding tables); every fix ships a canary that RUNS and
> asserts. Gates per iteration: canary_suite (judge by FAILURE-SET diff vs the
> named baseline below, never raw counts), native_filesystem_canaries (macOS
> 88/0), samples_compile (BOTH test fns -- compile set AND documented-exit
> set; skipping it hid the account_ledger silent-wrong for four pushes,
> 2026-07-11g), omega-interpreter coverage + differential + `run_canary_list`
> drift guard (run IMMEDIATELY after every rebase), real_fs. Push every iteration:
> fetch → rebase → survival-grep your recent symbols → re-verify → push.

## North star

A serious, ergonomic `std::fs` with Rust parity: portable `Filesystem` wrapper
(result enums / `File`) over a per-OS raw `FilesystemHost` seam (= Rust
`std::fs` over `std::sys`). Raw ops return syscall ints; the wrapper builds
results in Omega. Interpreter = full-parity reference oracle for everything.

## Current state (2026-07-11)

- **Interpreter**: full Rust-parity fs; the hermetic virtual fs is the
  differential oracle. OPT-IN real filesystem (`interpret_with_options`,
  `FilesystemAccess::{Virtual, RealUnscoped, RealScoped(FsGrants)}`) with
  FULL op parity vs virtual (unix-gated where std requires; symlink-inspecting
  ops ride no-follow grant resolution). Granted build entry
  (`evaluate_build_machine_with_filesystem`) allows fs, rejects every other
  host boundary. Pins: tests/real_fs.rs (6, incl. the 14-step parity probe).
- **macOS/aarch64**: the primary VERIFIED host right now. Full gates green:
  fs canaries 88/0, differential green, suite at the named baseline-7.
  Dir-walk wrapper family native end-to-end (`dir_walk_wrappers_exit`);
  note_vault sample = 14 steps incl. the fs↔time bridge.
- **windows/x86_64**: raw seam + wrapper verified through msvcrt rows
  (roundtrip/breadth/wrapper-breadth canaries); stat family + no-msvcrt ops
  (pread/*at/link/read_dir/flock/chown/futimens/realpath) keep the loud
  "no native lowering" error pending Win32 rows. Needs a Windows session.
- **std::time interop**: rung-10 darwin bindings native-confirmed
  (`runtime_time_host_native_darwin_exit`); fs↔time bridge pinned
  (`runtime_fs_mtime_system_time_interop_exit`, natural receiver spelling).
- **Per-instance receivers** (stolen deep fix, landed 2026-07-10): calls
  through non-first same-type receivers run on the RECEIVER's storage on
  BOTH routes — dispatch (per-dispatch `BackendPlan::receiver_bases` table)
  and inline (`receiver_base_for`'s unique-call recovery). Entry-machine
  callers only (slice 1); ambiguous multi-call states stay fenced. Pins:
  calls/runtime_dispatch_second_receiver_exit,
  calls/runtime_same_type_second_receiver_mutation_exit (the original
  aliasing repro, flipped from fail fence),
  references/runtime_nested_receiver_same_type_exit,
  time/runtime_value_machine_receiver_field_postentry_exit.
- **Dispatch return-write matrix**: complete — every terminal shape serves
  (incl. slice-element, alias-read, float PLACE terminals) or loud-bails
  (float BINARY terminals). The call-result fence is DELIVERY-PLACE granular
  (a consumption copy cannot mask a dropped production write). Recursion:
  REJECTED by owner directive 2026-07-07 (no cycles; loops are bare state
  self-transitions) — pinned by machine_self_call_recursion_rejected +
  terminal_self_call_recursion_rejected.
- **Dev tooling**: `omega-run` bin (compile+run a .omg; `--both` adds interp
  agreement; `--keep` preserves the build dir + backend report). Env-gated
  debug: OMEGA_DEBUG_RECEIVER, OMEGA_DEBUG_CALL_RESULT,
  OMEGA_DEBUG_DISPATCH_ROUTE, OMEGA_DEBUG_TAILCALL, OMEGA_DUMP_SLOTS.

## Open work

0. ~~efi red family~~ — **RESOLVED 2026-07-11t (diagnosis corrected:
   NOT a regression).** The efi milestone canaries are HOST-FORMAT
   dependent by construction: no target block and no registered
   uefi_x64 target — on the Windows sessions the PE image came from
   the HOST format + build.omg's subsystem-10/freestanding facts; on
   aarch64 the vtable/host-call encoder shapes have no lowering
   (hardcoded width 0). The two compile-failing members moved to
   WINDOWS_HOST_PASS_CANARIES and the three byte-asserting test fns
   are cfg(windows) — same class and precedent as the gdi32 blit
   canary. **The suite is 0-FAILURE on macOS for the first time.**
   OPEN (owner-surfaceable): register a `uefi_x64` TARGET (std targets
   catalog: PE32+ format facts + the uefi calling surface) so the efi
   family becomes cross-compile pins from any host — un-gate on land.

1. ~~build.omg compiler-side gate~~ — **COMPLETE 2026-07-11j/k** (owner
   answers #2–#5 in commit 14e02026e; implementation bc086f0a3 +
   0bc474e81). Current shape: std FilesystemHost declares
   `effects filesystem_io` rows (36 methods); the gate allows transitive
   {filesystem_io, stdout_io, stderr_io} DECLARED on the build machine
   (`effects` clause), refuses everything else with teaching messages
   (row-less boundary → host_boundary hint); effectful builds run the
   granted entry (RealUnscoped — owner de-scoped permissions), staging
   real assets at compile time; console writes are SERVED and flushed
   to the compiler's real streams (failure included). Build machine =
   free `build(b: &mut Build)` or ONE attached `<Component>::build`
   with the `b: &mut Build` single-param signature (name alone captured
   builder-pattern machines; eventual rule = "declared in build.omg",
   needs machine source-file plumbing — OPEN item). Pins:
   tests/build_config_granted.rs, fail/build/* (2),
   granted_build_serves_console_and_rejects_other_boundaries.
   OMEGA_DEBUG_BUILD_CONFIG dumps the gate. Owner follow-up: Q11 (std
   console boundary).

2. ~~reversed-operand receiver residual~~ — CLOSED 2026-07-10y
   (dynamic per-machine resolved_machine_base + attached-data
   equivalence at the sweep sites; a/b shuffle fully retired;
   runtime_system_time_after_2026_exit at natural spelling).

3. ~~Per-instance receivers, ALL routes~~ — **COMPLETE 2026-07-11b→l**
   (dispatch-table composition e0c718793; self-call inheritance
   16c3816f5; inline chain-walk recovery 9ac48266e; leaf-write scoped
   keys 465b82bbf + d8ff50e89's account_ledger fix; spliced-code fence
   visibility d8ff50e89; param-binding serve c94fb49ea; interp
   re-borrow collapse cd271c670). Current semantics: receiver identity
   composes through the parent-context chain (dispatch), a bounded
   call-chain walk with per-position PARAM ENVIRONMENTS (inline +
   spliced code, `&mut` args bind params to absolute bases, re-borrows
   forward), and the fence mirrors the walk (MachineAnchor with
   poisoning; serve-or-refuse: every shape delivers per-instance or
   refuses with guidance). Table indexed by ARENA index (1-based).
   Pins: ~12 canaries under calls/ + the dungeon; fail pins for the
   ambiguous shapes. Residual (needs a natural repro): args spelled in
   a DEEPER callee state colliding same-machine names ride the
   leaf-write name fallback.

4. **Windows-session bundle** (needs a Windows host): verify the stat-row
   migration natively; WINDOWS_IMPORT_ROWS migration into provides files;
   Win32 rows for the no-msvcrt ops; file_journal-on-windows recheck;
   WndProc entry stubs (title-bar close).

5. **linux** — binding tables are structural-only until a target host exists.

6. **Authored-bindings interp story** — OWNER_QUESTIONS.md #10 (native-only
   imports today; differential skips).

7. **[CLAIMED from TASKS_TIME 2026-07-11n]** D14 fires E+F — **LANDED
   2026-07-11n**: u64 literals in LET initializers (fire E; the native
   static resolver + interpreter were already bits-capable) and
   EQUALITY guards against u64-classed places (fire F; ==/!= only,
   ordering stays refused sign-blind; the guard-side resolver gained
   the bits fallback, sound under the gate; the walker recurses through
   the multi-arm desugar's `(subject) == true` nesting — that nesting
   cost the debug cycle). Pins:
   arithmetic/runtime_u64_literal_let_guard_exit (exact u64::MAX
   round-trip, differential),
   fail/arithmetic/u64_literal_into_i64_rejected,
   fail/arithmetic/u64_literal_ordering_guard_rejected. The saturating_*
   twins LANDED 2026-07-11o: Instant::saturating_add/subtract (clamp to
   the new Instant::MAX / Instant::EPOCH consts — MAX's u64 seconds is
   fire D) + SystemTime::saturating_add/subtract (SystemTime::MAX /
   SystemTime::MIN; i64 literals, incl. the -9223372036854775808 MIN
   spelling probe-verified), mirroring Duration's saturate idioms
   exactly. Pin: time/runtime_saturating_time_arith_exit (seven exact
   legs, fire-F guards on the extremes, differential). TASKS_TIME item
   6's deliberate gap is CLOSED — the claim completes. FOLLOW-ON claim
   (render item 9, interpreter domain, 2026-07-11p): the Gui/Input
   headless stub was already semantically complete — the gap was the
   10M step budget vs window_demo's ~40M software-rendered frames;
   OMEGA_INTERP_STEP_BUDGET overrides it and
   omega-interpreter/tests/gui_headless.rs pins interp exit 0 ==
   native exit 0 for the untouched flagship sample.

8. ~~Machine source-file plumbing~~ — **RESOLVED 2026-07-11m without a
   representation change**: per-file item attribution already exists at
   the SYNTAX stage (AssembledSyntax.files → root_items), so the
   compiler collects the build.omg-root machine names there and threads
   the list to the gate; is_build_machine = name (`build`/`::build`)
   AND declared-in-build.omg (the param-signature interim retired;
   syntax machine names are already full paths — `Stager::build`).
   Pins: build_config_granted (positive),
   pass/build/runtime_main_source_builder_is_ordinary_exit (a
   `Maker::build(b: &mut Build)` in MAIN source stays an ordinary
   runtime machine). Typed machines still carry no source file — fine
   until a second consumer needs one.

## Design decisions (ratified; user reviews later)

- **D1** Human-word API; C abbreviations only in binding tables.
- **D2** Two layers: portable `Filesystem` over raw `FilesystemHost`.
- **D3** Raw ops return ints; wrapper builds `File`/result enums in Omega.
- **D4** `create` → `_creat`/`creat` (register mode).
- **D5/D6** Raw-seam breadth COMPLETE; wrapper is the focus.
- **D7** Receiver-typed value-call resolution fixed (validation + interp).
- **D8** Flag math is branch-free bitwise.
- **D9** Deref-result host calls: LOCKSTEP widths/relocation/encoder keyed on
  one predicate (same discipline as `restores_stack()`).
- **D10** Machine-to-machine self value-calls work; wrappers rely on it.
- **D11** Runtime-length subslice write `write(fd, buf[0..n])` works.
- **D12** Per-OS values = DATA in `.omg` provides rows + a `cfg!(target_os)`
  interpreter mirror, differential-guarded (interim debt self-hosting
  dissolves).
- **D13** Per-OS decode = variation-as-DATA at the edge + ONE central decoder
  producing neutral `Metadata` (`Filesystem::decode_metadata`, the single
  decode site).
- **D14** (⚖️ leaning) `Metadata` absent/unknown = `0` (ZII-clean); quirks
  remap at the seam. No `-1` sentinels, no `Option`.
- **D15** A `provides` target name is valid iff compiler- or build-defined.
- **D-oracle** RUN canaries are interpreter-vs-native; unsupported constructs
  skip loudly.
- Misc in force: `remove_dir_all` fuel=4096; `*at` names trusted relative;
  `create_dir_all` intermediates best-effort; raw boundary in its own std
  module; `read_dir` single 512-byte fill per call; std/targets file shape =
  `std/targets/<target>.omg` + `std/targets/<target>/<sub>.provides.omg`.

## Doctrine (load-bearing, learned the hard way)

- **Fences are policy**: a loud refusal may be an owner directive, not a gap
  — grep ALL TASKS*.md for directives before "fixing" a rejection (the
  tail-call transform was built 2 days after the no-recursion ruling and
  fully retracted).
- **Serve-or-refuse**: a dispatched value call either delivers correctly or
  fails to compile — never silently ZII. Exemptions from silent-wrong fences
  need a RUNTIME differential proof per shape, not routing evidence.
- **Judge regressions by failure-SET diff** against named baselines, never
  counts. Suite baseline-5 (2026-07-11q; was 7 — the tick ×2 closed when
  the inline-Clock tick_count darwin row landed): efi ×3
  (efi_entry_arguments / efi_freestanding_skeleton / efi_ref_param_call_arg),
  runtime_gui_memory_dc_blit, pass_canaries_compile (= the same four
  canaries, no hidden extras). samples_compile macOS baseline-4:
  stdin ×3 (frontend WIP), uefi_hello. SUITE BASELINE: ZERO failures
  on macOS since 2026-07-11t (windows-hosted members are gated to
  windows hosts; judge by the empty set — ANY suite failure is a
  regression now). (2026-07-11r: the gui blit canary
  is windows-gated now — it IS the gdi32 pixel path; suite baseline
  drops to 4: efi ×3 own-test failures + pass_canaries_compile, whose
  TRUE inner compile-failure set is {targets/efi_ref_param_call_arg,
  targets/efi_vtable_call} — precisely measured 2026-07-11r/s. (The
  termination/default_order_nat_countdown_compile member greened
  2026-07-11s: its source predated explicit value-machine return types
  — no `-> usize`, so the planner minted no result slot and the
  call-result fence refused the let-binding loudly, exactly
  serve-or-refuse working; the declaration fixed it. NOTED, unfiled:
  console-less programs default-exit 1 natively vs 0 interpreted —
  compile-only members don't care, but a run canary of that shape
  would diverge.)
- **Canonical idioms**: field discipline for host-call args/results;
  errno-capture-in-entry; field-carrier for indexed element moves. The a/b
  first-field receiver shuffle is FULLY RETIRED (per-instance receivers,
  both routes + the splice/sweep holes closed 2026-07-10y).
- **C-strings at host boundaries**: subslices pass a POINTER (kernel reads to
  NUL); the interpreter slices by LENGTH — a standing native-vs-interp
  divergence class to check on string-ish bugs.

## Coordination

- OWNER_QUESTIONS.md (repo root) consolidates all pending owner decisions,
  batch-answerable.
- **SHARED WORKING TREE** with the parallel thread (their commit's words).
  Discipline: stage only files your change owns (never `-A` at repo root);
  after any parallel push, survival-grep your recently landed symbols (a
  clobber shows as your feature quietly reverted — happened 2026-07-10,
  restored same-day).
- Work-stealing is owner-authorized: survey their recent commits, migrate
  the claim into this file, commit the claim BEFORE working.
- The parallel thread currently works recursion aftermath + case-literal
  poison + text-equality lowering; hot shared files: edges.rs,
  call_result_blockers.rs (extend, don't rewrite).
