# Tasks — Filesystem (`std::fs`)

> **AUTONOMOUS LOOP (this file is the source of truth).** A `/loop` runs every
> 5 min re-reading this file to continue the fs work unattended. Cron job id
> **`371842c4`** — `CronDelete 371842c4` to stop (do this when fs is complete or
> blocked only on a user-only design decision). Keep this file current every
> fire: update **Current state**, **Next steps**, and **Design decisions** so the
> next fire (fresh context) can continue.

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
- Native raw ops now: create/open/read/write/close/remove/seek/create_dir/
  remove_dir — all run-verified on macOS.
- aarch64 value-returning host calls implemented (the foundational primitive).
- Runtime slice/path host-call args implemented.
- Ergonomic wrapper runs in the INTERPRETER; lowering it natively is a separate
  (deep) track (forwarded-param resolution) — see D5, pursued in parallel.

## Next steps (ordered; keep this list live)

1. [x] **Rename raw ops to human words** — DONE (create/open/read/write/close/
   remove; `find_lowering_prefer_exact` compiler feature; canaries updated + run).
2. **Raw-seam Rust-parity breadth (native, run-verified)** — the steady track
   (D5). Done: `seek`, `create_dir`, `remove_dir`. Next candidates:
   - `rename(from, to)` → `_rename` — TWO path pointers in one call; needs
     two-literal marshalling (find_data_object finds only one; resolve by arg
     index / two data objects). Small marshalling extension.
   - append mode: `open` already takes `flags`; a wrapper passes `O_APPEND`
     (0x8 on darwin) — no new raw op, just a canary + wrapper constant.
   - `metadata`/`stat`: `_fstat(fd, &stat_buf)` writes a `struct stat`; read
     `st_size` (darwin arm64 offset **96**, `off_t`/i64). Needs a stat-buffer
     out-param + a struct-field read. Heavier — do after rename.
   Check Rust `library/std/src/fs.rs` + `sys/pal/unix/fs.rs`.
3. **[deep, parallel] Forwarded-param → storage-place resolution** so wrapper
   machines that pass a `&[u8]` param to the boundary lower natively; then store
   enum result through a wrapper `&mut out`; const-folded-literal-arg fix. Unlocks
   the ergonomic `Filesystem` wrapper lowering NATIVELY.
4. Ergonomic `Filesystem` wrapper as the real `omega/language/std/filesystem.omg`
   (create/open/read/write/close/remove/seek + `File` + result enums). It ALREADY
   runs in the interpreter; wire native once (3) lands; migrate coverage onto it.
5. Error model: errno → bespoke Omega error `data` cases (negative raw returns).
6. x86_64/linux/windows seams (tables only; not tested here).

---

Self-contained backlog for bringing a real filesystem surface (`open`/`read`/
`write`/`close`/`seek`/`stat`) to Omega, following the path Console already
proves. Kept separate from `TASKS.md` (omega-rs backlog) and
`TASKS_BOOTSTRAP.md` (the lattice) so it doesn't collide with either workstream.

**Line:** omega-rs (`origin/main`). Every file below is on the omega-rs side and
disjoint from the bootstrap-lattice files (`compiler/{alpha,beta,delta,gamma}`).

## Status (2026-07-05)

### ✅✅ NATIVE fs CRUD RUNS END-TO-END

`canaries/pass/filesystem/native_crud/main.omg` builds to a mach-o that, when
run, performs a real round-trip on macOS syscalls and prints
`PASS: native fs CRUD round-trip (creat+write+read 17B+close+unlink)` — the file
is created (`0644`), written, read back (17 bytes), and deleted. The raw
boundary layer is VALUE-RETURNING (`creat`/`open_read`/`read_bytes`/`write_bytes`/
`close`/`unlink`, each returning its syscall result). What made it work:

- **aarch64 value-returning host calls** — built the missing primitive (store the
  return register into a caller place); disassembly-verified.
- **`RuntimeStorageAddress`** aarch64 call operand — a place's address for
  buffer pointers (`read`).
- **NUL-terminated static string literals** (terminator kept out of the byte
  span) — C-string paths for `_open`/`_creat`/`_unlink`.
- **`creat` not `open` for creation** — `open`'s mode is variadic (stack-passed
  on arm64, so dropped); `creat`'s mode is a named/register param.

Remaining ergonomics (not blocking): a thin Omega `std` layer wrapping the raw
value-returning ops into the `File`/outcome-enum API; registering the canary
once `canary_suite.rs` is free. The interpreter fs (below) remains the ergonomic
+ oracle surface.

