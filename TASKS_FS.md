# Tasks — Filesystem (`std::fs`)

> **AUTONOMOUS LOOP (this file is the source of truth).** A `/loop` runs every
> ~5 min re-reading this file to continue the fs work unattended. Cron job id
> **`371842c4`** — `CronDelete 371842c4` to stop (when fs is complete or blocked
> only on a user-only design decision). Keep **Current state**, **Next steps**,
> and **Design decisions** current every fire so the next fire (fresh context)
> can continue.
>
> **PUSH TO MAIN each fire (user-authorized).** After committing: `git fetch
> origin`; if behind, `git rebase origin/main` + re-verify; then `git push
> origin HEAD:main`. Our fs work and the other omega-rs work are on DISJOINT
> files (rebases stay conflict-free); the bootstrap-lattice agent is on a
> separate line. Keep main green.

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

**LIVE BLOCKERS (the remaining native ergonomic-wrapper work).** Both are
codegen bugs beyond the deep fix; the interpreter is fully correct for all of it.

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
2. **STILL OPEN — a BIG multi-field StructLiteral payload isn't fully delivered.**
   `metadata_path -> MetadataResult::Ok{meta: Metadata}` gives the right TAG but
   `meta.len == 0` (the 16-field ~120-byte `Metadata` payload is ZII). Parked repro:
   `canaries/run/filesystem/wrapper_metadata_repro/main.omg`. ISOLATED (2026-07-12): NOT
   the nullary-arm bug (fixed above); NOT local-valued fields (a 2-field `Ok{Pair{a:x,
   b:y}}` from locals delivers correctly); NOT the `Error{kind: last_error()}` nested
   value-call (a literal err arm still fails). Specific to the LARGE multi-field
   Metadata StructLiteral terminal through a value-call result slot. NEXT: the
   mutation-write for a big StructLiteral terminal (many fields) into a call-result slot
   — check whether every field is written and the frame→target copy width is right
   (look at how `StructLiteral` lowers in `writes/mutation.rs` ~L802 + the leaf copy).
3. **`Error{kind}` uses a nested `last_error()` value-call** (errno→ErrorKind); its
   `kind` may be ZII by the same nesting the `try_exists` rewrite sidestepped. The
   Yes/No/Error TAG is right; the kind PAYLOAD is the open item. Could inline the
   errno→kind `match` on the already-captured `self.stat_errno`.

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
2. [ ] **Fix the BIG multi-field StructLiteral payload** (blocker #2 above) —
   unblocks `metadata_path`/`open` struct payloads. Verify against
   `wrapper_metadata_repro` + full canary_suite.
3. [ ] **Fix `Error{kind}` payload** (blocker #3) — inline the errno→kind
   classification on `self.stat_errno` instead of the nested `last_error()`.
4. [ ] **Promote result-asserting wrapper canaries** once #2/#3 land
   (`metadata_path` len, `open` file usability, faithful `try_exists` Error).
5. [ ] **x86_64 / linux / windows seams** — binding TABLES only (structural
   readiness); macOS is the only tested target. See the cross-target reference.

## Observations (not fs, flagged for the user)

- `samples_compile` is broadly RED from a pre-existing aarch64 encoder bug
  (`b.ne target is not instruction aligned: N`) in ~35 branches across
  non-fs samples (algorithms/arithmetic/basics/…). NOT the fs work; `file_journal`
  compiles + runs cleanly. A task chip was spawned for it. The required gates
  (Console lowering, instr-sel/reloc/calling-conv crate tests, interpreter fs
  coverage, native fs harness) all stay green.

## Reference — cross-target seam

Each raw op has a per-target binding row (C symbol) + `insert_platform_lowering`.
macOS/aarch64 is wired + tested. x86_64/linux/windows need their binding tables
filled (symbol names differ, e.g. linux `open`/`openat` direct syscalls) but the
Omega surface + interpreter are target-agnostic, so adding a target is table work,
not surface work.

## Coordination

Local `main` (this fs work) and origin `main` (the other omega-rs workstream)
touch DISJOINT files; rebases have been conflict-free. Pull/push each fire. The
bootstrap-lattice agent is on a separate line.
