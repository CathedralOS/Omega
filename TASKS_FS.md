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
> asserts. Gates per iteration: canary_suite (expected-GREEN on macOS since
> 2026-07-11t — any failure is a regression), native_filesystem_canaries
> (macOS 88/0), samples_compile (BOTH test fns -- compile set AND
> documented-exit set; skipping it hid the account_ledger silent-wrong for
> four pushes, 2026-07-11g), omega-interpreter coverage + differential +
> `run_canary_list` drift guard (run IMMEDIATELY after every rebase),
> real_fs. PLUS the explicit ISA unit runs (`-p omega-isa-aarch64 -p
> omega-isa-x86_64`) when touching encoders/widths — workspace-root cargo
> test does NOT cover them (they rotted for days once behind a green
> suite). Push every iteration: fetch → rebase → survival-grep → re-verify
> → push, with rebase and push in SEPARATE calls (three recorded slips).

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

0. **std Console byte ops (Q11/Q12 owner direction, 2026-07-16).**
   SURVEY CORRECTED THE PREMISE: samples already share ONE declaration
   (std/console.omg's `platform Console`; cli samples consume it via
   compiler host-op lowering) -- only the CANARY/build.omg convention
   hand-spells inline boundaries, because platform entries carry no
   effect rows (the granted-build gate needs them). LANDED (slice 1):
   `entry read_byte() -> ByteRead` -- a std sum (`case Eof; case
   Byte(value: i32)`), Eof = ordinal 0 = the ZII zero case (owner
   VETOED the -1 sentinel, 2026-07-16) -- + `entry write_byte(byte:
   i32)` on platform Console; interpreter serving (statement +
   value-position dispatches); grammar pin for value-returning platform
   entries + echo/checksum coverage test
   (console_byte_ops_echo_and_checksum). Q10 CLOSED same day: authored
   imports DECLINE interpreted, differential skip is the design (no
   virtual stubs; "interpreter as a WASM-like target" recorded for the
   IR-shipping future).
   NATIVE (aarch64) LANDED: ReadRuntimeByte/WriteRuntimeByte
   composites end-to-end (selection gated on
   PlatformCallData::SingleByteRead/Write, rows in all three
   calling-convention tables, relocations, machine-instruction kinds,
   emission blocker for unserved shapes -- selection emits NOTHING
   generic for byte ops; a miss refuses the compile). Root-caused en
   route: `expression_platform_receiver_type` matched only boundary
   TRAITS, so ANY value-returning `platform` entry call (`let r =
   self.console.read_byte()`) silently missed the host-call plan and
   left its local ZII -- now also matches platform state signatures
   (OMEGA_DEBUG_HOSTCALL gates the collection trace). Pinned by
   pass/host/runtime_console_byte_echo_exit (empty stdin = Eof arm =
   the pre-zeroed slot, exit 70, differential-registered; piped "AB" =
   echo + exit 201 in the suite test). SAMPLES ZEROED: the stdin trio
   rewrote onto ByteRead + `self.console` byte ops -- samples_compile
   is FULLY GREEN (139/139, both fns; the baseline-3 era is over) and
   the INPUT-GRID rows verify natively with piped stdin ("AB"->"BC"/2,
   "Mix."->"MIX"/3). std's `Byte(value: i32 [0..=255])` now declares
   the honest payload range (construction-enforced). FOUND EN ROUTE
   (not blocking, field route works): a local initialized from a
   BINARY over another local-from-param (`let rotated = b + 1` in a
   non-entry state) hits "state values: CallArgument binary needs
   runtime value lowering" -- the samples route computed bytes through
   a field instead; give the local shape a lowering (or a better
   diagnostic) when it next surfaces. REMAINING: (x) x86_64 byte-op
   encoders (loud refusal today; windows rows registered incl.
   GetStdHandle pairs). (c) The effect-rows unification for platform
   entries (what BuildLog hand-spelling actually needs) -- separate
   rung, may need owner input on platform-vs-boundary-trait
   convergence.

