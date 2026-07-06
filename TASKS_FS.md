# Tasks — Filesystem (`std::fs`)

> **AUTONOMOUS LOOP (this file is the source of truth).** A `/loop` runs every
> 5 min re-reading this file to continue the fs work unattended. Cron job id
> **`371842c4`** — `CronDelete 371842c4` to stop (do this when fs is complete or
> blocked only on a user-only design decision). Keep this file current every
> fire: update **Current state**, **Next steps**, and **Design decisions** so the
> next fire (fresh context) can continue.
>
> **PUSH TO MAIN each fire (user-authorized).** After committing, publish to
> `origin/main`: `git fetch origin`; if behind, `git rebase origin/main` then
> rebuild + re-verify the fs canaries; then `git push origin HEAD:main`. Our fs
> work and the other omega-rs work are on DISJOINT files (rebases have been
> conflict-free), and the bootstrap-lattice agent is on a separate line. Keep
> main green.

## North star

A **serious, ergonomic `std::fs`** for Omega with **parity to Rust's `std::fs`**,
differing only where Omega is better: `Result<T,E>` → bespoke Omega `data` case
enums; **full human-word names** (`create`/`open`/`read`/`write`/`close`/
`remove`/`metadata`) — NO legacy-abbreviated C names (`creat`/`unlink`/`stat`)
anywhere in the Omega surface (C symbol strings like `_creat` live ONLY in the
per-target binding table). Portable wrapper over a per-OS raw seam (Rust's
`std::fs` over `std::sys`). macOS/aarch64 is the only TESTED target now; keep
x86_64/linux/windows structurally ready.

Working rules: consult `wiki/language_guide/*` before adding language features;
prefer ZII / arena / `Handle` / `HandleSpan` for compiler features; check Rust
source when unsure; every fire leaves regressions green (Console lowering;
`omega-instruction-selection`/`omega-relocations`/`omega-calling-conventions`
crate tests; interpreter fs coverage) and commits.

## Design decisions (judgement calls — user reviews later)

- **D1. Full human-word API, no legacy abbreviations.** `create`/`open`/`read`/
  `write`/`close`/`remove` in Omega; C symbols (`_creat`,`_unlink`) only in the
  darwin binding table. (User was explicit + annoyed about `creat`.)
- **D2. Two layers** — portable ergonomic `Filesystem` wrapper (hides flags/
  mode/fd behind `File`/result enums) over a raw `FilesystemHost` boundary
  (value-returning ints, per-OS lowering). = Rust `std::fs`/`std::sys`.
- **D3. Value-return + Omega-wrap** (ratified earlier): raw ops return syscall
  ints; wrapper builds `File`/result enums in Omega.
- **D4. `create` maps to libc `_creat`** (not `open`) because `open`'s mode is
  variadic (stack-passed/dropped on arm64); `_creat`'s mode is a register param.
- **D5. Grow the raw seam's Rust-parity BREADTH in parallel with (not blocked
  on) native-wrapper lowering.** The wrapper's forwarded-param native resolution
  is a deep backend area (parameter storage-place resolution across machine-call
  boundaries); rather than stall the whole effort on it, keep adding
  value-returning raw ops that DO lower natively (seek/stat/mkdir/…) and exercise
  them with run-verified canaries. The ergonomic wrapper already runs in the
  interpreter; native wrapper lowering is a separate track.
- **D7. FIXED (2026-07-06): receiver-typed value-call resolution.** A
  value-position `self.<field>.<method>(..)` where the receiver is a MEMBER
  expression (not a `self` name-path) was mis-classified as a self-call and
  resolved to a same-named sibling STATE (so a wrapper `Filesystem::create`
  calling `self.host.create` recursed / arg-count-mismatched). Fixed in BOTH
  engines: (1) `omega-validation/src/calls.rs::validate_expression_call_bounds`
  now extracts the receiver name from a `Member` receiver and only takes the
  self-call branch for a genuine self/receiverless call; (2) the interpreter's
  `resolve_value_call_target` guards its sibling-state / free-machine fallbacks
  to self-receiver calls, so a non-self receiver falls to the host/instance
  resolution. General language fix, not fs-specific. Added ZERO canary failures
  (154 pre-existing == 154 after; those are unrelated backend width mismatches
  on origin/main from the recent runtime-indexed-read work).
