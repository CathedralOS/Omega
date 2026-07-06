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
- **D10. Machine-to-machine self-calls WORK** (verified this fire). A machine can
  call a SIBLING machine on the same instance — `self.other_method(args)` — as a
  value call, including from inside a nested state and returning a `data` enum.
  Used to let every wrapper `err` state call `self.last_error()` to classify
  errno once, instead of duplicating the errno→ErrorKind cascade in ~15 places.
  This is a general composition primitive (not fs-specific): factor shared logic
  into a helper machine and call it via `self.`. (The interpreter's value-call
  resolution + the D7 receiver fix already handle it; no compiler change needed.)
  NOTE the interpreter runs the wrapper; native wrapper lowering (step 12) is
  still the separate deep track — so `self.last_error()` composition is proven on
  the interpreter, and will need forwarded-arg lowering to run natively.

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
- Native raw ops now: create/open/read/write/pread/pwrite/close/remove/seek/
  create_dir/remove_dir/rename/chmod/fchmod/link/symlink/readlink/stat/lstat/fstat/
  realpath/dup/ftruncate/futimens/fsync — all run-verified on macOS.
- **`File::set_times` landed natively** (Rust `File::set_times`) via `_futimens`,
  reusing the `fstat` fd+buffer operand shape. `native_set_times` canary RUNS: set
  mtime, fstat confirms. Introduced the `x as u8 in Wrapping` byte-decompose idiom
  + a `virtual_times` interpreter model. See step 10j.
- **`MetadataExt` landed** (Rust unix ext) — `nlink`/`ino`/`dev`/`uid`/`gid` +
  `ctime` (`changed()`), decode-only from the stat record (st_nlink u16@6, st_ino
  u64@8, st_dev @0, st_uid u32@16, st_gid u32@20, st_ctime @64), no new op.
  `native_metadata_nlink` / `native_metadata_ino` / `native_metadata_ctime_dev`
  canaries RUN. Time family (a/m/c/btime) + file-identity (dev,ino) complete. See
  steps 10k/10l/10m.
- **`File::sync_data` landed** (Rust) — reuses the `fsync` op (darwin has no
  fdatasync). `native_sync_data` canary RUNS. Sync family complete. See step 6.
- **`File::metadata` upgraded to `fstat`** (Rust `File::metadata`) via `_fstat`,
  a new `[result, fd, buffer]` operand arm; `metadata(file)` now reports the REAL
  mode/times (was a seek-based fake) and the stat/lstat/fstat trio is complete.
  `native_fstat` canary RUNS. See step 10h.
- **Positioned I/O `read_at`/`write_at` landed natively** (Rust `FileExt`) via
  `_pread`/`_pwrite` — new `[fd, buffer, count/len, offset]` operand arms (read/write
  + a trailing offset scalar). `native_positioned_io` canary RUNS: overwrite mid-file
  + read a slice, cursor untouched. See step 10i.
- **`try_clone` (dup) landed natively** (Rust `File::try_clone`) via `_dup`,
  reusing the `close` one-fd operand shape. `native_try_clone` canary RUNS: the
  clone stays valid after the original is closed. See step 10g.
- **`canonicalize` (realpath) landed natively** (Rust `fs::canonicalize`) via
  `_realpath`, reusing the `Stat` operand shape. `native_canonicalize` canary RUNS:
  realpath resolves `/tmp` → `/private/tmp` for real. First fs op to return a
  pointer-as-i64 success flag (no deref). See step 10f.
