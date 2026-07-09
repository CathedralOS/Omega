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

## Current state (2026-07-10)

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

1. **build.omg compiler-side gate** — DESIGN-UNBLOCKED 2026-07-11i
   (owner answered #2–#5 in OWNER_QUESTIONS.md, commit 14e02026e).
   Distilled: (a) INJECTION = dependency injection of a filesystem
   data instance into build's main (SAS-component style; build.omg
   still `use`s std::filesystem); (b) GRANTS: don't over-index on
   permissions AT ALL right now — build.omg lives in the dir being
   built, builds to build/; main.omg is NOT blessed (build.omg
   specifies the root; maybe a default-build.omg convention later);
   (c) EFFECT GATE: `filesystem` is a DECLARED effect on build's main
   fn — allowed there, forbidden elsewhere, enforced by the effect
   system; relax build_config.rs's empty-effect gate to exactly that;
   (d) CONSOLE: add to build.omg's declared effects ("harmless and
   everyone wants it"); the interpreter must treat it as a declared
   effect, never silently swallow logging. Implementation next: relax
   the gate, declare+enforce the two effects, thread the injected
   filesystem instance to the granted interpreter entry
   (evaluate_build_machine_with_filesystem is already there).

2. ~~reversed-operand receiver residual~~ — **CLOSED 2026-07-10y.** The
   three-session hunt bottomed out in TWO stacked holes past the resolver:
   (a) the runtime-bodies SPLICE stamps callee statements with the
   CALLER's source key, so their `self.X`/`earlier.X` member paths (which
   name no caller field) fell into machine_owned's CROSS-MACHINE SWEEP —
   which had no receiver awareness (both operands → first-SystemTime@8,
   the equal-operands signature); (b) the sweep matches ANY machine layout
   attached to the same data, so even receiver-aware lookup by machine
   SYMBOL missed (`from_unix_seconds`'s layout ≠ the called
   `duration_since`). FIX: machine_owned entry fns now take (input,
   dispatch_index) and resolve bases DYNAMICALLY per resolved machine
   (resolved_machine_base → receiver_base_for), including at the three
   sweep sites; receiver_base_for's unique-call match is by ATTACHED-DATA
   equivalence (the receiver is a property of the data instance, not the
   machine). The a/b shuffle is now fully retired:
   runtime_system_time_after_2026_exit swept to natural spelling (70/70).
   Debug instrumentation kept env-gated (OMEGA_DEBUG_RECEIVER + the BTW
   binary-write entry prints).