- **⚠ Pre-existing: 154 canary_suite failures on origin/main** (backend
  instruction-width mismatches, e.g. `CopyRuntimeMachineIndexedToRuntimeStorage`
  — from the other omega-rs workstream's runtime-indexed-read commits, NOT fs,
  disjoint files). Our mandated gates (Console lowering; instruction-selection/
  relocations/calling-conventions crate tests; interpreter fs coverage) stay
  green. Not ours to fix; flagged for the user.
- **D6. Raw-seam file-op breadth is now essentially COMPLETE** (create/open/read/
  write/close/remove/seek/create_dir/remove_dir/rename + append via flags). The
  highest-value remaining work is the **ergonomic wrapper** (the "serious,
  easy-to-use" std::fs — `File`/`Path`/result enums, no flags/fd leaking). Since
  native wrapper lowering is blocked (forwarded-param), pursue it INTERPRETER-
  FIRST: unify the interpreter onto the value-returning `FilesystemHost` raw seam
  (it currently handles an OLD out-param `Filesystem` shape in the coverage
  tests), then layer the ergonomic `Filesystem` wrapper (Omega machines) on top,
  running on both engines. Remaining raw ops (fstat metadata struct, read_dir,
  fsync, set_len) are lower priority than the wrapper.
- **D8. Flag math is BRANCH-FREE bitwise** (judgement call). Omega enforces exact
  arithmetic as a proof obligation (decision 17): `*`/`+` on `bool as i32` is
  REJECTED because the checker can't prove the operand is in {0,1}. `&`/`|`/`^`/
  `<<` carry no overflow obligation, so `open_with` composes the POSIX open flags
  purely bitwise (e.g. access mode = `(wbit & (rbit^1)) | ((wbit & rbit) << 1)`,
  `O_APPEND = (append as i32) << 3`). This is the pattern to reuse for any future
  flag/bitfield composition in the fs surface.
- **D8-open. Deferred: variadic-mode `open` host call.** Full `OpenOptions`
  (`.create`/`.create_new` = O_CREAT/O_EXCL) and a faithful `open(path, flags,
  mode)` need the variadic `mode` argument marshalled on the STACK per AAPCS64
  (arm64 passes variadic args on the stack; our host-call encoder passes args in
  registers, so a register mode is dropped — the D4 finding). Building a
  "one stack-passed trailing argument" path in the aarch64 host-call encoder is
  the right unblock; it's bounded backend work but deeper than a single loop
  fire, so deferred. Interim: `create` (→`_creat`, register mode) covers the
  common create-and-truncate case.
- **D9. Deref-result host calls (a reusable backend capability).** A host op can
  now return a POINTER whose pointee is the real result: `dereferences_result()`
  on `HostOperationKey` marks it, and the aarch64 lowering inserts one `ldr
  w0,[x0]` after the `BL` to deref before the store. This is GENERAL (not
  fs-specific) — the pattern for any libc that returns `T*` to an out-value.
  Correctness rule: the deref adds 4 bytes between the call and the result store,
  so the +4 MUST be applied in lockstep at exactly three sites keyed on
  `dereferences_result()` — `widths.rs` (layout width), `data_addresses.rs`
  (result-store adrp/add offset, operand 0), and the encoder — while the `BL`
  relocation offset is left ALONE (it precedes the ldr). First user:
  `Filesystem::read_errno` (darwin `___error`). Verify any new deref op by
  disassembly (`otool -tv`) as well as execution.

## Current state (update every fire)

- **Raw seam now has HUMAN method names** (create/open/read/write/close/remove)
  on the `FilesystemHost` boundary trait; ugly libc spellings only in binding
  symbols. Compiler feature landed: lowering lookup **prefers an exact-platform
  match over `"*"`** (`find_lowering_prefer_exact`), so fs `write`/`read` win
  over Console's wildcard. `canaries/pass/filesystem/native_crud` RUNS to PASS
  with the human names; `native_close` checks clean; no regressions.
- Native raw CRUD RUNS end-to-end on macOS via value-returning host calls.
- **`seek` (Rust `Seek`) landed natively** via `_lseek` (HostOperation::Seek,
  3 scalar args) — `canaries/pass/filesystem/native_seek` RUNS: seek-to-end
  reports the 17-byte size.
- **`create_dir`/`remove_dir` (Rust) landed natively** via `_mkdir`/`_rmdir`
  (HostOperation::MakeDir/RemoveDir; reuse the create/remove operand shapes).
  `canaries/pass/filesystem/native_dirs` RUNS: mkdir + nested file + rmdir → PASS.
- **`rename` (Rust) landed natively** via `_rename` (HostOperation::Rename) —
  needed a `find_nth_data_object` helper (two path literals in one call, resolved
  by creation/offset order = arg order). `canaries/pass/filesystem/native_rename`
  RUNS: create A + write + rename A→B + read B back (16 bytes) → PASS.
- Native raw ops now: create/open/read/write/close/remove/seek/create_dir/
  remove_dir/rename — all run-verified on macOS.
- **Append verified** (`canaries/pass/filesystem/native_append`): `open` with
  `O_WRONLY|O_APPEND` (0x9) appends; file grows 3→6 bytes. No new op — proves
  `open` handles arbitrary write flags (Rust `OpenOptions` parity). File-size is
  already available via `seek(fd,0,SEEK_END)` (Rust `metadata().len()`).
- aarch64 value-returning host calls implemented (the foundational primitive).
- Runtime slice/path host-call args implemented.
- Ergonomic wrapper runs in the INTERPRETER; lowering it natively is a separate
  (deep) track (forwarded-param resolution) — see D5, pursued in parallel.

## Next steps (ordered; keep this list live)

1. [x] **Rename raw ops to human words** — DONE (create/open/read/write/close/
   remove; `find_lowering_prefer_exact` compiler feature).
2. [x] **Raw-seam Rust-parity file-op breadth** — DONE (D6): create/open/read/
   write/close/remove/seek/create_dir/remove_dir/rename + append (via flags).
3. [x] **Ergonomic wrapper, interpreter-first** (D6) — DONE. Interpreter unified
   onto the value-returning `FilesystemHost`; the `Filesystem` wrapper (hides
   flags/mode/fd behind `File` + result enums) IS the shipped
   `omega/language/std/filesystem.omg`; imported via real `use` in coverage.
4. [x] **`set_len`** (Rust `File::set_len` via `_ftruncate`) — DONE, complete
   vertical (native `native_set_len` canary + interpreter `virtual_set_len` +
   wrapper `Filesystem::set_len`).
5. [x] **`metadata().len`** (Rust `File::metadata`) — DONE. Composed in the
   wrapper from `seek` (save/measure/restore cursor), non-destructive, no new
   native op. `Metadata`/`MetadataResult`; coverage `filesystem_std_module_metadata_len`.
6. [x] **`sync`** (Rust `File::sync_all` via `_fsync`) — DONE, complete vertical:
   `HostOperation::Sync` (op "fsync"→`_fsync`), darwin binding+lowering, operand
   arm (fd, same shape as `close`), interpreter no-op that validates the fd,
   wrapper `Filesystem::sync(file) -> UnitResult`. Native `native_sync` canary
   RUNS (prints `PASS: sync flushes 17 bytes`); coverage `filesystem_std_module_sync`
   (wrapper returns `UnitResult::Ok`, bytes intact). `sync_data`/`_fdatasync` not
   added (macOS has no `fdatasync`; Rust maps `sync_data`→`fsync`/F_FULLFSYNC there
   anyway — a later alias is trivial once an errno/fcntl story exists).
7. [x] **`OpenOptions`** (Rust `std::fs::OpenOptions`) — DONE for EXISTING-file
   opens. `data OpenOptions [copy, zero_init] { read; write; append; truncate }`
   + `Filesystem::open_with(path, options) -> OpenResult` compose the POSIX flags
   (access mode O_RDONLY/O_WRONLY/O_RDWR + O_APPEND + O_TRUNC) and call the raw
   `open`; plus `Filesystem::append(path)` convenience (O_WRONLY|O_APPEND). Pure
   Omega, no new native op. Flag math is BRANCH-FREE bitwise (see D8). Coverage
   `filesystem_std_module_open_options` (append grows 11→14, truncate empties to
   0); native `native_open_rw` canary RUNS (O_RDWR read+write on one fd → PASS).
   ⚠ CREATING opens (O_CREAT/O_EXCL: `.create`/`.create_new`) are NOT covered —
   they need `open`'s VARIADIC `mode` argument marshalled on arm64 (stack-passed
   per AAPCS64; today host-call args go in registers, so the mode is dropped —
   the same D4 issue that routed `create`→`_creat`). Until a variadic-mode host
   call lands, use `create` for the create-and-truncate case. See D8-open.