### Toward the portable ergonomic wrapper (the Rust-`std::fs` north star)

Architecture = Rust's `std::fs` over `std::sys`: one portable Omega wrapper
(hides flags/mode/fd behind `File`/results) over the per-OS raw boundary
(`FilesystemHost`), each OS supplying its seam via the lowering table. The
wrapper machines forward `path`/`bytes` PARAMETERS to the raw boundary, which
surfaced real native-marshalling gaps — being closed one by one:

- [x] **runtime slice args** — `write_bytes(fd, bytes)` where `bytes` is a
  `&[u8]` param/field (not a literal): load ptr+len from the descriptor
  (`slice_argument_operands`).
- [x] **runtime path args** — `creat`/`open_read`/`unlink` with a `&[u8] in
  Path` param: the slice's data pointer (points at a NUL-terminated literal
  underneath) via `path_pointer_operand`.
- [ ] **wrapper-param → storage-place resolution** — a `&[u8]` parameter
  forwarded from a CALLER (Main → `fs.create(path)` → `host.creat(path,…)`)
  doesn't yet resolve to a runtime storage place (cross-machine call-argument
  binding). This is the current blocker for the fully-wrapped API lowering
  natively. Not needed by the interpreter (which runs the wrapper today).
- [ ] **store enum result through `&mut out`** in a wrapper (untested past the
  above).
- [ ] const-folded literal host-call arg → no operand (workaround: stage in a
  field); worth fixing for clean call sites.

Each is a normal-Omega marshalling feature, not fs-specific — closing them makes
ANY Omega that calls the boundary lower cleanly.

---


- **DONE — the API surface, fully type/flow/ownership-checked.**
  [omega/language/std/filesystem.omg](omega/language/std/filesystem.omg): the
  `Path` byte-domain (`when no_nul(self)`), the ZII `File` handle
  (`[copy, zero_init]`), the four bespoke ZII outcome enums (no `Result<T>`),
  and `boundary trait Filesystem` with CRUD `machine` signatures.
- **DONE — a CRUD round-trip canary.**
  [canaries/pass/filesystem/crud_roundtrip/main.omg](canaries/pass/filesystem/crud_roundtrip/main.omg):
  create→write→close→open_read→read→close→remove. `omega --check` drives it
  through the full frontend + checker cleanly — every language-level construct
  validates (Path literal `no_nul`, payload binding `{ file }`/`{ count }`,
  `&mut self.buffer` → `&mut [u8]`, enum transitions, ZII handle threaded by
  `[copy]` through states).
- **BLOCKING the green canary — backend lowering.** `omega --check` runs host
  lowering (it is a proof stage, not just emission), so the canary currently
  ends with `no native lowering for target …MachO` on each `self.fs.*` call.
  That is the *only* class of error — the surface is sound. A green canary needs
  Step 1d/1e below. **Not yet registered in `ACTIVE_PASS_CANARIES`** (so it does
  not break the suite; the pass sweep only runs registered names).
- **DONE — fs CRUD actually RUNS in the interpreter.** The native backend is not
  on the interpreter's path (it runs on checked trees), so — following the
  codebase's established "interpreter-only coverage ahead of native codegen"
  pattern (see the case-payload tests) — `std::fs` executes today:
  - Step 0 landed: `evaluator.rs` host dispatch now routes on the boundary
    TRAIT (`receiver_boundary_type_name`), so `Filesystem::write` ≠
    `Console::write`.
  - A deterministic in-memory filesystem (`virtual_files`/`virtual_fds`, mirrors
    `virtual_ticks`) backs create/open_read/read/write/close/remove, building
    the ZII outcome enums into the `&mut out` params.
  - **5 green fs coverage probes** in `coverage.rs` (Console tests still pass ⇒
    Step 0 behavior-preserving): full CRUD round-trip (`count == 21`), read
    fills the caller buffer (`buffer[0] == 'o'`), open-missing → `Failed`, EOF
    (second read → `Read(0)`, cursor advances), and create-truncates-existing.
    These double as the behavioral spec native fs must match in the oracle.
  - Not wired to the differential oracle yet (needs native fs first, or an
    interpreter-only differential lane); the two failing differential tests in
    the suite are PRE-EXISTING native-side mismatches on this branch, unrelated.

## How host I/O actually works today (the model fs must follow)

