# Cathedral Alignment: Language Gaps By Urgency

Cathedral (`../Cathedral`) is an operating system that will be written in Omega.
Its design (see `Cathedral/wiki/design/part_0_foundations/01_omega_substrate.md`)
bets the entire OS architecture on specific Omega features. This page
cross-references those bets against the language's actual implementation status
and sorts the gaps by *when ignoring them starts costing rework*.

Statuses used below distinguish IMPLEMENTED (runtime canaries), PARTIAL
(working semantic slice with named gaps), SETTLED/UNIMPLEMENTED, and OPEN.

The current summary: Cathedral's language model is substantially specified,
but its critical path still crosses unfinished programmable layouts,
freestanding entry/hardware vocabulary, atomics/scheduling, and separately
compiled component replacement. `TASKS.md` owns implementation status; the
language guide and frozen briefs own semantics.

## Tier 1 — address while the language is still cheap to change

These are decisions whose *absence is silently being decided* by ongoing
implementation work. Each one gets more expensive to retrofit every month.

1. **ZII and establishment** (SETTLED; implementation tracked in TASKS).
   Zero-filled storage does not silently establish every constrained value.
   If a default domain excludes zero, the place is inaccessible until an
   explicit construction or qualification establishes it. `[zero_init]`
   separately promises that zero is a usable canonical value. Linear
   obligations begin at establishment, never at raw zero-fill. Cathedral may
   require `[zero_init]` on OS-facing storage without weakening constrained
   internal types.

2. **Programmable wire/layout semantics** (PARTIAL).
   Omega uses one `data` declaration form with optional stable field numbers
   and `retired N;`. Serialization and foreign layout are policies producing
   deterministic, validated plans; durability is a plan grade checked by
   storage APIs and publish-time predecessor comparison. The remaining work is
   the reflection/plan vocabulary, complete codecs and recasts, and removal of
   the legacy `wire data` compatibility surface.

3. **Persisted history and live replacement** (SEMANTICS SPLIT; PARTIAL).
   `Versioned<T>` and historical wire shapes belong to persisted data. Live
   component replacement instead anchors on normalized machine-contract
   identity and checked state-upgrade machines. Chapter 22 specifies typed
   upgrades, liveness pins, contract-pinned imports, and bounded coexistence as
   the leading deployment model. Component artifact/linking mechanics,
   outbound calls from old continuations, budgets, and eviction remain open.

4. **Separate compilation and a component artifact model** (ABSENT). Omega is
   a whole-program compiler emitting one image; the runtime model is a single
   global frame region with absolute offsets and one fused dispatch loop.
   Cathedral requires components that are compiled, signed, shipped, loaded,
   and **hot-swapped independently** (hermetic static linking per component,
   machines as swap points, content-addressed code dedup). Nobody needs to
   build the loader today — but every codegen decision that assumes
   whole-program-with-absolute-addresses deepens the eventual rework. Worth an
   explicit architecture note: which backend layers are allowed to assume
   whole-program, and which must stay relocatable/per-component.

   **Companion — the build & package model, SETTLED
   (`design_briefs/build_and_package_model.md`, 2026-07-02).** The per-package
   boundary manifest this item calls for is **`build.omg`** — an effect-free
   Omega function returning a typed `BuildDescription`, *interpreted* at build
   time (config-as-code with analyzable output; no TOML, no `build.rs` cliff).
   Pure plan / effectful executor: `build.omg` describes, the toolchain fetches
   and builds. No toolchain seed and no lockfile (interpreting a pure function
   is version-invariant, so `build.omg` can name its own toolchain without
   circularity; pins live in the manifest). Dependencies are local aliases bound
   to pinned sources — no semver solving. ch15 updated to end-state (package =
   reach boundary; imports resolve only against declared deps). **Remaining
   implementation ask — the import-side gate:** ch15 name resolution must
   consult the declared set so a fully-qualified path cannot bypass it
   (undeclared reach *unresolvable*, not lint-flagged), making the layer law
   self-enforcing. Interim enforcement is a graph check (`imports ⊆ declared
   deps`, build-failing — the omega-architecture-test pattern). Open: the
   `BuildDescription` schema and surface syntax.

5. **Concurrency: model under amendment.** Suspension remains an ordinary
   machine contract concern; there is no `async machine` species. Decisions
   20/21 settle non-suspending cleanup and explicit linear Join consumption.
   Decision 22 supplies the required `Suspend` / `Block` vocabulary and
   pinned-row laws. Suspension composes through ordinary calls without a
   marker; continuation storage and suspension-safe loans still require their
   amendment brief.

   Still compatible: a futex-shaped scheduler boundary, cancellation as an
   explicit outcome, scoped borrowing spawns, compiler-planned bounded task
   storage, and one-mailbox sums. Remaining: suspension elaboration,
   continuation layout, and cross-suspension loans.