8. [x] **Whole-file one-shot helpers** (Rust `fs::write` / `fs::read`) — DONE.
   `Filesystem::write_all(path, bytes) -> UnitResult` (create+write+close) and
   `Filesystem::read_all(path, buffer, count) -> IoResult` (open+read+close). All
   raw ops run in the ENTRY (no slice threaded through a state): a create/open
   failure leaves fd<0 so the write/read/close no-op with errors and the `n >= 0`
   guard reports Error. Pure Omega, no new native op — the primitives are already
   native-verified by `native_crud` (and the ergonomic wrapper doesn't lower
   natively yet, D5/step 12), so no redundant native canary. Coverage
   `filesystem_std_module_whole_file_helpers` (15-byte round-trip + read_all on a
   missing path == Error). Rust returns a grown `Vec`; Omega fills a caller buffer
   to keep the std surface allocation-free (size it via `metadata().len`).
9. [~] **Error model** — the errno CAPABILITY + `ErrorKind` classification are
   DONE (D9); wiring the kind INTO the result enums is the remaining follow-up.
   Landed this fire:
   - **Backend deref host call** — `HostOperation::ReadErrno` (op `read_errno` →
     darwin `___error`) + `HostOperationKey::dereferences_result()`. Its aarch64
     lowering (`encode_host_call_sequence_value_returning_deref_from_operands`)
     emits `BL ___error` then one `ldr w0,[x0]` (0xB9400000) to deref `&errno`
     before the result store. The +4 shift is threaded through the THREE lockstep
     sites keyed on `dereferences_result()`: the width fn (`widths.rs`), the
     result-store data-address offset (`data_addresses.rs` operand 0), and the
     encoder. The BL relocation is unaffected (BL precedes the ldr; no args).
     DISASSEMBLY-VERIFIED: `bl <___error stub>; ldr w0,[x0]; adrp x16; add x16;
     str w0,[x16,#8]`. Native `native_errno` canary RUNS → `errno == 2` (ENOENT)
     for a missing open. No regressions (isa-aarch64 31 / instr-sel 10 / reloc 5
     crate tests green; Console cli_mvp native green).
   - **Interpreter errno model** — `virtual_errno` field, set on failures
     (ENOENT=2 open/remove/remove_dir, EEXIST=17 create_dir, EBADF=9 close), read
     by the `errno` op. Not cleared on success (POSIX). Coverage
     `filesystem_value_returning_errno`.
   - **Omega surface** — raw `FilesystemHost::errno() -> i32`; `data ErrorKind`
     (Other/NotFound/PermissionDenied/AlreadyExists/BadDescriptor);
     `Filesystem::last_error() -> ErrorKind` classifies the errno. Coverage
     `filesystem_std_module_error_kind` (open(missing) → Error → last_error() ==
     NotFound).
   - REMAINING: thread the kind INTO the result enums so failures self-describe
     (`OpenResult::Error(kind)` / `IoResult::Error(kind)` etc.) instead of the
     caller separately calling `last_error()`; map more errno codes; set errno in
     the remaining virtual failure sites (read/write/seek/set_len/sync/rename).
     Also errno is only valid immediately after a failing op (the wrapper ops do
     one host call each, so `last_error()` right after works today).