0b. **[CLAIMED 2026-07-17, from TASKS.md NEXT PICK (owner priority
   2026-07-15)] Cathedral M2 unblock.** DIAGNOSIS (2026-07-17, this
   host): the "two red efi tests" appear STALE -- the uefi_x64
   cross-compiled PE contains the `mov rax,[rcx+8]; call rax` dispatch
   (needle verified at .text offset 1688, so the vtable encoder is NOT
   width-0 today), and all four efi suite tests that run here PASS
   (ref_param_call_arg + direct_faces report-checks included). The
   cfg(windows) byte-pin twins need the next Windows session to
   re-baseline, but the same encoder path proves out cross-target.
   THE REAL M2 WORK (their note agrees): the FIELD MODEL -- `provides
   Trait over VtableStruct { method -> field }` (extern brief SS12.1,
   decided 2026-07-04) has zero implementation; Cathedral is authored
   in it. Recipe from today's survey: (1) syntax: provides block gains
   the optional `over <Struct>` clause; arm RHS bare identifier ->
   ProvidesBindingKind::VtableField { field } (RHS is already
   expression syntax; VtableSlot parse stays for the existing
   canaries). (2) resolution: over-struct + field names resolve; the
   FIELD OFFSET comes from the layout plan (the byte-op
   payload-offset precedent) -- normalize BOTH forms to a byte offset
   (VtableSlot(n) = n x pointer_size) so one encoder serves.
   (3) encoding: generalize encode_vtable_call_sequence's `index` to
   the byte offset (calling-conventions lib.rs:988 is the
   binding-kind -> mechanism seam; emission/host.rs:27 the dispatch).
   (4) canary: the M1 greeting re-authored in the field model
   (header-prefixed EfiSimpleTextOutput struct, output_string as a
   NAMED field) pinning the SAME dispatch needle; then M2-ladder #1
   (`&mut` out-params, get_memory_map's five) gets its canary. NOTE
   this lane also owns the adjacent x86_64 byte-op encoder follow-up
   (item 0 (x)) -- same encoder territory.

1. **Windows-session bundle** (needs a Windows host): verify the stat-row
   migration natively; WINDOWS_IMPORT_ROWS migration into provides files;
   Win32 rows for the no-msvcrt ops; file_journal-on-windows recheck;
   WndProc entry stubs (title-bar close).

2. **linux** — binding tables are structural-only until a target host exists.

3. **Owner-question residuals**: Q10/Q11/Q12 all ANSWERED 2026-07-16
   (recorded inline in OWNER_QUESTIONS.md); the remaining engineering
   from them lives in item 0's REMAINING list.

4. **Recorded residuals:** (a) ~~deeper-callee-state name collisions~~
   — probed NOT-REPRODUCIBLE 2026-07-11x (two live same-named locals,
   deep delivering arm: correct on both engines) and pinned
   (calls/runtime_deep_state_name_collision_exit); (b) typed
   machines carry no source file (fine until a second consumer after
   is_build_machine needs one); (c) ~~console-less default exit~~ — FIXED 2026-07-11y: terminate
   edges (and edgeless cases) zero the return register when no terminal
   value writes it; natural termination exits 0 matching the oracle,
   value terminals and exit_process untouched. Pin:
   core/runtime_natural_termination_exit (differential).

(Closed arcs live in the git log and their canary headers — recent
pointers: receivers e0c718793→cd271c670; build.omg bc086f0a3/0bc474e81/
a09d23932; D14+twins 721175a1d/93b2127a3; gui headless 3528b9f5d;
tick rows 50a339aa0; uefi_x64 cross-target 3915d1cec/631fa6e28.)

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
  canaries, no hidden extras). samples_compile macOS baseline-3
  (2026-07-11v; uefi_hello cross-compiles with the registered
  uefi_x64 target — the harness gives target-shaped samples their
  explicit target): stdin ×3 (Q12-gated). The documented-exit fn's
  wrong-exit set was member-verified 2026-07-11z: all three entries
  are the SAME trio's compile-failure echoes — zero runtime
  misdeliveries hide behind the aggregate count. SUITE BASELINE: ZERO failures
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
