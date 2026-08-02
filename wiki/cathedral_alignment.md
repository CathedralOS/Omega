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
   All-zero bytes are a universally safe storage state, while whether they
   establish an accessible value is derived from the default domain, common
   fields, and first-case payload. If a gate excludes zero, explicit
   construction or qualification must establish it. Linear obligations begin
   at establishment, never at raw zero-fill. Cathedral APIs that depend on an
   inert or reset state require an authored domain fact rather than inferring
   semantics from representation shape.

2. **Programmable wire/layout semantics** (PARTIAL).
   Omega uses ordinary `data` with stable `#N` field/case identities and
   `retired #N;`. Serialization and foreign layout are policies producing
   deterministic, validated plans; channel and storage surfaces declare the
   compatibility facts they require. The remaining work is complete
   reflection, codecs, recasts, preserving decode, and edge-owned compatibility
   reports.

3. **Persisted history and live replacement** (SEMANTICS SPLIT; PARTIAL).
   Historical wire shapes are immutable ordinary data, sum envelopes, format
   metadata, and checked conversion machines. Live replacement anchors on normalized
   requirement/artifact identity and is Cathedral orchestration over
   requirement bindings, liveness pins, admitted runtime operations, and
   ordinary phase machines. Omega owns the generic artifact/contract substrate;
   Cathedral owns deployment selection, resource provisioning,
   drain/coexistence policy, migration scheduling, and reclamation. Artifact
   encoding and runtime representation remain open.

4. **Separate compilation and a component artifact model** (ABSENT). Omega is
   a whole-program compiler emitting one image; the runtime model is a single
   global frame region with absolute offsets and one fused dispatch loop.
   Cathedral requires selected provider realizations that are compiled, signed,
   shipped, loaded, and **hot-swapped independently**. A component is the
   realization plus its compiler-validated code/state/resource closure; it is
   not intrinsically a package. Nobody needs to
   build the loader today — but every codegen decision that assumes
   whole-program-with-absolute-addresses deepens the eventual rework. Worth an
   explicit architecture note: which backend layers are allowed to assume
   whole-program, and which must stay relocatable/per-component. An initial
   package-shaped-closure restriction is valid only as a monotone staging fence.

   **Companion — the build & package model, SETTLED
   (`design_briefs/build_and_package_model.md`, 2026-07-02).** The per-package
   boundary manifest this item calls for is **`build.omg`** — an ordinary Omega
   build entry augmenting typed `Build` data, interpreted with explicit scoped
   build-host providers (config-as-code with analyzable effects and output; no
   TOML and no unchecked `build.rs` cliff). Filesystem, network, process, and
   similar reach is admitted and receipted rather than ambient. Dependencies
   are local aliases bound to pinned sources — no semver solving — and the
   toolchain emits a unified lock/provenance artifact rather than asking the
   author to maintain a second configuration language. ch15 is updated to the
   end-state (package = reach boundary; imports resolve only against declared
   deps). **Remaining
   implementation ask — the import-side gate:** ch15 name resolution must
   consult the declared set so a fully-qualified path cannot bypass it
   (undeclared reach *unresolvable*, not lint-flagged), making the layer law
   self-enforcing. Interim enforcement is a graph check (`imports ⊆ declared
   deps`, build-failing — the omega-architecture-test pattern). Open: the
   `BuildDescription` schema and surface syntax.

5. **Concurrency: model under amendment.** Suspension remains an ordinary
   machine contract concern; there is no `async machine` species. Decisions
   20/21 settle non-suspending cleanup and explicit linear `Task<T>` lifecycle
   consumption.
  Decision 22's split amendment supplies independent `suspends` / `blocks`
  clauses and pinned-ceiling laws. Suspension composes through ordinary calls;
  `suspend` and `block` are exact searchable direct-call acknowledgements over
  the statically known call envelope. Suspension is restricted to direct-call
  positions because it creates a continuation boundary; blocking-only calls
  may nest. WCSU now derives one fixed nonmoving `StackPlan` per local
  activation; park/resume lowering and suspension-safe loans still require
  their amendment brief. Architectural preemption may occur anywhere and
  preserves opaque state; semantic cancellation, migration, and replacement
  occur only at explicit suspension points or under checked pinning contracts.
  A `block` without a finite wait ceiling makes structured response unbounded
  through that named call.

   Still compatible: a futex-shaped scheduler boundary, cancellation as an
   explicit outcome, ordinary machines started through an admitted
   `TaskRuntime`, linear `Task<T>` claims, compiler-planned local activation
   requirements, bounded Arena-backed provider packages, and one-mailbox
   sums. Bare/scoped `spawn` is retired; conservative suspension-safe loans
   replace its special borrowing rule. Remaining: fixed-stack park/resume
   lowering, child-lease accounting, and cross-suspension loans.