6. **Atomics and a memory model** — direction scouted + chapter 18 now
   carries the Rust-like atomics section (distinct core types, five C11
   orderings); the scout recommendation (compiler intrinsics, C11 model
   wholesale, atomics-only sharing with Mutex as library) awaits sign-off
   as C4/C5 in TASKS.md's register. The wait primitive itself is decided
   (decision 16): `wait_until_nonzero(&AtomicU32)` / wake — atomics are
   the words everything parks on, so C4/C5 gate IPC, the scheduler, and
   `spawn` implementation.

7. **Freestanding target + hardware access vocabulary** (ABSENT/narrow).
   Boot needs: a target with *no* host bindings and a custom entry (the
   current target model assumes stdout/stdin/process capabilities), linker/
   section/physical-address control for the image writers, **volatile/MMIO
   access semantics** (absent from chapter 20), and inline asm grown beyond
   `asm { jmp state(...) }` (CR3/MSR/port-IO instruction contracts — the
   appendix already lists the open questions; they need answers). The direct
   image emission bet is aligned with this; the gap is the freestanding
   flavor of it. The current freestanding model is recorded in
   `design_briefs/freestanding_boot_and_hardware_facts.md`: a typed entry
   provider, linear firmware/memory-map handoff, explicit region authority,
   capability-gated MMIO, and restricted interrupt entries. Boot needs no new
   `unsafe` or fact category: machine-state claims use evidence tokens
   (`IrqGuard`, `FinalMemoryMap`), value invariants, owned per-CPU state, or an
   audited target/provider assumption. Foreign structs ride the layout-policy machinery
   (`programmable_layouts.md`); foreign *pointers* use the extern brief's
   “Foreign pointer cases”
   taxonomy (borrows for call-scoped args, gated tokens / owned mints for
   returned pointers, admitted `ProviderPlan` entries over the closed `Binding`
   sum — `Syscall(n)` / `DllImport(m, s)` / table dispatch — for call targets).
   Provider definition is independent of selection; Cathedral's component
   manager may own dynamic service slots while `build.omg` supplies static
   platform-profile defaults. Remaining asks:
   no-host target + entry spelling, lowering-contract vocabulary (volatile
   `exactly_once`, `clobbers tlb`), and the **entry-stub lowering** — one
   design for interrupt entry, the firmware seam's UEFI export table, and
   outbound callbacks (foreign-initiated activation; shape recorded in
   `extern_boundary_and_format_domains.md`, frame provenance open).

8. **Case members (sum/mixed data shapes)** — SUM SHAPES IMPLEMENTED
   (2026-06-10): `case` members with named payloads parse, validate,
   interpret, and LOWER NATIVELY (tag-prefix construction writes, payload
   member reads, tag dispatch with payload binding in transition arms; all
   oracle-verified). Case-subset domains and MIXED shapes (common fields + a
   case part) remain pending, as does payload-aware structural equality
   (interim: `==` against a payload-bearing case is a compile error). See
   chapter 1 + TASKS.md frozen decisions 7/8.

## The first-boot ladder — UEFI/QEMU (2026-07-02)

The near-term goal: an Omega-emitted UEFI application booting under
QEMU/OVMF, once the layouts arc (L0–L3, landing now) grows its deriver
pieces. Harness is trivial (`qemu-system-x86_64 -bios OVMF.fd -drive
format=raw,file=fat:rw:dir` — OVMF loads `\EFI\BOOT\BOOTX64.EFI` and calls
its PE entry as an ordinary MS-x64 function: ImageHandle in RCX, SystemTable
in RDX; no reset-vector anything). Cathedral-side design blocks nothing; the
gate is three small Omega items, all milestone 1:

