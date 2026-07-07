# Tasks — Filesystem (`std::fs`) + Windows-native thread

> **LOOP LIVE (Windows thread, since 2026-07-06).** Zach's redirect: Windows/x86_64
> is the primary TESTED target ("focus on us"); macOS/aarch64 canaries stay green
> structurally but runtime confirmation needs a Mac. This file is the source of
> truth; finished-work narrative lives in git history + the canaries that pin it.
>
> **WORKING RULES.** Consult `wiki/language_guide/*` before language features;
> ZII / arena / `Handle` / `HandleSpan`; full human-word op names (C symbols only
> in per-target binding tables); every fix ships a canary that RUNS and asserts.
> Gates: canary_suite (654+ green on Windows), samples_compile (grep the whole
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
  re-confirmation pending on a real Mac.
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
end-to-end — `filesystem/windows_wrapper_results_exit` runs write_all→Ok,
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
`calls/runtime_value_call_shared_slot_straight_line_exit`). Still-broken
positions: (a) a scalar/bool user value-call as a GUARD SUBJECT is silently
ALWAYS-TRUE (designed-false probe re-confirmed; any sound fence forces
corpus-wide bind-to-local rewrites incl. the dungeon — POLICY Q for Zach).
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