3. ~~Receiver slice 2 (non-entry callers)~~ — **LANDED 2026-07-11b**
   (attempts 1-2 reverted 2026-07-10z/11a; the dungeon "regression" of
   attempt 2 turned out to be override over-application onto zero-size
   receivers' caller-owned reads). Final shape, three pieces: (a)
   `context_call_sites` carries the minting caller's PARENT context;
   per-context bases compose parent-first in `compute_receiver_bases`
   (parent base + receiver offset in the CALLER's machine layout;
   self/static/unresolvable stay `None` = by-type fallback); (b)
   ZERO-SIZE callee machines emit `None` deliberately (no self reads
   exist; an override could only mis-rebase caller-owned spliced reads
   — the dungeon lesson); (c) the contained-receiver fence serves
   non-entry DISPATCH calls by consulting the TABLE itself (every
   minted clone composed ⇒ serve; no re-derived predicate). Bonus
   fix: the table is now indexed by ARENA index (1-based) — the
   positional collect() was OFF BY ONE for every consumer, masked in
   slice-1 shapes because adjacent clone states share a context. Pin:
   calls/runtime_nonentry_second_receiver_exit (Holder-under-Main,
   second Tally, 21→70; wrong-instance delivers 300 / out-of-region
   writes). Self-call chain composition
   landed 2026-07-11c (a self-call context INHERITS its parent's
   composed base when attached data matches; pin:
   calls/runtime_selfcall_chain_second_receiver_exit). The INLINE
   route landed 2026-07-11d: receiver_base_for's recovery is a bounded
   CALL-CHAIN WALK (anchor = the case's composed table base; each hop
   adds the receiver's offset in the current machine's layout, self
   calls +0; distinct candidate bases = ambiguous -> refuse). The walk
   ignores the `reachable` flag -- spliced-out originals keep
   reachable=false while their copies run inside the case. FENCE
   VISIBILITY closed 2026-07-11g: the contained-receiver fence now
   examines SPLICED-LIVE calls (liveness fixpoint from the entry;
   serve = composed source-machine base + unique-in-family final hop +
   resolvable path, mirroring the walk; param/local receivers in
   spliced code CLOSED 2026-07-11h: the by-type walk is EXACT for a
   single-instance family (pass pin:
   calls/runtime_param_receiver_single_instance_exit) and read the
   FIRST instance regardless of the argument for multi-instance
   families (silent-wrong 7-for-9; now fenced loudly, fail pin:
   fail/calls/param_receiver_multi_instance_rejected). SERVE LANDED
   2026-07-11i: the receiver chain walk carries a PARAM ENVIRONMENT --
   each descent binds machine-typed `&mut` (MutableAlias) params to
   their argument path's ABSOLUTE base (field of the source at base +
   offset, or a bare name forwarded from the source's own env); a
   single-segment param-receiver hop resolves through it. Fence mirror
   in lockstep (MachineAnchor base + params, machine-granular with
   poisoning = strictly more conservative than the position-granular
   runtime walk). The multi-instance fail pin FLIPPED to
   calls/runtime_param_receiver_second_instance_exit (delivers 9).
   NOTE the state-calls plan has its OWN expression table --
   StateCallArgument.expression indexes state_calls.expressions, NOT
   control_flow's (cost one debug cycle). Residuals: param-ROOTED
   nested receiver paths (`t.inner.method()`) stay unrecoverable ->
   fenced; re-borrowed param FORWARDING (`self.inner(&mut t)`) serves
   natively but the INTERPRETER DECLINES the spelling ("unknown
   value-call target") -- an interp frontend gap, repro
   scratchpad/slice2/param_forward_chain, differential-unprovable
   until fixed). Pin:
   fail/calls/ambiguous_spliced_second_receiver_rejected (two
   same-family calls in one state: `second` blocked loudly; was
   silent-wrong 7-for-9 native). SAME-DAY REGRESSION FIX rolled in: the
   scope-fix key order (465b82bbf) broke account_ledger (samples gate
   was NOT in the iteration protocol -- now it is): a call-target
   resolution key stole each idx-arm's same-named `b` for arm 0's slot.
   The leaf-write's bare-name keys are now gated by GENUINELY scoped
   resolvability (runtime_frame_slot_for_expression_scoped -- the
   lenient last-resort arms in find_runtime_frame_slot_for_path made
   the first "strict" attempt lie), branch key first when it strictly
   resolves; the target key comes from the SLOT side (unique-per-machine
   by name; state symbols differ across planning layers). Pins:
   calls/runtime_multiarm_same_named_locals_exit + account_ledger's
   documented-exit sample test. NEW FRONTIER found by the
   inline probe (PRE-EXISTING, receiver-independent, repro
   scratchpad/slice2/nested_inline_chain_single + _second_receiver):
   nested inline VALUE-call chains (entry -> holder.run() ->
   self.only.get()) scramble their result-forwarding copies natively
   (`frame@12 -> frame@0` before frame@12 is written -> ZII delivered;
   interp 70 vs native 71). FIXED 2026-07-11e -- not
   ordering but a NAME-COLLISION scope bug: the outer leaf
   terminal-write's branch-key attempt (the arm target owns no slots)
   fell into the case-wide NAME fallback and copied the CASE's
   same-named local (Main's unwritten `total` -> ZII). Fix: the
   CALL-TARGET state (the arm-owning callee scope that spelled the
   args) is the FIRST resolution key in
   select_runtime_leaf_branch_terminal_value_write. Pins:
   calls/runtime_nested_inline_chain_result_exit (colliding names),
   calls/runtime_nonentry_inline_second_receiver_exit (the full
   slice-2 inline shape: chain-walk recovery + this fix). Residual
   (recorded, unfenced): args spelled in a DEEPER callee state than
   the call target's entry still miss the scoped lookup and ride the
   name fallback -- same class, needs an arm-owner key on the
   expansion representation if it surfaces. The two frontier
   return-write shapes SERVED 2026-07-11f: (a) the call-bound-local
   bare terminal was a SEGMENT miss (the terminal lives in the state's
   tail segment; the return-write's control-flow lookups normalize to
   segment 0 now — terminal_target_value_expression); (b) the
   field-binding delivery resolved `self.total` with a DUMMY dispatch
   index 0, so the caller's composed receiver base never applied
   (by-type wrote the FIRST Mid's field) — the fallback now resolves
   under the return edge's target dispatch case. Pins:
   calls/runtime_nested_local_terminal_second_instance_exit,
   calls/runtime_nested_field_terminal_second_instance_exit (both
   double-nested two-Mids × two-Tallys shapes, exact values).

4. **Windows-session bundle** (needs a Windows host): verify the stat-row
   migration natively; WINDOWS_IMPORT_ROWS migration into provides files;
   Win32 rows for the no-msvcrt ops; file_journal-on-windows recheck;
   WndProc entry stubs (title-bar close).

5. **linux** — binding tables are structural-only until a target host exists.

6. **Authored-bindings interp story** — OWNER_QUESTIONS.md #10 (native-only
   imports today; differential skips).

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
  counts. Suite baseline-7 (all other lanes', fully explained): efi ×3
  (efi_entry_arguments / efi_freestanding_skeleton / efi_ref_param_call_arg),
  tick ×2 (runtime_tick_count_monotonic / tick_paced_marquee — aarch64
  Clock lowering, time lane), runtime_gui_memory_dc_blit,
  pass_canaries_compile (= the same five canaries, no hidden extras).
  samples_compile macOS baseline-5: tick_marquee, stdin ×3 (frontend WIP),
  uefi_hello.
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