Console is the working reference. It does **not** use the `omega/host/contracts/*`
`capability` blocks (those + Chapter-18 authority are an aspirational,
not-yet-lowered layer — see console.omg's own comment). The live path is:

- **App surface:** `platform Console` in
  [omega/language/std/console.omg](omega/language/std/console.omg) —
  `write_line`/`write`/`read_line`/`exit_process`/`sleep`.
- **Native binding table:**
  [foundation/omega-calling-conventions/src/{darwin,linux,windows}.rs](compiler/omega-rs/foundation/omega-calling-conventions/src/darwin.rs).
  Two registries per target:
  - `bindings` — `(HostCapability, HostOperation) -> HostBindingMechanism`.
    Linux uses `Syscall{number,...}`; **macOS uses `Import` of libSystem
    symbols** (`_read`/`_write`/`_exit`), not raw `svc`; Windows uses `Import`
    of kernel32.
  - `platform_call_lowerings` — app method (e.g. `write_line`) → host op +
    `PlatformCallData` marshalling descriptor.
- **Native codegen:** [host_calls.rs](compiler/omega-rs/backend/omega-platform-interface/src/host_calls.rs)
  builds the `HostCallPlan`; aarch64 `encode_svc` /
  [primitives/system.rs](compiler/omega-rs/backend/instruction_set_architectures/omega-isa-aarch64/src/aarch64/primitives/system.rs)
  emit the actual call.
- **Interpreter (differential oracle):**
  [evaluator.rs `try_host_call`](compiler/omega-rs/orchestration/omega-interpreter/src/evaluator.rs)
  handles the same ops Rust-side into `self.stdout`/`self.stderr`.

### The identity model — and where it's clean vs. hacky

- **Native side is principled.** Identity is *enum*-keyed:
  `HostOperationKey { capability: HostCapability, operation: HostOperation }`
  (`lib.rs`). The string literals in the target files
  (`darwin_import("Stdout","write",…)`, `host_operation("Stdout","write")`)
  are a registration DSL resolved to enums at construction, not runtime
  dispatch.
- **Interpreter side is a hack** (this is the "weird string evaluation"):
  `try_host_call` does `match call.target.as_str() { "write" | "write_line" =>
  … }` — an independent, hard-coded string list divorced from the
  `HostOperation` enum, keyed on the **bare method name**, non-exhaustive.
- **Native lowerings also use a wildcard.** Console methods register under
  platform `"*"` (`insert_platform_lowering(plan, "*", "write", …)`), i.e.
  "any platform's `write`". So the leaf name `write` is currently assumed
  globally unique.

Both assumptions break the moment a second `write` exists — which is exactly
what `File::write(fd, bytes)` vs `Console::write(text)` introduces. Hence Step 0.

---

## Step 0 — Unify host-op dispatch (prerequisite cleanup)

Goal: one host-op vocabulary, keyed on `(capability/platform, operation)`, so a
second `write` can't collide or silently drift between the two engines. No new
feature — pure consolidation, guarded by the existing differential oracle
(interpreter vs native must still match on every current sample).

- [x] **0a. Interpreter dispatch is trait-keyed.** DONE (lighter than the
  original idea): `evaluator.rs::try_host_call` resolves the receiver's boundary
  trait (`receiver_boundary_type_name`) and routes `Filesystem` calls to
  `try_filesystem_call` before the Console-centric `match`, so `Filesystem::write`
  ≠ `Console::write`. Did NOT pull in the foundation `HostOperation` enum (the
  interpreter doesn't depend on `omega-calling-conventions`); keying on the trait
  name was enough. Sharing one enum with the native table is still worthwhile
  when native fs lands — fold into 1d/1e then.
- [ ] **0b. Disambiguate the platform wildcard.** Decide how a lowering keyed on
  a specific platform (`File`) beats/coexists with the `"*"` Console lowerings.
  Options: (i) exact-platform match wins over `"*"`; (ii) drop `"*"` and
  register Console explicitly. Pick one, document it next to
  `insert_platform_lowering`.
- [ ] **0c. Exhaustiveness guard.** Add a test (or a `debug_assert`) that every
  `HostOperation` variant has an interpreter handler and vice-versa, so the two
  lists can't drift. This is the check that would have caught the current
  duplication.
- [ ] **0d. Green gate:** `cargo test --workspace` + the differential oracle
  unchanged (Step 0 must be behavior-preserving).

**Exit criterion:** adding a host op requires touching exactly one vocabulary,
and `Console::write` vs a hypothetical `Other::write` resolve distinctly.

---

## Step 1 — Filesystem vertical slice (open → write → close, macos_arm64 first)

Smallest end-to-end slice that writes bytes to a real file and reads them back,
matched by the interpreter oracle.

- [x] **1a. Surface.** `boundary trait Filesystem` in
  `omega/language/std/filesystem.omg` mirroring `boundary trait Console`
  (`create`/`open_read`/`read`/`write`/`close`/`remove`). Chose `boundary trait`
  (the working Console shape) over the aspirational `platform`/`capability`
  form.
- [x] **1b. Return shape.** Bespoke ZII sum types, first case = `Failed` (the
  zero case). `OpenOutcome{Failed|Opened(File)}`, `ReadOutcome{Failed|
  Read(usize)}`, `WriteOutcome{Failed|Wrote(usize)}`, `RemoveOutcome{Failed|
  Removed}`. Errno-detail payload on `Failed` is a follow-up.
- [x] **1c. Path type.** `domain [u8]::Path when no_nul(self)` — `no_nul` is a
  compiler-recognized byte predicate, so it checks today, and NUL-free is the
  real POSIX path invariant. Deliberately not `Utf8`.
### ⛔ BLOCKER discovered 2026-07-05: aarch64 has no value-returning host calls

The native fs value-return path on **darwin/aarch64 (this Mac) is gated on a
foundational, NON-fs capability the backend lacks: value-returning host calls.**

- On **x86_64**, value-returning host ops (`TickCount`, `KeyState`, `Gui`) route
  through `encode_win64_import_call(operands, /*value_returning*/ true)` —
  operand[0] = result place, the return is stored into it.
- On **aarch64**, `encode_host_call_sequence` ignores the operation key and loads
  **every** operand into an arg register (`append_call_operands`) — there is **no
  result-store path**. Grep for one is empty.
- **Empirically confirmed:** a minimal `self.t = self.clock.tick_count()` fails
  to build for `Aarch64/MachO` with `no native lowering … Aarch64` AND
  `needs mutation lowering` (a second aarch64 gap on the assignment-result path).

Consequence: every USEFUL fs op needs its syscall return (fd / count / rc) —
which requires value-returning host calls — so ALL of them are blocked on
aarch64 until that primitive exists. `close` only "works" natively because it is
VOID (we discard its `-errno`; it can't detect a close error). `read`'s buffer
fill would work (the kernel writes through the pointer arg), but its byte-count
return would not.

**The unblock (a general ISA feature, not fs):** in the aarch64 backend, add a
value-returning host-call sequence — treat operand[0] as the result place, load
operands[1..] as args, `BL`, then `str x0/w0` into the result place — plus the
assignment-result ("mutation lowering") path. Bounded but foundational ISA work;
best done as its own focused, verifiable piece (and native exec is flaky on this
box, so it verifies via emitted bytes, not by running). Until then, native fs
CRUD on this Mac stays at `close`; the **interpreter fs is the working CRUD**.

### Executable recipe: aarch64 value-returning host calls (the unblock)

Fully traced 2026-07-05. Mirrors the working x86_64 path
(`encode_win64_import_call(operands, /*returns*/ true)`). ~5 coupled layers that
must agree byte-for-byte; verify each by `otool -tv` on the emitted mach-o
(existing `close` disassembly confirmed correct: `adrp/add/ldr x0; bl _close`).

1. **ISA encoder** (`omega-isa-aarch64/src/aarch64/mod.rs`): add
   `encode_host_call_sequence_value_returning_from_operands` — `append_call_operands`
   over `operands[1..]` (args), `encode_branch_link_placeholder()` (BL), then the
   result store: `encode_adrp_placeholder(16)` + `encode_add_page_offset_placeholder(16)`
   (result region base → x16) + `encode_store_w_to_x(0, 16, byte_offset, byte_count)`
   (or `encode_store_x_to_x` for 8-byte) storing x0/w0 → result place.
2. **Operand `byte_count`**: the aarch64 `RuntimeScalarInteger` call-operand drops
   `byte_count` (assumes 8-byte `ldr x`). Thread it through `aarch64_call_operand`
   so the result store picks `str w` (i32 fd/rc) vs `str x`.
3. **Width** (`aarch64/widths.rs`): value-returning width = args width + 4 (BL) +
   4 (adrp) + 4 (add) + 4 (str).
4. **Dispatcher** (`omega-instruction-selection/src/encoding/host.rs`): aarch64
   branch — match `operation_key` for value-returning ops (mirror x86_64's per-op
   list) → the new encoder. fs raw ops are ALWAYS value-returning.
5. **Relocations** — teach the aarch64 layout that the result is emitted LAST:
   - `offsets/data_addresses.rs::data_address_relocation_offset`: add an aarch64
     value-returning branch (like the x86_64 `host_call_data_relocation_site`) —
     args' adrp/add offsets shift by 0 (they come first); the result operand[0]'s
     adrp/add sits AFTER args-width + BL(4).
   - `offsets/external_calls.rs::external_call_relocation_offset`: BL offset =
     `text_offset + args-width` (args before the call), not counting the result.

Then: **operands.rs** value-returning arms `[result, ...args]` (KeyState shape);
**darwin.rs/linux.rs** bindings+lowerings; **surface+interpreter** reshape to
value-returning. Verify: `otool -tv` a `self.rc = self.fs.close(fd)` build shows
`ldr/... x0; bl _close; adrp/add x16; str w0,[x16,#off]` with the right addresses.

### Ratified 2026-07-05: value-return + Omega wrap (surface reshape)

The remaining ops turn a syscall return into an outcome. **Decision: the raw
boundary layer is VALUE-RETURNING** (each op returns the raw `i32`/`isize`
result — fd / byte count / `-errno`), and a thin **Omega** `std/filesystem.omg`
layer wraps those into the ZII `File`/outcome enums. Enum construction lives in
proven/checkable Omega, not the backend (aligns with "favor Omega, rip out
Rust"), and — critically — it reuses an **already-shipping generic code path**:

- `KeyState` (Input) and `tick_count` (Clock) are value-returning host ops
  TODAY. `collect_assignment_result_host_lowering` (host_calls/collection.rs)
  turns `self.rc = self.fs.close(fd)` into a host call whose **`argument[0]` is
  the result place**, real args following; emission stores `x0` into it. So the
  return-capture needs **zero new emission machinery** — proven by analogy to
  KeyState.

**Recipe for each value-returning fs op** (mirrors the `KeyState` arm in
operands.rs): an operands arm producing `[result_place, ...args]` where
`result_place = first_scalar_argument_operand` (arg[0]) and real args come from
`scalar_argument_operand_at(1..)` / an address operand for a buffer/path.

Reshape implications (the work): raw trait becomes all value-returning
(`close(fd:i32)->i32`, `open(path)->i32`, `read(fd,buf,count)->isize`,
`write(fd,bytes,count)->isize`, `unlink(path)->i32`); the shipped VOID `close`
+ its canary + the interpreter handlers all move to the value-returning shape
(interpreter `close` returns an i32; the discard rule means statement-position
calls need `_ =` or the wrapper always binds the result); add the Omega wrapper
machines that build `File`/outcomes; path ops still need NUL-terminated pointer
marshalling (the C-string wrinkle).

- [ ] **1d. New `PlatformCallData` marshalling** — **the keystone.** Today only
  `None`, `FirstTextArgument{append_newline}`, `MutableOutputBuffer{byte_capacity}`
  exist. fs needs new descriptors: a path pointer+len argument, an fd argument,
  a caller buffer pointer+len, integer flags/mode, and a byte-count/fd return.
  Adding a variant is not local — `PlatformCallData` is matched at **~8 backend
  sites** that each must learn it:
  `foundation/omega-calling-conventions` (the tables),
  `backend/omega-backend-report/src/host.rs`,
  `backend/omega-data-planning/src/{host_calls,planning}.rs`,
  `backend/omega-emission-planning/src/host_argument_blockers.rs`,
  `backend/omega-instruction-selection/src/selection/host_operations{,/runtime_text/*}.rs`,
  `backend/omega-runtime-text/src/host_uses.rs`. The aarch64 selection for a
  syscall/import with pointer+len args + a return is the real work (carries the
  proof obligations the `--check` error already counts).
- [~] **1e. macOS bindings (imports, not svc).** STARTED — `close` LANDED as the
  first native op. Added `HostCapability::Filesystem` + `HostOperation::Open/
  Close/Unlink` (`lib.rs`), `darwin_import("Filesystem","close","_close")` +
  `close` platform lowering (`PlatformCallData::None`), and a
  `(Filesystem, Close)` scalar-fd arm in `operands.rs`. **Key discovery: the
  machine-emission host path is fully generic over Import bindings**
  (`encode_host_call_sequence` = load operands → BL to the imported symbol), so
  a scalar/None op needs ZERO per-op codegen and ZERO of the 8 PlatformCallData
  sites. Verified: `omega --check` clean on a close-only program; full mach-o
  build (emission "host bindings: 5" incl. Filesystem/close, "host calls: 1");
  Console non-regressed; instruction-selection tests pass.
  ([canaries/pass/filesystem/native_close/main.omg](canaries/pass/filesystem/native_close/main.omg))
  - Run-verification note: native exit codes are unreliable on this macOS box
    (an empty no-fs program also exits 1) — a pre-existing issue, so `close` is
    verified through emission, not by running.
  - Remaining ops: `open`/`unlink` need a path POINTER arg (NUL-terminated —
    the C-string wrinkle); `read`/`write` need fd + buffer ptr+len; and all the
    non-`close` ops need the **enum out-param write** (store `Opened{fd}` vs
    `Failed` from the syscall return) — that's the real 1d marshalling +
    codegen, the genuinely hard part still ahead.
- [x] **1f. Interpreter fs execution.** DONE via a deterministic in-memory
  filesystem in `evaluator.rs` (`virtual_files`/`virtual_fds`/`virtual_next_fd`,
  `try_filesystem_call` + `virtual_open`/`virtual_write`/`virtual_read`). Chose
  Option A (hermetic in-memory) from the oracle section — reproducible, no real
  disk. Covered by `coverage.rs::filesystem_crud_round_trip_reads_back_written_bytes`.
  Still TODO: wire into the differential oracle (needs native fs, or an
  interpreter-only lane).
- [ ] **1g. Register + expand.** Add `filesystem/crud_roundtrip` to
  `ACTIVE_PASS_CANARIES` once 1d–1f make it check green; add `fail` canaries
  (path-domain / authority rejections) and one `samples/cli/...` demo (samples
  do a full native build via `refresh-samples`, so gate that on 1e landing).

---

## Step 2 — Linux + Windows parity

- [ ] **2a. `linux.rs`:** `Syscall{number}` entries for `openat`/`read`/`write`/
  `close`/`lseek`/`fstat` (arch-specific numbers already modeled via
  `linux_syscall_numbers`). Wire aarch64 + x86_64.
- [ ] **2b. `windows.rs`:** kernel32 imports `CreateFileW`/`ReadFile`/
  `WriteFile`/`CloseHandle`/`SetFilePointerEx`.
- [ ] **2c. Cross-target canary parity.**

---

## The genuinely hard part — the differential oracle

Console output is deterministic (bytes into a buffer), so interpreter and native
trivially agree. **A real filesystem is stateful and nondeterministic**, so the
oracle needs a deliberate model or canaries can't assert equality. Decide before
Step 1f:

- **Option A — sandboxed in-memory FS in the interpreter.** Deterministic,
  hermetic, no real disk. Best for canaries; native run must target the same
  sandbox semantics (temp dir, fixed starting state) to match.
- **Option B — real disk, both sides, over a fixed temp dir.** Simpler to
  implement, but ordering/errno/permission variance can desync the oracle;
  canaries must be written to only assert the deterministic subset.

Recommendation: **A** for canaries (hermetic + reproducible), with a small set of
**B**-style smoke samples run outside the differential gate.

---

## Design decisions to ratify (your calls)

1. **Rust minimization.** The per-target syscall/import *table* (numbers,
   symbols, `svc` marshalling) is the irreducible Rust seam — same as Console
   today. Everything above it (surface, path logic, error mapping) is Omega. The
   long-term roadmap (console.omg comment) wants even the table to become proven
   Omega over host capabilities; not in scope here.
2. **`&[u8] in Path` vs `&[u8] in Utf8`.** Path ≠ UTF-8 on POSIX (bytes) vs
   Windows (UTF-16). Leaning `Path` as its own byte-domain with explicit
   decode/encode at the OS edge, rather than assuming Utf8.
3. **Enum returns over `Result<T>`.** Confirmed direction — bespoke sum types
   per op. (Codebase already mixes `Result<(),IOError>` and
   `SyscallResult<usize,LinuxErrno>`, so a custom enum is consistent, not novel.)

---

## Perturbation / coordination

- **Bootstrap-lattice agent:** untouched. It's in the *primary* worktree on
  `compiler/delta/*`; the file-set intersection with everything here is empty.
- **omega-rs `TASKS.md`:** no in-flight task is modified — this is a new additive
  slice (new files + additive enum/table entries). The one dependency is
  conceptual, not a merge conflict: the aspirational `capability`/authority
  layer is *not* required (Console proves we don't need it to ship fs).
- This file is brand-new → zero collision with either existing TASKS doc.
