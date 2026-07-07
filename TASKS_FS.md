# Tasks — Filesystem (`std::fs`)

> **⏸ AUTONOMOUS LOOP STOPPED (2026-07-12).** The recurring `/loop` (cron job
> `371842c4`) was unscheduled at a clean stopping point for handoff — tree green,
> everything pushed to `origin/main`. This file is the source of truth; it has full
> context to pick up fresh. To resume unattended work, re-create a `/loop` that
> reads this file (or just continue manually from **START HERE** below).
>
> **START HERE (fresh pickup).** The fs is broadly complete (interpreter = full
> Rust-parity; native raw seam = complete; native ergonomic wrapper = scalar/tag
> results work). **Blockers #2 and #3 are FIXED in shared instruction selection
> (2026-07-06, Windows thread)** — see LIVE BLOCKERS for the decomposition, fix
> sites, and the two pure-language twin canaries that pin them (verified RUNNING
> natively on windows_x64 + by the fs repro's cross-compiled aarch64
> backend_report). The next concrete task is **macOS runtime confirmation +
> promotion**: on a Mac, run `canaries/run/filesystem/wrapper_metadata_repro`
> (expect "PASS: metadata_path Ok with meta.len == 5"), then promote the
> result-asserting wrapper canaries into `native_filesystem_canaries` (next
> steps #4).
>
> **WORKING RULES (were the loop's standing instructions).** Consult
> `wiki/language_guide/*` before adding language features; prefer ZII / arena /
> `Handle` / `HandleSpan`; check Rust source for `std::fs` parity; full human-word
> names (`create`/`open`/`read`/`write`/`close`/`remove`/`metadata`), C symbols only
> in the per-target binding table; macOS/aarch64 AND (since Zach's 2026-07-06
> redirect) windows/x86_64 are TESTED targets; keep linux structurally ready;
> build canaries that RUN and verify behavior. Gates that must stay green: Console lowering; `omega-instruction-
> selection`/`omega-relocations`/`omega-calling-conventions` crate tests; the
> interpreter fs coverage in `canary_suite`; the native fs harness. Verify
> canary_suite regressions by an **A/B failure-set diff** (the 85 failures are
> pre-existing and NOT ours), never by raw count.
>
> **PUSH TO MAIN.** After committing: `git fetch origin`; if behind, `git rebase
> origin/main` + re-verify; then `git push origin HEAD:main`. Our fs work and the
> other omega-rs work are on DISJOINT files (rebases stay conflict-free); the
> bootstrap-lattice agent is on a separate line. Keep main green.

## North star

A **serious, ergonomic `std::fs`** with **parity to Rust's `std::fs`**, differing
only where Omega is better: `Result<T,E>` → bespoke Omega `data` case enums;
**full human-word names** (`create`/`open`/`read`/`write`/`close`/`remove`/
`metadata`) — NO legacy C abbreviations (`creat`/`unlink`/`stat`) in the Omega
surface (C symbols like `_creat` live ONLY in the per-target binding table).
Portable `Filesystem` wrapper over a per-OS raw `FilesystemHost` seam (= Rust's
`std::fs` over `std::sys`). macOS/aarch64 is the only TESTED target; keep
x86_64/linux/windows structurally ready. Consult `wiki/language_guide/*` before
adding language features; prefer ZII / arena / `Handle` / `HandleSpan`. Every
fire leaves the gates green (Console lowering; `omega-instruction-selection`/
`omega-relocations`/`omega-calling-conventions` crate tests; interpreter fs
coverage; the native fs canary harness) and commits.

## Current state (top of mind)

**Green baselines:** canary_suite **518 pass / 85 fail** (the 85 are pre-existing
interpreter-differential-unsupported cases from the other omega-rs workstream —
NOT ours; verify no NEW failures by A/B failure-set diff, never by raw count).
Native fs harness **55/55** (`omega-compiler --test native_filesystem_canaries`).
(On WINDOWS hosts the suite is a different set — 621+ pass / 0 fail as of
2026-07-06; the 85-failure framing is the macOS run.)

**WINDOWS IS NOW A TESTED TARGET (Zach's redirect, 2026-07-06).** The raw seam
runs natively on windows_x64 through msvcrt bindings (open/open_create/creat/
read/write/close/unlink/lseek(_lseeki64)/mkdir/rmdir/rename/dup/fsync(_commit)/
chmod/read_errno(_errno, deref)) riding the general Win64 import-call encoder,
which gained data-address (string-literal path) + byte-length argument
marshalling and a deref-result flag. Differential canary:
`canaries/pass/filesystem/windows_raw_roundtrip_exit` (create/write/read/verify/
remove + ENOENT errno). Ops with NO clean msvcrt equivalent (pread/pwrite, *at,
link/symlink/readlink, read_dir, flock, chown, futimens, realpath) and the STAT
FAMILY (per-OS record layout — the portable-Metadata decode question) keep the
clean "no native lowering" error on windows; they are the remaining windows-seam
work and likely want Win32 calls rather than msvcrt.
**Flag portability note:** the raw seam passes flag WORDS through, so flag
VALUES are per-OS at the call site (windows O_BINARY=0x8000 is REQUIRED for
binary reads — msvcrt defaults to text mode; the interpreter's virtual fs uses
darwin numerology and ignores unknown bits, so O_BINARY is differential-safe).
**Showcase:** `samples/gui/image_viewer` — BMPs loaded from disk (msvcrt seam),
decoded in pure Omega (24bpp bottom-up BGR → top-down 32bpp, running-counter
walk, no computed indices), StretchDIBits-stretched into a real window;
RIGHT/LEFT reload+flip between three committed .bmp assets, ESC/X quits.
Verified live: window visible, three distinct renders screenshot-compared,
a REAL mouse click on the X closes it, works launched from build\ (asset
fallback). Compiled-not-run by the harness (no exit annotation, like
window_app).

**GUI close-path fixes (Zach's findings, 2026-07-07).** (a) The X button never
closed ANY sample: the `#32770` dialog proc swallows close signals — every
pump now intercepts posted WM_CLOSE (16), the X press (WM_NCLBUTTONDOWN 161 +
wParam HTCLOSE 20), and queue-visible WM_SYSCOMMAND/SC_CLOSE, destroying the
window itself (image_viewer + window_app + windowed_calculator). (b) The
"Ctrl+Shift+Esc closes it, wtf" mystery: GetAsyncKeyState is GLOBAL — the
chord contains ESC. New 0-arg user32 op `Gui.foreground_window`
(GetForegroundWindow, rides the general import call; interp = last live
virtual window) gates ALL sample key polling on focus. Canary:
`host/runtime_gui_foreground_window_exit` (call-path pin; no value assertion —
interp/native foreground semantics differ by design). The darwin provider grew
`MacosGui::foreground_window` (`[NSApp keyWindow]` — returns the NSWindow
`window_create` minted when focused, so `fw == hwnd` works unchanged); all
three samples cross-compile to macos_arm64 post-change. (c) The exe now falls
back to `../imgN.bmp` so double-clicking in build\ works. STILL OPEN: the
title-bar CONTEXT-MENU Close sends (not posts) WM_SYSCOMMAND — invisible to a
pump; the real fix is outbound WndProc entry stubs (extern brief §12.4).
DESIGN Q for Zach: build.omg asset copying (declarative `Build` asset list the
compiler copies at emit? — build.omg must describe, never do).
**Wrapper NATIVELY VERIFIED on windows_x64 (2026-07-07)** — canary
`filesystem/windows_wrapper_results_exit` runs the REAL ergonomic wrapper end
to end: write_all→Ok, create_dir×2→Error{AlreadyExists}, open→Ok{File}
destructured and USED (read through the File value-call arg), real close,
remove→Ok, remove(missing)→Error{NotFound}. Three finds along the way:
1. **Alias-resolved LITERAL host args (compiler, FIXED):** a wrapper param
   forwarded to a host call and bound to the caller's literal
   (`fs.read(file, &mut buf, 32)` → callee `count`=32) resolved as neither a
   place nor an immediate → clean encoder error. `scalar_argument_operand_at`
   now follows the alias to an Integer literal (`alias_resolved_integer_at`).
2. **Bare TERMINAL host calls silently drop (wrapper FIXED; compiler face
   OPEN):** `Filesystem::close`'s terminal `self.host.close(..)` emitted NO
   call natively — rc read ZII 0 ("success") while the fd stayed open; macOS
   never noticed (POSIX unlinks open files), Windows did. Wrapper sites now
   let-bind (close, create_dir_all). ⚠️ ENGINEERING NEXT: make a terminal-
   position host call a CLEAN COMPILE ERROR (the no-silent-fall-through
   doctrine) instead of a silent no-op.
3. **⚠️ FLAG-VALUE PORTABILITY (feeds design #6):** the wrapper's POSIX flag
   words carry DARWIN values; darwin O_CREAT (0x200) is msvcrt O_TRUNC, so
   `create_new`/`open_with` on Windows would silently TRUNCATE the file they
   must refuse to touch. The windows `open_create` row/lowering is REMOVED
   (clean "no native lowering" fence) until the portable-flags design — same
   design bucket as the stat-record layout: per-OS VALUES inside one portable
   wrapper (flags, stat offsets; errno values happen to agree so far).
   Also fixed: File-consuming wrapper machines capture `file.fd` into a
   `file_fd` scratch field (a param MEMBER is not a marshallable host arg; a
   let folds back to the member).

**Interp fs coverage FIXED (2026-07-07).** The 11 `filesystem_std_module_*`
failures ("non-integer operand", pre-existing at the fs handoff commit) were
the value-position `match` desugar doing TAG ARITHMETIC over payload-free
cases (`ErrorKind::NotFound - ErrorKind::Other`, parser primary.rs) that the
interpreter evaluated as `Value::Enum` operands. Fix (evaluator.rs): integer
binary operands accept a payload-free case at its TAG ORDINAL
(`arithmetic_operand_int`/`enum_variant_tag`), and `values_equal` compares a
tag INT against a case by ordinal (the desugar's Int result flows back into
enum-typed places; native compares tag constants either way). Also synced the
differential drift guard's RUN_CANARIES with 45 accumulated canaries from
both workstreams (incl. windows_raw_roundtrip + gui foreground_window).
✅ FOLLOW-UP DONE (2026-07-07, same day): `Value::Enum` now carries
`type_symbol` (like `Value::Struct`); every interp construction site threads
the declaring type (zero-case, bare-variant, case literal, wire verdict;
the build-time boundary stays symbol-less by design), and `enum_variant_tag`
resolves ordinals WITHIN the declaring type (name-global scan only as the
symbol-less fallback). Pinned by coverage test
`match_terminal_tag_arithmetic_resolves_type_locally` (a `Decoy` enum
declared first with a same-name `Ok` at ordinal 0 vs `Verdict::Ok` at 1 —
name-global resolution dispatches the wrong arm).

**What works today**
- **Interpreter:** full Rust-parity fs (all ops + the ergonomic `Filesystem`
  wrapper), the reference for correctness. Every op has interpreter coverage.
- **Native raw seam (`FilesystemHost`):** complete + real-macOS-correct — 54
  canaries covering create/open/read/write/close/remove/seek/mkdir/rmdir/rename/
  link/symlink/readlink/stat/lstat/realpath/chmod/fchmod/chown/dup/pread/pwrite/
  ftruncate/fsync/futimens/flock/openat/unlinkat + variadic-mode `open_create`.
- **Native ergonomic wrapper:** side effects work; SCALAR/tag-level results work
  after the deep fix (below) — `exists`→bool, `write_all`→UnitResult Ok/Error,
  `open`→OpenResult Ok/Error tag, `try_exists`→ExistsResult Yes/No, `last_error`.
- **Sample:** `samples/cli/systems/file_journal` — real end-to-end raw-seam
  workflow, `Expected exit: 7`, covered by `sample_file_journal_exits_7`.

**BLOCKERS (all three now FIXED in shared instruction selection; macOS runtime
confirmation pending).** The interpreter was fully correct for all of it.

1. **✅ ENUM-transition-leaf NULLARY-arm delivery — FIXED (2026-07-12).** A value-call
   whose callee transitions to enum leaves where an arm is a NULLARY variant (`Err`, a
   bare `Type::Case` Name) into a slot whose enum is LARGER than a scalar (>8 bytes)
   emitted NO terminal-slot write — the frame-slot value-write's scalar paths are gated
   on `byte_size ∈ {1,2,4,8}`, so a nullary variant into a 24-byte enum slot fell
   through to `None`; its result-slot copy then read the ZII frame and the whole 2-arm
   delivery mis-selected. FIX (`writes/mutation/frame_slots.rs`): when the value resolves
   to an enum variant (`enum_variant_value_in_table`) and the slot is non-scalar, write
   the variant's TAG (`ENUM_TAG_BYTES`) at the slot start. Regression:
   `canaries/pass/filesystem/native_enum_result`. canary_suite **85-baseline, zero new
   failures**; native fs harness **55/55**.
2. **✅ BIG multi-field StructLiteral payload — FIXED (2026-07-06, Windows thread).**
   The `meta.len == 0` symptom decomposed into TWO shared-selection bugs, isolated by
   CROSS-COMPILING the repro to macos_arm64 from Windows and reading the emitted
   `backend_report.txt` (build a temp `build.omg` declaring the target — the report's
   Target Operations list is the linear emission):
   - **#2B (the payload-ZII poison): the decode arm's straight-line statements were
     emitted ABOVE the entry's host call.** The deferred value-call leaf fired its
     GUARD after `stat` (the deep fix), but the arm's `let` locals (the straight-line
     expansion) stayed hoisted above it, so the terminal copied a pre-stat ZII
     stat_buf — right tag, zero payload. FIX (`runtime_dispatch.rs`): when a leaf
     defers past the callee's spliced HostCall/LocalStorage ops, its straight-line
     expansions defer WITH it and fire (locals first, then guard/terminal) at all
     three fire sites. Twin canary RUNNING on windows_x64:
     `canaries/run/value_call_entry_host_state_payload` (read_line as the entry host
     call; exit 72 = the regression).
   - **#2A: the one CAST-valued field (`mode: mode as u32`) had NO write at all.**
     The leaf-path scalar write cascade (`branches/mutation.rs`, a PARALLEL
     decomposition to `writes/mod.rs`) had no convert arm, so a Cast field failed
     every strategy and silently dropped while its 15 siblings landed. FIX: the
     cascade gained a convert-write arm (reuses
     `select_runtime_convert_mutation_write_in_table`). Twin canary:
     `canaries/pass/calls/runtime_value_call_struct_payload_cast_field_exit`
     (exit 74 = the regression).
   Verified: both twins run natively on windows_x64 (74→70, 72→70); the fs repro's
   arm64 report now shows stat → guard → locals → terminal and all 16 payload
   writes incl. the mode convert. AWAITING a real macOS RUN to promote.
3. **✅ `Error{kind}` nested `last_error()` value-call — rides fix #2B.** The kind
   payload chain (errno → classification → kind local → payload) was present but
   mis-ordered like the decode locals; the run twin's "no" leg pins the nested
   value-call err-kind shape natively (exit 76 = the regression). The
   inline-the-errno-match workaround is NOT needed. Confirm on macOS with the
   faithful `try_exists` Error canary when promoting.

**Wrapper pattern for nested host-call results (established):** capture a host
result into a FIELD in the machine ENTRY (before any transition), then guard on
the stored field — sidesteps the nested host-call-in-guard the deep fix doesn't
reach (used by `try_exists`: `self.stat_rc`/`self.stat_errno`). Payload-free
results also use TERMINAL-VALUE COMPLETION (`exists`: `self.stat_rc == 0` as the
bare final expr) or a value-yielding `match` terminal (`last_error`: errno→nullary
ErrorKind); a `match` terminal is only payload-FREE (it desugars to tag arithmetic
— a payload arm needs `Add` on the data type, undeclared).

## Design decisions (ratified judgement calls — user reviews later)

- **D1. Full human-word API**, no legacy abbreviations; C symbols only in the
  darwin binding table. (User was explicit about `creat`.)
- **D2. Two layers** — portable `Filesystem` wrapper (result enums / `File`) over
  a raw `FilesystemHost` boundary (value-returning ints, per-OS lowering).
- **D3. Value-return + Omega-wrap** — raw ops return syscall ints; the wrapper
  builds `File`/result enums in Omega.
- **D4. `create` → libc `_creat`** (register mode), not `open` (variadic mode).
- **D5. Grow raw-seam breadth in parallel with wrapper native lowering** (they are
  separate tracks; the wrapper runs in the interpreter regardless).
- **D6. Raw-seam file-op breadth is COMPLETE**; the ergonomic wrapper is the
  remaining focus (pursued interpreter-first, then native).
- **D7. FIXED — receiver-typed value-call resolution.** `self.<field>.<method>()`
  with a MEMBER receiver was mis-classified as a self-call → resolved to a sibling
  state; fixed in validation (`omega-validation/src/calls.rs`) + the interpreter.
- **D8. Flag math is BRANCH-FREE bitwise** — Omega's exact-arithmetic obligation
  rejects `*`/`+` on `bool as i32`; `&`/`|`/`^`/`<<` carry no obligation, so
  `open_with` composes POSIX flags purely bitwise. Reuse for any bitfield.
- **D8-open. Variadic-mode `open` DONE natively** — `open_create(path,flags,mode)`
  marshals the trailing `mode` on the STACK per Apple arm64 ABI via a
  `StackScalarInteger` operand + `restores_stack()` predicate (adds `sub sp,#16 …
  str w,[sp] … bl … add sp,#16`).
- **D9. Deref-result host calls (reusable).** `dereferences_result()` on
  `HostOperationKey` inserts one `ldr w0,[x0]` after the `BL` to deref a returned
  pointer. LOCKSTEP RULE: the +4 must be applied at exactly three sites keyed on
  the predicate — `widths.rs`, `data_addresses.rs` (result-store operand 0), and
  the encoder — while the `BL` relocation is left alone (it precedes the ldr).
  First user: `read_errno` (darwin `___error`). (Same lockstep discipline for
  D8-open's `restores_stack()` +4.)
- **D10. Machine-to-machine self-calls work** — a machine can value-call another
  (`self.last_error()`); the wrappers rely on it.
- **D11. FIXED — runtime-length subslice write `write(fd, buf[0..n])`** (checker +
  backend).
- **D-oracle.** A differential oracle runs RUN canaries interpreter-vs-native;
  interpreter-unsupported constructs go to a skip bucket (the 85 "failures").
- **D-sample.** A runnable NATIVE sample must exercise the seam that works today
  (raw seam), not the wrapper's not-yet-native result payloads.
- Misc judgement calls (recorded in git history, still in force): `remove_dir_all`
  uses ONE `fuel=4096` budget (D-rda); `*at` names are trusted relative `&[u8]`
  (D-at); `create_dir_all` intermediate creates are best-effort (D-cda); the raw
  boundary lives in its own std module `filesystem_host.omg` (D-fs-host-module);
  `read_dir` counts/walks a single 512-byte fill per call (D-readdir).

## Deep fix — LANDED (2026-07-10, reference)

The value-call transition-guard deep bug (the blocker for scalar/tag-level native
wrapper results) is FIXED, in `omega-instruction-selection`:
1. **Ordering** (`selection/runtime_dispatch.rs`): the `defers_to_local_
   initializer` Case-B matcher now also treats a callee-body `HostCall` op (not
   just `LocalStorage`) as a reason to defer the outer value-call's inline leaf,
   plus a firing block after the general-path `select_host_call`. The callee's
   host-call store now precedes its inline transition guard (guard reads the real
   result, not ZII zero).
2. **Field-mutation constant-fold** (`selection/runtime_dispatch/writes/mod.rs`):
   the Mutation storage-write for `self.field = self.g()` — a BARE, RECEIVER-FUL
   value-call with a guarded AssignmentValue leaf — returns early
   (`statement_has_guarded_assignment_value_leaf`), so it stops re-materializing
   the callee's first leaf terminal as a constant over the leaf's guarded result.
   The bare + receiver-ful guard preserves `self.f = self.g() + 1` (binary path)
   and `max(x, self.g())` (builtin, no receiver).

Regression canary `canaries/pass/filesystem/native_value_call_guard`.

## Next steps

1. [x] **ENUM-transition-leaf nullary-arm delivery** — FIXED (blocker #1 above).
2. [x] **BIG multi-field StructLiteral payload** — FIXED (blocker #2 above,
   two-bug decomposition; twins pin both natively on windows_x64).
3. [x] **`Error{kind}` payload** — rides fix #2B (blocker #3 above); no stdlib
   rewrite needed.
4. [ ] **macOS runtime confirmation + promote result-asserting wrapper canaries**
   (needs a Mac): run `wrapper_metadata_repro` (expect PASS + len 5), then wire
   result-asserting canaries into `native_filesystem_canaries` (`metadata_path`
   len, `open` file usability, faithful `try_exists` Error).
5. [x] **windows_x64 seam LIVE (2026-07-06, Zach's Windows-first redirect)** —
   msvcrt bindings for the core op set, TESTED natively (roundtrip canary +
   the image_viewer sample). Remaining windows-seam work: the stat family
   (needs the portable-Metadata design — the wrapper's byte-decode hardcodes
   darwin offsets), and the ops without msvcrt equivalents (likely Win32
   calls). linux tables remain structural only.
6. [ ] **Portable Metadata decode** — the `Filesystem` wrapper's stat_buf
   byte-assembly assumes darwin `struct stat` offsets; windows `_stat64` (and
   linux `statx`) differ. Needs a per-OS decode seam or a host-normalized
   record before `read_metadata`/`metadata_path` can go windows-native. Design
   conversation with Zach before building.

## Observations (not fs, flagged for the user)

- `samples_compile` is broadly RED from a pre-existing aarch64 encoder bug
  (`b.ne target is not instruction aligned: N`) in ~35 branches across
  non-fs samples (algorithms/arithmetic/basics/…). NOT the fs work; `file_journal`
  compiles + runs cleanly. A task chip was spawned for it. The required gates
  (Console lowering, instr-sel/reloc/calling-conv crate tests, interpreter fs
  coverage, native fs harness) all stay green.
- On a WINDOWS host, `samples_compile` has 4 pre-existing failures (A/B-verified
  identical on the pre-fix compiler, 2026-07-06): `cli__systems__file_journal`
  (fs raw seam has no windows_x64 binding rows — by design until next-step #5)
  and `stdin_checksum`/`stdin_rot1`/`stdin_upper` (frontend errors: a computed
  `exit_process` arg, and `no local state write_byte` — the other workstream's
  samples, presumably WIP). NOT the fs codegen work.
- The fail canary `canaries/fail/build/build_machine_wrong_arity` was referenced
  by the suite since 18c2acb6e (the build.omg workstream) but its FILES were
  never committed — `fail_canaries_reject_with_expected_diagnostic_fragment`
  red-lit on every fresh checkout. Reconstructed 2026-07-06 (main.omg +
  build.omg with a 2-param `build`, expected fragment = the actual arity
  diagnostic); if the original author's local copy differs, theirs can replace
  it.

## Reference — cross-target seam

Each raw op has a per-target binding row (C symbol) + `insert_platform_lowering`.
macOS/aarch64 is wired + tested; windows/x86_64 is wired + tested for the core
set (msvcrt rows in `WINDOWS_IMPORT_ROWS`, `FilesystemHost` lowerings x86_64-
gated in `windows.rs`, all riding the general Win64 import-call encoder —
which now marshals data-address and byte-length args and honors
`dereferences_result`). linux needs its table filled (`open`/`openat` direct
syscalls) but the Omega surface + interpreter are target-agnostic, so adding a
target is table work, not surface work.

## Coordination

Local `main` (this fs work) and origin `main` (the other omega-rs workstream)
touch DISJOINT files; rebases have been conflict-free. Pull/push each fire. The
bootstrap-lattice agent is on a separate line.