10. [ ] **Richer `Metadata`** via `fstat` — `st_size`/mode/times. Needs a
    stat-buffer out-param + struct-field reads (darwin arm64 `st_size` at off 96).
    Unlocks `is_file`/`is_dir`/permissions/modified. Medium-large.
11. [ ] **`read_dir`** (opendir/readdir) — directory iteration. Large (returns a
    growing sequence; needs an iterator/handle shape). Defer until an iteration
    idiom is settled.
12. [ ] **[deep, parallel] Native wrapper lowering** — forwarded-param →
    storage-place resolution across the machine-call boundary; store enum result
    through a wrapper `&mut out`; const-folded-literal-arg fix. Then the ergonomic
    wrapper lowers natively (today it runs only in the interpreter; the raw seam
    lowers natively). Deep backend work; see D5.
13. [ ] **x86_64 / linux / windows seams** — see the reference below. Tables only;
    macOS is the only TESTED target now. Note: linux value-return needs the
    value-returning result-store wired into the `svc` syscall path (today only the
    darwin `BL`/Import path stores the return register — see D8 when started).

---

## Reference — cross-target seam (for step 11)

Symbols/mechanisms per OS for the raw `FilesystemHost` ops (the only Rust seam;
everything above is portable Omega). macOS is done; the rest are structural.

