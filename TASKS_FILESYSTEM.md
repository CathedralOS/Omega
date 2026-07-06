# Tasks — Filesystem (`std::fs`)

Self-contained backlog for bringing a real filesystem surface (`open`/`read`/
`write`/`close`/`seek`/`stat`) to Omega, following the path Console already
proves. Kept separate from `TASKS.md` (omega-rs backlog) and
`TASKS_BOOTSTRAP.md` (the lattice) so it doesn't collide with either workstream.

**Line:** omega-rs (`origin/main`). Every file below is on the omega-rs side and
disjoint from the bootstrap-lattice files (`compiler/{alpha,beta,delta,gamma}`).

## Status (2026-07-05)

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

- [ ] **0a. Interpreter routes through `HostOperation`.** Replace the raw
  `match call.target.as_str()` in `evaluator.rs::try_host_call` with a resolve
  to `(HostCapability, HostOperation)` (share `HostOperation::from_name`, or
  better, carry the resolved key on the checked-tree call node so the
  interpreter never re-parses a name). Dispatch on the enum key, not the leaf
  string.
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
- [ ] **1e. macOS bindings (imports, not svc).** In `darwin.rs`, add
  `darwin_import("File","open","_open",…)`, `_read`/`_write`/`_close`/`_lseek`/
  `_unlink`, and `insert_platform_lowering` entries mapping the `Filesystem::*`
  methods to them. Extend the `HostCapability`/`HostOperation` enums (`lib.rs`
  `from_name`/`name`). Depends on 1d.
- [ ] **1f. Interpreter oracle for fs** (see the hard problem below) — enough to
  make the canary deterministic. Add handlers alongside the `"write"` arm in
  `evaluator.rs` (via the Step-0 unified dispatch, so `File::write` ≠
  `Console::write`).
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