6. **Atomics and a memory model** — direction scouted + chapter 18 now
   carries the Rust-like atomics section (distinct core types, five C11
   orderings); the scout recommendation (compiler intrinsics, C11 model
   wholesale, atomics-only sharing with Mutex as library) awaits sign-off
   as C4/C5 in TASKS.md's register. The wait primitive itself is decided
   (decision 16): `wait_until_nonzero(&AtomicU32)` / wake — atomics are
   the words everything parks on, so C4/C5 gate IPC, the scheduler, and
   task-runtime implementation.

7. **Freestanding target + OS memory/hardware foundation** (BOOT CORE PARTIAL;
   reusable primitives designed, engineering incomplete). Cathedral M1/M2 and
   M3 serial prove typed UEFI entry, firmware-table calls, the runtime-stride
   memory-map walk, `ExitBootServices`, first admitted physical Extent, port I/O, and `hlt`.
   The remaining foundation is now factored generically in
   `design_briefs/os_memory_and_hardware_foundation.md`: inert `addr` values;
   concrete-range `Extent` authority distinct from allocator `Arena`;
   `LayoutPlan` geometry, consumer `AccessPlan`, admitted offset-keyed
   `ResourceProfile`, and `Placed<P, T>` views;
   sealed per-operation atomic requirements shared by core and placed accessors;
   scoped DMA publication/acquisition and MMIO completion requirements;
   parsed checked asm; independent `CallPlan + StatePlan` (normalized compiler
   model and initial x86-64/AArch64 evaluators implemented); symbolic
   materialization; external-root reporting; DMA external loans; and carry /
   runtime admission. The next vertical slice is the x86 IDT
   and timer; placed views, address translation, DMA, hostile IPC, and AP bringup form
   the wider gauntlet. Value-side carry is a compiler-built-in four-axis
   product. Ordinary data derives structurally; accepted resource claims begin
   strict and gain positive per-claim permissions through result contracts.
   Suspension is checked locally against the operational envelope; CPU/thread/address demands
   join the runtime's born-pessimistic behavior contract at admission.

8. **Case members (sum/mixed data shapes)** — SUM SHAPES IMPLEMENTED
   (2026-06-10): `case` members with named payloads parse, validate,
   interpret, and LOWER NATIVELY (tag-prefix construction writes, payload
   member reads, tag dispatch with payload binding in transition arms; all
   oracle-verified). Case-subset domains and MIXED shapes (common fields + a
   case part) remain pending, as does payload-aware structural equality
   (interim: `==` against a payload-bearing case is a compile error). See
   chapter 1 + TASKS.md frozen decisions 7/8.

## The boot ladder — UEFI/QEMU (status 2026-07-28)

The Omega-emitted UEFI application now boots under QEMU/OVMF, owns the final
memory map, exits firmware, constructs inert shared `Extent` geometry, receives
its first `Extent in Granted` under the selected
`ExtentRootProvider::grant` receipt, carries that same linear root through its
own 16550 serial path, and retains it while idling with `hlt`. Physical-space,
rights, and backing-containment facts remain later resource-frontier work.
Harness:
`qemu-system-x86_64 -bios OVMF.fd -drive
format=raw,file=fat:rw:dir` — OVMF loads `\EFI\BOOT\BOOTX64.EFI` and calls
its PE entry as an ordinary MS-x64 function: ImageHandle in RCX, SystemTable
in RDX; no reset-vector path is involved.

**Milestone 1 — Hello from Omega:** COMPLETE. UEFI structs, no-host entry,
PE32+ EFI application emission, and runtime table-function calls serve.

**Milestone 2 — own the machine** (`GetMemoryMap` → `ExitBootServices` →
first admitted `Extent in Granted`): COMPLETE with positive firmware-return
evidence, a 98-descriptor runtime-stride walk, exact provider-plan receipt
identity, and state-local qualification forwarding into owned idle.

**Milestone 3 — alive after firmware dies:** serial + idle COMPLETE; timer tick
REMAINS. Generic `Calling<C>` trait composition, source-policy evaluation,
canonical evaluated-plan identity, and retention of the complete boundary plan
through checked lowering are complete. Its remaining dependencies are
checked-asm catalog work, `CallPlan + StatePlan` entry-stub derivation,
state-ceiling-aware codegen/final footprint validation,
fragmented IDT materialization, and the external-root ledger. TASKS.md records
the agent-ready order.

**Boundary entry plans** (`design_briefs/calling_plans.md`): ordinary ABI
placement (`CallPlan`) and interrupted-machine-state preservation (`StatePlan`)
are independent facets of one evaluated requirement policy. The published plan
identity is firewalled from per-provider emitted footprint evidence, which is
validated against the final placed artifact.

## IPC + scheduler alignment (2026-06-13)

Cathedral's `part_3/00_ipc_and_service_invocation` and
`part_2/01_scheduler_and_resources` lean directly on the concurrency model;
reconciliation against the amended chapter 18 + the atomics work:

REQUIRES REVALIDATION after the reach/suspension amendment:
- The scheduler document's single-level carry-set claim was tied to the
  superseded spawn-only model. Its one-mailbox `many_to_one` actor remains a
  strong bounded-storage pattern, and Omega still rejects an `async`/`Future`
  type split, but composable suspension may retain a bounded chain of planned
  frames.