- **`symlink_metadata` (lstat) landed natively** (Rust `fs::symlink_metadata`) via
  `_lstat`, reusing the `Stat` operand shape (just a new symbol). `native_symlink_metadata`
  canary RUNS: lstat distinguishes a symlink (S_IFLNK) from its target. `Metadata`
  now has `is_symlink` + a faithful `is_file()`. The stat/lstat pair is complete.
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
   (wrapper returns `UnitResult::Ok`, bytes intact). **`sync_data` (Rust
   `File::sync_data`) NOW ADDED** — it maps to `fsync` on darwin (macOS has no
   `fdatasync`; Rust's own std falls back to fsync there), so it REUSES the `Sync`
   op/operand arm entirely: just a new `FilesystemHost::sync_data(fd)` method + a
   darwin lowering to `fsync` + wrapper `Filesystem::sync_data(file) -> UnitResult`
   + interpreter `"sync" | "sync_data"` arm. Native `native_sync_data` canary RUNS
   (17 bytes intact); coverage `filesystem_std_module_sync_data`. Zero new
   enum/operand/encoder work. The sync family (sync_all + sync_data) is complete.
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
9. [x] **Error model** — DONE. Failures now SELF-DESCRIBE: each result enum's
   `Error` case carries an `ErrorKind` (`OpenResult::Error { kind }` /
   `IoResult` / `UnitResult` / `MetadataResult`), filled at the point of failure
   by `self.last_error()` — a machine-to-machine self-call from each wrapper's
   `err` state (see D10). ZII zero case is `Error { kind: Other }`. Interpreter
   sets `virtual_errno` on EVERY failure site now (ENOENT open/remove/remove_dir/
   rename, EEXIST create_dir, EBADF close/read/write/seek/set_len). Coverage
   `filesystem_std_module_error_kind` proves the kind VARIES per cause:
   open(missing) → NotFound, create_dir(existing) → AlreadyExists (a hard-wired
   kind would fail the 2nd check). All 13 fs coverage tests green; no backend
   change this fire, so native is untouched. Caveat (noted): errno is only valid
   right after the failing op — the multi-syscall helpers `write_all`/`read_all`
   run create/open then write/read/close in one entry, so on a first-op failure
   the trailing ops clobber errno to EBADF; their reported kind is the LAST
   syscall's, not the root cause. Faithful for all single-call wrappers. Below is
   the errno CAPABILITY that this builds on (landed the prior fire):
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
   - ERRNO CODES mapped so far: ENOENT→NotFound, EACCES→PermissionDenied,
     EEXIST→AlreadyExists, EBADF→BadDescriptor, EISDIR→IsADirectory (the last set
     in the interpreter when `open_with(write)` targets a directory; coverage
     `filesystem_std_module_is_a_directory`). Add ENOTDIR/ENOTEMPTY/ENOSPC as
     scenarios that produce them appear.
   - FOLLOW-UP: make the multi-syscall helpers capture the root-cause errno
     (needs a post-first-op branch, i.e. threading the slice through a state —
     blocked on D-thread).
10. [x] **Path-query helpers** — `exists(path) -> bool` (Rust `Path::exists`,
    open-probe) and `metadata_path(path) -> MetadataResult` (Rust `fs::metadata`,
    open+seek-end+close), the path-based counterparts to the fd-based ops. Pure
    Omega, open/seek/close only (no buffer/subslice/threading). Coverage
    `filesystem_std_module_path_queries` (exists false→true→false around
    write/remove; metadata_path.len == 12); native `native_exists` canary RUNS
    (present after create, absent after remove → PASS). CAVEAT: open-based
    `exists` reports false for an unreadable-but-present path (EACCES); a faithful
    stat-based `exists` waits on `fstat`. ADDED: `try_exists(path) -> ExistsResult`
    (Rust `Path::try_exists`) — the error-aware form: `Yes` / `No` (only ENOENT) /
    `Error(kind)` (any other errno), so a permission failure is surfaced, not
    silently reported as absent. Interpreter open now enforces the READ bit too
    (owner-read 0o400 for a read-open, mirroring the existing write-bit check), so
    a chmod-0 path is EACCES on read — makes the `Error` case testable. DIFFERENTIAL:
    native `native_try_exists` canary RUNS (present→open ok; missing→ENOENT;
    chmod-0→EACCES on read → PASS) AND coverage `filesystem_std_module_try_exists`
    (Yes/No/Error(PermissionDenied)).
10b. [x] **`set_permissions`** (Rust `std::fs::set_permissions`) via `chmod` —
    DONE, complete NATIVE vertical. `HostOperation::Chmod` (op `chmod` →
    darwin `_chmod`); reuses the `mkdir`/`creat` operand shape (path pointer +
    NAMED register mode scalar), so no new operand/encoder work. `data
    Permissions { mode: u32 }`; raw `FilesystemHost::set_permissions(path,
    mode: u32)`; wrapper `Filesystem::set_permissions(path, perms) ->
    UnitResult`. Interpreter models enforcement: a `virtual_perms` map records a
    chmod'd mode, and `virtual_open_flags` returns EACCES on a write-open when the
    owner-write bit (0o200) is cleared (only chmod'd paths are checked; default
    files stay writable, so existing tests are unaffected). DIFFERENTIAL-
    consistent: native `native_permissions` canary RUNS (chmod 0o444 then
    write-open → EACCES(13) → PASS) AND coverage `filesystem_std_module_set_permissions`
    (chmod read-only → open_with(write) → Error kind PermissionDenied). The
    `Permissions` type is now complete: `Permissions::readonly()` and
    `Permissions::set_readonly(bool)` (clear/set the write bits 0o222) round-trip
    (coverage `filesystem_std_module_permissions_set_readonly`), pairing with
    `metadata_path(..).permissions()` (step 11) for read-modify-write chmod.
    fd-based variant DONE: `set_file_permissions(file, perms)` (Rust
    `File::set_permissions`) via `_fchmod` — reuses the `set_len` operand shape
    (`[result, fd, mode]`), no new operand/encoder work. Native `native_fchmod`
    canary RUNS (fchmod 0o444 on an open fd → fresh write-open → EACCES(13) →
    PASS); coverage `filesystem_std_module_set_file_permissions`.
10c. [x] **`hard_link`** (Rust `std::fs::hard_link`) via `_link` — DONE, complete
    NATIVE vertical. `HostOperation::Link` (op `link` → darwin `_link`); reuses
    the two-path `rename` operand shape (`find_nth_data_object` ×2), so no new
    operand/encoder work — just added `Link` to that match arm. Raw
    `FilesystemHost::hard_link(original, link)`; wrapper
    `Filesystem::hard_link(original, link) -> UnitResult`. DIFFERENTIAL: native
    `native_hard_link` canary RUNS (link a file, read the alias back → same 11
    bytes → PASS) AND coverage `filesystem_std_module_hard_link` (alias reads 12
    bytes AFTER the original is removed; relinking onto an existing name is
    `AlreadyExists`). ⚠ INTERPRETER APPROXIMATION: the hermetic FS has no inodes,
    so `hard_link` COPIES the bytes — a later write to one name is NOT reflected
    in the other (native hard links DO share). Faithful only for create/readback/
    removal, which is all the tests assert. A shared-inode virtual model
    (path→Rc<RefCell<Vec>>) is a future refinement.
10d. [x] **`symlink` + `read_link`** (Rust `os::unix::fs::symlink` +
    `fs::read_link`) — DONE, complete NATIVE vertical. `HostOperation::Symlink`
    (op `symlink` → `_symlink`) reuses the two-path `rename` operand shape;
    `HostOperation::ReadLink` (op `readlink` → `_readlink`) uses a new
    `[result, path ptr, buffer ptr, count]` arm (path_pointer + address + scalar,
    like `read` but path-keyed). Raw `FilesystemHost::symlink(target, link)` +
    `read_link(path, buffer, count) -> i64`; wrappers `Filesystem::symlink(..) ->
    UnitResult` and `read_link(..) -> IoResult` (fills a caller buffer, returns
    the target byte count — Rust returns a PathBuf; Omega stays allocation-free).
    Interpreter models a `virtual_symlinks` map (link → target). DIFFERENTIAL:
    native `native_symlink` canary RUNS (symlink → read_link → 12-byte target
    back → PASS) AND coverage `filesystem_std_module_symlink` (target reads back;
    read_link on a non-link is Error). ⚠ INTERPRETER LIMITATION: the hermetic FS
    stores/returns symlink targets but does NOT RESOLVE them on open/stat/exists
    (native symlinks resolve for real) — so an open-through-a-symlink differential
    test would diverge; the tests only do symlink+read_link. Faithful resolution
    (follow links on path ops) is a future refinement.
10e. [x] **`symlink_metadata`** (Rust `fs::symlink_metadata`) via `lstat` — DONE,
    complete NATIVE vertical. `HostOperation::LStat` (op `lstat` → darwin `_lstat`);
    added `LStat` to the EXISTING `Stat` operand arm (identical `[result, path ptr,
    buffer ptr]` shape — lstat just doesn't follow a final symlink), so ZERO new
    operand/encoder/width work. Raw `FilesystemHost::read_symlink_metadata(path,
    buffer) -> i32`; wrapper `Filesystem::symlink_metadata(path) -> MetadataResult`
    (same byte-decode as `metadata_path`, plus `is_symlink = (st_mode & S_IFMT) ==
    S_IFLNK`, i.e. `(mode & 61440) == 40960`). `Metadata` gained an `is_symlink:
    bool` field (module convention: a field like `is_dir`, not a method) and
    `is_file()` is now `!is_dir && !is_symlink` so a symlink's lstat metadata is
    correctly NOT a file. Interpreter `read_symlink_metadata` handler: a path in
    `virtual_symlinks` → `S_IFLNK|0o777` with size = target byte length (POSIX
    symlink size); otherwise identical to `stat`. DIFFERENTIAL: native
    `native_symlink_metadata` canary RUNS on real macOS (lstat the link → S_IFLNK
    is_symlink true; lstat the target file → not a symlink → PASS) AND coverage
    `filesystem_std_module_symlink_metadata` (link: is_symlink, !is_file, len 11 =
    "/target.txt"; file: is_file, len 5). `metadata_path` (stat) still FOLLOWS
    links; the two now form the stat/lstat pair. NOTE: `as i64` casts on a host-call
    result don't lower natively ("needs runtime value lowering") — assign the raw
    i32 into the i64 field directly (implicit widen), as the other canaries do.
10f. [x] **`canonicalize`** (Rust `fs::canonicalize`) via `realpath` — DONE,
    complete NATIVE vertical. `HostOperation::Realpath` (op `realpath` → darwin
    `_realpath`), added to the EXISTING `Stat`/`LStat` operand arm (identical
    `[result, path ptr, buffer ptr]` shape), so ZERO new operand/encoder/width
    work. KEY DESIGN CALL: `realpath` returns `char*` (the resolved-buffer pointer,
    or NULL), NOT a byte count — so the raw seam `canonicalize(path, buffer) -> i64`
    stores the returned POINTER as an i64 and treats it purely as a NON-NULL SUCCESS
    FLAG (no deref; the useful output is the caller's NUL-terminated buffer). First
    fs op to return a raw pointer-as-i64; the value-returning store handles it fine
    (verified). Wrapper `Filesystem::canonicalize(path, buffer) -> UnitResult`
    (`Ok` = buffer holds the NUL-terminated absolute path, reusable as a `Path`;
    `Error{kind}` otherwise). Rust returns a fresh `PathBuf`; Omega fills a caller
    buffer (>= PATH_MAX = 1024) to stay allocation-free; there is NO length returned
    (realpath gives none) — the NUL terminator delimits it, and the common use
    (feed the canonical path back into open/stat) needs no length. Interpreter
    `canonicalize` handler: follows one symlink level (like `read_link`), then if
    the resolved path exists writes it NUL-terminated + returns 1, else ENOENT + 0;
    the hermetic FS is already absolute and does NOT resolve `.`/`..` (documented
    approximation). DIFFERENTIAL SPLIT (like the stat mtime split): native
    `native_canonicalize` canary RUNS on real macOS and asserts the REAL resolution
    — `/tmp/omega_canon.txt` → `/private/tmp/...` (buffer[1]=='p' proves /tmp was
    followed, not left as-is) → PASS; coverage `filesystem_std_module_canonicalize`
    asserts the CONTRACT — canonicalize a `/link`→`/target.txt` symlink yields the
    target path (buffer "/t..."), a missing path is `Error(NotFound)`. `metadata_path`
    (stat) and `symlink_metadata` (lstat) still form the follow/no-follow pair;
    `canonicalize` is the path-resolution primitive.
10g. [x] **`try_clone`** (Rust `File::try_clone`) via `dup` — DONE, complete
    NATIVE vertical. `HostOperation::Dup` (op `dup` → darwin `_dup`), added to the
    EXISTING `Close` operand arm (identical one-fd shape; dup just returns the NEW
    fd instead of a status rc), so ZERO new operand/encoder/width work. Raw
    `FilesystemHost::duplicate(fd) -> i32` (human word; `_dup` only in the binding
    table); wrapper `Filesystem::try_clone(file: File) -> OpenResult` (returns a
    second independent `File`). Interpreter `duplicate` handler clones the
    `VirtualFd` (same path/writable/is_dir, cursor snapshotted from the source),
    EBADF for an unknown fd. ⚠ APPROXIMATION (documented): native `dup` SHARES the
    underlying open file offset; the hermetic model gives the clone its OWN cursor
    (snapshotted, independent thereafter) — faithful for the clone-then-use pattern
    since a freshly-opened source starts at offset 0, so both engines agree. A
    shared-offset virtual model (fds → Rc<Cell<cursor>>) is a future refinement (same
    class as the hard_link shared-inode note). DIFFERENTIAL: native `native_try_clone`
    canary RUNS (open a file, dup it, CLOSE the original, read 5 bytes "hello"
    through the clone → PASS) AND coverage `filesystem_std_module_try_clone` (same:
    clone survives closing the original, reads count 5, first byte 'h'). NOTE
    (language gotcha recorded): a case-field pattern binds by the FIELD name — there
    is NO rename form `Case { field: newname }` (parse error); bind `{ field }` and
    rename the surrounding param instead to avoid a clash.
10h. [x] **`File::metadata` via `fstat`** (Rust `File::metadata`) — DONE, complete
    NATIVE vertical; UPGRADES the fd-based `metadata(file)` from a seek-based
    approximation to a real `fstat`. `HostOperation::FStat` (op `fstat` → darwin
    `_fstat`) with a NEW operand arm `[result, fd scalar, buffer pointer]` — like
    `read` WITHOUT the count, keyed by an open descriptor instead of a path (the
    only new operand arm this fire; the value-returning encoder handled the 2-arg
    fd+address shape with no changes). Raw `FilesystemHost::read_file_metadata(fd,
    buffer) -> i32`. Rewrote `Filesystem::metadata(file)` to call it and byte-decode
    the SAME record as `metadata_path` (len@96, mode@4, mtime@48, atime@32,
    btime@80) — so an open `File` now reports its REAL `mode`/`readonly`/
    `permissions`/`modified`/`accessed`/`created` (was a hard-wired 0o644 + zero
    times) and never moves the cursor. `is_symlink` false (fstat follows to the
    real file), `is_dir` from `st_mode`. Interpreter `read_file_metadata` handler
    maps fd→path then fills the stat record like `read_metadata` (EBADF for an
    unknown fd). DIFFERENTIAL: native `native_fstat` canary RUNS (create+write 10
    bytes, fstat the OPEN fd, decode len 10 / is_dir false → PASS) AND coverage
    `filesystem_std_module_file_metadata` (chmod 0o444 → open → metadata(file):
    is_file, readonly, len 4, modeled mtime 1e9 — the OLD seek-based impl would
    FAIL the readonly/mtime checks, confirming the upgrade). The 4 existing
    `metadata(file).len` tests stay green (fstat returns the real len too). The
    stat family is now complete: stat (path/follow) / lstat (path/no-follow) /
    fstat (open fd).
10i. [x] **Positioned I/O — `read_at`/`write_at`** (Rust `os::unix::fs::FileExt::
    read_at`/`write_at`) via `pread`/`pwrite` — DONE, complete NATIVE vertical.
    `HostOperation::PRead`/`PWrite` (ops `pread`/`pwrite` → darwin `_pread`/`_pwrite`).
    New operand arms = the `read`/`write` arms plus a TRAILING offset scalar:
    PRead `[result, fd, buffer ptr, count, offset]`, PWrite `[result, fd, buffer
    ptr, length, offset]` (PWrite keeps `write`'s literal-vs-runtime-slice split).
    The value-returning encoder handled the 4-call-arg (x0..x3) shapes with no
    changes. Raw `FilesystemHost::read_at(fd, buffer, count, offset) -> i64` /
    `write_at(fd, bytes, offset) -> i64`; wrappers `Filesystem::read_at(file,
    buffer, count, offset) -> IoResult` / `write_at(file, bytes, offset) ->
    IoResult`. Interpreter `virtual_read_at`/`virtual_write_at` read/write at an
    absolute offset WITHOUT moving the cursor (write_at zero-fills a gap past EOF);
    negative offset or unknown/non-writable fd → failure. DIFFERENTIAL: native
    `native_positioned_io` canary RUNS (write "0123456789", reopen O_RDWR,
    write_at("XY",2) → "01XY456789", read_at(4,1) → "1XY4" → PASS) AND coverage
    `filesystem_std_module_positioned_io` (same, via `open_with` for an RDWR fd).
    NOTE: `create` opens WRITE-ONLY (`_creat`), so a read_at needs a subsequent
    `open`/`open_with` with the read bit (the canary reopens O_RDWR). pwrite's
    literal-payload path is exercised by the "XY" write.
10j. [x] **`File::set_times`** (Rust `File::set_times`) via `futimens` — DONE,
    complete NATIVE vertical. `HostOperation::SetFileTimes` (op `futimens` → darwin
    `_futimens`), added to the EXISTING `FStat` operand arm (SAME `[result, fd,
    buffer pointer]` shape — fstat's kernel WRITES the buffer, futimens READS two
    `struct timespec` from it), so ZERO new operand/encoder/width work. Raw
    `FilesystemHost::set_file_times(fd, times: &mut [u8]) -> i32`; the caller packs
    two timespec (atime @0, mtime @16; {tv_sec i64, tv_nsec i64} each, whole-second
    precision, nsec=0). Wrapper `Filesystem::set_times(file, accessed, modified) ->
    UnitResult` byte-decomposes both seconds into a `times_buf: [u8; 32]` field.
    **Language idiom (recorded):** a narrowing `i64 -> u8` write uses the branch-free
    `x as u8 in Wrapping` cast-exit (chapter 8) — the low 8 bits of a shifted second
    (`(v >> 8) as u8 in Wrapping`); a plain narrowing cast needs a proof or a domain.
    **Interpreter model:** new `virtual_times: BTreeMap<path, i64>` (mtime secs), set
    by `set_file_times` (reads mtime from buffer bytes [16..24] LE), read by BOTH
    `read_metadata` (stat) and `read_file_metadata` (fstat) so a set mtime shows
    through `metadata`/`metadata_path`. Round-trips MODIFIED time only (whole
    seconds); accessed time is set natively but the hermetic model reports the fixed
    modeled atime (documented approximation). **Interpreter fix (general):**
    `eval_fs_bytes` now derefs a `Value::Ref` (a `&mut buffer` passed by reference),
    so any buffer-arg-by-reference host call works, not just literals/bare arrays.
    DIFFERENTIAL: native `native_set_times` canary RUNS (futimens sets mtime
    1500000000, fstat @48 confirms → PASS) AND coverage `filesystem_std_module_set_times`
    (set_times → metadata(file).modified() == 1500000000).
10k. [x] **`MetadataExt::nlink()`** (Rust `os::unix::fs::MetadataExt::nlink`) — DONE,
    complete NATIVE vertical, DECODE-ONLY (no new syscall/op). `Metadata` gains an
    `nlink: u64` field decoded from `st_nlink` (u16 @6) in ALL THREE stat decoders
    (`metadata_path`/`symlink_metadata`/`metadata`); accessor `Metadata::nlink()`.
    Interpreter `write_fs_stat` writes `st_nlink = 1` (fixed) -- the hermetic FS does
    NOT model hard-link groups (its `hard_link` copies bytes), so every path reports
    1; the real 1→2 increment is a NATIVE-only assertion. DIFFERENTIAL SPLIT: native
    `native_metadata_nlink` canary RUNS (create → nlink 1; `hard_link` → re-stat the
    original → nlink 2 → PASS) AND coverage `filesystem_std_module_metadata_nlink`
    (a fresh file reports nlink 1). First `MetadataExt` field; `ino`/`uid`/`gid`
    followed in step 10l.
10l. [x] **`MetadataExt::ino()`/`uid()`/`gid()`** (Rust unix ext) — DONE, complete
    NATIVE vertical, DECODE-ONLY. `Metadata` gains `ino: u64`/`uid: u32`/`gid: u32`,
    decoded from `st_ino` (u64 @8), `st_uid` (u32 @16), `st_gid` (u32 @20) in all
    three stat decoders; accessors `ino()`/`uid()`/`gid()`. Interpreter reports FIXED
    modeled constants (`VIRTUAL_INO`=1000000, `VIRTUAL_UID`=501, `VIRTUAL_GID`=20)
    written by `write_fs_stat` -- it has no real inodes or process identity, so it
    can't model inode SHARING (its `hard_link` copies). DIFFERENTIAL SPLIT: native
    `native_metadata_ino` canary RUNS and asserts the REAL relationships (two sibling
    files share an owner uid/gid but have DISTINCT inodes; a `hard_link` shares the
    original's inode → PASS); coverage `filesystem_std_module_metadata_ext` asserts
    the exact modeled constants (ino 1000000, uid 501, gid 20). MetadataExt core
    (nlink/ino/uid/gid) is now complete; `dev`/`ctime` followed in step 10m.
10m. [x] **`MetadataExt::dev()` + `ctime()` (`changed()`)** (Rust unix ext) — DONE,
    complete NATIVE vertical, DECODE-ONLY. `Metadata` gains `dev: u64` (decoded from
    `st_dev` @0) and `changed_secs: i64` (`st_ctime`, `st_ctimespec.tv_sec` @64);
    accessors `dev()` and `changed()` (Rust `ctime()`). Completes the time family
    (accessed/modified/changed/created = atime/mtime/ctime/btime) and pairs `dev`
    with `ino` for file identity. Interpreter reports fixed constants
    (`VIRTUAL_DEV`=16777220, `VIRTUAL_CTIME_SECS`=1000000050). DIFFERENTIAL SPLIT:
    native `native_metadata_ctime_dev` canary RUNS (a real recent ctime > 1e9; two
    same-FS files share a nonzero device → PASS); coverage
    `filesystem_std_module_metadata_ctime_dev` (modeled changed()==1000000050,
    dev()==16777220). Remaining `MetadataExt` fields (rdev, blocks, blksize) are the
    same decode-only pattern if ever needed. `MetadataExt` is now effectively
    complete for the common surface.
11. [x] **Richer `Metadata`** via `stat` — DONE, complete NATIVE vertical (used
    `stat(path)`, not `fstat(fd)`, so it works on DIRECTORIES with no open/read
    perm). `HostOperation::Stat` (op `stat` → darwin `_stat`); operand arm
    `[result, path ptr, buffer ptr]` reusing `path_pointer_operand` +
    `address_argument_operand_at`. `Metadata` gains `is_dir` (Rust
    `Metadata::is_dir`/`is_file`). `Filesystem` carries a `stat_buf: [u8; 144]`
    scratch field; `metadata_path` now fills it via `read_metadata` and DECODES
    `st_size` (i64 @96) and `st_mode` (u16 @4) by little-endian BYTE-ASSEMBLY
    (`(buf[k] as i64) << 8*i | …`), with `is_dir = (st_mode & 0o170000) ==
    0o040000`. Interpreter `write_fs_stat` fills the virtual stat record (S_IFREG|
    0o644 for a file with its size, S_IFDIR|0o755 for a dir). DIFFERENTIAL:
    native `native_stat` canary RUNS (decodes len 10, is_dir false → PASS) AND
    coverage `filesystem_std_module_metadata_is_dir` (file → is_dir false len 3;
    dir → is_dir true). The fd-based `metadata(file)` is now `fstat`-based too (see
    step 10h — it was seek-based when this step was written). st_mode perm bits DONE:
    `Metadata` now carries `mode: u32`, with `Metadata::is_file()` (= !is_dir),
    `Metadata::readonly()` (owner-write bit 0o200 clear), and
    `Metadata::permissions() -> Permissions` (`st_mode & 0o777`) — all `&self`
    methods on the `[copy]` data type (verified those + `!bool` work). The
    interpreter's stat now folds `virtual_perms` into `st_mode`, so a prior
    `set_permissions` shows through `readonly()`. DIFFERENTIAL: native
    `native_metadata_readonly` canary RUNS (chmod 0o444 → stat → write bit clear
    via runtime |,<<,& → PASS) AND coverage `filesystem_std_module_metadata_permissions`
    (fresh file is_file & writable; after chmod 0o444: readonly & permissions().mode
    == 292). `st_mtime` DONE: `Metadata` carries `modified_secs: i64` (whole
    seconds since epoch, `st_mtimespec.tv_sec` @48) with `Metadata::modified()`
    (Rust `Metadata::modified()`, which returns SystemTime — Omega returns the
    seconds). The hermetic FS reports a FIXED modeled epoch (`VIRTUAL_MTIME_SECS`
    = 1_000_000_000, it has no clock); native `stat` returns the real time.
    DIFFERENTIAL split accordingly: coverage `filesystem_std_module_metadata_modified`
    asserts `modified() == 1_000_000_000`, native `native_metadata_modified` canary
    asserts `modified() > 1_000_000_000` (a real recent timestamp). ALL THREE
    TIMES DONE: added `Metadata::accessed()` (`st_atimespec.tv_sec` @32) and
    `Metadata::created()` (`st_birthtimespec.tv_sec` @80, darwin's birth time)
    alongside `modified()` — same i64 byte-assembly. The hermetic FS models
    DISTINCT epochs (accessed 1_000_000_100, modified 1_000_000_000, created
    999_999_900) so a decode-wrong-offset bug is caught: coverage
    `filesystem_std_module_metadata_times` asserts each exact value; native
    `native_metadata_times` canary asserts all three > 1_000_000_000 (real recent
    times). `Metadata` is now at full Rust parity:
    len/is_dir/is_file/readonly/permissions/modified/accessed/created.
    - **D-bitwise (backend feature, general).** The byte-assembly needs runtime
      `|` on aarch64, which the MVP encoder REJECTED (only logical `And`/`Or` and
      the shifts were wired; `BitwiseAnd`/`BitwiseOr`/`BitwiseXor` fell to the
      "cannot lower" arm). Added them to `append_runtime_binary_operation`
      (ORR/AND/EOR register form, single instr) AND to
      `runtime_binary_operation_width` (4 bytes, in lockstep) — 31 isa-aarch64
      tests still green. Runtime bitwise ops now lower natively for ANY program,
      not just fs.
12. [x] **`copy(from, to)`** (Rust `fs::copy`) — DONE (interpreter). Enabled by a
    small interpreter fix: `eval_fs_bytes` now accepts a `Value::Array` (a byte
    array or a subslice view) as a host-call byte arg — the write-side mirror of
    `write_fs_buffer`'s `Array` arm — so a buffer can be written, not just a
    string literal. `Filesystem::copy(from, to, buffer, cap) -> IoResult` uses the
    WRITE-THEN-TRUNCATE idiom (write the whole buffer, then `set_len(n)`) entirely
    in the ENTRY, because the interpreter does NOT thread a `&[u8]` across a state
    transition (a threaded slice param comes through empty/wrong — see D-thread
    below). Coverage `filesystem_std_module_copy`: copies 14 bytes through a
    64-byte buffer, verifies the count is 14 (truncated to n, not cap) and the
    content matches. Two documented differences from Rust (from staying in the
    entry): it writes `cap` bytes then truncates, and a source-open failure still
    creates an empty `to`. NATIVE copy is NOT possible yet: the raw `write` op
    marshals a runtime `&[u8]` SLICE (descriptor) but not a fixed `[u8; N]` array
    (read uses an address helper; write expects a descriptor) — a bounded backend
    follow-up (add a fixed-array-address + static-length arm to the write operand
    handler). The faithful copy (branch after read; write `buffer[0..n]`) lands
    once the interpreter threads slices through states.
    - **D-thread (interpreter limitation — DIAGNOSTIC MAP from a full probing
      pass).** Threading refs/slices as transition-target args into sibling state
      params is INCONSISTENT in the interpreter — a fix must make these agree:
        * scalars (`i64`/`usize`) and `[copy]` structs thread fine (any depth);
        * `&mut [u8]` single-hop: `buf.len` ✓, `buf[i]` ✓, and `write(fd, buf)`
          (whole, via `eval_name`'s one-level Ref deref) ✓ — wrote all 32 bytes;
        * `buf[0..LITERAL]` subslice single-hop ✓ (no bounds guard needed);
        * `buf[0..count]` subslice with a RUNTIME `count` needs a `count <=
          buf.len` guard, which is a SECOND transition → a 2-hop forward, and a
          2-hop threaded subslice comes through EMPTY (the real break);
        * threading a shared `&[u8] in Path` (a domain slice) and/or a scalar
          ALONGSIDE a slice into the same state can bind WRONG (observed a copy's
          `set_len(fd, n)` receive 0 for a threaded `n`, and a threaded `dst`
          path resolve empty) — arg/param positional binding for mixed
          slice+scalar+path arg lists is suspect.
      Net: faithful `copy` (branch after read → guard `count` → write
      `buf[0..count]`) needs 2 hops + mixed args, both broken; the shipped
      entry-only set_len-trick `copy` avoids all of it. Fix belongs in
      `eval_argument`/`bind_frame`/`eval_subslice` (Ref forwarding + subslice
      deref across ≥2 hops, and mixed-arg positional binding). Would unblock
      faithful `copy`, root-cause errno in `write_all`/`read_all`, and general
      slice-passing to helper states. Bounded interpreter work, but its own fire.
13. [~] **`read_dir`** — directory iteration. NATIVE op + INTERPRETER model DONE
    (differential-consistent). INTERPRETER ITERATION IDIOM now DONE too; native
    iteration + ergonomic wrapper remain (gated on the runtime-indexed-read blocker).
    - **This fire:** (a) promoted `read_dir` into the SHIPPED raw seam
      `omega/language/std/filesystem.omg::FilesystemHost` (was canary-local only);
      (b) added coverage `filesystem_read_dir_iteration` proving the ITERATION
      idiom on the interpreter — fill the buffer, then WALK the packed dirent
      records with a runtime-indexed cursor (`buffer[off+16]`/`[off+17]` → the LE
      u16 `d_reclen`), advancing `off` by `reclen` until the filled byte count,
      counting 4 entries (`.`,`..`,two files) order-independently. Notes for the
      idiom: computed indices must be materialized into a field first
      (`self.idx = self.off + 16; buffer[self.idx]`); cursor arithmetic uses
      `usize/i32 in Wrapping`; a dominating guard (`off < 480`) discharges the
      static index-bounds obligation (array `in Trapping` does NOT auto-discharge
      it — the checker still demands a static proof). Native iteration reuses this
      exact idiom once the runtime-indexed-read backend bug (below) is fixed.
    - **Platform note:** classic `getdirentries` is UNAVAILABLE on darwin arm64
      (64-bit inodes deliberately break it — it links to a `_..._is_not_available`
      stub). Uses `___getdirentries64(fd, buf, bufsize, &position)` instead (the
      private syscall behind `readdir`), which IS linkable and works. Avoids the
      `readdir`→`dirent*` pointer-struct deref entirely (kernel fills OUR buffer).
    - **Done:** `HostOperation::ReadDir` (op `getdirentries64` → `___getdirentries64`),
      operand arm `[result, fd, buf ptr, count, position ptr]` (a NEW 5-operand
      shape: two addresses — the buffer and the in/out i64 `position` cursor —
      plus two scalars; the value-returning encoder handled 4 args cleanly). Raw
      seam method `read_dir(fd, buffer, count, position: &mut i64) -> i64`. Native
      `native_read_dir` canary RUNS: `create_dir` + a file, `open` the dir (POSIX
      opens dirs read-only — worked natively), `read_dir` returns EXACTLY 104
      bytes = the 3 dirent records (`.` reclen 32 + `..` 32 + `hello_entry` 40),
      proving directory reading end-to-end on real syscalls.
    - **dirent layout** (this variant): `d_reclen` u16 @16, `d_namlen` u16 @18,
      `d_type` u8 @20, `d_name` @21 (d_namlen bytes); advance by `d_reclen`;
      `n`=0 at end.
    - **INTERPRETER model DONE (this fire).** `VirtualFd` gained `is_dir`;
      `virtual_open_flags` now mints a DIR fd on a read-open of a `virtual_dirs`
      path (which ALSO fixes the exists/try_exists divergence on dirs — a dir now
      opens read-only, matching native). A `read_dir` handler packs `.`/`..` +
      each immediate child (paths directly under `dir/` in `virtual_files`/
      `virtual_dirs`) as dirent records with the EXACT darwin layout
      (`d_reclen = round_up_8(25 + namlen)`), so byte counts match native; the
      in/out `position` (a `&mut i64`, read/written via `read_fs_position`/
      `write_fs_position`) makes a second call return 0 (end). DIFFERENTIAL:
      coverage `filesystem_value_returning_read_dir` (create_dir + a file → open →
      read_dir == 104, third record @64 is `hello_entry` namlen 11 name 'h',
      second call == 0) MATCHES the native canary (both 104). A non-dir fd →
      ENOTDIR, unknown fd → EBADF.
    - **NEXT:** the ergonomic wrapper + an ITERATION IDIOM. `read_dir` fills a
      caller buffer; then a cursor `next_entry(buffer, offset) -> (name_off,
      name_len, next_off)` walks the packed records. The Omega-side parse needs
      RUNTIME-INDEXED buffer reads (`buffer[self.i]` where `i` is a runtime
      `usize`). **BLOCKER (diagnosed in depth this fire):** runtime-indexed reads
      work in the INTERPRETER (probe: `buffer[3]==42` → v=42) but are BROKEN
      NATIVELY on aarch64 — the `CopyRuntimeMachineIndexedToRuntimeStorage`
      instruction (from the other omega-rs workstream, committed on origin/main,
      NOT concurrent) has ≥3 bugs. A minimal probe (`self.buffer[3]=42;
      self.i=3; self.v=self.buffer[self.i]; exit(self.v)`) exits 71 with v=1
      natively (want 42). Found and understood:
        1. **Width off-by-4 (fix known).** The aarch64 width fn
           `runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width`
           hardcodes a `32`-byte fixed part but the RuntimeFrame-region encoder
           emits 36 (9 four-byte insns: the adrp+add+load_w index setup adds 8).
           Symptom: "layout planned 56, encoder emitted 60" — the width mismatch
           SAFETY NET fires and refuses to emit. Fix: make `fixed` region-aware
           (`RuntimeFrame => 36, Machine => 28`) and thread `index_region` through.
        2. **Hardcoded `index_region` (fix known).** The aarch64 encoder
           `encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage`
           hardcodes `RuntimeStorageRegion::RuntimeFrame` when calling
           `append_runtime_machine_index_target_address`, ignoring the
           instruction's actual region. For a Machine-region base (a `self`
           field buffer) it must pass the real `index_region`. Fix: thread
           `index_region` from the SelectedInstruction through
           instruction-selection widths.rs + encoding/runtime_storage.rs, aarch64
           widths.rs + runtime_storage.rs, and machine-emission layout.rs (5 files).
        3. **Value bug — UNIDENTIFIED (blocks completion).** With BOTH fixes above
           applied (verified: compiles, isa-aarch64 31 tests pass), the probe still
           reads v=1 instead of 42, and this is INDEPENDENT of region (present with
           both hardcoded-RuntimeFrame and threaded-Machine). `v=1` == the guard
           `i < 16` true-result, so suspect register/storage aliasing between the
           guard's bool temp and the indexed-read's base/scale/store — the emitted
           `scale_x_register_by_constant(26,17,1)` / target-address / load sequence
           is functionally wrong somewhere. Needs disassembly of the emitted bytes.
      **Fixes 1+2 were IMPLEMENTED then REVERTED this fire** — they are correct and
      safe but INSUFFICIENT (fix 3 remains), and committing them would convert the
      loud width-mismatch compile error into SILENT wrong values for every program
      using this instruction (the ~154 canary_suite failures). Kept the loud-error
      safety net instead. When resuming: re-apply fixes 1+2 (recorded above),
      then disassemble the probe's emitted sequence to find fix 3, THEN commit all
      three together. Interim workaround for read_dir: expose entries by copying
      each name into a caller slot via existing (constant-index) machinery, OR
      have the interpreter/const-eval own iteration until native indexing lands.
      Also `read`/`write` on a dir fd should be EISDIR (not yet modeled; no test
      needs it).
14. [~] **Native wrapper lowering — PARTIALLY WORKS (investigated in depth).**
    The ergonomic `Filesystem` wrapper now COMPILES natively and the simplest
    ops RUN correctly: `create`/`open`/`close` with a literal path + a `File`/
    scalar arg produce a real file and exit cleanly (verified: create+close →
    exit 70). So the "wrapper is interpreter-only" note is outdated for the
    scalar/path-literal subset. BUT there are several correctness bugs for the
    richer ops, all in native operand/place resolution across the wrapper-machine
    call boundary — each is deep backend work, not a quick fix:
    - **Forwarded byte-slice LENGTH.** `write_all` forwards its `bytes: &[u8]`
      param to `self.host.write(fd, bytes)`; the POINTER resolves (the file is
      created at the right path) but the write emits an EMPTY file — the
      forwarded slice's LENGTH comes out 0. NARROWED (this fire): it reproduces at
      ONE machine hop with a LITERAL — `Main::put(bytes: &[u8]) { self.fs.write(fd,
      bytes) }` called as `self.put(fd, "hello")` writes 0 bytes natively (5 in
      the interpreter). So the descriptor's `len` field is not materialized into
      the callee's param slot when a slice LITERAL is passed as a machine-call
      argument (distinct from `descriptor_argument_blockers`, which only covers
      SUBSLICE args). The fix is in the machine-call argument materialization for
      slice descriptors (the caller must store {ptr, len} into the callee param
      slot, not just ptr) — deep, NOT in `slice_argument_operands` (which reads
      the descriptor place fine when it is correctly materialized, as the raw-seam
      literal writes prove). LOCATED (this fire, but deep/spread): the arg model
      is `omega-state-calls/src/arguments.rs::build_call_arguments`; the actual
      param-slot writes are the argument BINDINGS in
      `omega-runtime-branching/src/branching/expansions.rs` (`leaf_argument_bindings`
      / `straight_line_argument_bindings` / `branch_parameter_bindings*`) plus the
      state-storage materialization. A slice-typed binding must emit BOTH the ptr
      and len stores into the param slot; today the len store is missing for a
      literal. Real multi-fire backend work in the binding/materialization system.
    - **Wrapper self-field buffers.** `metadata_path` fills `self.stat_buf` (a
      `[u8;144]` FIELD of the `Filesystem` receiver) via `read_metadata`, then
      byte-decodes it; natively the decode is wrong (`is_file()` came back false
      for a regular file) — the receiver-field buffer address does not resolve
      like a top-level `Main` field does (the raw `native_stat` canary, which
      uses a `Main` field buffer, decodes fine).
    - **Forwarded struct-field reads.** `open_with(path, options)` reads bool
      fields of the forwarded `OpenOptions` struct param to compute the open
      flags; natively it computed the wrong flags (a write-open of a chmod'd-RO
      file SUCCEEDED instead of EACCES) — reading a field of a forwarded struct
      param resolves wrong.
    - **Multi-op sequences** over the same literal path also desynced (a
      create→exists→remove→exists chain saw the file still present after remove).
    Net: the fix is real "storage-place resolution across the machine-call
    boundary" work in instruction-selection (resolve forwarded slice length +
    receiver-field buffer address + forwarded-struct field reads consistently
    with the raw-seam path). Multi-fire. Until then, native programs should use
    the RAW `FilesystemHost` seam (fully native) and reserve the ergonomic
    wrapper for interpreter/const-eval. Start with the forwarded-slice-length bug
    (`slice_argument_operands`), the most localized.
    - **DEEPENED DIAGNOSIS (investigated read-only this fire; confirmed multi-fire,
      did NOT attempt a fix).** Reproduced cleanly: a `Main::put(fd, bytes: &[u8]) {
      self.fs.write(fd, bytes) }` called as `self.put(fd, "hello")` seeks-to-end == 0
      natively (the interpreter writes 5). The store site is now narrowed: the
      argument BINDINGS in `omega-runtime-branching/.../expansions.rs`
      (`leaf_argument_bindings` / `straight_line_argument_bindings`) are only
      `param ← expression` MAPPINGS — they carry no ptr/len store. The actual
      materialization of a slice-typed param binding into the callee's slot is
      SPREAD across instruction-selection: `selection/state_bodies.rs`,
      `selection/storage_places.rs`, and `selection/runtime_dispatch/writes/
      subslice_copy.rs` (plus the `runtime_storage.rs` descriptor-write encoders,
      which take a `descriptor_offset` and clearly CAN write {ptr,len} — the raw-seam
      literal write proves the encoder is fine). So the missing len-store is in the
      SELECTION layer that decides what to emit for a forwarded slice param, NOT the
      encoder. JUDGMENT (recorded for the user): this is genuinely multi-crate
      backend work whose only regression gate is the full `canary_suite` (already
      154 pre-existing failures, so a new regression is hard to spot) — a speculative
      one-fire fix is high-risk (it would touch the materialization for ALL state
      calls, not just fs). Better suited to a dedicated focused session than a 5-min
      loop fire. The raw `FilesystemHost` seam remains fully native; the ergonomic
      wrapper stays interpreter/const-eval until this is done as a deliberate effort.
    - **SURGICAL PINPOINT (deeper read-only trace, a later fire — the diagnosis is
      now precise enough to fix in one focused sitting).** THE file is
      `omega-instruction-selection/src/selection/runtime_dispatch/argument_materialization.rs`.
      Its main loop tries an ORDERED CHAIN of strategies to materialize each
      transition/state-call argument into the callee's param slot (enum-tag →
      `emit_runtime_detached_frame_slice_argument_materialization` (as_slice, writes
      BOTH ptr+len) → `emit_runtime_frame_slot_slice_descriptor_write_in_table`
      (literal-SUBSLICE `buf[a..b]`, runtime-subslice, `as_slice`) → call-result
      place-copy → fixed-array → pointee → indexed → same-size place-copy → Indexed
      value → local-initial-value → static integer/bool → float → struct-literal).
      A BARE STRING LITERAL (`"hello"`) is a `StringLiteral` node that matches NONE
      of these: it is not a Call (`as_slice`), not an `Indexed`+`Range` (literal
      subslice), not a same-size storage PLACE (a literal has no frame place; its
      bytes are a rodata DATA OBJECT), and not a static scalar. So it FALLS THROUGH
      the whole chain and the 16-byte descriptor slot keeps its zero bytes → ptr 0,
      len 0 (matching the repro: seek-to-end == 0). THE FIX is a new ADDITIVE
      strategy in that loop: when the argument is a slice-typed `StringLiteral` (or a
      literal-backed slice) and the slot is `slice_descriptor_size()`, resolve its
      DATA OBJECT (cf. `find_data_object` in the host-call literal path) and emit the
      descriptor-write PAIR — an address-write of the data object into the slot's ptr
      field + a `WriteRuntimeStorageInteger` of the byte length into
      `slot.byte_offset + descriptor.len_offset()` (exactly the pattern
      `emit_runtime_fixed_array_slice_argument_materialization` uses at lines
      ~956–975, but sourcing the address from RODATA instead of a frame place).
      LOW BLAST RADIUS: the arm only fires for a case that is currently 100% broken
      (writes 0 bytes), so it cannot regress any working call. OPEN QUESTION for the
      implementer: whether an existing `SelectedInstructionKind` writes a rodata
      DATA-OBJECT address into a runtime-frame slot (the existing
      `WriteRuntimeStorageAddressToRuntimeFrame` takes a frame-PLACE source, not a
      data object). If none exists, one must be added (enum + aarch64/x86 encoders +
      width + layout, in lockstep) — that is the only part that could push this past
      a single focused fire. This unblocks the ergonomic wrapper's `write_all`/`copy`
      and every forwarded-slice-literal call.
    - **FIX LANDED — TRANSITION path (this fire).** The additive strategy is now in
      `argument_materialization.rs::select_runtime_dispatch_argument_materialization`
      (the transition-EDGE arg materializer, called from `edges.rs`): for a slice-
      typed `StringLiteral` arg into a `slice_descriptor_size()` slot, resolve its
      rodata data object (`string_literal_data_handle`) and emit ONE
      `WriteRuntimeFrameString { byte_offset: slot.byte_offset, data, byte_length }`
      — the full `{ptr, len}` descriptor. No new instruction kind was needed:
      `WriteRuntimeFrameString` already existed (used by string local/field
      initializers). VERIFIED: `native_forwarded_slice_literal` canary RUNS
      (`transition … -> forward("hello")` then `write(fd, bytes)` → 5 bytes → PASS;
      was 0 before). Additive + safe: instr-sel 10 / reloc 5 / isa-aarch64 31 crate
      tests green, fs coverage 38, Console cli_mvp green, and native_crud/read_dir/
      positioned_io/try_clone (heavy transition users) still PASS.
    - **REMAINING — VALUE-CALL path.** A slice literal passed to a machine that
      RETURNS a value (`self.n = self.put(fd, "hello")`, and the ergonomic
      `fs.write_all("/f","hello")` which is a value call) materializes its args via a
      DIFFERENT path (NOT `edges.rs`; value/inline-branching arg setup). The same
      additive `WriteRuntimeFrameString` strategy must be added to that path too. The
      `fwd_repro` (value-call `self.put`) still writes 0 until then. Find the value-
      call/inline-branching argument materializer and apply the identical fix.
15. [ ] **x86_64 / linux / windows seams** — see the reference below. Tables only;
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