- **linux** (`linux.rs`, `Syscall{number}`, arch-specific via
  `linux_syscall_numbers`): `openat`/`read`/`write`/`close`/`lseek`/`ftruncate`/
  `mkdirat`/`unlinkat`/`renameat`/`fsync`/`fstat`. aarch64-linux reuses the
  aarch64 value-returning encoder BUT via `svc` — the value-returning store must
  be added to the syscall path (darwin uses `BL`/Import, which already stores).
- **windows** (`windows.rs`, kernel32 imports): `CreateFileW`/`ReadFile`/
  `WriteFile`/`CloseHandle`/`SetFilePointerEx`/`SetEndOfFile`/`DeleteFileW`/
  `CreateDirectoryW`/`RemoveDirectoryW`/`MoveFileExW`/`FlushFileBuffers`. Paths
  re-encode UTF-8→UTF-16 at the boundary (the `Path` byte-domain stays portable).

## Reference — differential-oracle decision (ratified, D-oracle)

A real filesystem is stateful/nondeterministic, so the interpreter can't just
"match native". Chosen: **hermetic in-memory virtual FS in the interpreter**
(deterministic, no real disk) for the differential lane + coverage; native
canaries run against a fixed temp path and assert only the deterministic subset.
This is implemented (interpreter `virtual_files`/`virtual_fds`); native canaries
live in `canaries/pass/filesystem/native_*`.

## Coordination (unchanged, still true)

- **Bootstrap-lattice agent:** disjoint files (`compiler/{alpha,beta,delta,gamma}`);
  zero intersection. Rebases onto `origin/main` have been conflict-free.
- **omega-rs `TASKS.md`:** this is an additive slice (new files + additive
  enum/table entries); no in-flight task touched.
- The **154 pre-existing `canary_suite` failures** on origin/main are the other
  omega-rs workstream's backend width mismatches (runtime-indexed-read), NOT fs —
  disjoint files. Our mandated gates stay green. Flagged for the user.