**Milestone 1 — "Hello from Omega" (print via `ConOut->OutputString`, exit):**
1. *Layouts + mints over the UEFI structs* — IN FLIGHT (L0–L3 landed; the
   deriver's validate/materialize/projection remainder is the same arc).
2. *No-host target + EFI entry* — NOT STARTED. A target with an empty
   host-provider set (current targets assume stdout/stdin/process caps) whose
   `main` has the EFI signature. The bounded-trivial case of the entry-stub
   problem: one entry, two args, no re-entrancy.
3. *PE32+ `EFI_APPLICATION` emission* — MOSTLY EXISTS. `omega-image-pe` needs
   the subsystem value (10), an empty import table (EFI apps import nothing —
   services arrive via the SystemTable argument), and — **verify early** —
   base relocations / a `.reloc` section: OVMF loads at an arbitrary base; a
   fixed-base assumption is a silent blocker.
4. *Runtime-pointer call* — NOT STARTED. The existing MS-x64 encoder's
   `call rax` variant (target from a projected value instead of the import
   table). On the critical path even for hello-world
   (`SystemTable → ConOut → OutputString`).

**Milestone 2 — own the machine** (`GetMemoryMap` → `ExitBootServices` →
first `Region` mint): **zero new language asks** — buffer dance, MapKey
retry, runtime-stride walk, token mint are all expressible with milestone-1
machinery per the boot brief.

**Milestone 3 — alive after firmware dies** (timer tick, serial, idle):
inline asm beyond `asm { jmp state(...) }` (`cli`/`hlt`/port-IO — serial is
port IO, i.e. instruction contracts, not MMIO) + the real interrupt entry
stub; then atomics/scheduler (C4/C5) for a kernel proper.

**Calling plans** (`design_briefs/calling_plans.md`, direction settled
2026-07-02): conventions = stated layouts over the register file
(policy → validated CallPlan → call encoder + entry stub from one plan;
internal convention stays compiler-sovereign; Binding kind implies the edge's
plan). Explicitly NOT a milestone-1 blocker — the refactor target is when
entry stubs or the second stated convention land.

## IPC + scheduler alignment (2026-06-13)

Cathedral's `part_3/00_ipc_and_service_invocation` and
`part_2/01_scheduler_and_resources` lean directly on the concurrency model;
reconciliation against the amended chapter 18 + the atomics work:

REQUIRES REVALIDATION after the effects/suspension amendment:
- The scheduler document's single-level carry-set claim was tied to the
  superseded spawn-only model. Its one-mailbox `many_to_one` actor remains a
  strong bounded-storage pattern, and Omega still rejects an `async`/`Future`
  type split, but composable suspension may retain a bounded chain of planned
  frames.

CRITICAL-PATH SHARPENING:
- The IPC `many_to_one` mailbox REQUIRES atomic claim-a-slot (`fetch_add`
  index bump + `compare_exchange` claim + a per-slot publish release-store).
  The RMW ops lower to real LOCK instructions on x86 (`lock xadd` /
  `lock cmpxchg`, width-dispatched 1/2/4/8; pinned by the
  runtime_atomic_fetch_add / compare_exchange / load_store canaries), so the
  mailbox's claim protocol is encodable today. load/store are plain aligned
  movs — atomic on x86; the producer's release-store on the index is covered.

NEW GAPS these docs surface (design TBDs, no clear implementation action yet):
- **Wake-reason sum.** Park must return `Signaled | PeerDied | Revoked |
  Timeout`, not just ready-or-cancelled. ch18's cancellation-as-value is a
  SUBSET; it must generalize to a wake-reason sum, with `PeerDied`/`Revoked`
  sourced from the OS grant arena (the one liveness fact only the kernel
  has). Fold into ch18's cancellation section when the scheduler arc starts.
- **`SharedRegion<Untrusted>` — a THIRD memory category** ch19 does not name:
  adversarially-mutable (neither proved nor boundary-accepted), reads return
  raw/unproven values, snapshot-then-validate to close the TOCTOU hole
  (a shared-mutable re-read after a check is unsound). ch19 currently knows
  only proved and boundary-accepted memory.
- **`protocol <Name> version vN { call ...; stream ...; }`** — a typed RPC
  surface over `wire data` (the IPC doc's typed-layer example). No language
  construct yet; rides the wire-stage-2 + capabilities-as-values work.

## Tier 2 — note now, design later (TBD register)

Flagged by Cathedral as "what Omega must grow," or absent from both wikis.
None block current compiler development; all should stay visible.

- **TBD: serialized capability representation** preserving attenuation +
  revocability across IPC/reboot/network (Cathedral's #1 flagged gap).
  Depends on `wire data` + the capability runtime story.
- **TBD: quiescence proofs** under interrupts, timers, async work, hardware
  (the hot-swap precondition). Depends on the concurrency model.
- **TBD: borrows as swap back-pressure** — borrow checker refusing to let a
  borrow outlive a machine swap point; cross-IPC borrows.
- **TBD: multi-version concurrency mode** — old + new machine versions
  running simultaneously with versioned dispatch (when quiescence is
  impractical).
- **TBD: operation-capabilities for secrets** (`Capability<SignWithKey(K)>`)
  and **purpose-tagged authority** (`Capability<Read<X>, Purpose<Y>>`) —
  likely generics + domains, may need no new core feature; prove it.
- **TBD: interrupt-handler entry convention** — how hardware enters the state
  graph (a machine entry with a target-specific calling convention?).
- **TBD: allocator story** — `Vec` has no runtime. Decision 22 rejects ambient
  legacy `alloc` as the resource model: a kernel wants explicit allocator/arena
  capabilities and dependent bounds. Decide the resource algebra before
  implementing `Vec` lowering, not after.
- **TBD: repr control for hardware structures** — packed, explicit
  offsets/alignment, untagged unions (page-table entries, descriptor tables,
  device registers). Chapter 20 has `repr native` only.
- **TBD: function pointers / first-class machine references** — driver
  dispatch tables; partially covered by `dyn Trait` (single-impl works,
  multi-impl backend pending).
- **TBD: const evaluation** — const params are structural; compile-time
  function evaluation is unspecified. Kernels lean on this hard.
- **TBD: authority-flow completeness** — facts through returns/derives across
  nested calls (the package capability manifest is only as good as this
  inference).
- **DECIDED, engineering pending: effects vocabulary operationalization** —
  decision 22 replaces global lowercase names with boundary-trait service
  identities plus core operational possibilities (`Suspend`, `Block`). Define
  Cathedral's `DeviceIo`, `MemoryMap`, DMA, and interrupt-control boundaries
  with explicit capability parameters; do not grow the compatibility bitset.
- **Provider injection direction settled; implementation queued:** deterministic
  scheduler / virtual-service injection uses admitted provider plans plus an
  attenuated selection capability scoped to the harness/component subtree.
  No Cathedral-only injection mechanism is needed; exact slot-capability and
  runtime-admission APIs land with Omega's provider-plan slice.
- **TBD: tail calls into machines** (already in the language appendix).
- **TBD: partition-tolerant lease semantics, remote attestation facts,
  CRDT/merge obligations** — distributed tier, farthest out.
- **TBD: trusting-trust resistance + TCB minimization for the self-hosted
  compiler.** Under SAS the compiler IS the isolation boundary, so the
  toolchain's trust story is a first-class deliverable: a hand-audited
  bootstrap seed (no Thompson trusting-trust binary in the lineage), a small
  proof-checking kernel that re-checks artifacts, and a verified Omega→machine
  translation (CompCert/CakeML-style). Trust bottoms out at {seed, checker,
  specs, hardware}; Gödel caps it at small-but-nonzero, and a proof only shows
  code-meets-spec (validation of the spec stays human). **Sequencing: SKIP in
  the current Rust implementation** — you bootstrap *from* an existing
  toolchain, and the differential oracle is the interim mitigation — **and make
  it canon once Omega is self-hosted in Omega.** Novel as a default-canon
  stance; earned here because the compiler is Cathedral's security kernel.

- **TBD: information-flow / secrecy labels.** Cathedral wants a *propagating*
  secrecy label on values (a `Secret<T>` taint — NOT a content-domain and NOT a
  `[property]` bracket; secrecy is about provenance/policy and must FLOW through
  operations: `hash(secret)` is secret). It would *derive* a component's
  side-channel isolation level and its constant-time obligation automatically
  instead of hand-passing them. This is real information-flow typing (label
  creep, declassification, covert channels) — a genuine new feature, farthest-out
  tier.
- **TBD: constant-time verification.** Given the secrecy labels above, the
  checker CAN prove the constant-time *discipline* (no secret-dependent branch or
  memory index) and codegen CAN restrict secret-touching code to
  data-independent-timing instructions. What it CANNOT prove is constant
  wall-clock — that is a hardware fact (ARM DIT / Intel DOITM) and lands in the
  {hardware} TCB. So constant-time = provable discipline on one named hardware
  assumption; depends on the label feature above.

## What is already aligned (no action)

- Machines/states/transitions as inspectable graphs — implemented, and the
  state graph artifact (`07_graph`) is exactly what Cathedral wants to schedule.
- Ownership/borrowing/moves — implemented; the single-writer-by-construction
  IPC story builds on what exists.
- Domains, contracts, proof obligations — implemented at the depth Cathedral
  currently needs.
- Direct native image emission with no external linker — Cathedral's boot
  chain wants exactly this.
- The differential interpreter oracle: under a single-address-space OS,
  **compiler correctness is the isolation boundary** (kernel_architecture.md
  lists miscompilation as an open trust question). The oracle is the standing
  mitigation; growing it alongside the backend is strategic, not hygiene.

## Maintenance

When a Tier 1 item gets a real design decision, record it in TASKS.md's
"Resolved Design Decisions" and update this page. When Cathedral's substrate
doc changes its "What Omega Still Needs to Grow" list, re-sync the TBD
register.
