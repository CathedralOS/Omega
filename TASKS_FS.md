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
    stat-based `exists` waits on `fstat`.
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
    (path→Rc<RefCell<Vec>>) is a future refinement. `symlink`/`read_link` (the
    other link ops) need a buffer out-param (readlink) + open-time resolution
    modeling — deferred.
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
    dir → is_dir true). The fd-based `metadata(file)` stays seek-based (an open
    `File` is always a regular file → is_dir false). st_mode perm bits DONE:
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
    asserts `modified() > 1_000_000_000` (a real recent timestamp). `Metadata` is
    now at good Rust parity: len/is_dir/is_file/readonly/permissions/modified.
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
13. [ ] **`read_dir`** (opendir/readdir) — directory iteration. Large (returns a
    growing sequence; needs an iterator/handle shape). Defer until an iteration
    idiom is settled.
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
      forwarded slice's LENGTH (`RuntimeStringLength` off the descriptor place)
      comes out 0. `slice_argument_operands` resolves a forwarded param's pointer
      but not its length.
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