CRITICAL-PATH SHARPENING:
- The IPC `many_to_one` mailbox REQUIRES atomic claim-a-slot (`fetch_add`
  index bump + `compare_exchange` claim + a per-slot `Publish` store).
  The RMW ops lower to real LOCK instructions on x86 (`lock xadd` /
  `lock cmpxchg`, width-dispatched 1/2/4/8; pinned by the
  runtime_atomic_fetch_add / compare_exchange / load_store canaries), so the
  mailbox's claim protocol is encodable today. load/store are plain aligned
  movs — atomic on x86; the producer's `Publish` store on the index is covered.

CURRENT GAPS these docs surface:
- **Wake-reason sum.** Park must return `Signaled | PeerDied | Revoked |
  Timeout`, not just ready-or-cancelled. ch18's cancellation-as-value is a
  SUBSET; it must generalize to a wake-reason sum, with `PeerDied`/`Revoked`
  sourced from the OS grant arena (the one liveness fact only the kernel
  has). Fold into ch18's cancellation section when the scheduler arc starts.
- **Hostile shared-page discipline.** This is not a third memory species.
  Ordinary data/layouts ride an authorized extent, but protocol leases establish
  stability only among compliant peers. Against a peer retaining writable
  mapping, Cathedral must copy then validate or revoke/remap and complete
  cross-core invalidation before zero-copy validation.
- **Typed RPC package.** Protocol declarations are ordinary data schemas,
  layout/codec policies, boundary traits, and endpoint capabilities. Build the
  serious package before proposing any `protocol` grammar.

## Tier 2 — note now, design later (TBD register)

Flagged by Cathedral as "what Omega must grow," or absent from both wikis.
None block current compiler development; all should stay visible.

- **TBD: serialized capability representation** preserving attenuation +
  revocability across IPC/reboot/network (Cathedral's #1 flagged gap).
  Depends on ordinary numbered schemas/layout policies + the capability runtime
  story and will use the same codec requirement surface.
- **DECIDED semantics, representation pending: replacement accounting.** A
  replaceable provider realization owns a closed code/state/resource graph.
  Every old-era activation, borrow, continuation, registration, authority, and
  hardware claim receives a drain/coexist/migrate/cancel/redirect/acknowledged-
  transfer disposition before its lifetime cohort is reclaimed. Cathedral owns
  the era ledger, drain policy, peak provisioning, and device quiescence;
  artifact/binding/receipt encoding remains.
- **TBD: operation-capabilities for secrets** (`Capability<SignWithKey(K)>`)
  and **purpose-tagged authority** (`Capability<Read<X>, Purpose<Y>>`) —
  likely generics + domains, may need no new core feature; prove it.
- **DECIDED, engineering pending: interrupt entry** — an ordinary boundary
  machine satisfies a target requirement carrying evaluated `CallPlan +
  StatePlan`; build/provider selection retains sealed entry identity;
  materialization fills the IDT; installation records an external root.
- **TBD: allocator story** — `Vec` has no runtime. Decision 22 rejects ambient
  legacy `alloc` as the resource model: a kernel wants explicit allocator/arena
  capabilities and dependent bounds. Content-bearing claim conservation is
  settled independently; quantitative allocation/work accounting remains the
  resource-algebra prerequisite for general `Vec` lowering.
- **Engineering pending: hardware representation** — programmable
  `LayoutPlan` geometry, name-keyed fragments, consumer `AccessPlan`, provider
  `ResourceProfile`, qualified-Extent borrow admission, and `Placed<P, T>` field access.
  The source model is in chapter 20 and the OS foundation brief;
  implementation is tracked in `TASKS.md`.
- **DECIDED, engineering pending: registered callback entry** — a callback is
  an ordinary boundary requirement carrying `Calling<C>`; a named static
  boundary machine explicitly satisfies it, and the registration parameter
  gives the compiler the exact context for private thunk/relocation lowering.
  Durable protocols return a linear registration value and keep instance state
  in Omega behind an inert context token or generational handle. Platform
  adapters normalize native re-entry into locally checked handler surfaces;
  direct synchronous entry is published through `invokes` and must form an
  acyclic component-boundary graph.
  General runtime function values remain a separate facility.
- **TBD: const evaluation** — const params are structural; compile-time
  function evaluation is unspecified. Kernels lean on this hard.
- **TBD: authority-flow completeness** — facts through returns/derives across
  nested calls (the package capability manifest is only as good as this
  inference).
- **DECIDED, engineering pending: reach vocabulary operationalization** —
  decision 22 replaces global lowercase names with boundary-trait service
  identities, direct `invokes` ceilings, and independent `suspends` and
  `blocks` clauses. Define
  Cathedral's `DeviceIo`, `MemoryMap`, DMA, and interrupt-control boundaries
  with explicit capability parameters; do not grow the compatibility bitset.
- **TBD: deterministic scheduler / virtual-provider injection hooks** for
  simulation (Cathedral's testing story; the recursive-provider pattern).
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
