# Tasks

Last pruned: 2026-08-17.

This file is the current execution queue, not a changelog. Git retains completed
implementation history; architecture pages and design briefs describe the
current model and only the implementation state needed to explain remaining
work. A task belongs here only when it names:

- the remaining work;
- the owning design and code area;
- a real blocker, if one exists; and
- a concrete acceptance condition.

Before taking a task, fetch `main`, inspect the newest commits in that lane, and
avoid overlapping another active change. Commit and push coherent milestones.
Engineering difficulty is not a design blocker. Unresolved owner decisions live
in `OWNER_QUESTIONS.md`; deliberately deferred research stays at the end of this
file.

## Ownership firewall

Psi operates on Omega files and owns parsing plus all target-neutral semantics
through terminal Psi. Omega consumes terminal Psi and owns provider
installation, optimization, ABI/storage realization, native emission, and
general execution machinery. Target backends own unavoidable ISA, ABI,
object-format, and relocation encoding. Cathedral owns OS data structures,
policies, protocols, and lifecycle.

If Cathedral cannot express a subsystem, identify the missing general Omega
primitive or mark the slice blocked. Do not implement page tables, descriptor
tables, schedulers, process tables, timer queues, or drivers as compiler-owned
Rust models.

Compiler validation and code generation may consume general plans. They must
not acquire customer-shaped semantic types, lifecycle states, writers,
scanners, or receipts.

## Execution order

The numbered groups express dependency order, not an exclusive assignment.
Independent compiler lanes may proceed in parallel when their files and
semantic owners do not overlap.

### P1 — Authority, content, and admitted roots

Owners:

- `wiki/design_briefs/authority_values_and_boundary_evidence.md`
- `wiki/design_briefs/canonical_ir_fuel_and_resource_provisioning.md`
- `wiki/language_guide/chapter_8_domains.md`

Remaining:

- **ENTRY-CONTENT-ROOTS.** Complete the generated native entry bridge and
  explicit-entry corpus migration. The production-facing installation carrier
  now joins the exact selected provider plan, arrival requirement, calling-plan
  fingerprint, physical provider/invocation, and both roots before consuming
  either grant. Its physical path maps and zeroes the exact receiver reservation
  and returns one exclusive activation loan; the separate local seam rejects
  provider-issued roots. Connect an emitted target entry stub to that carrier,
  consume the activation loan while invoking the selected source continuation,
  and retain the resulting generated-bridge evidence. Pure language/checker
  fixtures stop at
  checked artifacts; deployable/provider/artifact/ABI/layout/native fixtures
  select an exact target-owned `ProgramEntry`; temporary ABI probes name their
  explicit fixture seam. Sample refresh and native execution must use authored
  roots and never invent one, while targetless checking selects none.

  The function spine now retains a sealed compiler-private identity from
  abstract operations through target/assigned operations, machine
  instructions, encoded bytes, and object-entry selection. `Source(StateKey)`
  and `ProgramStorageEntryWrapper(continuation StateKey)` are distinct: object
  planning selects one exact symbol/identity pair, and synthetic wrapper gates
  retain the source continuation's symbol and text interval separately rather
  than relabeling it. Duplicate, redirected, missing-continuation, and
  interval-drift claims reject. Object planning now publishes every encoded
  function through a compiler-private identity-to-text-symbol linkage table;
  duplicate identities, canonical link-name collisions, overlapping or out-of-
  bounds intervals, and tampered table bindings fail closed. A future wrapper
  call relocation can therefore target the exact retained source continuation
  without rediscovering it by name. For the currently admitted
  `ProgramStorageApplication`/`ImageAndInitialStorage` schema, the bridge also
  retains an address-free wrapper transfer plan that maps both physical root
  ordinals to their exact source-visible parameter, frame byte range, and
  disjoint capture-instruction rows, plus free-versus-borrowed activation-loan
  receiver behavior. It deliberately does not call the physical arrival plan a
  source-call ABI. The retained bridge plan also owns a sealed platform
  executor gate: only the exact selected physical-provider
  installation and mapped, zeroed receiver activation can construct its
  borrowed continuation handoff, and the executor runs before that activation
  is finished. This gate intentionally does not claim that native bytes
  executed. The main backend now also owns a generic compiler-private direct-
  call operation from abstract operations through target/assigned operations,
  machine instructions, real x86-64 `call rel32` / AArch64 `bl imm26`
  placeholders, exact-identity object relocations, and final-image byte,
  relocation, and footprint replay. Missing, duplicate, redirected, wrong-
  width, and opcode-tampered call claims fail closed. No production builder
  emits that operation yet, and it deliberately carries no invented argument
  or receiver placement. Production builds still emit and select
  `Source(entry StateKey)`. The native bridge now retains an address-free
  outbound continuation ABI, distinct from the physical arrival
  `BoundaryEntryPlan`. The bridge separately retains the exact checked
  source declaration signature captured before typed ownership moves into the
  backend: target slot, machine/state symbols and names, canonical normalized
  callable identity, Unit result, free-versus-mutable receiver identity, and
  ordered receiver-excluded visible parameter type/mode/shape rows. Those facts
  are rechecked against the exact lowered continuation, selected slot, arrival
  parameter identities, and checked receiver layout. For the currently
  admitted UEFI x86-64 schema only, the compiler-private Microsoft x64 policy now derives
  one complete `CallPlan` over the optional receiver followed by Image and
  InitialStorage, with Unit result, and validates every placement against the
  sealed declaration shape. A future SysV/AAPCS schema remains fenced until the
  structural classification graph is retained. The production executor gate
  binds the attached receiver placement to the exact mapped address and live
  activation loan; identity, shape, alignment, and loan-length drift reject.
  The free form has a complete layout but no production executor traversal
  because the current gate requires receiver activation. The ABI plan carries
  no runtime root value, `Extent`, root authority, wrapper body, emitted call,
  or callee inbound realization. After activation ends, installed roots can now
  move into a sealed authority-disposition carrier that revalidates the exact
  initial-storage geometry, lineage, rights, provenance, mapping era, origin,
  receiver selection, and complete partition coverage. A receiver-free
  disposition may release Image and whole InitialStorage as two owned root
  authorities. An attached disposition keeps the receiver's
  `OwnedExtentPartition` intact, exposes its potentially noncontiguous before
  and after residuals only by borrow, and fails closed while returning the
  intact carrier if asked for two whole roots. This is not an outbound source
  argument: it cannot move residual authority during the live receiver loan or
  make two separated remainders satisfy one `Extent in Granted` formal. The
  receiver-free whole-root form can now bind to the exact emitted bridge's
  retained free continuation ABI. That sealed carrier owns both `Extent`
  authorities and retains ordered Image/InitialStorage declaration indices,
  call indices, nominal identities, shapes, and address-free placements;
  bridge-binding, target-slot, source-continuation, callable, role/order,
  type/shape/placement, receiver, and Unit-result drift reject while returning
  the intact authority. It does not materialize operand bytes, populate
  registers or stack, emit the call edge, or claim native execution. Attached
  and zero-sized-receiver forms remain deliberately excluded. The
  receiver-free path now has one sealed transition from the recorded
  production installation through validated root disposition and whole-root
  authority into that argument carrier. A borrowed preflight rejects binding,
  source/ABI, receiver, role/order, type/shape, or placement drift without
  consuming the recording; fail-closed errors after ownership starts moving
  retain the highest successfully constructed authority carrier for recovery.
  This linkage still creates no runtime operand bytes, wrapper body, call edge,
  or native-execution evidence. The selected source signature now also retains
  the exact checked `Extent` record graph—`base: addr` at byte 0 and
  `length: u64` at byte 8—and replays its data/field symbols, names, primitive
  types, offsets, aggregate size/alignment, and absence of alternate storage
  encodings against the backend layout. The receiver-free argument carrier can
  move into one sealed non-clone logical-value carrier that keeps Image and
  InitialStorage authority intact while binding their exact base/length
  observations to those declaration and call rows. Structural, role/index,
  target-layout, or wrapping-geometry drift returns the intact prior carrier.
  These are logical values only: no bytes, registers, stack locations, wrapper
  body, call, or execution evidence is produced.
  Those receiver-free logical values can now move into a sealed indirect-
  operand image carrier for the exact admitted UEFI/Microsoft x64 ABI. It
  retains little-endian `{base,length}` bytes beside each immutable
  `ValuePlacement`, requires Image through RCX with caller copy `32..48` and
  InitialStorage through RDX with caller copy `48..64`, and rejects role,
  index, field-layout, shape, pointer, range, size, alignment, overlap, or
  target drift while returning the intact authority-bearing logical carrier.
  The byte images are geometry, not authority. This slice deliberately does
  not allocate or write the caller-copy stack area, populate RCX/RDX, emit a
  wrapper body or call edge, realize the callee inbound ABI, claim native
  execution, or admit attached/zero-sized-receiver entries.
  The operand carrier can now move into a sealed planning-only wrapper caller-
  frame plan. For that same exact ABI it retains the balanced 72-byte outgoing
  reservation/release, the 32-byte shadow area, four ordered eight-byte recipe
  writes at `rsp+32/+40/+48/+56`, and subsequent address-binding recipe rows
  for `RCX=&rsp+32` and `RDX=&rsp+48`. Reservation, release, alignment,
  role/index, operand bytes, field offsets, copy ranges, pointer registers, and
  step ordering are revalidated; rejection returns the intact operand and
  authority chain. These are immutable planning rows only: no machine
  operation allocates or writes stack storage, changes RCX/RDX, inserts a
  wrapper, emits a call or relocation, selects a new object entry, or proves
  native execution.
  The main backend now also owns a compiler-private RSP-relative outgoing-stack
  address-load operation across abstract, target, assigned, machine-
  instruction, encoded-byte, and final-image replay. For x86-64 it retains
  exact `lea register,[rsp+disp32]` bytes, rejects non-positional registers,
  offsets outside nonnegative `disp32`, and non-x86 targets, carries no
  relocation, and derives the exact selected-register plus stack-pointer
  footprint. Synthetic gates pin `RCX=&rsp+32` and `RDX=&rsp+48` and reject
  opcode, ModRM, displacement, metadata, and footprint drift. No production
  builder emits this operation, the authority-bearing caller-frame plan
  remains unconsumed, and no stack reservation/write, wrapper insertion, call
  edge, object-entry switch, or native-execution evidence is claimed.
  The main backend now also owns paired compiler-private outgoing-stack frame
  reserve/release operations through abstract, target, assigned, machine-
  instruction, encoded-byte, and final-image replay. For x86-64, frame size is
  validated to cover Microsoft shadow space, fit positive `disp32`, and
  preserve pre-call alignment; reserve/release bytes and
  X86Rsp/Flags/StackPointer footprints replay with no relocation. Independent
  lowering and final-image scans reject orphan, nested, mismatched, unreleased,
  and out-of-range address-use rows. Synthetic gates pin the balanced 72-byte
  frame around RCX/RDX caller-copy address recipes. No production builder emits
  these operations; no stack stores, wrapper insertion, call edge, object-entry
  switch, or native-execution evidence is claimed.
  The receiver-free wrapper caller-frame plan can now move into a sealed,
  non-clone reserved-outgoing-frame planning authority. It retains the intact
  authority-bearing operand chain and authorizes only four ordered eight-byte
  writes: Image base/length at `rsp+32/+40` and InitialStorage base/length at
  `rsp+48/+56`; shadow `[0,32)` and alignment padding `[64,72)` remain
  unwritable, and rejection returns the intact prior caller-frame plan. The
  main backend also owns `WriteOutgoingStackU64` through abstract, target,
  assigned, machine-instruction, encoded-byte, and final-image replay. X86-64
  emits canonical full-width `mov rax,imm64; mov [rsp+disp32],rax`, reusing the
  host-call qword-store mechanic, with exact RAX/StackPointer footprint,
  untouched flags, and no relocation. Independent assigned and final scans
  require the exact four writes under a live 72-byte reservation before RCX/RDX
  caller-copy address bindings and reject shadow, padding, range, order,
  metadata, byte, footprint, incomplete-sequence, or AArch64 drift. No
  production builder consumes this authority; no physical stack mutation,
  wrapper insertion, call edge, object-entry switch, or native execution is
  claimed.
  The main backend now also retains a compiler-private launch-value copy
  operation across abstract, target, assigned, machine, encoded-byte, and
  final-image replay. For the receiver-free UEFI/Microsoft x64 shape, exact
  indirect `{base,length}` fields arriving through RCX/RDX can be copied into
  the live 72-byte outgoing frame at `rsp+32/+40/+48/+56` before the retained
  address loads. Canonical x86 bytes, RAX/StackPointer footprint, zero
  relocation, exact tuple ordering, and immediate-versus-dynamic write-mode
  separation fail closed. No production builder emits this sequence yet; no
  generated wrapper body, source-continuation call, object-entry selection, or
  native execution is claimed.
  The receiver-free UEFI/Microsoft bridge now also retains sealed source-
  continuation inbound-realization evidence. It joins the independently
  derived free Unit `CallPlan` to the exact encoded `Source(StateKey)` function,
  symbol/text interval, and two immediately-post-`FunctionEnter`
  Image/InitialStorage captures: 16-byte indirect values through RCX/RDX into
  their exact retained frame destinations. Role, declaration/call index,
  normalized type, physical-versus-internal placement, capture order/count,
  pointer, destination, instruction, byte-range, identity, and interval drift
  fail closed; final-image validation independently replays the capture bytes
  and static-storage relocations. Attached entries retain no receiver-free
  realization. This emits no wrapper body or call, consumes no installation-
  derived values, does not switch the object entry, and does not claim native
  execution.
  The emitted receiver-free UEFI/Microsoft bridge now also retains a sealed
  post-encoding phase-alignment template for the generated wrapper body.
  It binds the canonical generated-wrapper identity and symbol to the exact
  retained Source identity, symbol, and text interval, then pins eleven ordered
  compiler-private steps: function entry, a balanced 72-byte outgoing
  reservation, four launch-time indirect Image/InitialStorage field copies
  from RCX/RDX into `rsp+32/+40/+48/+56`, RCX/RDX caller-copy address loads,
  one exact Source-identity call, balanced release, and Unit return. Receiver,
  role/index, shape/placement, identity, interval, call-target, frame, and
  sequence drift fail closed. Installation-owned operand bytes and authority
  are deliberately not routed backward into compilation. A transactional
  second backend pass now consumes this template: it privately relabels the
  retained `Source(StateKey)`, appends the exact generated wrapper operations,
  and rebuilds target, assigned, machine, encoded, object, and relocation plans
  as candidates before publishing any mutation. Independent replay pins the
  wrapper bytes and validation rows, retained Source interval, object entry,
  and exact single `call rel32` relocation to Source. The final bridge names
  `ProgramStorageEntryWrapper(StateKey)` as entry while retaining Source as its
  continuation, and the rebuilt plan proceeds through checked final-image
  validation. Candidate failure leaves the original backend unchanged.
  Written receiver-free builds now bind one sealed emitted-wrapper evidence
  carrier after checked relocation but before any executable, bundle, or final-
  image artifact is published. It joins exact wrapper and Source object
  identities to their placed executable regions, retains offsets, addresses,
  sizes, byte fingerprints, compiler text/function validation, and executable-
  inventory identity, and independently replays the single final `call rel32`
  bytes against the Source interval. The compile report and optional manifest
  retain that carrier; non-writing builds retain none. This proves final image
  content only, not firmware invocation, installed roots, or native execution.
  That carrier now also retains independently replayed physical-arrival
  evidence for the exact UEFI x64 receiver-free path: the checked
  Image/InitialStorage `BoundaryEntryPlan` placements must be the same indirect
  RCX/RDX placements consumed by the generated wrapper, and the four ordered
  launch-value copy rows must occupy exact in-wrapper byte ranges, match their
  canonical 15-byte encodings in both encoded and final text, and own no
  relocation. Placement, role/index, wrapper identity/interval, row inventory,
  byte, or relocation drift fails closed. This remains final-image evidence;
  it does not connect installed authority to firmware launch or prove that the
  platform invoked the wrapper.
  The receiver-free whole-root argument carrier can now move into a sealed,
  non-cloneable emitted-wrapper binding. This transition keeps both installed
  `Extent` authorities intact while requiring final wrapper evidence from the
  same bridge, then replays wrapper/Source identities and intervals, executable
  fingerprints, the physical calling-plan fingerprint, Image/InitialStorage
  roles and indices, exact indirect placements, and all four canonical
  launch-value copy rows. An unwritten bridge or any identity, placement,
  interval, byte, or row drift rejects while returning the intact authority
  carrier. This proves that those installed ordinary values match one exact
  final wrapper certificate; it does not prove firmware supplied them to that
  wrapper, invocation, or native execution.
  Production builds still lack a source-compatible attached-root
  value/authority carrier (or separate hidden supply), final firmware
  composition, and native-execution evidence; those remain before this slice
  is complete. The next receiver-free production boundary is now exact: Q2
  must define the target-owned adapter that turns UEFI `ImageHandle` and
  `SystemTable` arrival into the two provider-issued semantic root values and
  one invocation receipt. Until that producer exists, no compiler carrier may
  claim that installed Image/InitialStorage authority occupied RCX/RDX at this
  wrapper invocation. Native-invocation evidence belongs after that adapter;
  another compiler-side authority row would duplicate facts without closing
  the boundary.

  The CLI corpus is rooted on all hosted targets except the four GUI samples,
  which currently select Windows x64 and macOS arm64. Linux needs an ordinary
  source-level `Gui`/`Input` provider plus its general call/result realization;
  that is engineering work, not a language-design blocker. Proof-only and
  deliberately trapping fixtures remain targetless. Final firmware composition
  of `ImageHandle`/`SystemTable` inputs with semantic roots is design-blocked on
  owner Q1; the remaining physical bridge and corpus work is not. The native
  differential RUN corpus now routes every host-authored fixture through
  production entry selection (including bounded outer-job/single-worker native
  compiles) instead of silently retaining the legacy test-entry seam. Eight
  result-as-process-exit probes now keep their value-returning logic in ordinary
  helpers while target-rooted Unit entries consume those results through the
  explicit exit provider; that migration also closed named unsigned-conversion
  signedness and logical-NOT helper-result lowering gaps. Six additional
  residual scalar-result and host-deployable probes now use the same authored
  four-host root and Unit-entry discipline. Eight indexed-array and slice-loop
  native probes now also route their existing Unit entries through authored
  four-host roots without weakening their bounds, mutation, or conversion
  regression shapes. Nine further indexed-access, mutable-slice, subslice, and
  two-pointer native probes retain their exact regression programs while using
  the same authored production roots. Ten direct/dispatched slice reads,
  element copies, frame aliases, and bounded or dynamic subslice probes now
  likewise compile and run only through authored production roots. The tracked
  nested-window, parameter-subslice, runtime-end, and descriptor-pointer probes
  add ten more unchanged Unit-entry programs to that rooted native cohort.
  Eight linear ownership handoff, transfer, and transparent-record frontier
  fixtures now preserve their ownership and transition programs in direct Unit
  entries with explicit exit providers. Ten named float provider and conversion
  matrices now also use authored roots for native and cross-target differential
  execution. Ten indexed string-concat, bounded-carrier, slice-alias, and guard
  probes now consume the same checked-in four-host roots in their native and
  cross-target artifact tests. Ten further fixed-index, pointee, mutable-
  parameter, copied-struct, and lookup-driven text probes now use authored
  roots in their native and cross-target artifact coverage. Ten array
  reduction, indexed-write, indexed-guard, and stack-algorithm probes now also
  run their unchanged Unit entries through authored four-host roots. Four
  nested-loop/index probes and six dependent range, ordering, subtraction, and
  product-index probes likewise retain their exact programs under authored
  four-host roots. Ten dungeon reentry, Boolean/ordered dispatch, and
  string-field lookup regressions now also run their unchanged Unit entries
  through authored roots. Eight atomic operation probes and two structural
  dispatch/nested-field probes now share authored roots across native execution
  and the existing AArch64 opcode checks. Ten aggregate construction, nested-
  field, and value-copy probes now likewise run unchanged Unit entries through
  authored roots. Ten call-result, machine-owned storage, sum-payload, and
  subslice-window probes now likewise use authored production roots. Ten text-
  storage, string-reference, room-dispatch, and tuple-matrix probes now also
  use authored production roots. Ten domain-membership, address-value, finite-
  matrix, and static generic-dispatch probes now likewise consume authored
  roots. Nine proof/runtime, dependent-call, saturating-arithmetic, storage-
  alias, and case-membership probes now also consume authored roots. Ten
  indexed-write, target-selection, stdin, host-result, room-dispatch, and
  accepted-proof probes now likewise consume authored roots. Ten trapping-
  conversion, trapping-float, and portable filesystem probes now also consume
  authored roots. Ten portable filesystem wrapper probes now likewise consume
  authored roots. Ten value-call, indexed-collection, and result-domain probes
  now likewise consume authored roots. Fifteen typed-dispatch, fixed-integer,
  saturating-time, wire-policy, portable-filesystem, and console-byte probes now
  likewise consume authored roots. Fifteen Option/record value calls,
  computed-transition arguments, dispatched result deliveries, and distinct-
  receiver call chains now likewise consume authored roots. Fifteen further
  nested/parameter receiver, text-guard forwarding, dispatched terminal,
  partition, and slice-result probes now likewise consume authored roots. The
  next fifteen borrow/view, alias-transition, nested-terminal, builder/time,
  host-output, state-loop, reference-field, and dungeon-guard probes now retain
  the same direct Unit entries under authored four-host roots; their three
  target-specific footprint consumers use those checked-in roots as well. The
  final broad portable cohort adds twenty-four expression, slice/index, Result,
  text-domain, storage, and closed trait-dispatch fixtures without changing
  their direct Unit programs; the frame-indexed footprint consumer now uses
  that checked-in root as well. The tuple-transition and referenced-local
  sibling-guard result probes now keep their value-returning dispatch in
  ordinary helpers while rooted Unit entries route the exact results through
  the console exit provider. The referenced-local migration exposed and fixed
  native nested-splice ordering: a deferred branch prelude, straight-line arm,
  and leaf expansion now fire as one inner-first bundle after every local,
  host-call, or mutation effect in the contiguous callee splice, matching the
  interpreter instead of letting the parent entry mutation overwrite a nested
  leaf mutation. That ordering now distinguishes the newly deferred nested
  statement-call prelude from assignment-value, transition-result, and host-
  argument preludes that must still run at the call site: their declaration-
  time local capture is preserved while their straight-line and leaf result
  selection waits for the contiguous splice. The bounded-product index probe
  now retains its exact runtime
  coupling under an authored root: the contract widens each u32 factor to u64
  so the proposition is total without citing itself as overflow evidence, and
  both typed validation and resolved hoist synthesis project only independently
  checked value-preserving unsigned widenings back to the original field
  identities. The i64-backed interval lattice's missing u64 endpoint is closed
  by a structural width proof limited to two unsigned widening operands whose
  source-width sum fits their common target. The local-named dynamic probe also
  now has an authored root. Raw Windows and GUI fixtures remain platform-bound.
  The final three non-GUI gaps—User32 key-state and the two raw-filesystem
  Windows probes—now retain authored Windows entry selection. Their exact
  Windows roots are structurally cross-compiled on every development host,
  while native execution remains Windows-gated; this does not imply Linux
  `Gui`, `Input`, or raw-filesystem lowering. A registry-derived inventory now
  pins 890 `RUN_CANARIES`, 886 with authored roots, and exactly the four
  excluded GUI fixtures rootless. The tracked non-GUI authored-root backlog is
  zero. The earlier reported
  backlog of 18 was incorrect: its baseline parser omitted 39 multiline-form
  RUN rows, then
  the migration ledger subtracted 34 authored roots outside `RUN_CANARIES` as
  if they belonged to the differential corpus.
  Continue migrating those fixtures through production entry
  selection; replace result-as-process-exit probes with ordinary Unit entries
  and explicit exit providers rather than preserving the legacy entry seam.
- **CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS.** Take one real
  content-bearing source program through terminal Psi. Add sealed introduction
  and custody-exit frontiers, derive residual geometry at partial bodyless
  boundaries, and admit only provider custody. Infer identity-preserving
  reshuffles; partition changes require an authored theorem. Before emitting an
  introduction or exit, checked facts must bind the exact content subject and
  geometry to the selected provider plan, invocation receipt, backing/root
  lineage, installed occurrence, and route; a generic established-claim identity
  is insufficient. Checked source already derives exact identity-reshuffle and
  authored-partition composition rows, and terminal Psi independently validates
  their canonical replay. The exact root-only source passthrough now produces a
  structural result/return carrier with claim transfer, exit-time content
  replay, interpretation, and fuel. Omega preserves that carrier through the
  exact one-fragment native ABI path and all artifact/install layers, with claim
  identity retained as zero-runtime metadata. The remaining work is real
  authorized introduction, custody exit, residual geometry, and provider binding—not
  another passthrough representation.
- **INSTALLED-PROGRAM-LOCAL-ROOT-INTRODUCTION.** Implement the settled
  domain-route/installation model without a provision declaration. A
  content-bearing domain remains the sole source-level authority for one exact
  requirement. Its statically enumerable installed parameter positions may
  introduce fresh program-local lineages; ordinary calls and result routes with
  no parent lineage reject. Reconstruct exact per-occurrence capacity from the
  requirement instance, qualification, and owner-unique `Content<A>` projection,
  including owner-constrained const families. Join that schema to finite slot
  cardinality and lifecycle epoch during installation verification and derive,
  rather than trust, the aggregate for one installed artifact instance.

  Migrate the current `ExtentCompilerProvisioning`/`sealed_declaration`
  implementation carrier to route-position, capacity-schema, occurrence,
  cardinality, and epoch identities. Preserve provider issuance as a distinct
  admitted origin. Add source, terminal, artifact, and installation canaries for
  a one-root introduction, a finite multi-instance aggregate, an ordinary-call
  mint attempt, an unbounded installation shape, understated producer totals,
  cross-origin composition, stale epoch replay, and coexistence-peak reporting.
  A shared cap is one aggregate parent root divided among children; another
  child without supply rejects. Cross-epoch limits require persistent authority.
- **BOUNDARY-ISSUANCE** (after conservation): derive invocation geometry from
  parameters, entry places, and results. Keep ownership, issuance, custody,
  aliasing, and partition succession distinct. Providers may attest custody,
  never computable interval arithmetic.
- Under **TR3-TR8**, finish routed task claims, stack authority, cancellation,
  and transactional custody. Deferred acknowledgements lease the interrupt root
  and controller configuration; reconfiguration drains them.

Acceptance: reconstructed carriers mint no authority; every introduced content
claim traces to a verifier-reconstructed installed program-local occurrence or
admitted provider issuance; artifact-instance aggregates are derived for an
exact epoch and Cathedral can compose coexistence peaks; external effects have
an exact root-to-provider backing chain;
partition and residual arithmetic are compiler-derived; overlapping children,
gaps without a custody exit, algebra drift, receipt replay, and cross-root
recomposition reject.

### P2 — Source-visible materialization and placed access

Owners:

- `wiki/design_briefs/programmable_layouts.md`
- `wiki/design_briefs/os_memory_and_hardware_foundation.md`
- `wiki/language_guide/chapter_20_memory_layout_abi.md`

#### L4/L5 — plan-laid views

- Finish source-visible materialization over owned storage, including
  non-scalar tiling and mutable views beyond current record/array/slice checks.
  Raw bytes establish no typed fact without a selected validated plan and exact
  field identities. The accepted fixed subset reflects primitive arrays and
  recursively fixed arrays/records as whole `Repeated` or `Nested` fields, and
  one plan may place multiple independently keyed aggregate fields. View paths
  retain one whole `At` extent; owned materialization also admits an outer
  fixed array tiled by exactly one compiler-sized element `At` at one validated
  constant destination stride. Compiler-derived strides and offsets drive the
  interpreter and all three native target paths. Mutable fact-free byte views
  write and reread through those same extents, including two runtime indices
  through a gapped outer fixed array of recursively fixed arrays while
  retaining the plan-derived outer stride and compiler-derived inner stride,
  and through a gapped outer fixed array of fixed records whose interior fixed
  array retains the compiler-derived member offset between those indices.
  Typed owned materialization
  derives complete bytes from the exact schema (or a checked zero-argument Psi
  evaluator) while Omega supplies byte order, zeroes padding, and validates
  completely before mutation. Fully specialized generic records now participate
  through their synthesized concrete `CheckedShape` symbol and substituted
  member types, including specializations nested under fixed-array wrappers;
  spelling is never layout authority, and distinct specializations retain
  distinct symbols and widths. Retained layouts for numbered aggregate fields
  now rejoin the current typed schema by stable member identity rather than
  presentation spelling; a field rename preserves materialization, while
  missing or drifted identities reject before destination mutation. Numbered
  ordinary scalar materialization and decoding now use the same identity join
  across whole, stored-integer, and fragmented entries; decoded values retain
  the current schema spelling, while identity drift or collision rejects
  transactionally. Every fixed materialization and scalar-decoding entry point
  now preflights the retained layout identity set: one stable identity under
  multiple names, or one name under multiple identities, rejects before
  destination mutation, value exposure, or symbolic resolution. Retained access
  plans now retain complete source layout geometry and authorize replay only by
  a hash-free exact structural relation over schema identity, placements, size,
  alignment, offsets, and canonical identity sets. Authored entry order and
  numbered-member presentation renames are nonsemantic; compact fingerprints
  remain report/cache identity and cannot hide geometry drift.
  Erased terms remain semantically mandatory but add no bytes, including nested
  records and fixed arrays whose entire runtime shape is erased. Scalar
  placement/access semantics remain fenced for aggregates. Continue beyond
  this fixed subset. Sum materialization is design-blocked on the unsettled
  tagged-case placement vocabulary.

#### L6b — `AccessPlan` and `Placed<P, T>`

- Implement the settled borrowed/owned `Placed<P, T>` establishment and
  retirement model from `Extent in Granted`, using ordinary subrange borrows
  and no source-visible admission intermediate. Add the distinct core `view`,
  `initialize`, and `validate` operations; provider-specific adopt/open wrappers
  establish their external domains before `view`. Derive unconditional
  non-runtime Type inputs by canonical declaration path, keep proposition
  inputs and outputs in the `;` proof lanes, reject generic case-dependent Type
  custody, and emit the canonical per-outcome disposition table. The Type-only
  rejection payload returns exactly the formal inputs marked `returned`;
  embedded inputs become retirement debt and consumed inputs cite their exact
  consumer. Finish owned destruction/move-out evidence before returning
  `Granted & Vacant`.
- Derive readable, destructive-read, writable, and atomic field accessors while
  keeping logical extents distinct from whole-transfer footprints. Enforce
  total decode/encode, exact provider width/alignment, and operation-specific
  atomic laws. Continue rejecting External initialization, multi-transfer
  reads, and synthesized RMW. Retained numbered layouts now rejoin the current
  reflected schema and source-authored access decisions by stable member
  identity rather than presentation spelling; positional renames, identity
  drift or collision, and derived-offset drift reject before an access plan is
  sealed.
- Keep alias-exclusion admission separate from access rights; `&mut` does not
  claim exclusivity against a device. Sealed primitive events now specialize
  linearly into Stable read/take/write/swap, External read/take/write, or one
  exact Atomic operation and ordering while preserving the original authority
  on pre-event rejection. Carry the settled address-free placed-occurrence,
  resident-claim, loan, mapping/revision, exact footprint, and boundary-reach
  identities through Terminal Psi, installation, the interpreter, and both
  native backends without replaying source layout. Emit claim-local
  introduction, forwarding, transformation, exit, and loan rows.
  Every Stable primitive/compound, External primitive, and Atomic
  specialization independently replays the exact admitted effective-supply
  row—field key/name, width, authority-relative offset/address, and
  alignment—and returns the unchanged sealed request on drift. Sealed primitive
  requests also retain their validated field descriptor; every Stable,
  External, and Atomic specialization independently replays copied logical
  extent, concrete footprint, observation, and operation/borrow authorization
  before lowering, with unchanged authority-bearing custody returned on drift.
  Primitive specialization retains the complete sealed placement witness and
  independently replays its plan, profile receipt, admission, boundary reach,
  exact resource row/descriptor, source-loan polarity, resident claim, and
  placed occurrence; drift returns the unchanged authority-bearing request.
  Primitive requests also retain the original sealed field authorization and
  independently replay descriptor, current/source borrow polarities, and
  operation before specialization; coordinated privilege rewrites reject while
  returning the unchanged authority-bearing request.
  Stable read/write primitive specialization now exposes a borrowed outward-
  lowering preflight that independently replays the exact retained placement,
  admitted profile/resource row, descriptor geometry, resident-content
  custody, borrow polarity, authorization, and operation specialization.
  Rejection consumes nothing, so copied-evidence drift can be corrected and
  the same sealed request retried; no memory event or target lowering is
  established. Stable bounded compound-mutation specialization exposes the
  same borrowed preflight over its exact placement/profile/resource,
  descriptor and footprint, resident custody, exclusive current/source loans,
  authorization, and `CompoundMutation` identity. Rejection likewise consumes
  nothing and establishes no read-patch-write event or target lowering.
  External Read/Take/Write specialization now replays exact placement,
  profile/resources, descriptor/footprint, authorization, admitted External-
  or-conservative-Stable supply, and retained operation before outward
  lowering. Rejection performs no storage observation and consumes no custody,
  so repair and retry use the same sealed request; no external transfer or
  target lowering is established.
  Atomic primitive specialization now exposes borrowed outward-lowering replay
  of the exact placement/profile/resource authority, descriptor and footprint,
  resident custody, admitted Atomic supply, operation family, ordering law,
  and retained specialization. Rejection performs no atomic attempt and
  consumes no custody, so corrected retry uses the same sealed request; no
  target lowering or synthesized retry loop is established.
  Placement admission now retains the complete admitted resource profile
  through borrowed, owned, and borrowed-resident access; primitive
  specialization independently replays the exact profile/loan/plan join and
  rejects profile-root or compatibility drift while returning the unchanged
  request. Placed field projection now independently replays the retained
  placement plan, admitted profile/receipt, exact resource compatibility,
  admission, base, and source-loan polarity before field lookup or address
  derivation; rejection borrows and therefore preserves the complete placed
  authority for repair and retry. Placed field authorization independently
  replays the retained placement plan, admitted profile/resources, exact field
  descriptor and supply row, admission/reach, loan/resident identities, and
  derived primitive address before issuing an authorized access; rejection
  only borrows the projection, preserving its complete authority for repair
  and retry.
  Stable content adoption independently replays that retained profile against
  the exact owned extent and placement before establishing resident custody;
  rejection returns both the unchanged owned admission and provider content
  for corrected retry. Borrowed placed-view establishment now independently
  replays the retained placement, admitted profile/receipt, exact loan, and
  resource compatibility before creating a `PlacedView`; rejection returns the
  complete loan-bearing admission for corrected retry or withdrawal. Owned
  resident-view establishment now independently replays retained owned
  placement/profile/resource authority before activating a requested
  occurrence; rejection returns the exact dormant resident and occurrence for
  corrected retry, without claiming global occurrence freshness. Resident-
  preserving retirement now independently replays the active carrier's
  retained owned placement/profile/resource authority before returning dormant
  custody; rejection returns the exact active occurrence, resident claim,
  receipts, and Extent authority for corrected retry. Shared and exclusive
  borrowed-resident view establishment likewise replays the lender's retained
  owned placement/profile/resource authority before creating a whole-range
  loan; rejection consumes nothing, leaving the exact dormant resident
  authority available for repair and retry. Borrowed-resident retirement now
  independently replays the retained admitted profile/receipt, exact
  whole-range loan, placement plan, and resource compatibility before ending
  the placed occurrence; rejection returns the complete active borrowed
  carrier, preserving its loan, occurrence, resident claim, and provider
  receipts for corrected retry without reminting lender custody. Stable
  resident custody now retains the complete non-Clone provider existing-content
  grant rather than reducing it to copied receipt identities. Owned view,
  resident-preserving retirement, and shared/exclusive borrowed-resident
  establishment independently replay that grant's exact interpretation,
  origin, lineage, geometry, address space, provenance, era, resident claim,
  and provider receipts against the retained placement; drift returns the
  complete dormant or active carrier for corrected retry without reconstructing
  custody. Shared and exclusive borrowed-resident carriers now retain a
  lifetime-bound reference to that exact grant rather than copying claim and
  receipt identities. Borrowed retirement replays its interpretation, origin,
  lineage, geometry, address space, provenance, and era against the whole-range
  loan and placement before release; rejection returns the complete carrier for
  retry and neither clones nor remints lender custody. Placed projection, field
  authorization, and Stable/External/Atomic primitive specialization now also
  replay any retained resident grant against the exact owned extent or borrowed
  whole-range loan. Coordinated copied claim/occurrence rewrites cannot
  substitute unrelated custody; rejection borrows the carrier or returns the
  unchanged sealed request for corrected retry.
- Implement `Extent::Resident<P, T>` as the owned exact-range dormant-content
  qualification, including invariant type indices, mutual exclusion with
  `Vacant`, split/merge rejection, borrow versus owned-view continuity,
  resident-preserving retirement, partial-view retirement fences, and explicit
  migration through `Vacant`. Carry non-runtime custody in the resident claim.
  The concrete foundation now seals a provider-issued nonzero
  `ResidentClaimId` into dormant owned Stable content; explicit view consumes
  that carrier into a fresh nonzero `PlacedOccurrenceId`, field/access/lowering
  requests retain both identities, and resident-preserving retirement returns
  the same claim and receipts for a later fresh view. Ordinary borrowed views
  retain neither identity. Borrowed resident views retain the lender's exact
  claim and provider receipts, one fresh placed occurrence, and a whole-range
  shared or exclusive `ExtentLoan`; ending the view releases only that loan and
  remints nothing. Source-visible domain establishment, `Vacant` transitions,
  partial moves, Terminal propagation, and installation remain.
- Complete the atomic 2x2 compare-exchange family: existing observing strong
  and weak forms require copyable residents; new non-observing strong and weak
  forms return the proposal on failure and may transfer affine or linear
  custody using one copyable comparison key and exact selected encoding law.
- Close generic `ResidentContentTransfer<P, T>` applications at final
  composition from concrete and symbolic artifact demand, verify one selected
  provider covers the reconstructed application set, and bind exact issuance
  occurrences at installation. Do not create a slot per monomorph.
- Schema/device correspondence now has a distinct provider-issued,
  provenance-bearing authority carrier separate from storage compatibility.
  It binds one exact validated placement and resource-profile grant to a
  provider identity and stable device instance; optional runtime revision
  evidence retains its observation, predicate, observed value, and the same
  provider/device/grant identities. Admission independently replays every
  binding, and rejection returns the complete non-Clone grant or revision
  evidence for corrected retry. This establishes no storage compatibility,
  content validity, device observation, placed access, or publication.
  Admitted schema/device correspondence can now bind transactionally to one
  exact borrowed placement admission while remaining separate from storage
  compatibility. The join independently replays revision/provider/device/
  profile evidence and the admission's exact loan, placement, admitted profile,
  and resource compatibility. Rejection returns both complete non-Clone inputs
  for corrected retry; withdrawal returns the original loan and
  correspondence. No placed view, content qualification, field access, or
  device operation is established.
  Schema/device provider grants and admitted correspondence now retain the
  complete validated placement plan—layout, access policy, and boundary
  reach—rather than treating compact `PlacementPlanId` as authority. Admission
  and every later placement/view/access/retirement replay compare exact
  structure; same-ID/different-geometry or policy drift rejects transactionally
  and returns the complete non-Clone inputs for repair. Compact identities
  remain reporting/cache keys only.
  Corresponded borrowed placement admission now establishes a sealed placed-
  view carrier only after independently replaying both its physical
  correspondence and exact loan/profile/plan/resource admission. Rejection
  returns the complete bound carrier for corrected retry or withdrawal. The
  established carrier retains correspondence beside the placed view but
  deliberately exposes no projection or inner-view escape until primitive
  requests can carry and replay that evidence; no content, field access, device
  operation, or target lowering is established. Corresponded borrowed views
  now project through a distinct lifetime-bound placement-authority variant
  that retains the exact admitted schema/device correspondence through field
  projection, authorization, primitive requests, and External specialization
  preflight. Each boundary independently replays provider/device/revision
  evidence against the retained view placement/profile; ordinary views remain
  correspondence-free. Drift rejects without consuming the view/request, and
  no Terminal, device operation, or target-lowering authority is established.
  Stable primitive/compound, External primitive, and Atomic primitive outward
  specialization carriers now expose the exact lifetime-bound admitted schema/
  device correspondence retained by their sealed primitive request when one
  exists. Their borrowed preflights independently replay that correspondence
  with the complete placement authority; drift rejects without consuming the
  specialization, repair/retry preserves the same provenance, ordinary
  storage remains correspondence-free, and no device operation or target
  lowering is established. Corresponded borrowed placed views now retire
  transactionally: retirement independently replays the correspondence-to-
  view plan/profile identity and the exact loan/plan/admitted-profile/resource
  join, returns the original loan and non-Clone correspondence as distinct
  authorities on success, and returns the complete view on drift for corrected
  retry. Coordinated copied-receipt drift and correspondence drift fail closed;
  retirement establishes no content or device operation. Ordinary borrowed
  placed views now retire through a checked loan-release transition that
  independently replays the exact loan, placement, admitted profile/receipt,
  and resource compatibility. Drift returns the complete view for corrected
  retry; success returns the original loan with its origin, lineage, geometry,
  and polarity unchanged and establishes no content, vacancy, or destruction.
  Corresponded retirement reuses the same placement replay before returning
  its separate non-Clone correspondence. Every Stable primitive/compound,
  External primitive, and Atomic primitive outward specialization now exposes
  a shared borrow of its exact sealed primitive request. Consumers can inspect
  the complete lifetime-bound placement, authorization, resident, and optional
  schema/device provenance without copied-identity reconstruction or mutation;
  rejection preserves that same carrier, repaired replay reproduces it exactly,
  and no transfer, device operation, or target lowering is established.
  Provider/device-bound External lowering now crosses a distinct
  correspondence-required preflight after generic External specialization. The
  sealed carrier retains the exact lifetime-bound non-Clone schema/device
  correspondence and independently replays the complete placed request,
  supply, operation, and correspondence identity; correspondence-free or
  substituted authority rejects without observation and returns the exact
  External specialization for repair or alternate use. No provider operation
  is selected, no transfer occurs, and no target lowering is established.
  Provider/device-bound Atomic lowering now crosses the same distinct
  correspondence-required boundary after generic Atomic specialization. Its
  sealed carrier retains the exact lifetime-bound non-Clone correspondence and
  independently replays the complete placed request, admitted Atomic supply,
  operation/ordering law, and correspondence identity; correspondence-free or
  substituted authority rejects without an atomic attempt and returns the
  exact Atomic specialization for repair or alternate use. No provider
  operation is selected and no target lowering is established.
  Provider/device-bound Stable primitive lowering now crosses a distinct
  correspondence-required preflight after generic Stable read/write
  specialization. Its sealed carrier retains the exact lifetime-bound non-
  Clone correspondence and independently replays the complete placed request,
  admitted Stable supply, operation, and correspondence identity;
  correspondence-free or substituted authority rejects without a memory event
  and returns the exact Stable specialization for repair or alternate use.
  Ordinary Stable storage remains correspondence-optional; no provider
  operation is selected and no target lowering is established. Provider-bound
  bounded Stable compound lowering now applies the same boundary after generic
  `CompoundMutation` specialization, replaying exclusive placed custody,
  admitted Stable supply, bounded read-patch-write identity, and exact
  correspondence. Rejection performs no read or write and returns the exact
  compound specialization; ordinary Stable compound access remains
  correspondence-optional.

#### L6c — symbolic materialization

- Carry symbolic sources, placement constraints, immutable post-handoff bytes,
  exact footprint, and invocation plan through final artifacts. Connect placed
  fragments to source-level provider invocation after establishment; provider
  preparation generates no host code. Validate exact bytes and placement;
  fingerprints remain report/cache identity, never authority. Numbered
  symbolic fields now rejoin fragmented layout rows by stable member identity
  rather than presentation spelling: renames preserve generated-writer
  identity, while identity drift or collision rejects before resolver
  invocation. Symbolic materialization now preflights every retained write's
  static bit geometry and byte range before invoking any provider/compiler
  resolver; an invalid later field produces no resolver observation or partial
  action plan. Post-handoff execution resolves each exact relocation target
  once, immediately validates that value against every retained same-target
  write, and does not observe unrelated targets after rejection. Fully resolved
  materialization then independently replays every write's geometry and
  stored-integer fit before staging any byte; tampered or out-of-range values
  reject without truncation or destination mutation. Static writer validation
  and reusable-fragment lowering consume the same known-value validator, so
  invalid pre-resolved fit evidence rejects before any dynamic resolver
  observation or destination mutation. Reusable post-handoff invocation
  evidence is sealed behind validated lowering; installation, external-root,
  and instruction-selection consumers may inspect but cannot reconstruct or
  weaken the exact fragment, placement, source-slot, or fit evidence. Sealed
  invocation evidence now also supports independent borrowed structural replay
  of its context ABI, placement alignment, exact fragment geometry, canonical
  source-slot order, target uniqueness, stored-fit linkage, and recomputed
  fingerprint before source values are accepted; rejection leaves the
  invocation unchanged for corrected retry. Instruction-selection binding now
  independently replays the sealed invocation, target architecture, exact
  re-encoded bytes, state footprint, normalized fragment identity, and emitted
  fingerprint; every rejection returns the unchanged lowered writer evidence
  for corrected retry without regeneration. External-root prepared-writer
  execution independently replays the retained invocation structure,
  writer-derived invocation, opaque context binding, and exact installed-code,
  artifact, and architecture identities before destination mutation; rejection
  returns both the exact prepared invocation and destination for corrected
  retry. External-root writer binding independently replays retained lowered
  invocation structure, canonical bytes, footprint, and emitted identity;
  every bind rejection returns the exact lowered fragment and non-clonable
  provider preparation for corrected retry. Bound external-root writer
  execution independently replays the retained lowered fragment and exact
  provider preparation/context relationship before destination consumption;
  rejection returns the complete bound carrier and exact destination for
  corrected retry. Writer
  derivation, lowering, validation, and execution uniformly require at least
  one retained fragment; an empty provider program cannot claim materialization.
  Validation also binds every supplied source word to any exact pre-resolved
  value sealed for that slot, so numeric substitution under unchanged evidence
  rejects before resolver observation or destination mutation. Post-handoff
  writer execution now stages the complete resolved fragment program and
  commits its writer range once. Any late application rejection leaves the
  provider's exact destination bytes unchanged for recovery/retry, while
  successful bytes remain unpublished until the existing consumer-specific
  validation/publication transition. Successful post-handoff destination
  writing now retains the exact non-clonable resolved context, including sealed
  invocation, placement, source-slot values, and fingerprint, rather than
  reducing it to copied report identities. Failure returns both the context and
  prepared destination intact for corrected retry. The external-root consumer
  replays that context against the exact installed realization and destination
  preparation before writing and again before exposing the still-unpublished
  written carrier; this establishes neither consumer semantics nor publication
  authority. Successful bound external-root writer execution now returns a
  sealed non-clonable carrier retaining the exact AOT-lowered fragment beside
  the installation-owned written destination and resolved context. The outward
  consumer independently replays canonical lowered bytes, footprint, emitted
  identity, invocation, target architecture, and the exact installed
  realization; rejection only borrows the carrier, preserving every input for
  corrected retry. The destination remains unpublished and this transition
  establishes neither consumer semantics nor publication authority. Successful
  external-root writer execution now also retains the exact admitted provider
  execution, target architecture, source invocation, writer plan, and
  installation-owned written destination/context. Its outward consumer
  independently re-lowers and replays the writer against those retained
  provider and installation facts, while the compiler's written bound carrier
  retains that complete provider evidence beside the exact AOT-lowered
  fragment. Validation rejection only borrows the carriers, preserving complete
  retry ownership; no consumer semantics or publication authority is
  established.
  Written-but-still-unpublished external-root writer destinations now expose a
  checked recovery transition. The external-root and compiler-bound consumers
  independently replay the exact invocation, lowered fragment, provider
  execution, installed realization, mapping, and destination preparation
  before returning the sealed prepared/bound invocation with its exact
  destination for retry. Rejection returns the complete non-clonable written
  carrier unchanged; success preserves the current unpublished bytes and
  establishes neither consumer semantics nor publication authority. Compact
  fingerprints remain replayed identity only and create no authority.
  External-root post-handoff writer preparation now binds the admitted entry to
  one exact canonical provider-resolved source slot. A copied pre-resolved
  numeric entry cannot substitute for sealed provider resolution. The selected
  entry identity and source-slot correspondence remain attached through
  prepared, written, and recovered non-clonable carriers, and each consumer
  independently replays them; drift rejects while preserving the complete
  carrier for corrected retry. This establishes no provider-operation
  authority, consumer semantics, publication, or native execution, and compact
  fingerprints remain identity rather than authority.
  External-root symbolic writer preparation now retains the exact validated
  requirement-bearing root evidence beside its admitted provider-execution
  evidence throughout preparation, writing, and recovery. Each consumer
  replays their full structural equality before accepting the terminal summary,
  selected entry, and canonical provider-resolved source slot; a separately
  valid root with substituted requirement identity rejects while preserving
  the complete carrier for corrected retry. Compact normalized identities
  remain consistency/report keys rather than authority, and this establishes
  no provider-operation authority, consumer semantics, publication, or native
  execution.
  Compiler-side external-root writer binding now consumes and retains the exact
  selected source `ServiceSchema` beside the lowered fragment and non-clonable
  provider preparation. Binding and every later bound/written/recovery consumer
  replay the selected provider-plan identity, unique exact requirement row,
  boundary arity, complete parameter-identity row cardinality, calling-plan
  identity, and admitted entry claims against the retained requirement-bearing
  root evidence. Rejection returns the selected schema, lowered writer, and
  prepared invocation intact for corrected retry. The schema and compact
  fingerprints remain identity/shape evidence rather than provider-operation
  authority; no device operation, consumer semantics, publication, or native
  execution is established. Selected source-schema correspondence is now also
  preflighted before provider preparation resolves any symbolic source or
  populates the opaque writer context. The compiler borrows the exact admitted
  provider/root evidence and replays the same provider-plan, requirement,
  boundary, calling-plan, and entry claims; drift rejects with every input
  unchanged and no resolver observation. AOT binding and later consumers retain
  their independent replay.
  Selected external-root source schema and provider-populated writer context now
  cross preparation as one sealed non-clonable carrier. Preparation consumes
  the exact selected plan only after preflighting provider identity,
  requirement, boundary shape/calling identity, and entry claims; every
  rejection returns that selected plan unchanged before resolver observation.
  AOT binding accepts only the sealed preparation, and binding rejection returns
  the exact lowered writer plus complete preparation for corrected retry,
  preventing same-plan schema substitution after context population. Successful
  binding transfers the original schema/context pair through bound, written,
  and recovery custody. This grants no provider-operation authority and
  establishes no device event, publication, or native execution.
  Selected external-root writer preparation now consumes the exact AOT-lowered
  fragment and preflights its canonical structure, target architecture, and
  invocation against the retained provider writer plan before the installed
  resolver may observe any symbolic source. The non-clonable preparation seals
  selected source schema, lowered fragment, and provider-populated context
  together; binding accepts only that carrier and independently replays all
  three. Early schema/lowering drift and later destination-preparation rejection
  return the exact selected schema and lowered fragment for corrected retry.
  This establishes no provider-operation authority, consumer semantics,
  publication, or native execution.
  External-root writer preparation now consumes and seals the exact activated,
  pinned, writable, unpublished destination before the installed resolver
  observes symbolic sources. The destination's non-clonable mapping,
  preparation receipt, placement, and mutable byte view remain joined to the
  selected schema, exact AOT lowering, and provider-populated context through
  binding and execution. Preparation rejection returns the selected schema,
  lowering, and destination intact; execution rejection and written recovery
  return the complete bound carrier with that same destination, preventing
  same-geometry destination substitution after resolution. This establishes no
  provider-operation authority, consumer semantics, publication, or native
  execution.
  Prepared post-handoff destinations now expose a borrowed exact replay of
  their activated mapping, provider receipt, required write rights, pinning,
  unpublished state, placement, and byte-view geometry. External-root writer
  preparation performs that replay before the installed resolver observes
  symbolic sources; drift returns the selected schema, lowering, and complete
  non-clonable destination unchanged. Corruption rejects without modifying
  destination bytes, and repaired evidence supports retry through the same
  carrier. This grants no provider-operation, write, publication, consumer-
  semantic, or native-execution authority.
  External-root writer preparation now independently replays the complete
  admitted provider execution before symbolic-source resolution: exact
  validated root structure, retained validated boundary carrier, execution-to-
  root binding, exit assurance, and recomputed normalized execution identity.
  `ValidatedExternalRoot` preserves `ValidatedBoundaryEntryPlan` rather than
  downgrading it to raw plan data, while its existing raw-plan accessor remains
  available. Execution-fingerprint drift rejects before resolver observation;
  repaired evidence supports retry unchanged. This establishes no provider-
  operation authority, consumer semantics, publication, or native execution.
  External-root writer preparation now retains the exact borrowed installed-
  code realization beside the selected schema, AOT lowering, activated
  unpublished destination, and provider-populated context before symbolic-
  source resolution. Binding independently replays the context against that
  exact installation and destination; execution, outward validation, and
  written recovery reuse the same retained installation rather than accepting
  a substitutable resolver parameter. A colliding installed artifact rejects
  during preparation while returning the selected schema, lowering, and
  destination intact for corrected retry. This establishes no provider-
  operation authority, consumer semantics, publication, or native execution.
  Compiler-bound written external-root destinations now require a consuming
  outward-consumer replay against the exact retained installed realization
  before bytes or decomposed written state are observable. An equal-looking
  substitute installation rejects before observation and returns the complete
  non-clonable carrier for corrected retry; only the validated still-
  unpublished carrier exposes bytes, parts, and recovery. This establishes no
  provider-operation authority, consumer semantics, publication, device event,
  or native execution.
  The external-root written destination now requires its own consuming outward
  replay before bytes or decomposed installation-written state are observable.
  Its sealed non-clonable validated carrier retains exact provider/root,
  invocation, installed realization, mapping, context, and destination
  evidence; rejection returns the complete raw carrier unchanged. Compiler-
  bound validation retains this lower validated carrier instead of downgrading
  it after replay, so observation and recovery remain gated through both
  custody layers. Bytes remain unpublished and this establishes no provider-
  operation authority, consumer semantics, device event, or native execution.

Acceptance: UART/MMIO, shared-page IPC, and ordinary RAM use one extent/layout
foundation with different profiles. Misalignment, insufficient rights,
unplanned offsets, narrow External writes, destructive reads through a
repeatable accessor, overlapping transfer footprints, forged profile evidence,
and unsupported atomic operations reject before code generation.

### P3 — Terminal Psi, proof-carrying artifacts, and fuel

Owners:

- `wiki/architecture/pipeline/terminal_psi.md`
- `wiki/design_briefs/canonical_ir_fuel_and_resource_provisioning.md`
- `wiki/architecture/bootstrap_lattice/proof_kernel.md`

Remaining:

- **PSIIR.** Extend terminal Psi only as complete vertical slices: canonical
  encoding, independent obligation reconstruction and verification,
  interpretation, fixed fuel, Omega lowering, native evidence, artifact/image
  custody, and installation must move together. The detailed accepted
  vocabulary and current fences live in
  [`terminal_psi.md`](wiki/architecture/pipeline/terminal_psi.md); do not
  duplicate its operation-by-operation ledger here.

  The accepted baseline covers bounded scalar/direct and structural/content
  calls, guarded crash continuations, structural results, fixed-array custody,
  exact affine cleanup and partial transfer, bounded acyclic control, selected
  provider catalog/dispatch, and verified Boolean/integer shared cleanup
  convergence. Integer leaves retain the documented policy arithmetic, casts,
  shifts, division/remainder, exact-operation evidence, bounded nesting, and
  independent exact leaves in distinct proof-free subtrees across interpreter
  and every native target. A finite same-carrier exact-add chain may have a
  landed literal sibling at each link, while a finite exact-subtract chain may
  continue only through its left operand and must have a landed literal right
  operand at each link. A finite same-carrier chain may mix exact addition and
  subtraction when both kinds occur, every link continues through its left
  operand, and every right operand is a landed same-carrier literal. The
  verifier combines additions and mathematical negations of subtrahends in a
  checked sign/magnitude offset and derives every carrier-tight prefix bound
  from the direct root. A finite same-carrier chain may also mix exact divide
  and remainder while continuing only through its left operand, with a landed
  nonzero unsigned divisor or signed divisor other than `0` and `-1` at every
  link. A finite same-value-carrier exact-right-shift chain may likewise
  continue only through its left operand, with an independently landed fixed
  native integer count satisfying `0 <= count < value width` at every link.
  A finite same-value-carrier exact-left-shift chain may likewise continue only
  through its left operand, with independently landed fixed native integer
  counts satisfying `0 <= count < value width`; count carriers may differ.
  The verifier accumulates those counts with checked arithmetic and derives the
  carrier-tight bound on the direct root for every prefix, including the
  zero-only root when the cumulative count reaches the value width.
  A finite same-carrier exact-multiply chain may continue only through its left
  operand, with an explicitly landed same-carrier nonnegative literal factor at
  every link. All seven forms require a direct machine-parameter root. The
  verifier walks ordered definitions for addition, subtraction, their mixed
  chain, multiplication, and left shift and reconstructs every retained
  operation's safety obligation independently;
  multiplication accumulates nonnegative factors with checked arithmetic and
  derives carrier-tight root bounds, while divide/remainder and right-shift
  links need no producer-definition authority because each safe landed divisor
  or count reconstructs independently. One direct
  fixed-integer parameter may also pass through a finite chain of valid
  widenings and then exactly narrow back to its original carrier; Terminal
  retains every operation and independently derives the narrowing obligation
  from the ordered, uniquely defined widening chain. Separately, one exact
  fixed-native cast may consume a finite nonempty left-associated same-carrier
  exact-add/subtract literal-offset chain rooted at a direct machine parameter.
  The verifier retains every prefix proof, accumulates the checked offset, and
  independently derives target-range-minus-offset bounds intersected with the
  source carrier, including signed and cross-sign conversions. One
  validator-legal partial fixed-native cast may also consume a finite nonempty
  left-associated same-source-carrier exact-multiply chain rooted at a direct
  machine parameter, with independently landed nonnegative literal factors.
  Every multiply prefix keeps its own evidence; the cast uses the checked
  cumulative product to reconstruct the inverse target interval and intersect
  it with the source carrier. Product zero makes only the cast obligation true,
  product one uses the ordinary target/source intersection, and larger products
  divide the signed or unsigned target bounds without erasing earlier proofs. A
  validator-legal partial fixed-native cast may likewise consume a finite
  nonempty left-associated same-source-carrier exact-left-shift chain rooted at
  a direct machine parameter, with independently landed legal fixed-native
  counts whose carriers may differ. Every shift prefix keeps its own evidence;
  the cast uses the checked cumulative count to shift the target interval right
  and intersect it with the source carrier. Count zero uses the ordinary
  target/source intersection, a sub-source-width count uses signed or unsigned
  inverse target bounds, and a source-width-or-larger count makes only the cast
  true because any successfully produced exact source result is zero.
  A finite nonempty same-source-carrier exact-right-shift chain may feed the
  same partial cast under the same direct-root and heterogeneous landed-count
  fences. The cast independently reconstructs the arithmetic/zero-fill shift
  preimage of the target interval; at or above source width, unsigned roots
  yield zero while signed roots yield `-1` or `0` and therefore require a
  nonnegative root only when the target is unsigned.
  A finite nonempty same-source-carrier exact-divide/remainder chain may now
  feed the same partial cast when verifier-owned toward-zero division and
  dividend-sign remainder interval-hull replay maps the full source carrier
  wholly inside the target. Every arithmetic prefix and the cast retain
  independent evidence; guard-sensitive nonconvex preimages remain fenced.
  Conversely, one
  validator-legal partial fixed-native cast of a direct parameter may root a
  finite nonempty left-associated same-target-carrier exact-add/subtract chain
  with independently landed literal right siblings. The cast keeps its own
  direct representability evidence; every arithmetic prefix keeps distinct
  evidence for the target interval shifted by its checked cumulative offset and
  intersected with the source carrier. Cancellation cannot erase an earlier
  prefix obligation. The same direct partial-cast root may instead feed a
  finite nonempty left-associated same-target-carrier exact-multiply chain with
  independently landed nonnegative literal factors. Every multiply prefix
  keeps distinct evidence for the target interval divided by its checked
  cumulative product and intersected with the source carrier; zero and one
  produce a true current-prefix obligation without erasing earlier proofs. The
  direct partial-cast root may also feed a finite nonempty left-associated
  same-value-carrier exact-left-shift chain with independently landed in-range
  fixed-native counts whose carriers may differ. Every prefix keeps distinct
  evidence for the target interval shifted right by its checked cumulative
  count and intersected with the source carrier; a cumulative count at least
  the target width admits only the zero root.
  One direct fixed-native parameter may now also root a finite left-associated
  same-carrier affine chain that contains both an exact add/subtract offset and
  an exact multiply. Every right sibling is an independently landed
  same-carrier literal, multiply factors are nonnegative, and every ordered
  prefix retains independent evidence. The verifier replays each prefix as
  `A * parameter + B` with checked nonnegative `A` and checked signed `B`, then
  derives the carrier preimage; constant prefixes are true or false from `B`
  alone. A later zero factor or offset cancellation cannot erase an earlier
  proof. The same unified mixed affine chain may now feed one validator-legal
  partial fixed-native exact cast. The cast independently reconstructs the
  target interval through `(A, B)` and intersects it with the source carrier;
  `A == 0` decides only the cast from target representability of `B`, while all
  earlier arithmetic-prefix evidence remains mandatory.
  The converse unified family is now retained as well: one validator-legal
  partial fixed-native exact cast of a direct parameter may root a finite
  nonempty same-target-carrier affine chain containing both offset and multiply
  operations. The verifier independently reconstructs every prefix through
  checked `(A, B)` composition and the target/source interval intersection;
  `A == 0` decides only the current prefix from target representability of `B`,
  while cast and earlier arithmetic evidence remain mandatory.
  The direct partial-cast root may now also feed a finite nonempty
  left-associated same-value-carrier exact-right-shift chain with independently
  landed heterogeneous legal counts. The cast proof remains independent, and
  every shift prefix is reconstructed from only its own `0 <= count < width`
  fact without cumulative count, value-definition, or evidence import.
  The same direct partial-cast root may now feed a finite nonempty
  left-associated same-target-carrier exact-divide/remainder chain. Every
  prefix retains independent evidence derived only from its own landed safe
  divisor; cast evidence, prior operation proofs, value definitions, and
  quotient/remainder algebra supply no authority.
  The direct-root and post-cast exact-divide/remainder chain families now share
  one runtime-divisor widening when at least one right sibling is a direct
  same-carrier machine parameter. Every runtime divisor retains an independent
  positive or at-most-`-2` proposition. The joint signed `-1` exception remains
  restricted to the first direct-root operation when its dividend bound is
  independently available; computed and post-cast dividends import no prior
  proof authority. Literal-only chains remain on their existing paths.
  One direct machine-parameter root may now feed any finite left-associated
  same-carrier chain containing both exact-left and exact-right shifts, with
  independently landed heterogeneous legal counts. Every left prefix maps its
  carrier-tight safe interval backward through all prior canonical mixed-shift
  definitions, intersecting the carrier after each inverse left or right step;
  every right proof remains its own legal-count proposition. No prior shift
  proof supplies authority, so later right shifts cannot erase unsafe prefixes.
  The same finite mixed exact-shift chain may now feed one validator-legal
  partial fixed-native exact cast. The cast starts from the target/source
  carrier intersection and independently maps that interval backward through
  every ordered mixed-shift definition; mathematical emptiness reconstructs
  falsehood, while checked interval-arithmetic failure admits nothing. Every
  shift prefix and the cast retain separate mandatory evidence.
  Conversely, one validator-legal direct partial fixed-native cast may root a
  finite nonempty same-target-carrier chain containing both exact-left and
  exact-right shifts. Each left prefix independently replays the ordered
  canonical post-cast definitions back to the cast, intersects its surviving
  target interval with the source carrier, and reconstructs source-root bounds;
  the cast and every shift retain separate evidence. Mathematical emptiness is
  falsehood, while checked transfer failure admits no family.
  A unified finite exact-arithmetic prefix may now feed a finite exact-shift
  suffix on the same fixed-native carrier when the arithmetic prefix is a
  left-associated add/subtract/nonnegative-multiply literal chain and the
  suffix contains at least one exact-left shift. Every left prefix maps its
  safe interval backward through prior shifts and the checked affine form
  `A * root + B`; every arithmetic operation, count, and left prefix retains
  independent evidence. `A == 0` decides only the current left obligation,
  mathematical emptiness is falsehood, and checked replay failure admits no
  family.
  Conversely, a finite exact-shift prefix may now feed a finite exact-
  arithmetic suffix on the same fixed-native carrier. Each arithmetic prefix
  maps the carrier backward through checked `A * shifted_root + B`, then
  replays the complete ordered shift prefix to the direct root. Every count,
  left overflow, and arithmetic obligation remains independently mandatory;
  `A == 0` decides only the current proposition after full shape validation,
  mathematical emptiness is falsehood, and checked replay failure admits no
  family.
  A unified finite affine/cast/affine sandwich may now cross one validator-legal
  partial fixed-native exact cast. Both sides are nonempty left-associated
  add/subtract/nonnegative-multiply literal chains. The cast independently maps
  its target/source interval through the checked source form; every target
  prefix maps the target carrier through its own checked form, intersects with
  the source carrier, then maps through the complete source form. Every source
  arithmetic, cast, and target arithmetic obligation remains independently
  mandatory. A zero coefficient on either side decides only the current
  proposition after full ordered shape validation; mathematical emptiness is
  falsehood and checked replay failure admits no family.
  A unified finite exact-shift/cast/exact-shift sandwich may likewise cross one
  validator-legal partial fixed-native cast, with nonempty left-associated
  shift chains on both sides and independently landed heterogeneous legal
  counts. Every source shift, cast, and target shift keeps separate mandatory
  evidence. Each target-left prefix replays its target definitions to the cast,
  intersects the surviving interval with the source carrier, then replays the
  complete source chain to the direct parameter. Mathematical emptiness is
  falsehood; checked transfer failure admits no family.
  The two heterogeneous affine/shift cast sandwiches are retained as one
  consolidated family. A nonempty source affine chain may cross one partial
  fixed-native exact cast into a nonempty target shift chain, or a nonempty
  source shift chain may cross the cast into a nonempty target affine chain.
  Each side uses its established landed-literal/count rules and ordered
  canonical replay. Every source operation, the cast, and every target
  operation keeps separate mandatory evidence; zero coefficients decide only
  the current obligation after full shape validation. Mathematical emptiness
  is falsehood, while checked composition or interval-transfer failure admits
  no family. Empty-sided shapes remain on their narrower existing paths.
  A consolidated divide/remainder cross-cast family now covers all four
  compositions between one nonempty landed-literal exact-divide/remainder
  chain and one nonempty affine or shift chain. When divide/remainder precedes
  the cast, the existing carrier-total quotient/remainder hull must fit the
  target carrier; each target prefix is true when that full hull lies inside
  its reconstructed safe interval, false when disjoint, and otherwise remains
  unadmitted rather than inventing a guard-sensitive nonconvex preimage. In the
  converse direction, the source affine or shift chain and cast reconstruct by
  their existing rules while every target divisor proof stays independent.
  Every source operation, cast, and target operation retains separate evidence.
  The corresponding direct same-carrier family now retains all four nonempty
  divide/remainder-to-affine/shift compositions without a cast. A leading
  divide/remainder chain supplies its complete verifier-owned carrier hull to
  each following affine or left-shift safe interval: containment is true,
  disjointness is false, and partial overlap remains unadmitted. In the
  converse direction, affine or shift proofs use their established direct-root
  reconstruction while each following divide/remainder proof depends only on
  its own landed safe divisor. Every operation retains independent evidence.
  A finite nonempty exact-divide/remainder chain may also cross one
  validator-legal partial fixed-native exact cast into another finite nonempty
  exact-divide/remainder chain. The cast replays the complete carrier-total
  source hull and is admitted only when that hull wholly fits the target
  carrier; it does not manufacture a partial-overlap or falsehood case. Every
  source divisor proposition, the cast, and every target divisor proposition
  keeps separate mandatory evidence, and each target operation depends only on
  its own independently landed safe divisor.
  The three homogeneous exact-multiply placements—direct, feeding one partial
  exact cast, and rooted at one direct partial exact cast—also admit finite
  signed-carrier chains containing at least one negative independently landed
  factor. A checked sign/magnitude cumulative product reverses negative
  preimages, handles the signed minimum without host negation, and keeps zero
  local to only the current proposition. Every multiply prefix and cast
  remains independently mandatory; mathematical emptiness is falsehood while
  checked product or interval failure admits no family.
  A separate signed-affine family now covers three placements: a direct
  signed-carrier chain, the same chain feeding one partial fixed-native exact
  cast, and one direct partial cast feeding the chain. Each chain is finite,
  nonempty, left-associated, rooted at one direct machine parameter, contains
  at least one add/subtract offset and at least one negative multiply, and has
  an independently landed same-carrier literal on every right edge. The
  verifier replays every shrinking prefix as checked sign/magnitude
  `A * root + B`; a negative `A` reverses the interval preimage, `MIN` never
  uses host negation, and `A == 0` decides only the current obligation. Every
  arithmetic prefix and cast retains independent evidence. Mathematical empty
  preimages are falsehood, while checked coefficient, offset, division, or
  interval failure admits no family. Homogeneous signed products,
  nonnegative-affine chains, two-sided sandwiches, and conversion-chain forms
  remain on their existing or fenced paths.
  A consolidated two-sided signed-affine sandwich now crosses exactly one
  validator-legal partial cast between signed fixed-native carriers. The
  source and target are finite nonempty left-associated add/subtract/multiply
  chains with independently landed same-carrier signed literals. Either the
  source itself contains an offset and a negative multiply, permitting any
  target affine prefix, or the source remains on the established nonnegative
  affine algebra and the current target prefix contains both an offset and a
  negative multiply. The verifier replays checked sign/magnitude `(As, Bs)`
  and `(At, Bt)`, reverses either negative preimage, intersects the exact cast
  carriers, and reconstructs only the current target obligation. Zero on
  either side remains local after full shape validation; every source prefix,
  cast, and target prefix keeps separate evidence. Mathematical emptiness is
  falsehood, while checked coefficient, offset, division, or interval failure
  admits no family. The all-nonnegative sandwich, one-sided signed-affine and
  homogeneous signed-product paths, thin product/offset permutations, and
  conversion-spine forms retain their existing priority or fence.
  A finite chain of at least two validator-legal partial fixed-native exact
  casts may now start at one direct integer machine parameter. For each cast
  prefix, the verifier follows only ordered shrinking cast definitions and
  intersects the root carrier with every carrier reached so far. The resulting
  canonical root bounds prove only that cast; every earlier cast retains its
  own mandatory evidence. A mathematical empty intersection is falsehood,
  while malformed definitions or carrier reconstruction failure admit no
  family.
  The same finite cast core may now follow one nonempty already-admitted
  computed prefix: landed-literal affine arithmetic (including the homogeneous
  signed-product path), an exact-shift chain, or a carrier-total exact-
  divide/remainder chain. For every cast prefix, the verifier intersects every
  carrier reached so far and reuses only that computed family's verifier-owned
  inverse algebra to reconstruct the direct root. Every computed-prefix and
  cast obligation remains independently evidenced. Empty affine/product/shift
  preimages are falsehood, checked replay failure admits no family, and the
  divide/remainder hull remains admissible only by complete containment.
  Conversely, a finite chain of at least two partial exact casts may feed one
  nonempty already-admitted target-carrier affine, homogeneous signed-product,
  exact-shift, or landed-safe-literal divide/remainder suffix. The verifier
  first validates and intersects the complete ordered cast chain without
  importing any cast evidence, then reuses only the selected post-cast
  family's existing inverse algebra for the current suffix prefix. Every cast
  and suffix operation remains independently evidenced; mathematical empty
  preimages are falsehood and checked replay failure admits no family.
  The two directions compose into one unified nonempty computed-prefix,
  at-least-two-partial-cast, nonempty computed-suffix sandwich across the same
  affine, homogeneous signed-product, exact-shift, and carrier-total landed-
  divisor families. Each source prefix, each shrinking cast prefix, and each
  target prefix is reconstructed independently from ordered canonical
  definitions. The verifier intersects every cast carrier, applies only the
  selected target inverse and source inverse/hull algebra, and never imports
  another operation's evidence. Zero coefficients remain local to the current
  target obligation; a mathematical empty interval is falsehood, while
  malformed shape or checked transfer failure admits no family.
  A separate wider-arithmetic composition now permits one nonempty admitted
  computed prefix to pass through a finite nonempty chain of strict valid
  fixed-native integer widenings and feed one nonempty admitted computed
  suffix. Both sides independently select affine, homogeneous signed-product,
  exact-shift, or landed-safe-literal divide/remainder algebra. Every widening
  definition is retained and validated in order; each target interval pulls
  back through numeric-identity widening by intersecting the source carrier,
  then reuses only the selected source inverse or carrier-total hull. Every
  exact operation retains independent evidence. Mathematical emptiness is
  falsehood, divide/remainder partial overlap and checked replay failure admit
  no family, and zero coefficients remain local to the current obligation.
  A heterogeneous conversion-spine sandwich now composes the same nonempty
  computed prefixes and suffixes across a finite contiguous word containing at
  least one strict valid fixed-native integer widening and at least one
  validator-legal partial fixed-native exact cast. Every adjacent carrier and
  shrinking definition is validated in order. Each cast prefix independently
  intersects all preceding conversion carriers and replays only the selected
  source inverse or complete-hull algebra; each target prefix walks the entire
  conversion word before the same source replay. Widenings remain retained
  numeric-identity operations without invented evidence, while every source
  operation, partial cast, and target operation keeps separate evidence. Pure
  widening, pure cast, one-edge, direct, and narrower sandwich shapes retain
  their existing dispatch priority. Mathematical emptiness is falsehood,
  source divide/remainder casts require complete hull containment without a
  partial or falsehood admission, and checked replay failure admits no family.
  A same-root affine fork/join now admits one exact add or subtract whose two
  operands are disjoint, nonempty, independently admitted landed-literal
  affine branches over the same fixed-native carrier and the exact same direct
  machine parameter. The verifier replays each branch separately as checked
  sign/magnitude `Al * root + Bl` and `Ar * root + Br`, then reconstructs the
  join from `(Al + Ar, Bl + Br)` or `(Al - Ar, Bl - Br)`. A zero combined
  coefficient decides only the join after both complete ordered branch walks;
  every operation in both branches and the join retains separate evidence.
  Mathematical empty preimages are falsehood, while checked coefficient,
  offset, division, or definition-walk failure admits no family. The branch
  definition walks must be disjoint and source ordered apart from their common
  root. Distinct-root joins, one empty side, outer operations other than add or
  subtract, conversions, runtime siblings, locals, members, calls, effects,
  and stale or redirected definitions remain fenced.
  A distinct-root signature-bounded affine fork/join separately admits the
  same outer exact add or subtract when the two disjoint, nonempty, ordered
  landed-literal affine branches end at different direct machine parameters
  of one fixed-native carrier. For each root, the verifier selects only the
  tightest landed unary lower and upper bounds appended by its signature,
  intersects them with the carrier, and maps the resulting interval forward
  through that branch's checked signed affine form. The join range is the
  Minkowski sum or difference of the two independent branch ranges. Complete
  containment in the join carrier yields the canonical conjunction of the
  selected root bounds; a wholly disjoint range yields falsehood; partial
  overlap admits no family. Relational cross-root premises, absent or
  one-sided unary bounds, shared roots, overlapping or reordered branch walks,
  computed roots, carrier drift, conversions, and unchecked arithmetic remain
  fenced. Every operation in both branches and the join retains independent
  evidence.
  A same-root signature-bounded signed affine quadratic product join now
  admits one outer exact multiply whose two disjoint, nonempty, ordered
  landed-literal affine branches end at the same direct signed fixed-native
  parameter with nonzero coefficients. The verifier selects the tightest
  landed unary lower and upper signature bounds, composes the correlated
  integer quadratic, and evaluates its exact discrete range at both endpoints
  and the in-range floor/ceiling lattice points adjacent to the rational
  vertex. Complete carrier containment yields the canonical two-bound
  conjunction; a wholly disjoint range yields falsehood; partial overlap or
  checked coefficient, vertex, or evaluation failure admits no family. Every
  branch operation and the outer multiply retains separate evidence. Constant
  collapse, distinct roots, relational premises, one-sided bounds, unsigned
  carriers, malformed walks, computed roots, conversions, and stale evidence
  remain fenced.
  A same-root signature-bounded signed affine divide/remainder safety join now
  admits one outer exact divide or remainder whose two disjoint, nonempty,
  ordered landed-literal affine branches end at the same direct signed
  fixed-native parameter with nonzero coefficients. The verifier selects the
  tightest landed unary lower and upper signature bounds and solves the exact
  integer-lattice equations for divisor zero and divisor `-1`. A `-1` root is
  forbidden only when the correlated dividend evaluates to the carrier
  minimum at that exact root. No forbidden root emits the canonical two-bound
  conjunction; forbidden roots covering the whole integer interval emit
  falsehood; a partially unsafe interval or checked equation/evaluation
  failure admits no family. Every branch operation and the outer divide or
  remainder retains separate evidence. Distinct roots, constant collapse,
  relational premises, one-sided bounds, unsigned carriers, malformed walks,
  computed roots, conversions, and stale evidence remain fenced.
  A distinct-root signature-bounded signed affine product join now admits one
  outer exact multiply whose two disjoint, nonempty, ordered landed-literal
  affine branches end at different direct signed fixed-native parameters. The
  verifier requires and selects the tightest landed unary lower and upper
  signature bounds for both roots, maps each interval forward through its
  checked signed affine form, and takes the exact hull of all four rectangle
  corner products. Complete carrier containment yields the canonical
  four-bound conjunction; a wholly disjoint hull yields falsehood; partial
  overlap or checked corner overflow admits no family. Every branch operation
  and the outer multiply retains separate evidence. Same-root quadratic
  correlation, relational premises, one-sided bounds, unsigned carriers,
  overlapping walks, computed roots, conversions, and stale definitions remain
  fenced.

  Next engineering frontiers are other proof-bearing results feeding another
  proof-bearing operation, wider multivariate and other computed-sibling joins,
  other computed exact-cast and wider exact-arithmetic
  premises, member/comparison mixtures, calls and effects, wider partial-value
  cleanup, nested ownership, returned transfer, loops, suspension, scoped
  ordering, and ranked tail recursion. Dynamic/nested indexing, wider
  projections and signatures, content-bearing splits, and unsupported contracts
  remain fail-closed until independently verifier-owned.

  Retire checked/source-tree consumers with each slice. Nothing below terminal
  Psi may depend on typed/source trees, `ExpressionHandle`, source rendering, or
  an Omega-to-Psi bridge. Partition replay binds the exact operation and
  verifier-selected callee guarantee; fingerprints are identity, never
  authority.
- **CRASH-CONTRACT.** Extend guarded implication beyond the accepted acyclic
  scalar slice. Direct and staged calls retain invocation-specific substitutions
  and verifier-reconstructed continuations. Canonical Boolean and fixed-integer
  member paths rebase across whole-root, fixed-index, and all-field-projected
  structural calls. The proposition carrier covers Boolean composition,
  relevant-record equality, fixed-width bitwise terms, policy-distinct integer
  arithmetic, evidence-bounded division/remainder, and exact or wrapping shifts;
  codecs, verification, fuel, and interpretation reject missing or redirected
  premises. Genuinely empty-record equality normalizes to the existing Boolean
  constant carrier through calls and terminal verification; all-erased records
  remain distinct and fenced. The distinct `addr` carrier is also explicitly
  excluded from both direct structural-member predicates and whole-record leaf
  expansion, and Terminal lowering rejects the retained source contract rather
  than encoding address equality as fixed-integer evidence. Built-in IEEE
  `f32`/`f64` equality now retains one atomic, format-annotated proposition per
  relevant structural leaf, including whole-record expansion and projected-call
  rebasing. Direct structural-field `!=` uses the same atomic carrier with an
  explicit comparison kind. The verifier resolves both exact paths and formats
  independently; the carrier preserves IEEE NaN and signed-zero behavior rather
  than laundering either operator through mathematical `Equal`. Whole-record
  float `!=` canonically negates the already-sorted equality conjunction as
  `P -> Falsehood`; projected calls rebase every leaf below that implication.
  Aggregate equality now also retains byte-sequence fields as one content atom
  over two nonempty canonical structural paths. The checked and Terminal
  carriers distinguish borrowed views from bounded owned storage (including
  the exact owned capacity) without admitting native pointer/descriptor layout
  into semantic identity; equality itself is live length plus the exact live
  byte prefix, never pointer, capacity, or unused-byte equality. Both roots are
  independently resolved and rebased through structural calls. Borrowed
  `&[u8] in Domain` and bounded `[u8; N] in Domain` fields participate in
  synthesized `Equatable` record equality, while literals and direct text `!=`
  remain outside this slice. Payload-less sums now retain a closed structural
  sum shape with exact case identities. Intrinsic `==` lowers to the flat
  canonical conjunction of both membership implications for every declared
  case; `!=` is that equality proposition implying falsehood. The verifier
  independently resolves both structural subjects and every case identity.
  Payload-bearing pure sums now retain exact case-payload field identities and
  direct relevant Boolean, fixed-integer, IEEE, and byte-sequence leaf types.
  Their intrinsic `==` is the canonical disjunction of per-case conjunctions:
  matching membership for both roots plus that case's exact payload-leaf
  equalities. `!=` is that complete equality proposition implying falsehood.
  Unknown or redirected case/field identities reject independently. Mixed
  common-field/case shapes, nested or recursive payload expansion, address and
  erased payload equality, and runtime sum layout remain fenced. Semantic codec
  v18, proof-bundle v18, and installation-record v24 retain the structural
  shapes, case-payload paths, and proposition. Continue with the fenced mixed,
  nested, recursive, and erased aggregate cases. Concrete machine/state
  contracts plus domain/data predicates, trait invariants and signatures,
  machine-parameter requirements, and root/domain operator contracts now reject
  direct binary and named-float `Trapping` arithmetic plus direct Trapping
  conversions. Comparisons, bitwise inspection, float classification,
  Wrapping/Saturating operations, and non-reserved custom float calls remain
  total; proof expressions do not create crash sites. Wrapping/Saturating
  division and remainder now form in concrete and direct abstract Prop only
  when an independently accepted prior fact proves the divisor interval
  nonzero; carrier-overflow policy does not define division by zero, and the
  fact containing a partial term cannot justify that term's own formation.
  Exact and Saturating shifts in direct abstract Prop now retain the same
  independently-prior `[0, operand_width)` count obligation already enforced
  for concrete machine/state contracts; `Saturating` defines value overflow,
  not an invalid count, while `Wrapping` continues to define every count by
  modulo reduction. A count bound inside the proposition containing the shift
  cannot authorize that shift's formation.
  Exact division and remainder in concrete machine/state and direct abstract
  Prop now retain the catalog's complete primitive-definedness judgment: an
  independently accepted prior fact must exclude a zero divisor and, for
  signed carriers, the `MIN / -1` primitive pair (including remainder's shared
  hardware-definedness edge). A guard in the proposition containing the
  operation supplies no authority for that operation's own formation.
  Finish the settled total-specification arithmetic slice: retain explicit
  fixed-integer/address embeddings into proof `Int` with their derived carrier-
  range facts. This remains blocked on a real vocabulary dependency: the
  shipped core `embed` is a transitional ordinary machine returning structural
  `Nat`, while typed primitive expressions and Terminal `ScalarTerm` have no
  unbounded proof-`Int` embedding that can retain and independently verify the
  source carrier identity/range; do not substitute dead checked evidence.
  Explicit same-carrier policy-erasure `as` coercions now retain the
  ordinary Exact representability obligation in concrete machine/state Prop
  and across the direct abstract signature form; only independently accepted
  prior `requires` facts discharge it, so the proposition containing an
  operation cannot justify that operation's own formation. Add the per-
  primitive Exact, Wrapping, Saturating, and Trapping denotation bridges;
  compiler-derived Trapping guards remain executable crash-site facts rather
  than predicate effects. Imported crash capsules remain blocked on artifact
  identity and certificate binding.
- **PROOF-CERTIFICATION-BRIDGE.** Emit kernel-checkable certificates from source
  automation. One recursive certificate owns one SCC, cites its ranking and
  well-foundedness evidence once, and proves every internal edge decreases;
  ordinary calls remain contract applications. Normalization cites exact
  conformance/law evidence and preserves transitive trust. Thread recursive
  components and cited laws through the accepted trust record and synopsis.

  Acceptance: perturbing any recursive edge decrease, component
  well-foundedness reference, normalized-law identity, or cited premise
  rejects or changes the recorded trust closure; measured mutual proof
  recursion checks while an unmeasured cycle rejects; an admitted law makes
  every dependent normalization admission-dependent.
- **PCC-CANONICAL-SEMANTIC-LEDGER.** Replace the current trusted Rust fusion of
  artifact traversal and algebraic reduction with the settled two-part closure.
  A total low-rung generator consumes canonical terminal-Psi bytes, validates
  the exact structure, directly denotes each primitive operation, and emits one
  ordered canonical ledger of goals plus local premise introductions. Clever
  interval, affine, shift, quadratic, and divide/remainder analysis becomes an
  untrusted certificate producer that proves the unchanged canonical goal.

  First expose the migration state honestly: add exact trust-graph nodes for the
  Rust decoder/verifier, each sufficient-form reduction family, the ledger
  framework, each unproved leaf denotation schema, and each unproved call-
  composition row. Every dependency must resolve
  through an acyclic graph to a registered root with kind, semantic subject,
  digest/version, owner, scope, rationale, and accepting policy; unknown leaves
  reject. Do not encode an uncertified reducer as an admitted program premise.

  Current migration inventory: the canonical proof synopsis now publishes one
  validated source-bound trust graph for the exact Rust decoder, proof kernel,
  verifier, eight sufficient-form reduction families, the current unproved
  ledger framework, 35 closed leaf-schema rows, and three separate call-
  composition rows covering all 38 `OperationKind` variants. Node
  digests bind the exact deciding Rust/specification bytes and explicit
  versions; the graph identity also binds every canonical dependency edge.
  Unknown, cyclic, unreachable, duplicate, malformed-root, and noncanonical
  graphs reject, and the current artifact closure reports `fully-derived false`.
  The first production Rust ledger slice now has one closed
  `psi-terminal-semantics` table covering all 38 operation kinds and preserving
  the 35 leaf / 3 call-composition custody split. Twenty goal-free scalar leaves
  carry explicit result, operand, denotation, goal, fact, crash, fuel, and
  frontier axes and reconstruct their local equations through one generic
  interpreter. Exact lookup rejects missing or duplicate rows. The terminal
  verifier and codec trust graph consume that shared inventory instead of
  maintaining independent operation matches. Structural/effect rows and
  call/control composition remain separate and are not promoted into the
  goal-free scalar table. The second production Rust ledger slice
  now mirrors Gamma's separate exact-unique three-row structural/effect table:
  Boolean field reads, port writes, and trivial affine-local establishment keep
  result, custody, action, external-effect, fuel, and place-frontier axes
  explicit. One generic interpreter emits distinct fact, effect, or frontier
  observations; the verifier consumes its Boolean equation instead of
  reconstructing that row independently. The trust graph consumes the same
  table as 32 direct-denotation plus three structural/effect nodes while
  preserving the 35 leaf / three call-composition operation-custody split.
  The modular verifier source split is also fully rebound into trust identities:
  evidence provenance, integer foundations, proof-bundle custody,
  reconstruction, and substitution bytes can no longer change outside the
  registered verifier/ledger dependency digests.
  The third production Rust ledger slice now mirrors Gamma's exact-unique
  three-row call-composition table. Scalar, structural Unit, and boundary calls
  retain independent target, result, argument, requirement, transfer, outcome,
  crash-route, evidence-lifetime, fuel, and frontier policies. The verifier's
  contract composition moved out of general operation reconstruction into one
  focused table-selected module; existing module validation still proves the
  concrete signature, movement, coverage, substitution, outcome, crash, and
  evidence invariants before composition. Call policy and implementation bytes
  are both bound into the same three call trust nodes.
  The fourth production Rust ledger slice now owns the twelve proof-bearing
  scalar leaves in a separate exact-unique table. Exact cast, left/right shift,
  exact add/subtract/multiply, and exact/wrapping/saturating divide/remainder
  retain declared result and operand shape, direct denotation, one of six
  canonical goal shapes, normal-successor result equation, crash policy, fuel,
  and frontier policy as independent axes. One generic interpreter emits a
  typed canonical-goal carrier and the post-discharge result equation; malformed
  type/row custody rejects before reduction. Artifact reconstruction consumes
  that observation instead of rebuilding twelve result equations. The current
  sufficient-form algorithms remain trusted migration dependencies and select a
  reduced proposition through one isolated dispatcher; neither the table nor
  the dispatcher falsely claims a kernel derivation of the canonical goal. The
  trust graph binds the table to exactly those twelve denotation nodes and the
  dispatcher to every affected reducer.
  Eight of those Terminal rows now rejoin the settled shared integer-policy
  catalog by exact primitive/domain identity: exact add/subtract/multiply,
  exact divide, exact left/right shift, and wrapping/saturating divide. Row
  validation derives their existing canonical goal shapes from the catalog's
  formation conditions, so representability, divisor, and shift-count policy
  are no longer a disconnected Terminal authority. Exact cast and all
  remainder rows remain explicitly unbound because the catalog defines no
  corresponding primitive; no policy is inferred past the settled vocabulary.
  The independent Terminal verifier's structural crash-policy validation now
  also obtains exact/wrapping/saturating divide and exact left/right-shift
  formation conditions directly from the shared catalog before applying its
  own retained-fact safety checks. Exact division still requires both nonzero
  and representability custody, policy division requires nonzero custody, left
  shift requires count and representability custody, and right shift requires
  count custody. Remainder remains on its explicit verifier path because the
  settled catalog has no remainder primitive.
  Checked-to-Terminal integer operation emission now allocates formation
  obligations from the shared catalog's nonempty formation-condition rows for
  exact add/subtract/multiply/divide, exact left/right shift, and
  wrapping/saturating divide. Goal-free catalog rows allocate none.
  Exact/wrapping/saturating remainder retain their existing explicit
  obligations but remain catalog-unbound because no remainder primitive is
  settled. One operation-local allocator replaces repeated policy-specific
  identity arithmetic; obligation identity and Terminal operation shapes are
  unchanged.
  The dedicated concrete/abstract Exact division definedness checker now also
  obtains its nonzero-divisor and signed-result-representability requirements
  from the shared catalog before applying its existing interval/fact analysis.
  Exact remainder retains the same explicit two-condition hardware-
  definedness path because the settled catalog has no remainder primitive.
  Diagnostic ordering, accepted fact frontier, and rejection behavior are
  unchanged.
  The next bounded proof-calculus parity slice exposes canonical disjunction
  introduction in the production certificate kernel. One
  `DisjunctionIntroduction` node owns exactly one independently checked child
  and one selected canonical arm index; a non-disjunction conclusion, absent or
  out-of-range arm, mismatched child conclusion, stale proof vocabulary, or
  excess proof depth rejects. Canonical proof-bundle v13 assigns rule tag 9.
  The registered proof-calculus root now binds the exact proposition,
  proof-rule, primitive/evidence, and proof-codec definitions, while the Rust
  kernel remains an explicit trusted implementation. Independent Beta/Gamma
  `inl`/`inr` gates agree. This adds certificate capability only: all eight
  sufficient reducers and all unproved semantic rows retain `TrustedJudgment`,
  and terminal codec v18 / installation record v24 remain unchanged.
  The next bounded certificate capability is also complete.
  `NonzeroDivisor` now has an exact fail-closed kernel proposition projection:
  unsigned fixed integers use `1 <= d`, signed fixed integers of at least two
  bits use the ordered disjunction `(d <= -1) OR (1 <= d)`, and signed one-bit
  integers use `d <= -1`; address and mismatched carriers reject, while the
  other five canonical goal shapes remain unprojected.
  `IntegerLessOrEqualTransitivity` checks two recursively derived `<=` premises
  with an identical middle and exact outer endpoints, allowing existing
  `d <= -2` evidence plus the closed `-2 <= -1` relation to establish the
  negative disjunct. Proof-bundle v14 assigns rule tag 10; the registered
  calculus is v11 and the Rust kernel v3. This initially landed as capability
  only; the bounded wrapping-divide pilot below is its first production
  consumer.
  The next bounded certificate capability adds
  `IntegerLessOrEqualSubstitution`. Two independently checked children prove
  one integer `<=` relation and one equality; endpoint 0 or 1 selects the left
  or right relation endpoint to replace, the other endpoint must remain exact,
  and either equality orientation is accepted. A non-order relation,
  non-equality evidence, unknown endpoint, changed untouched endpoint, or
  mismatched replacement rejects. Proof-bundle v15 assigns rule tag 11; the
  registered calculus is v12 and the Rust kernel v4. This initially landed as
  capability only; the bounded wrapping-divide pilot below is its first
  production consumer.
  The complete `WrappingIntegerDivide` semantic row now reconstructs the
  canonical `NonzeroDivisor` goal and uses an untrusted, kernel-checked
  certificate producer over only machine requirements and pre-site semantic
  axioms. The producer deterministically prefers the signed negative arm and
  supports exact citation, integer-order transitivity, and literal equality
  substitution. Missing projection or evidence rejects with no operation-result
  self-justification and no fallback to the legacy reducer. The complete
  `WrappingIntegerRemainder` row now uses the same canonical proposition and
  untrusted, kernel-checked prior-fact certificate producer. Reconstruction
  selects the goal solely from the exact operation tag, fails closed without a
  valid certificate, and cannot cite the operation's own result equation. Both
  wrapping divide/remainder rows are now canonical. The complete
  `SaturatingIntegerDivide` row now also uses canonical `NonzeroDivisor`
  reconstruction and the same untrusted, kernel-checked prior-fact certificate
  producer. Signed `MIN / -1` remains total through the saturating denotation,
  so nonzero is the complete precondition; reconstruction is exact-tag selected
  and fails closed without a valid certificate. The complete
  `SaturatingIntegerRemainder` row now uses exact-tag canonical
  `NonzeroDivisor` reconstruction and the same untrusted, kernel-checked
  prior-fact producer. Signed `MIN % -1` remains total with result zero. All
  four wrapping/saturating divide/remainder rows are now canonical. One
  complete family shared by exact divide and exact remainder now also bypasses
  trusted sufficient reduction: when the pre-site semantic ledger lands an
  unsigned nonzero literal divisor, or a signed literal divisor other than
  zero and `-1`, reconstruction selects canonical `ExactDivisionDefined` and
  the existing untrusted recursive producer proves its order arm solely from
  that landing equality and a closed integer relation. The complete signed
  `-1` exceptional family is canonical too when the dividend is independently
  landed as any literal above the carrier minimum; the producer recursively
  composes the exact third disjunct, or the two-conjunct `i1` goal, from both
  landing equalities and closed order. The next complete existing-fact family
  accepts the same landed `-1` divisor when the exact canonical
  `MIN + 1 <= dividend` proposition (`0 <= dividend` for `i1`) is independently
  retained in the machine requirements or pre-site semantic ledger. The
  producer cites that proposition directly as the second recursive premise;
  it does not import a reduced obligation or infer a wider interval. Missing,
  stale, or weaker bounds reject. The next complete one-hop family accepts a
  retained literal lower bound `K <= dividend` when closed same-carrier order
  independently proves `MIN + 1 <= K`. The producer composes that primitive
  relation with the exact prior citation through integer-order transitivity;
  reversed, mistyped, weaker, or wrong-dividend facts reject. Missing or zero
  divisor evidence, or a `-1` divisor without either a nonminimum dividend
  landing or the exact retained bound, rejects these paths. The next complete
  direct safe-divisor family now selects the canonical goal from an exact prior
  `1 <= divisor` proposition for unsigned or signed fixed carriers, or
  `divisor <= -2` for signed widths of at least two. Unsigned certificates cite
  the goal directly; signed
  certificates cite the selected first or second disjunct and wrap it with
  disjunction introduction. The complete signed-width-at-least-two joint family
  now selects the third canonical arm when both `divisor <= -1` and
  `MIN + 1 <= dividend` are independently available through the supported exact
  citation or checked transitivity paths. The producer proves each conjunct,
  constructs their conjunction, and introduces only that ordered disjunct;
  either missing premise or wrong operand identity rejects. The mixed member of
  that family also accepts a retained `divisor <= -1` bound with an independently
  landed nonminimum dividend literal. The producer derives the dividend floor
  only by closed integer order plus substitution of that exact landing equality;
  a minimum or wrong-identity landing rejects. The same complete substitution
  family now accepts exact literal equalities retained as machine requirements,
  not only pre-site semantic landings. The selector checks every same-carrier
  equality for the exact operand, and the producer cites it as an `Assumption`;
  zero-only, minimum-dividend, mistyped, or redirected premises reject. The next
  complete endpoint-transport family pairs an exact retained bound on `K` with
  an independently retained equality connecting `K` to the canonical divisor
  or dividend endpoint in either orientation. The producer cites both children
  under `IntegerLessOrEqualSubstitution`, replacing only that endpoint. Dividend
  transport remains inside the signed joint arm and therefore also requires its
  independent `divisor <= -1` premise. A missing companion bound, unrelated
  equality, weak bound, or changed untouched endpoint rejects. The complete
  signed `i1` transport family may independently transport both conjuncts:
  `Kd <= -1` through `Kd == divisor`, and `0 <= Kn` through `Kn == dividend`.
  Both substitutions and both canonical conjuncts remain mandatory; missing or
  crossed equalities reject. The next complete nested family transports a
  one-hop stronger bound: closed same-carrier order first derives the canonical
  bound on `K` (for example `2 <= K` or signed `K <= -3`), then endpoint
  substitution carries it to the divisor. The producer nests one checked
  transitivity node beneath substitution; weak bounds, missing equalities, or
  wrong endpoints reject. The next complete nested family replaces that
  transitivity node's closed side with a second exact citation: unsigned
  `1 <= M` and `M <= K`, or signed `K <= M` and `M <= -2`, followed by
  `K == divisor`. The producer nests the two-citation transitivity proof beneath
  endpoint substitution in deterministic ledger order. A missing or
  disconnected middle relation, weak signed ceiling, redirected equality, or
  wrong endpoint rejects. The signed joint arm now admits the corresponding
  complete dividend sibling: an exact `divisor <= -1`, plus
  `MIN + 1 <= M`, `M <= K`, and `K == dividend`. The producer constructs the
  ordered conjunction and nests the two dividend-floor citations beneath
  endpoint substitution; a missing or disconnected middle fact cannot prove
  the arm. The complete nested signed-`i1` family transports both mandatory
  conjuncts from two exact citations each: `Kd <= Md`, `Md <= -1`,
  `Kd == divisor`, and `0 <= Mn`, `Mn <= Kn`, `Kn == dividend`. The producer
  emits the ordered conjunction of two transitivity-under-substitution proofs;
  either missing middle relation rejects the whole goal. The signed
  width-at-least-two joint arm is also complete when both conjuncts use direct
  two-citation chains: `divisor <= K`, `K <= -1`, and
  `MIN + 1 <= M`, `M <= dividend`. The producer introduces only arm 2 and
  constructs its ordered conjunction from the two transitivity proofs; a
  missing or disconnected citation rejects the entire arm. A signed `i1`
  divisor fact alone remains
  insufficient because its canonical conjunction also requires the dividend
  premise. The complete retained-bound `i1` family now selects that conjunction
  when both exact prior propositions `divisor <= -1` and `0 <= dividend` are
  independently present; the untrusted producer cites both and composes them
  through conjunction introduction. A missing premise or wrong operand identity
  rejects. One-hop stronger retained safe-divisor facts are now complete too:
  `K <= divisor` is accepted when closed same-carrier order proves `1 <= K`,
  and signed `divisor <= K` is accepted when it proves `K <= -2`. The verifier
  selects only the exact operand identity and the untrusted producer composes
  the canonical arm by integer-order transitivity before disjunction
  introduction. Missing, reversed, weakened, mistyped, or wrong-divisor facts
  reject. The next complete transitive family replaces the closed side of that
  step with a second exact prior citation: `1 <= K` plus `K <= divisor`, or
  signed `divisor <= K` plus `K <= -2`. Reconstruction requires the exact shared
  middle term and operand identity; the producer cites both facts in deterministic
  ledger order under one checked transitivity node. Missing, disconnected,
  reversed, or redirected pairs reject. An exact retained canonical goal is now
  cited directly, and an exact retained canonical arm is introduced at its
  ordered disjunct index. Reconstruction uses the same recursive
  `LessOrEqual`/conjunction/disjunction shape as the producer instead of separate
  safe-divisor and exceptional branches; redirected goals, reordered joint
  conjunctions, or wrong operand identities reject. The operation result is not
  available as proof authority. The current
  proof rules and proof-bundle v18 codec carry the certificates without a
  vocabulary change. All remaining
  exact divide/remainder families stay on trusted sufficient reduction, so
  neither complete row changes trust status. Their exact-defined
  prerequisite is nevertheless canonical and exact: unsigned requires
  `1 <= d`; signed widths at least two require the disjunction of `d <= -2`,
  `1 <= d`, and `(d <= -1) AND (MIN + 1 <= n)`; `i1` requires
  `(d <= -1) AND (0 <= n)`.
  Address or type mismatch rejects. Existing kernel rules suffice, but the
  producer does not yet materialize canonical certificates for the accepted
  affine and correlated families. Keeping the exact rows trusted is therefore
  an implementation gap, not a language-design blocker. The producer now has
  a kernel-checked recursive compositor for exact prior citations, atomic
  integer-order proofs, conjunctions, and arbitrary ordered disjunctions,
  covering the common certificate spine for the signed three-arm and `i1`
  exact goals. A producer-visible proof-kernel checker now binds signed fixed
  same-carrier affine normalization to a nonempty, strictly ordered set of
  prior semantic-axiom equalities. It independently replays exact
  add/subtract/multiply-by-literal definitions and recomputes checked
  `A * root + B`, rejecting stale, reordered, malformed, ambiguous,
  cross-carrier, non-value-root, target-drifted, or overflowing witnesses. This
  is a common prerequisite for direct definition chains and both affine
  branches used by same-root/correlated analysis, not an order proof or a
  serialized proof rule. A companion producer-visible checker now maps one
  independently established canonical root `<=` proposition through that
  checked affine form. It preserves order for positive coefficients, reverses
  it for negative coefficients, deterministically maps zero coefficients to
  the constant offset, and rejects wrong shapes, checked-arithmetic overflow,
  or an out-of-carrier endpoint. Proof rule `IntegerAffineBound` now performs
  the intentional integration. One recursively checked root-bound child and
  one `IntegerAffineWitness` bind the exact root, target, and strictly ordered
  semantic-axiom definition indices; the kernel rechecks normalization and the
  mapped conclusion, and records every selected definition in accepted premise
  closure. Non-order or wrong-root children, stale/reordered/malformed words,
  target/carrier drift, arithmetic overflow, or a mismatched mapped bound
  reject. Proof-bundle v18 retains tag 12; the registered calculus is v15 and
  the Rust kernel v7, with the affine and cast checkers included in both
  trust-graph source sets. The first bounded producer family now uses the rule
  for one to four prior signed fixed affine definitions whose exact retained
  root bound maps directly to a canonical safe-divisor arm. Reconstruction and
  production enumerate shortest words first and advance only prefixes accepted by the
  affine witness checker; within each depth, semantic-axiom indices stay
  strictly ordered. The kernel independently checks continuity, algebra, the
  mapped conclusion, and accepted-premise custody. Missing root custody,
  incomplete, reversed, redirected, or stale words, wrong targets, and
  noncanonical mapped arms reject. Root custody may now also use one exact
  prior landed literal or value-alias transport. A typed `root == literal`
  citation substitutes the root into either endpoint of one closed reflexive
  relation; a value alias instead combines one directly cited integer bound at
  the alias endpoint with its independently cited equality. One exact
  two-citation order chain may instead reconstruct the root bound through one
  shared SSA middle under a checked transitivity child. Direct roots remain
  preferred, then landed literals, alias transport, and transitivity; equality
  facts stay in ledger order, while bound and second-leg indexes use their exact
  value endpoint. A missing bound, equality, or order leg, unsafe or mistyped
  literal, identity, non-value, disconnected, redirected, cross-carrier, or
  same-citation join rejects. Three-or-more-alias or three-or-more-leg root
  reconstruction, words of five or
  more definitions, joins, cast/shift compositions, and correlated results
  remain on trusted reduction; neither complete exact row changes trust and
  `fully-derived false` remains. An exact mapped affine bound may also close to
  the canonical arm through one typed closed-literal order bridge on the
  unchanged target endpoint. A stronger lower bound places the primitive
  bridge before `IntegerAffineBound`; a stronger upper bound places it after.
  Candidate mapping supplies no authority: the kernel rechecks the exact
  affine conclusion and the enclosing transitivity certificate. A nonclosed,
  mistyped, redirected, or weaker bridge rejects, and no variable-endpoint or
  cited-fact search is added. Affine completion now lives in dedicated,
  side-local `affine_custody` modules. Production and reconstruction
  independently own the fixed four-definition witness frontier, exact mapped
  bound, and optional closed relaxation; no authority is shared. Fixed affine-
  witness candidate enumeration now lives in independent side-local
  `affine_custody/frontier` modules. Production and reconstruction each
  enumerate shortest definition words first, preserve source-ordered semantic-
  axiom indices, advance only prefixes independently accepted by the affine
  witness checker, and stop at the explicit four-definition ceiling. Candidate
  pruning grants no proof authority: mapped-bound construction, optional closed
  relaxation, and final proof or retained-bound checking remain in each side's
  affine-custody parent. Witness order, rejection behavior, proof shapes, and
  the finite frontier are unchanged. Affine frontier prefix replay now lives in
  paired, side-local `affine_custody/frontier/prefix` modules. Producer and
  reconstruction independently validate each indexed equality row, retain
  left-before-right Value-target precedence, and ask the proof kernel to replay
  the exact accumulated definition word before that prefix advances. Fixed-
  depth frontier expansion, proof shape, rejection behavior, and the four-
  definition boundary remain unchanged.
  Ordered affine-witness candidates now live in paired, side-local
  `affine_custody/candidates` modules. Producer and reconstruction independently
  require an exact `LessOrEqual` goal, enumerate left-before-right Value targets
  and the existing definition-word frontier, and construct the same
  `IntegerAffineWitness`; root-evidence custody and completion remain in their
  prior side-local authorities. Proof shape, rejection behavior, and the fixed
  bounded frontier are unchanged.
  Optional affine endpoint relaxation now
  lives in independent side-local `affine_custody/relaxation` modules.
  Production alone maps the checked affine root bound, constructs
  `IntegerAffineBound`, and places one closed primitive bridge on the exact
  unchanged endpoint before final certificate checking; reconstruction
  independently recomputes and kernel-checks the mapped conversion before
  replaying the same closed relation. Direct affine conversion remains
  preferred, while witness selection and final acceptance remain in each
  affine-custody parent. Endpoint orientation, proof shape, nonclosed,
  mistyped, redirected, or weaker rejection, and the single-bridge frontier
  are unchanged. Affine
  evidence selection now lives in dedicated, side-local `affine_selection`
  modules. Production and reconstruction independently preserve the exact
  preference order across direct, literal-landed, fixed one-/two-alias, and
  exactly-two-leg transitive custody before invoking affine completion; no
  generic path search or additional evidence shape is introduced. Direct
  affine-root custody now lives in independent side-local
  `affine_selection/direct` modules. Production alone retains the exact root-
  bound citation and tries its left then right value endpoints before
  constructing affine completion; reconstruction independently scans
  requirements then semantic axioms, tries the same endpoint order, and
  rechecks the retained root bound through affine custody. Direct evidence
  remains preferred before landed literals, one-alias transport, direct and
  alias-substituted two-leg transitivity, and two-alias transport. Citation and
  endpoint order, proof shapes, missing, redirected, or mistyped rejection, and
  every finite evidence frontier are unchanged.
  Source-ordered direct retained affine-bound candidates now live in paired,
  side-local `affine_selection/direct/candidates` modules. Producer and
  reconstruction independently enumerate requirements before semantic axioms,
  exact `LessOrEqual` rows, and left-before-right Value endpoints; only the
  producer retains citation custody. The direct custody completion, proof
  shape, rejection behavior, and fixed search frontier remain unchanged.
  Fixed affine root-alias
  completion now lives in independent side-local `affine_selection/alias`
  modules. Production alone adapts the existing origin-indexed one- and two-
  alias substitution proofs into affine completion; reconstruction
  independently adapts its reconstructed root bounds and rechecks affine
  custody. Direct, landed-literal, one-alias, direct-transitive, alias-
  transitive, then two-alias precedence is unchanged. Equality/citation order
  and distinctness, nested substitution shapes, missing, reused, cyclic, or
  mistyped rejection, and the explicit one-/two-alias frontier remain
  unchanged; no hop parameter or graph search is introduced. Exact two-
  citation affine-chain custody now lives in independent side-local
  `affine_selection/transitive/chains` modules. Production preserves citation
  identities while reconstruction independently retains propositions; each
  enumerates left facts in ledger order, indexes right legs by the exact shared
  value endpoint, and rejects reuse of the same fact before completion. Direct
  transitive affine custody and its fixed one-equality substitution remain
  separate consumers with unchanged precedence, endpoint orientation, proof
  shapes, and rejection behavior. The catalog exposes exactly two legs—no
  depth parameter, recursion, or generalized path search. One-equality
  transitive affine-root custody now lives in independent side-local
  `affine_selection/transitive/alias` modules. Production alone retains the
  equality citation and ordered two-leg citation identities, constructs one
  transitivity child and one endpoint substitution, then invokes affine
  completion; reconstruction independently rechecks the same distinct value
  alias, exact two-leg chain, substituted root bound, and affine custody.
  Direct transitive affine custody remains in each parent. Equality and chain
  order, endpoint precedence, proof shapes, missing, reused, redirected, or
  mistyped rejection, and the fixed two-citation/one-alias frontier are
  unchanged.
  Prior-evidence primitives now live in dedicated, side-local
  `integer_evidence` modules. Production alone owns citation indices and proof
  nodes; reconstruction independently resolves retained integer literals and
  replays closed order. Selectors depend on these leaf helpers without sharing
  authority, changing precedence, or expanding the search frontier. Canonical
  integer coordination now lives in dedicated, side-local `integer_selection`
  modules. Production independently builds the recursive
  Truth/conjunction/disjunction/order proof shape before the public entry
  applies the kernel check; reconstruction independently replays canonical
  proposition shape and fixed bound dispatch. Each preserves its prior
  precedence and finite evidence frontier. Primitive integer-order selection
  now lives in independent side-local `integer_selection/order` modules:
  production alone builds exact-citation, closed-strengthening, and exact
  two-citation transitivity proofs, while reconstruction independently checks
  its retained literal, closed-strengthening, and exact two-fact forms. Fixed
  endpoint substitution likewise lives in independent side-local
  `integer_selection/substitution` modules; each side owns its existing one-
  and two-equality completion without sharing authority. One-substitution
  inner-relation custody now lives in independent side-local
  `integer_selection/substitution/relation` modules. Production alone preserves
  exact or closed-strengthened prior relation, exact two-fact transitivity,
  affine custody, then eligible pure closed relation precedence;
  reconstruction independently rechecks its retained-fact, two-fact, and
  affine forms. Equality orientation, citation identity, endpoint selection,
  and the outer `IntegerLessOrEqualSubstitution` proof remain in each parent.
  The fixed two-equality affine sibling, proof shapes, rejection behavior, and
  finite search frontier are unchanged. Fixed two-equality endpoint
  substitution now lives in independent side-local
  `integer_selection/substitution/two` modules. Production alone retains the
  outer and inner equality citations, proves the final-alias affine relation,
  and nests inner then outer `IntegerLessOrEqualSubstitution` nodes on the
  unchanged endpoint; reconstruction independently rechecks the same three
  distinct same-carrier values and final affine relation. One-equality relation
  custody remains preferred in each parent. Equality order, citation
  identities, endpoint orientation, proof shape, missing, reused, redirected,
  mistyped, or cyclic rejection, and the exact two-equality frontier are
  unchanged; a third alias remains outside. Exact two-fact integer-order
  custody now lives in independent side-local
  `integer_selection/order/transitive` modules. Production alone retains the
  ordered left/right citation identities and constructs one
  `IntegerLessOrEqualTransitivity` proof; reconstruction independently rechecks
  the exact goal-left endpoint, shared middle value, and goal-right endpoint.
  Direct retained relations, closed strengthening, and landed-literal checks
  remain in each order parent. Citation order, proof shape, disconnected or
  missing-leg rejection, and the exact two-fact frontier are unchanged; no
  third leg or generalized path search is introduced. One-bridge closed
  integer-order custody likewise now lives in independent side-local
  `integer_selection/order/closed` modules. Production preserves retained
  citation order and constructs either retained-bound-then-closed-tail or
  closed-head-then-retained-bound transitivity; reconstruction independently
  checks the same endpoint and closed bridge. Exact retained relations remain
  preferred, while landed-literal and exact two-fact custody remain separate.
  Citation order, endpoint orientation, proof shape, nonclosed, mistyped, or
  weaker rejection, and the single-bridge frontier are unchanged. Canonical
  compound-proposition custody now lives in independent side-local
  `integer_selection/logical` modules. Production alone constructs conjunction
  children in source order and selects the first provable disjunction arm;
  reconstruction independently requires every member of a nonempty
  conjunction and accepts the first retained disjunct. Exact retained
  proposition precedence, recursive child dispatch, Truth and atomic-bound
  ownership, arm indices, proof shapes, incomplete or reordered rejection, and
  the finite evidence frontier are unchanged. Exact whole-proposition custody
  now lives in independent side-local `integer_selection/exact` modules.
  Production alone resolves the first exact assumption or semantic axiom in
  ledger order and constructs its origin-indexed citation proof;
  reconstruction independently checks the same requirements-before-semantic-
  axioms retained order. Exact custody remains preferred before Truth, atomic-
  bound dispatch, and compound recursion. Citation origin/index, proposition
  identity, precedence, proof shape, redirected or reordered rejection, and
  the finite evidence frontier are unchanged. Atomic integer-bound custody now
  lives in independent side-local `integer_selection/bound` modules.
  Production alone preserves exact/closed order, exact two-citation
  transitivity, fixed substitution, cast, then affine proof precedence;
  reconstruction independently preserves closed/direct-literal/two-fact,
  substitution, cast, then affine retained-evidence precedence. Exact whole-
  proposition citation remains preferred in each parent, while Truth and
  recursive compound coordination remain separate. Citation identities,
  endpoint orientation, proof shapes, rejection behavior, and every finite
  evidence frontier are unchanged. Canonical proposition-kind dispatch now
  lives in independent side-local `integer_selection/dispatch` modules. After
  exact whole-proposition custody, production alone routes Truth, atomic bounds,
  conjunctions, and ordered disjunctions to their existing proof owners;
  reconstruction independently routes atomic and compound retained-evidence
  checks. Recursive children return through each side's entry facade,
  preserving exact-first selection at every depth. Variant order, arm and
  conjunct order, proof shapes, unsupported-shape rejection, and every finite
  evidence frontier are unchanged. Recursive proposition
  coordination, precedence, ledger citation order, equality orientation,
  endpoint selection, proof shapes, rejection behavior, and the finite search
  frontier are unchanged. Certificate-entry custody now lives
  in dedicated, side-local `certificate_entry` modules. Production exposes a
  selected proof only after the kernel accepts its exact context, goal,
  assumptions, and semantic axioms; reconstruction independently projects the
  canonical scalar goal before retained selection. Invalid projection or
  failed checking yields no authority, and neither side imports the other's
  decision. The producer's 30 certificate regressions and reconstruction's 25
  independent selection regressions now live in side-local `tests` modules.
  Production facades are 35 and 608 lines respectively, while every test name
  and assertion is retained; no proof logic, authority, precedence, or search
  frontier moved between sides. Reconstruction control-flow evidence
  propagation now lives in a side-local `path_facts` module. It alone decodes
  retained condition predicates, binds successor parameters, emits edge
  equalities before rewritten facts, and deduplicates propagated facts. The
  reconstruction parent still owns traversal, merge intersection, and
  certificate selection; this extraction grants no proof authority and changes
  no fact order. Per-operation obligation reconstruction now lives in a
  side-local `operation_facts` module. It preserves the exact goal-free,
  proof-bearing, structural-effect, then call dispatch order; only the
  proof-bearing branch may choose canonical certificate custody or trusted
  sufficient reduction before recording the pre-result axiom snapshot. CFG
  traversal and return intersection remain in the parent, and an unclaimed
  validated operation still fails closed. Terminator custody now lives in a
  side-local `terminator_facts` module. It owns the exact
  Jump/Conditional/return/crash dispatch, successor fact propagation,
  scalar-result equality, nominal-cleanup obligations, structural-return facts,
  and the rule that Crash contributes no normal exit. CFG scheduling and final
  all-return intersection are separately owned below; cleanup order, axiom
  snapshots, and noncanonical cleanup status are unchanged. Immutable machine
  reconstruction context now lives in a side-local `machine_context` module.
  It alone derives the existing path-fact enablement predicate, exact
  value-type proposition context, machine-parameter custody set, and
  block/machine identity indexes. Traversal consumes that read-only context;
  operation and terminator modules retain their independent decision authority,
  and no dispatch, fact, proof, or search order changes. Deterministic machine
  fact flow now lives in a side-local `machine_flow` module. It owns the
  existing sorted-ready topological schedule, per-block all-incoming fact
  intersection, and final all-return fact intersection. The parent retains
  operation-before-terminator traversal; no successor, fact, exit, proof, or
  search order changes. Direct cast-root custody now lives in independent side-
  local `cast_selection/direct` modules. Production alone retains the exact
  root-bound citation and tries its left then right value endpoints before
  checked cast completion; reconstruction independently scans requirements
  then semantic axioms, tries the same endpoint order, and rechecks cast
  custody. Non-order goals still reject in each parent, and direct evidence
  remains preferred before landed-literal and fixed alias transport. Citation
  and endpoint order, cast proof shape, missing, redirected, or mistyped
  rejection, and every fixed cast evidence frontier are unchanged. Direct
  landed-literal cast completion now lives in independent side-local
  `cast_selection/literal/completion` modules. Each parent retains its own
  requirements-before-semantic-axioms equality discovery, orientation, and
  typed value/literal filtering; production alone remaps the source endpoint,
  constructs the closed relation and one substitution proof, then completes
  the cast, while reconstruction independently replays the same endpoint,
  closed-order, root-bound, and cast checks. Direct-root, landed-literal, then
  alias precedence, citation order, endpoint orientation, proof shape, unsafe,
  redirected, or mistyped rejection, and the fixed evidence frontier are
  unchanged. Exact integer-cast definition-spine selection now lives in
  independent side-local `cast_custody/chain` modules. Production and
  reconstruction each walk backward from the selected target through exactly
  one retained `IntegerExactCast` definition per value, reject ambiguous or
  reused definitions and failure to reach the exact root, then require the
  recovered semantic-axiom word to be source ordered. Production's cast-
  custody parent still owns target precedence, witness/proof construction, and
  full certificate checking; reconstruction independently owns witness and
  bound-conversion checking. Cast legality, continuity, carrier validation,
  target order, proof shape, rejection behavior, and the finite unique-spine
  frontier are unchanged; no alternate-path, permutation, or generic graph
  search is introduced. Exact integer-cast certificate completion now lives in
  independent side-local `cast_custody/completion` modules. Each consumes only
  its own deterministic exact-cast spine selection. Production alone preserves
  target-endpoint order, constructs the `IntegerCastChainWitness` and
  `IntegerCastBound` proof, and accepts it only after full kernel certificate
  checking; reconstruction independently checks its witness and mapped bound
  conversion. The cast-custody facades retain the existing entry points and
  literal remapping. Root-bound custody, witness indices, target order, proof
  shape, rejection behavior, and the finite unique-spine frontier are
  unchanged; no alternate-path, permutation, or generic graph search is
  introduced. Exact integer-literal carrier remapping now lives in independent
  side-local `cast_custody/literal` modules. Production and reconstruction each
  resolve the retained literal's exact source carrier and value, apply exact
  integer-cast semantics, and rebuild the target-carrier literal before their
  own direct, alias-landed, or stronger-bound cast completion proceeds. The
  cast-custody facade keeps the same private entry point, while chain selection
  and certificate completion remain separate. Candidate order, endpoint
  orientation, citation and proof shapes, failed or out-of-range conversion
  rejection, and every fixed cast-evidence frontier are unchanged; no generic
  literal or path search is introduced. Cast-
  specific alias transport now lives in independent
  side-local `alias_transport/cast` modules. Production alone constructs the
  closed-strengthening and alias-landed-literal substitution proofs before cast
  completion; reconstruction independently enumerates and rechecks the same
  retained facts. Generic one-/two-alias transport remains in each parent, and
  citation order, endpoint precedence, proof shapes, rejection behavior, and
  the finite search frontier are unchanged. Fixed-depth alias transport now
  lives in independent side-local `alias_transport/one` and
  `alias_transport/two` modules. Production alone retains origin-indexed
  equality and bound citations and constructs respectively one substitution or
  the exact inner-then-outer two-substitution proof; reconstruction
  independently scans and indexes retained facts and rebuilds the same root
  bounds without importing producer authority. The facade retains separate
  named one-/two-alias entry points and exposes no depth parameter, recursion,
  or graph search. Requirements-before-semantic-axioms order, equality
  orientation/distinctness, endpoint-index order, proof shapes, missing,
  reused, cyclic, or mistyped rejection, and both finite frontiers are
  unchanged. Fixed transitive affine evidence
  now lives in independent side-local `affine_selection/transitive` modules.
  Production alone constructs the exact two-citation transitivity proof and
  its optional single equality substitution before affine completion;
  reconstruction independently indexes and rechecks those same retained
  facts. Direct, literal-landed, one-alias, transitive, alias-transitive, then
  two-alias precedence, citation order, rejection behavior, proof shapes, and
  the finite search frontier are unchanged. Affine landed-literal custody now
  lives in independent side-local `affine_selection/literal` modules.
  Production alone constructs the closed reflexive order and one or two exact
  substitutions for direct and fixed one-intermediate-alias literal roots;
  reconstruction independently enumerates and rechecks the same retained
  equalities and typed literals. Direct-bound, literal, one-alias, transitive,
  alias-transitive, then two-alias precedence, citation and endpoint order,
  rejection behavior, proof shapes, and the finite search frontier are
  unchanged. Direct landed-literal affine-root custody now lives in independent
  side-local `affine_selection/literal/direct` modules. Production alone
  preserves exact equality citation origin and orientation, constructs the
  closed reflexive relation plus one endpoint substitution, and completes the
  affine proof; reconstruction independently scans requirements then semantic
  axioms and rechecks the same typed literal, root-bound orientations, and
  affine custody. Direct literal landing remains preferred before the fixed
  one-intermediate-alias sibling. Citation/order orientation, proof shape,
  unsafe, missing, redirected, or mistyped rejection, and both finite literal
  frontiers are unchanged.
  Source-ordered direct landed-literal affine candidates now live in paired,
  side-local `affine_selection/literal/direct/candidates` modules. Producer and
  reconstruction independently enumerate requirements before semantic axioms,
  both equality orientations, and the exact Value/integer carrier eligibility;
  only the producer materializes the retained equality citation. Completion,
  affine custody, proof shape, rejection, and the fixed search frontier remain
  unchanged.
  Direct landed-literal affine completion now lives in
  independent side-local `affine_selection/literal/direct/completion` modules.
  Each parent retains its own requirements-before-semantic-axioms equality
  discovery, citation and orientation order, value-root custody, and typed-
  literal filtering; production alone constructs the closed reflexive
  relation, one endpoint substitution, and affine proof, while reconstruction
  independently rechecks the same two root-bound orientations through affine
  custody. Direct literal landing remains preferred before the fixed one-
  intermediate-alias sibling. Proof shape, endpoint order, unsafe, missing,
  redirected, or mistyped rejection, and both finite literal frontiers are
  unchanged; no recursive alias or generic evidence search is introduced.
  Fixed one-intermediate-alias affine literal custody now lives in
  independent side-local `affine_selection/literal/alias` modules. Production
  alone retains the distinct root-alias and alias-literal citation identities
  and constructs the closed reflexive relation followed by two exact
  substitutions; reconstruction independently rechecks the same typed
  equalities and both root-bound orientations. Direct landed-literal custody
  remains preferred in each parent. Citation order, endpoint orientation,
  nested proof shape, reused, redirected, or mistyped rejection, and the
  single-intermediate-alias frontier are unchanged; no recursive alias search
  is introduced.
  Source-ordered one-alias landed-literal affine candidate catalogs now live in
  paired, side-local `affine_selection/literal/alias/candidates` modules.
  Producer and reconstruction independently index only integer-literal
  equality landings by their exact alias while retaining assumptions-before-
  semantic-axioms outer traversal, equality orientation, inner citation order,
  distinct same-carrier Value checks, and same-row rejection. Completion and
  affine custody remain unchanged; this removes repeated full inner-ledger
  scans without changing proof shape, rejection, or the fixed definition
  frontier.
  One-intermediate-alias affine literal completion now lives in
  independent side-local `affine_selection/literal/alias/completion` modules.
  Each parent retains its own outer-then-inner equality discovery, distinct
  citation/value custody, and typed literal filtering; production alone
  constructs the closed reflexive relation, inner alias substitution, outer
  root substitution, and affine proof, while reconstruction independently
  rechecks the same two root-bound orientations through affine custody. Direct
  landed-literal custody remains preferred. Equality/citation order and
  distinctness, endpoint orientation, nested proof shape, missing, reused,
  redirected, or mistyped rejection, and the fixed one-intermediate-alias
  frontier are unchanged; no recursive alias search is introduced.
  Source-ordered one-alias transitive affine candidate traversal now lives in
  paired, side-local `affine_selection/transitive/alias/candidates` modules.
  Producer and reconstruction independently enumerate assumptions before
  semantic axioms, both equality orientations, and the existing ordered exact
  two-citation chains; the producer alone materializes citation proofs, while
  reconstruction retains proposition references only. Completion, affine
  custody, proof shape, rejection, and the fixed non-recursive frontier remain
  unchanged.
  One-equality transitive affine completion now lives in independent side-local
  `affine_selection/transitive/alias/completion` modules. Each parent retains
  its own ledger-ordered equality discovery, distinct root/alias custody, and
  exact two-citation chain enumeration; production alone constructs the
  transitivity child, one endpoint substitution, and affine proof, while
  reconstruction independently maps the same root-bound endpoint and rechecks
  affine custody. Direct transitive affine custody remains preferred. Equality
  and chain order, endpoint orientation, citation identity, nested proof shape,
  missing, reused, redirected, or mistyped rejection, and the fixed two-
  citation/one-alias frontier are unchanged; no generalized path or alias
  search is introduced. Direct two-citation affine completion now lives in
  independent side-local `affine_selection/transitive/completion` modules.
  Each parent retains its own exact ordered `TwoCitationChains` enumeration and
  citation custody; production alone constructs the
  `IntegerLessOrEqualTransitivity` child, tries the left then right value root,
  and completes the affine proof, while reconstruction independently rebuilds
  the same retained root bound, endpoint order, and affine custody. The fixed
  one-equality alias sibling and all outer precedence remain unchanged.
  Citation identity/order, shared-middle continuity, proof shape, missing,
  reused, disconnected, or mistyped rejection, and the exact two-leg frontier
  are unchanged; no longer path, permutation, or generic graph search is
  introduced. A
  single exact prior value equality may also transport a completed affine bound
  from its checked target alias to the canonical goal endpoint. The producer
  replaces that one endpoint, constructs the bounded affine relation directly,
  and wraps it in `IntegerLessOrEqualSubstitution`; reconstruction repeats the
  same exact identity selection. A missing, redirected, crossed, or mistyped
  target equality rejects. The affine relation builder cannot recurse into
  another target alias, so this adds one wrapper only and no alias-chain search.
  One fixed sibling may instead carry a completed affine bound across exactly
  two distinct same-carrier target equalities. It nests two
  `IntegerLessOrEqualSubstitution` nodes outside `IntegerAffineBound`; missing,
  reused, redirected, cyclic, or mistyped equalities reject. The constructor
  builds the affine relation directly at the final alias and never recurses
  through the general order prover, so a third target alias remains outside the
  family.
  One bounded mixed root-custody sibling may instead compose exactly two prior
  order citations at an alias endpoint, transport that completed bound through
  exactly one retained value equality to the affine root, and then apply
  `IntegerAffineBound`. Its proof nests `IntegerLessOrEqualTransitivity` beneath
  `IntegerLessOrEqualSubstitution`; missing or disconnected order legs and
  absent or redirected equalities reject. The constructor calls the affine
  builder directly, so it cannot add another equality or order leg and does not
  introduce recursive path search. Three-or-more-alias and three-or-more-leg
  custody remain outside the producer. One fixed two-alias sibling may instead
  transport one directly cited bound to the affine root through exactly two
  distinct retained value equalities. Its proof nests two
  `IntegerLessOrEqualSubstitution` nodes beneath `IntegerAffineBound`; the root,
  middle alias, and bound alias must be distinct same-carrier values. A missing,
  reused, redirected, crossed, cyclic, or mistyped equality rejects. The
  constructor has no recursive alias walk, and a third alias remains outside
  the producer. One literal-ending sibling may land the affine root through
  exactly one intermediate value alias and one exact same-carrier literal
  equality. It proves a closed reflexive integer order, substitutes the alias,
  substitutes the root, and only then applies `IntegerAffineBound`. Missing,
  redirected, reused, or mistyped equalities reject, and a second value alias
  is not followed. This is another fixed two-substitution path, not a recursive
  alias search. A second non-serialized common checker now
  normalizes the contiguous pure
  fixed-integer cast spine used by the accepted one-cast and multi-cast
  sandwiches. It binds strictly ordered canonical semantic equalities to exact
  root/target SSA values, validates every adjacent partial 8/16/32/64
  `IntegerExactCast`, retains all selected indices and carriers, and computes
  their exact surviving root-range intersection. Identity, widening-shaped,
  address, non-native, reversed, stale, reordered, discontinuous, cyclic, and
  target-drifted words reject; narrowing and cross-sign edges claim only their
  representable intersection, never total or lossy conversion. The checker
  accepts no proof authority, does not establish machine-parameter custody or
  surrounding prefix/suffix algebra, and leaves heterogeneous widening/cast
  words separate. `IntegerCastBound` is the versioned integration for that
  core. One recursively checked root-bound child and one nonempty contiguous
  word of partial fixed-native exact-cast definitions map the same mathematical
  literal endpoint into the final carrier. The kernel rechecks the complete
  cast witness and conversion and records every selected definition in accepted
  premise closure. A non-order or wrong-root child, empty, stale, reordered,
  discontinuous, total/widening-shaped, or cyclic cast definitions,
  target/orientation drift, or a changed endpoint reject. Proof-bundle v18
  retains tag 13; the producer and reconstruction independently follow the
  unique exact-cast SSA definition spine backward from the goal, reject
  ambiguous target definitions, and require its source-ordered ledger word.
  They perform no recursive path or permutation search. Cast-chain custody now
  lives in dedicated, side-local `cast_custody` modules. Production and
  reconstruction independently own unique-spine selection, exact
  witness/kernel replay, and final `IntegerCastBound` completion; the broader
  evidence selectors retain their existing order and proof shapes. Cast
  evidence selection now lives in dedicated, side-local `cast_selection`
  modules. Production and reconstruction independently preserve direct-bound,
  landed-literal, fixed one-alias, closed-strengthening,
  alias-landed-literal, then fixed two-alias precedence; source-carrier literal
  remapping remains with cast custody. No proof shape or search frontier
  changes. Direct landed-literal cast custody now lives in independent
  side-local `cast_selection/literal` modules. Production alone constructs the
  closed source-carrier relation and exact equality substitution before
  `IntegerCastBound`; reconstruction independently remaps the target endpoint
  and rechecks the same typed literal landing. Existing direct-bound,
  direct-literal, one-alias, stronger-alias, alias-literal, then two-alias
  precedence, citation orientation, endpoint order, rejection behavior, and
  the finite frontier are unchanged. This completes contiguous cast-chain
  custody for exact divide/remainder goals. Fixed cast alias-family dispatch
  now lives in independent side-local `cast_selection/alias` modules.
  Production alone constructs one-alias, closed-strengthened alias,
  alias-landed-literal, then two-alias proofs before cast completion;
  reconstruction independently enumerates and rechecks those fixed families.
  Closed-strengthened and alias-landed-literal transport are further separated
  into paired `alias_transport/cast/stronger` and `cast/literal` modules. Each
  producer constructs only its exact closed bridge/substitution proof, while
  reconstruction independently enumerates and rechecks the same typed facts;
  the cast-alias parent is now a small facade over those authorities. Stronger
  cast-alias completion now lives in independent side-local
  `alias_transport/cast/stronger/completion` modules. Each parent retains its
  own ledger-ordered exact equality and bound candidate discovery; production
  alone remaps the typed source endpoint, constructs the one closed
  transitivity bridge and root substitution, then completes the cast proof,
  while reconstruction independently replays the same carrier, endpoint,
  bridge, root-bound, and cast checks. Equality/bound citation order,
  orientation precedence, proof shapes, redirected, mistyped, or nonclosed
  rejection, and the single-alias/single-bridge frontier are unchanged.
  Closed stronger alias-bound transport for exact casts now lives in
  independent side-local
  `alias_transport/cast/stronger/completion/bound` modules. Each completion
  parent retains exact goal/target projection, literal carrier remapping, and
  cast-custody completion, while its outer parent retains ledger-ordered
  equality and bound discovery. Production alone constructs the closed bridge,
  one `IntegerLessOrEqualTransitivity` child, and one endpoint substitution;
  reconstruction independently checks the same closed relation and rebuilds
  the same root-bound proposition. Citation order, endpoint orientation, nested
  proof shape, weaker, nonclosed, redirected, or mistyped rejection, and the
  fixed one-alias/one-bridge frontier are unchanged; no recursive strengthening
  or generic search is introduced.
  Direct landed-literal root-bound construction for exact casts now lives in
  independent side-local `cast_selection/literal/completion/bound` modules.
  Each completion parent retains exact goal/target precedence, literal carrier
  remapping, and cast-custody completion, while its outer parent retains
  requirements-before-semantic-axioms equality discovery. Production alone
  constructs the closed relation and one endpoint substitution;
  reconstruction independently checks the same closed relation and rebuilds
  the resulting root-bound proposition. Citation and endpoint order, proof
  shape, unsafe/missing/redirected/mistyped rejection, and the fixed direct-
  literal frontier are unchanged; no recursive alias or generic search is
  introduced.
  Direct retained root-bound completion for exact casts now lives in
  independent side-local `cast_selection/direct/completion` modules. Each
  parent retains requirements-before-semantic-axioms retained-order discovery
  and exact citation/proposition custody; production completion preserves
  left-then-right value-root order and applies its own citation proof before
  cast custody, while reconstruction independently preserves the same endpoint
  order and rechecks the retained proposition through cast custody. Direct
  evidence remains first in cast selection. Citation order, endpoint order,
  proof shape, non-order/non-value/missing/redirected rejection, and the fixed
  direct-root frontier are unchanged; no generic evidence search is introduced.
  Fixed two-alias exact-cast completion now lives in independent side-local
  `cast_selection/alias/two` modules. Each child adapts only its side's existing
  exact two-alias transport to cast custody: production retains origin-indexed
  equality/bound citations and the nested two-substitution proof, while
  reconstruction independently replays retained facts and the resulting root
  bound. The alias-family parents preserve direct one-alias, stronger-bound,
  landed-literal, then two-alias precedence. Citation identity/order, equality
  orientation/distinctness, proof shape, missing/reused/cyclic/redirected/
  mistyped rejection, and the exact two-alias frontier are unchanged; no third
  alias, depth parameter, recursion, or graph search is introduced.
  Fixed one-alias exact-cast completion now lives in independent side-local
  `cast_selection/alias/one` modules. Each child adapts only its side's existing
  exact one-alias transport to cast custody: production retains origin-indexed
  equality and bound citations plus the single endpoint-substitution proof,
  while reconstruction independently replays retained facts and the resulting
  root bound. The alias-family parent preserves one-alias, stronger-bound,
  landed-literal, then two-alias precedence. Citation order, equality
  orientation, endpoint order, proof shape, missing/redirected/mistyped
  rejection, and the exact one-alias frontier are unchanged; no depth
  parameter, recursion, or graph search is introduced.
  Fixed two-alias bound completion now lives in independent side-local
  `alias_transport/two/completion` modules shared only through each side's own
  private facade. The producer parent retains origin-indexed outer/inner
  equality citations, distinct same-carrier value custody, cycle/reuse
  rejection, and endpoint-indexed relation discovery, then its completion alone
  nests inner followed by outer `IntegerLessOrEqualSubstitution` nodes.
  Reconstruction independently retains equality order/distinctness and bound
  indexing, then substitutes the same exact endpoint before invoking its
  consumer. Cast and affine consumers, citation and endpoint order, nested proof
  shape, rejection behavior, and the exact two-alias frontier are unchanged;
  no third alias, recursion, or graph search is introduced.
  Fixed one-alias bound completion now lives in independent side-local
  `alias_transport/one/completion` modules. The producer parent retains origin-
  indexed equality and bound citations, same-carrier distinctness, equality
  orientation, and endpoint-indexed relation discovery, then its completion
  alone constructs the single `IntegerLessOrEqualSubstitution` node.
  Reconstruction independently retains equality and bound order and substitutes
  the same exact endpoint before invoking its consumer. Cast and affine
  consumers, citation and endpoint order, proof shape, missing/redirected/
  mistyped rejection, and the exact one-alias frontier are unchanged; no depth
  parameter, recursion, or graph search is introduced.
  Fixed-alias endpoint-bound indexing now lives in independent side-local
  `alias_transport/index/bounds` modules. Production preserves citation origins
  while scanning assumptions then semantic axioms; reconstruction independently
  scans requirements then semantic axioms. Each inserts a value's left endpoint
  before its distinct right endpoint, suppresses the duplicate reflexive right
  entry, preserves per-endpoint ledger order, and uses deterministic `BTreeMap`
  lookup. Value-identity custody and endpoint substitution remain separate in
  each index facade. One-/two-alias candidate order, citation identity, proof
  shapes, rejection behavior, and finite frontiers are unchanged; the index
  grants no proof authority or graph search.
  Direct retained bounds and direct landed literals remain earlier in each
  parent. Citation orientation, endpoint order, proof shapes, rejection
  behavior, and the finite two-alias frontier are unchanged. Alias-landed-
  literal cast completion now lives in independent side-local
  `alias_transport/cast/literal/completion` modules. Each parent retains its
  own ledger-ordered discovery of distinct root-alias and alias-literal
  equalities; production alone remaps the typed source endpoint, constructs the
  closed relation and nested alias-then-root substitutions, then completes the
  cast proof, while reconstruction independently replays the same carrier,
  endpoint, closed-order, root-bound, and cast checks. Equality citation order
  and distinctness, endpoint precedence, proof shapes, redirected, mistyped,
  or unsafe rejection, and the fixed two-equality frontier are unchanged.
  Landed-literal alias root-bound transport for exact casts now lives in
  independent side-local `alias_transport/cast/literal/completion/bound`
  modules. Each completion parent retains exact target precedence, literal
  carrier remapping, and cast-custody completion, while its outer parent retains
  ledger-ordered root and literal equality discovery. Production alone
  constructs the closed relation, inner literal-to-alias endpoint substitution,
  and outer alias-to-root substitution; reconstruction independently checks the
  same closed relation and rebuilds the resulting root-bound proposition.
  Citation order and distinctness, endpoint orientation, nested proof shape,
  unsafe, missing, reused, redirected, or mistyped rejection, and the fixed
  two-equality frontier are unchanged; no recursive alias or generic search is
  introduced.
  These slices do
  not promote either whole row: affine/cast,
  shift/cast, joins, and correlated results remain trusted-reducer work, and
  `fully-derived false` is unchanged. The root-bound child may now also come
  from exactly one retained same-carrier `root == literal` fact when that
  literal equals or strengthens the canonical bound endpoint. The producer
  remaps the endpoint into the source carrier, checks the closed bridge to the
  landed literal, substitutes the root endpoint once, then applies the cast
  rule; reconstruction independently selects the same exact equality and
  rechecks the bridge. Direct bounds remain preferred. Missing, redirected,
  mistyped, or weaker facts reject. One exact same-carrier `root == alias`
  citation may instead transport one directly cited canonical bound at that
  alias. Its fixed proof nests one `IntegerLessOrEqualSubstitution` under
  `IntegerCastBound`; reconstruction repeats the same exact equality/bound
  selection. Missing, redirected, cross-carrier, or weaker bounds reject.
  Production now routes this one-alias order transport for both cast and affine
  completion through one indexed constructor; reconstruction independently
  mirrors that constructor, so the family is no longer re-enumerated per
  completion rule. One
  closed source-carrier endpoint bridge may also strengthen the cited alias
  bound. Its fixed proof nests `IntegerLessOrEqualTransitivity` under the one
  substitution; exact alias bounds remain preferred. Production and
  reconstruction recheck the same bound, bridge, and equality. They do not
  search alternate bounds or aliases, and a weaker bridge rejects. One fixed
  sibling may instead land that alias through exactly one same-carrier
  `alias == literal` citation. It proves the closed canonical bridge,
  substitutes the alias, substitutes the root, then applies
  `IntegerCastBound`; production and reconstruction select the same two exact
  equalities. Missing, reused,
  redirected, mistyped, or weaker literals reject. One fixed two-alias sibling
  may instead transport one directly cited canonical bound through exactly two
  distinct same-carrier value equalities. It nests two
  `IntegerLessOrEqualSubstitution` nodes under `IntegerCastBound`; production
  and reconstruction independently enumerate that exact three-citation shape
  through their own local indexed constructor shared by cast and affine
  completion. Endpoint-indexed alias-bound custody now lives in independent
  side-local `alias_transport/index` modules. Production preserves ordered
  citation origin, proposition identity, and endpoint orientation for every
  retained bound; reconstruction independently catalogs only the retained
  proposition and endpoint it must recheck. The fixed one- and two-alias
  constructors consume those separate indexes without sharing authority.
  Ledger order, same-carrier identity checks, endpoint substitution,
  citation/proof shapes, rejection behavior, and the finite two-alias frontier
  are unchanged; no hop-count parameter or recursive search is introduced.
  Those fixed one-/two-alias constructors now live in dedicated,
  side-local `alias_transport` modules rather than the broader certificate and
  reconstruction engines. The cast-specific closed strengthening and
  alias-landed-literal shapes live beside them while retaining their distinct
  transitivity and substitution proofs. They prefer every one-alias family and
  perform no recursive or parameterized alias walk. Missing,
  reused, redirected, crossed, cyclic, mistyped, or weaker facts reject. A
  third alias, literal landing through two aliases, affine/cast, shift/cast,
  joins, and correlated results remain outside this sibling; neither complete
  exact row changes trust and `fully-derived false` remains.
  A third
  non-serialized common checker now normalizes the
  complete exact-shift core shared by direct, cast-adjacent, affine-adjacent,
  and divide/remainder-adjacent families. It binds a nonempty, strictly ordered
  word of canonical exact-left/right semantic equalities from one fixed-native
  SSA root to its target. Closed counts require no cited fact; every nonclosed
  count must be landed by an exact earlier canonical equality. Heterogeneous
  fixed-native count carriers are retained, and every mathematical count must
  be nonnegative and less than the value width. The checked form preserves the
  exact direction/count/index word rather than an unsound cumulative summary
  for mixed shifts. Unsupported carriers, nonexact operations, unlanded, late,
  reversed, mistyped, negative, or out-of-range counts, stale or reordered
  definitions, discontinuity, cycles, and target drift reject. This checker
  accepts no proof authority, establishes no root custody, and proves neither
  left-shift overflow safety nor a surrounding interval/preimage claim.
  A fourth non-serialized checker now binds the complete correlated
  forbidden-root family shared by exact divide and remainder. It independently
  replays both nonempty landed-literal affine branches, requires disjoint
  source-ordered definitions ending at the same direct signed fixed-native
  signature parameter with nonzero coefficients, and binds exact prior landing
  facts for nonclosed siblings. It reselects the tightest strict unary signature
  bounds after the definition boundary, requires their exact axiom identities,
  and solves the divisor's zero and `-1` lattice roots. The latter is forbidden
  only when the dividend evaluates to the carrier minimum at that same root.
  No forbidden root yields the canonical ordered two-bound conjunction; roots
  covering the complete interval yield falsehood; partial safety rejects.
  Stale definition, literal, or bound identity; correlation/order/type/root
  drift; constant collapse; one-sided bounds; and checked arithmetic failure
  reject. The result remains custody only: it accepts no proof authority and
  neither its bounds nor conclusion are certificate premises. The general
  affine-bound rule does not certify this two-branch lattice result; a dedicated
  certificate conversion for the checked correlated result remains. No trusted
  reducer proposition is imported as proof
  authority and no partial exact row migrated. No schema, reducer,
  semantic-operation, or other trust status is promoted; terminal `PSITERM\0`
  codec v18, proof-bundle v18, installation record v24, and `fully-derived
  false` remain unchanged.
  The bounded Gamma spike is complete. It canonical-decodes four exact current
  `PSITERM\0` v18 fixtures and audits a 54-row scalar ledger covering constants,
  Boolean not/equality, integer equality/order, bitwise operations, strict
  i8-to-i16 widening, partial i16-to-i8 exact cast, exact/wrapping shifts with
  independently typed counts, and the complete
  exact/wrapping/saturating add/subtract/multiply and divide/remainder cohorts,
  signed toward-zero division, `MIN / -1`, conditional
  equations, branch-local scope/invalidation, all-predecessor merge rejection
  and acceptance, exact call-clause enumeration/substitution, and strict
  justification ranks. A separate 3-row structural/effect ledger covers exact
  relevant-Boolean field custody, affine-local establishment and retirement,
  published port-service authority, the observable port-write effect, and the
  three distinct place-frontier policies. Matching/asymmetric/malformed cases
  agree between the Beta-written reference interpreter and the independent
  Python evaluator.
  The 1,983-byte fixture yields a 3,607-byte modeled ledger and 2,984-byte
  prospective certificate; the 695-byte structural/effect fixture yields a
  185-byte modeled ledger and 164-byte prospective certificate. A separate
  697-byte fixture canonical-decodes exact `CallUnit` and `BoundaryCall`
  custody, including qualified affine resources, structural requirements,
  claim transfer, completion receipt, and boundary identity. The assembled
  typed core is 4,982 lines / 198,971 bytes / 423 functions, with maximum
  source nesting 25. Its PSITERM-neutral byte cursor, checked
  `u8`/little-endian `u16`/`u32`, and exact low/high-half `u64` primitives are
  now a separately gated 109-line reusable layer, including exact unsigned
  `u64` order and an exact four-limb `u128` carrier. A separately gated
  592-line terminal-codec layer owns the exact current
  magic/format/vocabulary envelope
  plus canonical Boolean, optional and required full-width semantic-ID carriers,
  exact identity equality/order, and length-prefixed UTF-8 grammar, together
  with the complete Boolean/fixed-signed/
  fixed-unsigned/address scalar-type grammar and exact widths `1..=128`, plus
  exact signed/unsigned 128-bit integer-value payloads; it
  rejects header/scalar/type/value drift plus overlong, surrogate, out-of-range,
  isolated-continuation, and truncated encodings. Its separate v18 structural-
  leaf module additionally owns exact IEEE kind/format, byte-sequence carrier,
  full-width canonical paths with exact case segments, and atomic proposition
  tags `11`/`12`/`13`, including nonempty paths and canonical operand order. All
  three bounded decoders consume
  only the 302-line header/scalar/type/value subset; the structural-leaf module
  remains independently gated and outside their claimed semantics. Scalar
  declarations and boundary results now retain
  the complete decoded type grammar; the bounded operation rows still admit
  only Boolean/i8/i16. Integer-constant operations retain exact signed/unsigned
  128-bit payloads until that row policy selects and narrows signed i8. The
  bounded spike narrows identities to a zero high half only in explicit adapters
  after complete decoding;
  remaining recursive vocabulary and monomorphic type-specific results remain
  spike-owned. The
  bounded thirty-two-kind scalar leaf slice now resolves through five composed,
  exact-unique policy-cohort schema tables: each row owns result shape,
  denotation, goal, post-discharge fact, crash policy, fuel, and frontier
  behavior, while calls remain separate coverage/substitution algebra.
  Missing, duplicate, and altered table rows reject end to end without changing
  either canonical ledger. The generator's known-value environment now
  retains exact typed declarations rather than IDs alone: duplicate result
  identities, operand-type drift, duplicate declarations, join-parameter
  overlap, and call argument-type drift reject before row publication. The
  structurally owned `EstablishTrivialAffineLocal` and
  `BooleanStructuralField` plus effectful `PortWrite` now resolve through their
  own exact-unique schema table and separate decoder/evaluator modules rather
  than scalar-row permutations. Erased relevance, field/service/port drift,
  cleanup drift, establishment-target drift, and missing affine retirement all
  reject. The three call-composition definitions now live in their own
  exact-unique table and one generic axis checker rather than three more
  evaluator branches. Target/result custody, positional binder shape,
  requirement coverage, capture-free substitution, claim/receipt transfer,
  guarded outcomes, crash routes, evidence lifetime, fuel, and frontier policy
  remain independently visible. The canonical scalar call consumes its row end
  to end; canonical-byte Unit and boundary sites exercise the same checker.
  Missing,
  duplicate, cross-kind, weakened-evidence, wrong-requirement,
  weakened-frontier, signature, state-version, move/reborrow, coverage,
  substitution, outcome, crash, evidence-lifetime, raw identity, target,
  argument, transfer, receipt, truncation, and trailing-byte drift reject. The
  first Rust producer-modularity checkpoint is also complete. Structural Unit
  planning no longer owns the shared Boolean/integer convergence classifier
  body: that sufficient-form family and its forty focused tests live in
  dedicated `shared_convergence` modules. The six exact binary families and
  exact-cast family select their existing ordered recognizers through seven
  declarative registries and one generic dispatch path rather than repeated
  `or_else` permutations. The production parent shrank from 10,926 to 5,626
  lines; shared convergence is now a 493-line orchestration module plus four
  responsibility modules for cast chains, affine forms, products/divisors, and
  shifts/cross-family composition, none larger than 1,317 lines. The test
  parent shrank from 10,785 to 2,915 lines, and its forty classifier cases are
  separated into chain, affine-join, and nominal-cleanup modules, none larger
  than 3,411 lines. The remaining structural parent is now a 250-line
  orchestrator over return analysis, control/boundary construction, cleanup,
  call closure, and type/shape custody modules, none larger than 1,356 lines;
  its fifty-three tests are separated into cleanup and call-closure modules
  behind a 57-line test root. The downstream checked-to-terminal producer no
  longer embeds its 3,891-line shared runtime-parameter classifier in the
  23,735-line crate root. That classifier now has a 706-line orchestration and
  registry module over Boolean, conversion, affine, product/divisor, and
  shift/cross-family responsibilities, none larger than 1,083 lines. Six exact
  binary cohorts plus exact cast consume named ordered registries through two
  generic dispatchers rather than maintaining another set of repeated
  `or_else` permutations. Structural scalar-return custody is now separate as
  a 553-line lowering/orchestration module over a 258-line expression-shape
  responsibility and a 1,431-line nominal-cleanup specialization behind one
  parent-facing entry point. The distinct structural Unit control path is a
  601-line module. Structural Unit cleanup is a 733-line nominal
  lowering/orchestration module over separate 828-line ordered-nominal and
  352-line partial-affine responsibilities. Attached Unit closure assembly is
  an 826-line orchestrator over 132-line provider discovery, 203-line exact
  call closure, 487-line type/domain/service catalog, and 168-line parameter
  transfer responsibilities. Result-bearing boundary custody and general
  structural-result transfer are separate 393-line and 314-line modules over a
  shared 246-line structural-type retention responsibility. Scalar-graph
  terminal-module assembly is now a separate 1,061-line responsibility behind
  one parent-facing builder. Content conservation, identity reshuffling, and
  partition composition now form one 788-line lowering module with only three
  public APIs and two explicit internal contracts. The root-level regression
  corpus is now a separately compiled 334-line fixture/orchestration parent over
  isolated 767-line Unit-cleanup, 179-line scalar-graph, 506-line content-ledger,
  957-line structural-control, 457-line attached-Unit, and 852-line
  structural-return families instead of a second responsibility embedded in
  production. The 9,597-line `nominal_affine_source` regression file is now a
  31-line root over five focused responsibilities with all 33 tests retained;
  its remaining 6,238-line integer-comparison case is one atomic cross-layer
  mutation matrix, not a completed decomposition target. Proposition
  vocabulary, evidence-term identity, contract lanes,
  proof-output invocations, and producer provenance now form one 906-line evidence
  module behind a single lower-and-install API. Scalar and structural crash
  routes, checked crash-site/frontier custody, argument-root substitution, and
  canonical proposition construction now form one 1,727-line module with
  eleven explicit internal contracts.
  Terminal operation emission and proof finalization now form one 597-line
  module with five explicit entry points. Short-circuit Boolean decisions and
  terminal control emission now form one 734-line module, while replaceable
  debug-map presentation is a separate 188-line module. Scalar-graph
  preparation, validation, partial evaluation, and lowering now form one
  1,297-line module with fourteen explicit internal contracts. Reachable
  scalar-call discovery and multi-machine assembly now form one 158-line
  module with two explicit entry points. The crate root is now 1,017 lines.
  The verifier's former 9,239-line sufficient-form reconstruction test parent
  is now a 15-line root over fifteen cast, conversion, add/subtract,
  multiply/affine, join, shift, and divide-policy responsibilities. All 76
  cases remain, and no family module exceeds 1,248 lines.
  Terminal proof replay now has the same production boundary. Its former
  2,233-line root is a 256-line public verification orchestrator over a
  44-line canonical proof-bundle model, 1,036-line executable-site/path-fact
  reconstruction, exact evidence-producer provenance (139), integer arithmetic
  foundations (337), and proposition/value/place substitution (494). The
  existing public proof and substitution surfaces remain explicitly
  re-exported, while sufficient-form selection retains its specialized owners
  instead of being merged into one generic permutation dispatcher.
  Exact-shift reduction now follows that same boundary. Its former 2,376-line
  production file is a 237-line precedence/orchestration parent over a
  944-line direct-chain/foundation responsibility and a 1,254-line
  cross-family cast/arithmetic/divide composition responsibility. The existing
  public reducer surface and precedence are unchanged, and the integer-shift
  trust node binds all three exact source files.
  Exact conversion reduction now has the same split. Its former 2,219-line
  file is a 243-line cast-precedence/direct-fallback parent over a 977-line
  conversion-chain and interval-foundation responsibility and a 1,063-line
  divide/product/affine/offset composition responsibility. Existing reducer
  contracts and ordering remain unchanged, and the integer-conversion trust
  node binds every implementation source.
  The checked-lowering regression file that had accumulated ranking,
  operational-contract, write-frame, crash-route, and data-fact verification
  is now a 23-line root over eight exact test families. All 67 tests and the
  shared exact-symbol helper remain, and no family exceeds 3,614 lines.
  Checked-tree visualization has also separated view production from its
  regression corpus: the former 11,465-line file is now a 5,092-line
  production module with a 609-line shared fixture parent over eleven exact
  behavior, content, qualification, carry, and machine-contract test families.
  All 188 embedded tests and 215 test/helper functions remain, and no family
  module exceeds 1,043 lines.
  The checked interpreter now follows the same responsibility boundary. Its
  former 9,938-line evaluator is a 1,205-line state/model parent over separate
  execution, statement/call, wire-codec, host-dispatch, filesystem, console,
  expression/value-call, name/place, cast/recast, record-view, type-metadata,
  scalar-operation, and typed-program-lookup modules. The complete function
  and declaration inventories remain; cross-responsibility collaboration is
  narrowly exposed through `pub(super)`, local helpers remain private, and no
  child responsibility exceeds 1,408 lines. This is a semantics-preserving
  split; exact host-service grant custody remains a separate authority task
  rather than being hidden inside the refactor.
  Target-neutral call validation has begun the same responsibility split. Its
  former 9,103-line `calls.rs` parent is now 1,065 lines. Its 137-line
  `calls/inline_assembly.rs` child owns shared-catalog lookup, source operand
  constraints, and value-producing intrinsic destination checks; the existing
  crate destination query and two parent-private validation seams are
  unchanged. A 112-line `calls/generic_bounds.rs` child owns bodyless-boundary
  executable admission and positional machine type-parameter bound checking.
  A separate 53-line `calls/result_use.rs` leaf owns strict non-Unit result
  consumption and the proof-citation exemption. Both expose one parent-private
  validation entry point and preserve existing diagnostics. Runtime
  recursive-call position checking lives in a 222-line `calls/recursion.rs`
  child; its
  943-line `recursion/proof_machines.rs` child owns proof-machine
  structural/cited decrease validation, substitution matching, guard
  provenance, and sub-state descent closure. The proof validator is its only
  crate-visible export and reuses one parent-private self-call identity check.
  Value-position per-call bound validation and exact diagnostics form a
  748-line `calls/expression_scanning.rs` child. Its 838-line
  `expression_scanning/traversal.rs` child owns source-ordered recursive
  statement/expression scanning, malformed-name checks, and nested-indexed-read
  fences, delegating through one parent-private call-validation seam. A
  separate 222-line `expression_scanning/target_resolution.rs` child owns
  declared-receiver type discovery, lowering-aligned target-channel replay,
  and the fail-closed unresolved-call decision. Existing crate queries are
  unchanged; only type-shell normalization and unresolved-call reporting are
  shared privately back to per-call validation. A separate 273-line
  `expression_scanning/result_realization.rs` child owns the fail-closed
  runtime-result fences for LET-local receivers, nested unmaterialized machine
  calls, and void callees in value position. It exposes the same two
  crate-visible diagnostics plus one parent-private void-callee check; target
  selection, argument validation, diagnostic text, and source order are
  unchanged.
  Complete-or-opaque caller write-frame inference, alias-origin propagation,
  and transition-cycle frame equations now form a 2,921-line
  `calls/write_frames.rs` child. Its 459-line `write_frames/demand.rs` child
  owns the public resolver facade plus expression/statement demand collection
  and conservative fallback; a separate 123-line
  `write_frames/boundary_calls.rs` child owns boundary-trait signature
  resolution and receiver/exclusive-argument write frames. A focused 214-line
  `write_frames/isolation.rs` child owns caller-isolated local/aggregate
  classification, exact struct-literal field/type lookup, and bounded direct-call
  initializer-shape admission through six parent-private predicates; it has no
  callback into frame inference. A separate 52-line
  `write_frames/isolated_initializers.rs` leaf owns complete caller-isolated
  initializer admission, including the symbol-table and isolated-write fences;
  recursive frame collection remains in the parent behind one callback. A
  separate 99-line `write_frames/transparent_effects.rs` leaf owns recursive
  syntactic effect classification, compiler-owned slice-view transparency, and
  place-root symbol recovery through three parent-private queries, likewise
  without resolving or summarizing a call frame. A 72-line
  `write_frames/place_paths.rs` leaf owns exact-versus-collection-coarse frame
  path provenance, root/suffix composition, and typed-expression path recovery;
  collection coarsening remains absorbing and the leaf has no call-resolution
  dependency. An 87-line `write_frames/state_paths.rs` leaf owns state-relative
  visibility, positional parameter-root normalization, exact symbol forwarding,
  and duplicate-free visible-path collection; it has no call- or
  frame-resolution callback. A 50-line `write_frames/type_capabilities.rs` leaf
  owns constrained-reference recognition and the type/parameter classification
  for carrying caller-visible writes, with no expression traversal or
  resolution callback. A 243-line `write_frames/local_aliases.rs` leaf owns
  canonical local-alias path rebasing, direct-place resolution through already
  established stable origins, syntactic mutable-reborrow detection for stable
  parameter/local bindings, and read-only reference-shaped replacement
  classification; it neither recursively infers origins, mutates bindings, nor
  resolves frames. A separate 59-line `write_frames/alias_bindings.rs` leaf
  owns exact stable-local rebinding admission and slot mutation through one
  immutable origin-inference callback; recursive origin analysis remains in
  the parent. A 114-line `write_frames/parameter_aliases.rs` leaf owns the narrow
  parameter-relative origin carrier, exact symbol/name alias lookup, and
  syntax-only transparent mutable-reborrow detection; recursive origin and call
  analysis remain in the parent. A 125-line
  `write_frames/transition_topology.rs` leaf owns named-edge target resolution
  and acyclicity checking within one machine plus exact write-capable namespace
  preservation for cycle-closing edges, without constructing or solving frame
  equations. A 91-line `write_frames/transition_equations.rs` leaf owns
  the private equation/edge carriers, exact named-edge capture, and read-only
  equation-graph reachability; construction, permutation validation, and
  fixed-point solving remain in the parent. A 61-line
  `write_frames/assignment_targets.rs` leaf owns
  declared target-type lookup and structural/effectful assignment-place shape
  classification; it depends only on typed-place and syntactic-effect queries,
  not alias mutation or frame resolution. An 80-line
  `write_frames/call_targets.rs` leaf owns free-machine entry selection, exact
  state-symbol lookup, and the fail-closed concrete discarded-result shape
  query; the established crate/calls visibility surface is re-exported
  unchanged, and the leaf performs no write-frame inference. An
  81-line `write_frames/path_instantiation.rs` leaf owns receiver/parameter/local
  substitution for relative write paths and preserves exact versus
  collection-coarse origins; its only callback is the existing parent-private
  actual-argument origin query. The parent
  preserves the existing public and crate-private query surface;
  receiver-member-chain and resolved-state lookup remain the only top-level
  sibling seams. The frame engine privately reuses two demand collection
  helpers, while the boundary child exposes only its two existing queries and
  one engine-internal parts helper; every other decrease, citation,
  provenance, expression-walk, frame-equation, and diagnostic helper remains
  private to its owner. Validation order, the 141-function inventory, and
  public API are unchanged.
  Profiling the differential corpus also ruled out a wholesale Arena-to-
  `PagedArena` migration as a concurrency fix: `PagedArena` provides stable
  paged storage, not concurrent mutation, and the existing sound parallel
  pattern remains worker-local `Arena`s followed by deterministic ordered
  merge. Checked lowering now builds one call-frame/incoming-guard index for
  all range, contract, crash, and multiplicity consumers, reducing the
  helper-rich Mandelbrot canary's checked phase from about 15.6s to 9.5s.
  Backend state-value planning now builds the separate exact value-call
  dependency closure required before pruning. Runtime-flow and required-call
  states seed canonical `(machine symbol, state symbol)` identities; the index
  transitively visits local initializers, transition guards and values,
  terminal expressions, and nested call receivers and arguments through the
  simplifier's shared symbol-based resolver. Known-state resolution ambiguity
  conservatively retains the full program. Collection omits only states outside
  that closure: `StateValueUse.required` remains an independent emission fact,
  and the `runtime_nested_named_conversion_alias_exit` regression still exits
  70 with its off-flow nested value-machine expansion retained.
  On the exact warmed Mandelbrot stress canary, state-value planning fell from
  the documented 27.9s to 2.062ms while state storage took 1.360ms and the
  complete backend plan took 30.451ms. A full artifact-producing compile took
  7.470s, of which checked lowering was 7.207s; the exact one-canary
  interpreter/native differential run took 21.29s end to end, down from the
  documented 69.4s profile, with identical output and exit status. The
  dependency slice preserves worker-local `Arena`s and deterministic ordered
  merge; neither `PagedArena` nor cross-worker mutation is involved. The old
  hotspot remains useful history: a 10-second sampled profile landed in the
  `simplify_call_expression` / `helper_state_model` recursion; its two hottest
  leaf stacks are source-provenance `Arc` clone and drop (2,639 and 2,629
  samples), reflecting repeated reconstruction of expression trees and their
  identifiers. Prefer memoized normalized helper models or an indexed
  expression recipe over changing the backing arena. A linear structural
  helper-model cache was prototyped and rejected: on the exact warmed
  `text/runtime_mandelbrot_render_exit` differential canary, disabling it took
  60.21s wall/385.84s aggregate CPU while enabling it took 75.44s wall/76.77s
  CPU. It removed duplicated parallel work but moved the critical path onto one
  worker doing linear cache scans, increasing latency by 25%. No cache code was
  retained; the next attempt needs an indexed canonical key/recipe.
  Two later sampled Stage-05 fixes keep that rule. Default-domain analysis now
  builds one invocation-local call-frame resolver instead of reconstructing it
  for every fixpoint state visit, reducing the warmed checked phase from 6.754s
  to 5.338–5.386s. Named result-overload resolution now builds one source-
  ordered machine-family index keyed by exact normalized path and parameters,
  with direct entry-symbol lookup; operator and trait semantics are unchanged
  and the index is not retained. On the same canary that reduced Stage 05 again
  to 4.743–4.817s and full compile wall time to 5.51–5.64s.
  Checked-fact loop-invariant analysis now also reuses that pass's existing
  immutable call-frame resolver instead of rebuilding it below indexed-access
  checking. On a fresh exact profile this removed the former 1,056-sample
  construction stack and reduced warmed Mandelbrot Stage 05 from 4.810s to
  3.421s, with byte-identical output and native/interpreter exit 70.
  A subsequent eager complete-state write-summary prototype was rejected rather
  than retained: against a fresh 3.505s warmed baseline it moved 170 samples
  into resolver construction while targeting a 130-sample recursive summary
  stack, and regressed Stage 05 to 3.606–3.648s. Output remained byte-identical,
  but the remaining cyclic summaries need a genuinely lazy memo or SCC/fixpoint
  design; no cache code remains.
  A fresh output-only profile of
  `expressions/runtime_numeric_cast_exit` confirms that its 4.4s warm compile
  is likewise semantic work rather than viewer generation: Stage 05 consumed
  3.700s, split across typed validation (1.328s), checked-fact construction
  (1.201s), and checked-fact replay (1.031s), while normalization,
  specialization, overload resolution, and terminal cleanup totaled about
  140ms. The exact canary still exits 70. This profile gives no support to a
  shared mutable `PagedArena`; the next useful optimization boundary is an
  indexed or worker-local/deterministically merged implementation inside those
  three measured semantic phases. A subsequent exact native stack sample of
  `runtime_float_operations_exit` placed all 375 validation samples in default-
  domain analysis, including 150 in repeated state/call-frame summary
  recursion. The immutable call-frame resolver now uses a genuinely lazy memo:
  it caches only a statement call's complete-or-opaque normalized frame under
  its exact owning-machine and program-node identity, behind a concurrency-safe
  lock. Default-domain samples fell to 152 and the warmed exact canary moved
  from 4.75s to 4.08–4.12s while retaining exit 70. No eager whole-program
  summary or shared mutable arena was introduced.
  A follow-up lazy exact-state cache was profiled and rejected rather than
  retained. A fresh three-second sample placed 219 stacks in one demanded
  `inferred_state_write_frame`, including 100 below recursive state
  summarization, but the exact warmed float canary moved from 4.13s to 4.21s:
  the cost was one expensive state per resolver, not repeated identical state
  queries. No state-cache code remains. The next optimization must reduce or
  incrementally solve that demanded recursive summary itself. Recursive
  statement-call inference now shares one invocation-local completed-state
  summary set instead of discarding it at every resolved call boundary. The
  same exact sample reduced the demanded state-frame stack from 219 to 180 and
  recursive summarization from 100 to 80; warmed canary runs moved from 4.13s
  to 3.93–3.98s with unchanged exit 70. The memo remains local to one demand,
  carries exact state identities and complete relative paths, and introduces no
  eager whole-program solve or shared mutable state.
  The measured nominal affine integer-comparison reconstruction hotspot now
  uses independent producer- and verifier-local affine-definition indexes.
  Each immutable invocation maps an exact current Value term to source-ordered
  semantic equality rows that can extend the fixed add/subtract/multiply
  definition frontier; candidate prefixes and completed proofs are still
  independently replayed by the proof kernel, so four-definition depth,
  citation precedence, proof shapes, rejection, and the producer/verifier trust
  boundary are unchanged. The exact mixed nominal regression fell from
  approximately 306s to 27.50s test-body time (29.29s wall; 476,823,552-byte
  maximum resident set), while the exact mixed-shift regression fell from
  92.78s to 6.08s test-body time (6.76s wall; 421,036,032-byte maximum resident
  set). No persistent cache or generalized search was introduced. Affine-
  definition candidate indexing now lives in paired, side-local
  `affine_custody/definition_index` modules. Producer and reconstruction
  independently own their immutable source-order Value-to-definition indexes,
  while frontier modules only enumerate the existing four-definition words and
  replay each candidate through the proof kernel. This responsibility split
  changes no citation order, proof shape, rejection, or search frontier. The
  complete checked-to-Terminal package suite consequently fell from 401.56s to
  35.68s wall while all tests remained enabled and green.
  Syntactic affine-definition discovery now lives in paired, side-local
  `affine_custody/definition_index/candidates` modules. Producer and
  reconstruction independently retain semantic-row order, both equality
  orientations, exact Value-target eligibility, and add/multiply left-before-
  right versus subtract-left input projection; the invocation-local
  `DefinitionIndex` remains responsible for ordered per-input insertion and
  adjacent-row deduplication. Proof shape, rejection behavior, and the fixed
  frontier are unchanged.
  Exact affine relaxation mapping now lives in paired, side-local
  `affine_custody/relaxation/mapping` modules. Producer and reconstruction
  independently derive the mapped literal endpoint, carrier, sign reversal,
  and overflow-checked coefficient/offset image. The producer parent retains
  affine-proof construction and the closed transitivity bridge;
  reconstruction independently rechecks kernel conversion and closed-order
  relaxation. Candidate order, proof shape, rejection, and the fixed affine
  frontier are unchanged.
  Exact affine-root endpoint custody now lives in paired, side-local
  `affine_custody/relaxation/mapping/endpoint` modules. Producer and
  reconstruction independently require a retained `LessOrEqual` row and
  preserve left-root-before-right-root selection while returning the same bound
  endpoint and lower-versus-upper orientation. Signed carrier validation,
  checked affine mapping, sign-directed target orientation, proof shape,
  rejection, and the fixed frontier remain unchanged.
  Checked affine scalar mapping now lives in paired, side-local
  `affine_custody/relaxation/mapping/value` modules. Producer and reconstruction
  independently require an exact signed-integer endpoint of the affine carrier,
  apply checked coefficient multiplication and offset addition, and reject
  overflow or an unrepresentable mapped scalar. Endpoint custody, sign-directed
  target orientation, proof shape, rejection behavior, and the fixed frontier
  remain unchanged.
  Sign-directed mapped affine-bound orientation now lives in paired, side-local
  `affine_custody/relaxation/mapping/orientation` modules. Producer and
  reconstruction independently reverse lower-versus-upper direction only for a
  negative coefficient and preserve the exact target-versus-mapped endpoint
  placement. Endpoint custody, checked scalar mapping, proof shape, rejection
  behavior, and the fixed frontier remain unchanged.
  Direct retained affine-bound custody handoff now lives in paired, side-local
  `affine_selection/direct/completion` modules. The producer independently
  converts the selected origin-indexed citation into its exact proof before
  affine custody, while reconstruction independently passes the retained
  proposition into its own custody replay. Parent selectors retain assumptions-
  before-semantic-axioms traversal and left-before-right value endpoints, so
  citation order, proof shape, rejection, and the fixed definition frontier are
  unchanged.
  Closed affine-relaxation completion now lives in paired, side-local
  `affine_custody/relaxation/completion` modules. After their independent mapped-
  endpoint derivations, the producer child constructs the exact closed-order
  bridge and `IntegerLessOrEqualTransitivity` proof, while reconstruction
  independently checks the same endpoint alignment and closed relation.
  Mapping, kernel affine conversion, citation order, proof shape, rejection,
  and the fixed affine frontier are unchanged.
  Per-witness affine custody completion now lives in paired, side-local
  `affine_custody/completion` modules. The producer independently constructs and
  kernel-checks the direct `IntegerAffineBound` proof before its existing
  relaxed fallback; reconstruction independently normalizes the same enumerated
  witness and checks direct conversion before its own relaxation replay. Parent
  custody retains exact goal-endpoint and source-ordered definition-word
  enumeration, so precedence, proof shape, rejection, and the four-definition
  frontier are unchanged.
  The two ordered landed-literal alias root bounds now live in paired, side-
  local `affine_selection/literal/alias/completion/bound` modules. The producer
  independently constructs the exact closed reflexive relation and nested
  inner-alias then outer-root substitutions for endpoint 1 before endpoint 0;
  reconstruction independently rebuilds the same two root-bound propositions.
  Completion parents retain affine custody, so equality order, proof shape,
  rejection, and the fixed definition frontier are unchanged.
  Direct two-citation affine root-bound construction now lives in paired, side-
  local `affine_selection/transitive/completion/bound` modules. The producer
  independently constructs the exact `IntegerLessOrEqualTransitivity` node from
  the ordered citations, while reconstruction independently rebuilds the same
  `left <= right` proposition. Completion parents retain left-then-right value-
  root traversal and affine custody, so citation order, proof shape, rejection,
  and the fixed definition frontier are unchanged.
  Alias-substituted transitive affine root-bound construction now lives in
  paired, side-local `affine_selection/transitive/alias/completion/bound`
  modules. The producer independently constructs the ordered two-citation
  transitivity proof and substitutes endpoint 0 or 1 from the exact alias
  equality; reconstruction independently selects and rebuilds the same
  resulting root-bound proposition. Completion parents retain affine custody,
  so citation/equality order, proof shape, rejection, and the fixed definition
  frontier are unchanged.
  The two ordered direct landed-literal affine root bounds now live in paired,
  side-local `affine_selection/literal/direct/completion/bound` modules. The
  producer independently constructs the closed reflexive relation and endpoint
  substitution for endpoint 1 before endpoint 0; reconstruction independently
  rebuilds the same two root-bound propositions. Completion parents retain
  affine custody, so equality order, proof shape, rejection, and the fixed
  definition frontier are unchanged.
  Corpus-level bounded parallelism is viable at the harness boundary: the
  differential runner now defaults to four independent jobs with one native
  backend worker each, retains deterministic corpus-order reporting, and
  exposes `DIFF_JOBS`, `DIFF_LIMIT`, and exact `DIFF_CANARY` controls. On this
  14-core host the first eight canaries fell from 8.30s to 4.00s; a 32-canary
  concurrency probe passed completely, while eight outer jobs improved only
  101s to 97s over four and therefore is not the default. The native leg now
  selects output-only artifact emission because it consumes only the certified
  executable, not pipeline viewers or diagnostic reports; the same cached
  eight-canary probe fell again from 4.00s to 2.86s with all pairs matching.
  Semantic validation, trust policy, and final-footprint certification remain
  enabled. The disposable `omega-run` probe now follows that same policy while
  preserving full reports under explicit `--keep`. On the exact warmed
  Mandelbrot compile, suppressing reports that were immediately deleted reduced
  median wall/CPU from 4.36s/4.43s to 4.05s/4.12s and retired instructions from
  73.4B to 67.6B; a small unary-entry probe fell from 0.05s/654M instructions to
  0.02s/292M. `--keep` still produced the complete 35-file, 1.8 MiB inspection
  directory, and native/interpreter results remained identical. The native leg
  uses each original source's authored target-owned `ProgramEntry` when present
  and the bounded legacy `Main::main` seam only for the remaining unrooted
  corpus; the former generated target wrapper discarded value-returning entry
  codes and produced false mismatches.
  The compiler pass/fail corpus umbrellas now use the same deterministic outer
  scheduler for checked-only, cross-target, rooted-target, Windows-host, and
  active backend members instead of leaving the large backend registries
  serial. The first measured version used two inner workers, with
  `OMEGA_CANARY_JOBS` and `OMEGA_CANARY_INNER_WORKERS` retained as explicit
  profiling controls. On an eight-program heavyweight probe, four outer jobs
  and two inner workers reduced wall time from the one-outer/one-inner 65.42s
  to 31.12s. The dominating float total-order canary fell from 58.61s with one
  inner worker to 29.63s with two; four and fourteen inner workers provided no
  material wall improvement (29.11s and 29.28s) while increasing aggregate CPU
  from 78.29s to 112.16s and 226.53s. This measured ceiling is why the harness
  does not inherit unrestricted host parallelism inside every outer job. After
  repairing the corpus drift this broader gate exposed, the complete active
  pass umbrella finishes in 234.92s and the complete active fail umbrella in
  21.05s on the same host; both collect the whole registry rather than stopping
  at the first failure. Dedicated native-canary helpers initially used the same
  two-worker ceiling instead of multiplying Rust test-thread concurrency by a
  host-wide compiler pool. The exact float total-order test fell from 128.30s
  wall/998.98s aggregate CPU with fourteen inner workers to 91.83s/256.84s with
  two. A later fixed 112-compile mixed-corpus profile established the stronger
  harness boundary now in production: eight independent outer jobs with one
  inner worker completed in 13.00s, versus 21.26s for four outer jobs with two
  inner workers; twelve outer jobs were no faster and consumed more memory.
  Corpus compiles therefore default to outer eight / inner one, retain both
  environment overrides, and leave production compiler defaults unchanged.
  The compiler canary integration suite is no longer a 48,301-line permutation
  file. Its shared compile helpers, exact corpus registries, and umbrella
  orchestration now form a 3,277-line root over twenty-one responsibility
  modules for target artifacts, reports, content, ranges, arithmetic, providers,
  calls, ABI, proofs, layouts, and runtime families. All 1,241 tests and 1,272
  functions remain; the sole cross-family float differential helper is imported
  explicitly, and no family module exceeds 3,795 lines.
  Artifact presentation has begun the same responsibility split. The
  2,963-line `omega-artifacts` root retains artifact carriers and general
  writing orchestration. A focused 296-line `wire_report.rs` child owns only
  the stable wire-protocol text projection and its field/case/verdict
  formatters, while a separate 197-line `timing_report.rs` child owns phase
  timing/allocation aggregation, table layout, and numeric presentation. The
  public `ArtifactWriter` methods, exact outputs, and 79-function inventory are
  unchanged.
  Development and test profiles now both omit full DWARF by default, with an
  explicit `CARGO_PROFILE_{DEV,TEST}_DEBUG=2` escape hatch for debugger
  sessions. On the same macOS host, rebuilding the development CLI after the
  profile change reduced the executable from 140,687,560 to 118,904,896 bytes;
  the semantic canaries remained 0.01-second work once compiled. This reduces
  codegen/link and artifact-I/O pressure without weakening compiler diagnostics
  or semantic validation.
  A later apparent Rust frontend regression was build-cache accumulation, not
  compiler execution: `target/debug/deps` had grown to 1,359,819 entries and a
  rustc sample spent its startup in `SearchPath::new`/`readdir` before parsing
  the crate. `cargo clean` removed 1,483,970 derived files (195.1 GiB). The same
  proof-codec target then fell from 58.8s incremental to 2.86s cold and 0.68s
  after touching `psi-core`. Treat a uniform per-crate pre-parse delay as Cargo
  cache hygiene first; it is not evidence for Arena concurrency, test sharding,
  linker changes, or disabled semantic gates.
  The real-source terminal-Psi differential suite now applies the same boundary:
  its former 10,520-line file is an 852-line artifact/native execution harness
  over ten contract, call/control, exact-arithmetic, scalar-operation, and
  crash/admission families. All 115 tests and 137 functions remain, and no
  family exceeds 2,030 lines.
  Terminal native machine emission has undergone the same split: its
  12,922-line crate root is now an 891-line production orchestrator with the
  complete 58-case, 5,028-line regression corpus compiled separately.
  Unit-body and calling-policy emission, per-target parameter homes, aggregate
  argument staging/copying, and Unit stack/fuel/effect evidence form a separate
  1,301-line responsibility. Scalar-return and Boolean-control cleanup,
  nominal-cleanup admission, exact residual partitioning, and cleanup
  stack/fuel/call evidence form a separate 1,120-line responsibility. Scalar
  control/expression emission now has a 31-line orchestration/re-export root
  over distinct 1,861-line x86-64 encoding, 1,775-line AArch64 encoding, and
  1,067-line shared conditional-shape/stack-evidence responsibilities. All
  eighty-five implementation functions retain their exact architecture or
  shared owner, and the parent-facing surface remains explicit rather than
  becoming one permutation dispatcher.
  Terminal-module validation has begun the same split: its parent shrank from
  7,498 to 282 lines, with structural/service foundation (956 lines),
  structural/boundary operation custody (822), public error vocabulary (803),
  structural ownership/frontier cleanup (750), per-machine
  registration/orchestration (716), scalar crash/frontier and Boolean-predicate
  custody (674),
  content-conservation validation/replay (534), operation operand/type custody
  (522), partial/nominal affine cleanup custody (473), evidence/proposition
  custody (410), control-flow/dominance validation (301), proposition-root
  projection (146), contract proposition scope (120), and call-graph acyclicity
  (68) in separate responsibilities. Public validation types remain
  re-exported from the crate boundary.
  Final-image validation has begun the same responsibility split: its parent is
  down from 22,945 to 712 lines. Its regression corpus is a 25-line root over
  separate 701-line final-validation, 1,197-line place-replay, and 1,037-line
  guard/assembly families instead of a second responsibility embedded in
  production. Imported-call replay now has a 1,335-line parent, while table and
  vtable indirect calls form a separate 549-line responsibility. Runtime
  byte/line/text-boundary replay is a separate 504-line responsibility,
  and syscall replay plus exact relocation-target derivation is a separate
  507-line responsibility. The contract-entailment owner has begun a matching
  split: its 7,124-line arithmetic, inductive, citation, and structural-law
  parent delegates the 242-line exact quotient-congruence judgment to a focused
  child. That child alone recognizes quotient mint equality and requires the
  quotient's retained relation premise; it performs no ambient proof discovery
  and cannot fall through to generic arithmetic or structural tiers.
  Its structural-term algebra, explicit ring/semiring licensing, substitution,
  and structural judgment now form a separate 1,839-line responsibility; the
  remaining coordinator then delegates exact polynomial normalization,
  interval propagation, difference-bound closure, and arithmetic verdicts to
  a separate 984-line child. Inductive transition-arm recognition, path-fact
  preparation, strict-decrease discharge, and hypothesis instantiation form a
  separate 456-line child. Boundary-operator contract matching, proposition-law
  binder synthesis, carrier-slot substitution, and structural diagnostics form
  a separate 1,040-line conformance child. The remaining citation coordinator
  is 2,535 lines, with the existing parent-facing judgments, conformance checks,
  and proved-index-algebra surface unchanged.
  Compiler footprint derivation now has a 509-line composition/partition parent
  over a declarative four-family registry: 249-line control/entry, 621-line
  storage/place, 866-line outbound-call, and 512-line buffer/wire/text
  responsibilities. A
  separate instruction-selection boundary-footprint owner has begun the same
  split: its 2,255-line `entry.rs` parent delegates all eleven compact-binary
  append/read footprint derivations to a 433-line `entry/wire.rs` child, while
  a separate 373-line `entry/text.rs` child owns bounded-buffer, string-
  descriptor, and runtime-text assembly footprints. A focused 152-line
  `entry/runtime_values.rs` child owns atomic and conversion-write footprint
  derivation over the retained runtime-operand arena. A focused 222-line
  `entry/guards.rs` child owns static, runtime-text, place-shaped, and recursive
  runtime-value dispatch-guard footprints. A 199-line `entry/control.rs` child
  owns ordinary call/return mechanics and compiler-generated dispatch-scaffold
  footprints. A 139-line `entry/assembly.rs` child owns the x86 checked-
  assembly catalog footprint over retained selected instructions and runtime
  operands. A 158-line `entry/exit.rs` child owns the derived-exit carrier,
  normalized result placement, and direct/indirect result footprints. The
  277-line `entry/inbound.rs` sibling owns inbound-storage carriers, normalized
  parameter/result-pointer writes, descriptor scratch, and exact target
  clobber validation. A 264-line `entry/place_writes.rs` child owns immediate
  integer, address, runtime-binary, and bit-field write footprints. A 433-line
  `entry/place_copies.rs` sibling owns ordinary place-copy shape dispatch to
  exact target encoder clobber contracts. A 240-line `entry/runtime_io.rs`
  child owns byte-read, byte-write, and line-read host-adapter footprint
  derivation. A 75-line `entry/constant_results.rs` child owns per-target
  constant host-result materialization footprints. A 901-line
  `entry/direct_imports.rs` child owns all sixteen direct-import footprint
  classifications and their shared retained-plan evaluator. A 121-line
  `entry/indirect_calls.rs` child owns table/vtable call footprints without
  conflating them with direct import relocation. A 709-line
  `entry/syscalls.rs` child owns the complete simple, relocatable-argument,
  result, and timespec syscall footprint family plus its closed-shape test. The
  public re-export surface, validation order, and 135-function inventory are
  unchanged; the
  children depend only on retained instructions/operands, the validated
  boundary plan, place-shape classification where applicable, and architecture
  encoder clobber/state facts. A
  separate 1,547-line
  module owns assembly footprints, operand-loader semantics, exact instruction
  bytes, and retained relocation checks behind two parent entry points; and a
  1,598-line module owns exact compiler relocation sets, symbol custody, and
  unchanged instruction-bit validation. Compiler atomic-operation replay and
  recursive runtime-operand storage-site derivation now form a separate
  752-line responsibility. The closed place-copy shape vocabulary and exact
  classifier form a 1,218-line responsibility; indexed and pointee offset
  decomposition is a separate 946-line responsibility. Place-pair and
  place-copy shapes map to exact
  architecture-specific relocation sites in a separate 505-line module. The
  closed place-write shape vocabulary and its exact classifier family form a
  separate 304-line responsibility. Retained place-write encoding plus exact
  register and relocation-site derivation form a separate 1,039-line
  responsibility. The closed compiler instruction-relocation recipe vocabulary
  and exact final-byte/site replay form a separate 1,539-line responsibility.
  Exhaustive expected-byte, class, position, and relocation-recipe
  reconstruction now has a 55-line specification-family dispatcher behind a
  single typed entry point; fixed mechanics, guards, return transport, and
  entry and dispatch transport form a separate 477-line family, while compiler atomics,
  place copies and writes, and storage results form a separate 1,083-line
  family. Imported calls, runtime I/O, indirect calls, and syscalls form a
  separate 858-line family. Bit fields, bounded buffers, wire encoding, and
  text materialization form a separate 1,480-line family. Binary arithmetic
  and scalar conversion writes form a separate 478-line family. The separate
  native-refinement lane now applies the same engineering boundary to x86-64
  byte encoding: the public root is down from 19,412 to 89 lines and
  re-exports 106-line function-frame, 591-line entry/result ABI,
  662-line privileged-effect, 578-line Linux-syscall, and 760-line atomic
  responsibilities with their focused byte/width tests. Compact Binary wire
  append/read, scalar, byte-slice, nested, repeated-field, predicate, and UTF-8
  encodings now form a separate 1,880-line responsibility. Stored/literal text
  append, materialization/comparison, Win64/Linux line reads, and bounded text
  carriers form a separate 1,580-line responsibility. Generic host dispatch,
  authored imports, normalized Win64/System V argument and result placement,
  direct/vtable/table calls, byte I/O, and exact relocation-site replay form a
  separate 4,399-line production responsibility; its 2,005-line ABI regression
  corpus is separately compiled. Runtime value comparison, operand replay,
  binary arithmetic, conversion, and text equality form a separate 4,340-line
  scalar responsibility; integer/bit-field/indexed place writes and copy-layout
  contracts form a separate 675-line responsibility. Their 652-line arithmetic
  and conversion regression corpus is separately compiled. Dispatch-loop,
  case-entry, state-write, case-leave, and static-guard encoding now form a
  separate 176-line responsibility. Shared register moves, loads/stores,
  displacement checks, copy-chunk iteration, and atomic byte helpers form one
  explicit 1,114-line crate-internal primitive layer. These
  are semantics-preserving responsibility splits, not trust promotions: the
  full low generator, row proofs, and composition bridges remain open, and no
  trust-graph node becomes derived from the spike. The corresponding AArch64
  cleanup has begun: its former 14,216-line runtime-storage parent is now an
  810-line address/load/store orchestration parent. Runtime operand replay,
  text equality, integer and floating arithmetic, arithmetic-domain policy,
  classification, and their exact width contracts form a separate 2,172-line
  runtime-value responsibility. Atomic load/store, read-modify-write, ordering,
  result-site, and width policy form a separate 697-line responsibility, and
  scalar conversion, placement, trap, saturation, and width policy form a
  separate 746-line responsibility. Direct place-pair, place-value,
  computed-value, register, machine-state, and exact failure-branch comparison
  contracts form a separate 356-line responsibility. Recursive-operand
  register/state contracts plus
  immediate integer, bit-field, direct binary, pointee binary, saturation, and
  trapping writes form a separate 1,048-line scalar-write responsibility.
  Direct, pointee, indexed, and double-indexed bounded-buffer writes plus
  literal and source appends form a separate 935-line responsibility. Direct,
  pointee, indexed, and double-indexed string-descriptor writes plus their
  register/state ceilings form a separate 392-line responsibility. Direct,
  pointee, frame-indexed, machine-indexed, and double-indexed place-address
  writes plus exact clobber/state ceilings form a separate 440-line
  responsibility. Descriptor, pointee, frame, and machine single- and
  double-indexed integer/binary writes form a separate 897-line responsibility.
  Direct, pointee, single-/double-indexed, cross-region, and indexed-pair copy
  encoders plus exact chunk and clobber contracts form a separate 2,597-line
  responsibility. The 3,361-line byte/width/policy regression corpus remains
  separately compiled, and the exact public, function, and test inventories
  remain preserved.

  Define a closed typed schema language with no opaque callbacks. One row per
  leaf operation owns well-formedness, direct mathematical denotation, canonical
  goals, post-discharge facts, crash behavior, and local fuel/frontier effects.
  Missing operation rows reject mechanically. A change to control, validity,
  effect, or frontier machinery is a visible ledger-algebra revision rather than
  an ordinary row addition. Schema and artifact identities pin the exact state
  model, mathematical definitions, operational clauses, and semantics version.

  Before committing the full low implementation, build a Gamma spike that
  canonical-decodes bytes and covers Exact and Wrapping arithmetic, signed
  divide/remainder with toward-zero behavior and `MIN / -1`, one conditional
  result equation, one branch-local premise, an asymmetric join that rejects,
  the positive all-predecessor merge dual, exact call-requirement enumeration
  and substitution, justification ranking, dominance, and invalidation. Measure
  Gamma/specification size, audit complexity, Beta-reference runtime and memory,
  ledger size, and prospective reconstruction-certificate size. Difficulty does
  not weaken the endpoint; an inability to express the total definition cleanly
  triggers a rung-design correction.

  The production ledger records premise origin, prerequisites, establishment
  point, value/place versions, validity scope, invalidating events, and an
  acyclic logical-justification rank. Rank prevents circular evidence but does
  not replace dominance and all-path availability. Ordinary merge evidence is
  acyclic and requires valid matching facts on every predecessor; cyclic control
  requires invariant establishment and preservation. Partial-operation result
  equations become available only on the proved normal successor. Calls check
  clause coverage separately from capture-free positional instantiation across
  arity, binder kinds/types, state versions, moves/reborrows, outcome guards,
  crash routes, and evidence lifetimes.

  Establish every deployed ledger by direct low-rung evaluation or a low-kernel-
  checked derivation of the same total definition. Rust agreement is a
  differential oracle whose disagreement rejects and whose agreement grants no
  authority. Convert reduction families incrementally: a converted family emits
  a certificate; an unconverted family remains an exact versioned trusted-
  judgment dependency.

  Prove separate composition bridges. Safety/partial correctness combines
  exhaustive derivation, sound schema rows, valid premises, and checked goals.
  Progress/total correctness combines well-founded measures, per-edge descent,
  complete SCC/call closure, and explicit environmental progress premises. Fuel
  is sponsor scheduling and discharges neither. Row proofs are universally
  quantified low-rung metatheory, with derived status computed from an accepted
  proof and exact dependencies. Conservative semantic extensions need checked
  transport; relevant changes require reproof, while old artifacts retain their
  pinned semantics identity. Native ISA/hardware refinement remains a separate
  trust closure.

  Acceptance: byte mutation, omitted sites, extra premises, stale versions,
  one-arm-only join facts, post-write stale facts, circular justification,
  wrong call substitution, premature result equations, altered schema rows,
  unknown roots, and changed semantic dependencies all reject or change the
  recorded closure. A reducer cannot replace the canonical goal with its
  sufficient preimage. An artifact report lists every remaining trusted
  implementation/row and cannot appear fully derived until both applicable row
  proofs and the relevant global composition bridge are accepted. A Psi-hosted
  kernel port alone emits no ledger and supplies no reconstruction assurance.
- **IRFUEL.** Extend entry/segment certificates to loops and build-time use;
  the generic terminal inspection path now independently verifies a selected
  source closure and publishes its recomputed acyclic entry certificate, with
  Cathedral's first timer root pinning that evidence. Add attributed response
  outcomes only when terminal wait/foreign edges can derive them. Inserted native
  metering must consume the installed exact-site
  attribution rows, but is design-blocked on the sponsor counter, exhaustion
  transfer, and resumable continuation ABI in owner Q2. Keep WCET and wall-clock
  conversion separate.
- **PROOF-RELEVANCE-MIGRATION.** Finish binding-level `[erased]`, checked
  noninterference, erased-stripped layout, and obligation preservation across
  the remaining consumers. Explicit relevance remains in semantic/proof
  identity while supported runtime carriers recursively omit erased storage,
  initialization, topology, bytes, tags, and ABI transfer; runtime use rejects
  and omitted evidence remains a required semantic term.

  Continue moving any remaining target-neutral generic/build-time probe
  sequencing out of `omega-compiler`; Psi owns those services and normalized
  plan carriers, while Omega owns target filtering and ABI/provider realization.
  This is engineering, not a language-design blocker. Unsupported computed,
  chained, dynamic-receiver, unresolved-generic, non-checked-supply, and
  unresolved-machine-parameter shapes keep failing closed. Carry the settled
  `Placed<P, T>` non-runtime-field input paths and per-outcome dispositions
  through checked and terminal representations. Relevance does not invent a
  runtime carrier or public ABI for otherwise non-layoutable types.
- **EFFECTFUL-TYPED-COMPUTATION:** specify the value/computation judgments
  connecting effectful machines to the future typed proof calculus. Treat both
  migrations as staged semantic work, not prerequisites for extending the
  existing terminal vocabulary.

Acceptance: a canonical terminal artifact can be verified after source and
producer state are discarded; the verifier independently reconstructs every
obligation and rejects missing/extra/mismatched evidence; interpretation and
native execution consume that same verified artifact; proof replacement does
not change semantic identity. Crash sites are never represented as ordinary
terminal transitions or absent cleanup, and concrete safe invocations can
disprove all crash routes.

### P4 — Calling plans, final footprints, and callbacks

Owners:

- `wiki/design_briefs/calling_plans.md`
- `wiki/design_briefs/os_memory_and_hardware_foundation.md`
- `wiki/language_guide/chapter_23_inline_assembly.md`

#### ENT2c — normalized ABI lowering

- Finish foreign-storage custody and provider-view invalidation. Borrowed
  custody ends at return; durable retention consumes an owned claim and ends
  through a receipt. Successful bodyless terminal boundary calls now retain the
  exact verifier-derived completion-receipt set through canonical encoding,
  interpretation, native lowering, machine-code evidence, and installation;
  provider rejection records no receipt and leaves custody live. The checker
  accepts only one compatible consumed input
  for inferred post-return custody and rejects borrow-only sources. Ambiguous
  multiple-owned sources are accepted only when an exact authored equality
  relates one whole input entry projection directly to the whole current result
  projection in the same content algebra; partition/subplace equations and
  borrowed selections remain fail-closed. Primitive result-bearing calls now
  carry exact whole-root receipts from source checking through terminal Psi
  encoding, verification, and retry-safe interpretation. Omega retains the
  result in its abstract plan and rejects the old metadata-only settlement path
  rather than dropping it. An admitted x86-64 `u8` port-read provider now has
  an exact result-returning native realization whose arguments, receipts,
  instruction interval, and provider identity survive installation. Other
  result shapes and targets remain fail-closed. Explicit provider views now
  borrow one linear validity claim: consuming invalidation is accepted after
  the view's last use and rejected while the view remains live. Projected/
  content-bearing result calls remain fail-closed.
- **WRITE-ONLY-BORROW — implement the settled `&write T` access mode.** Parse
  and resolve the third borrow kind; preserve its exclusive loan and restricted
  operation set through type checking, reborrows, projections, calls, provider
  selection, canonical plans, Terminal Psi, both execution engines, native ABI
  lowering, and diagnostics. Permit explicit `&mut T` attenuation only. Reject
  observation, readable reborrows, take/swap/read-modify-write, content-driven
  projection, non-discardable displacement, and invariant restoration that
  depends on reading the referent. Retain exact per-outcome write footprints so
  untouched ranges and their facts survive. Checked implementations prove the
  restriction over their call closure; opaque providers publish an admitted,
  implementation-pinned non-observation judgment. Migrate byte-output boundary
  surfaces only after the borrow kind is executable; do not reinterpret
  `&write` as vacant storage or typed construction.

#### ENT4 — registered callbacks

- **CALLBACK-PARAMETER-REQUIREMENT — implement the settled nominal binder.**
  Parse and resolve `where machine Selected satisfies Trait::requirement`,
  deriving the complete callable contract from one uniquely resolved
  requirement row. Centralize that exact resolver for domain route lists and
  every other signature-free requirement site. Reject overloaded paths,
  structural coincidence, and visible-unique selection. Retain a checked
  per-use row with call site, static-machine ordinal, selected
  machine/satisfaction row, exact requirement overload, separate published and
  actual envelopes plus their refinement proof, and the target thunk-placement
  recipe. Emit the private
  relocation only from validated binding lowering. Registration is linear,
  explicitly unregisters, retains required code/component leases, and keeps
  selected identity in provenance without importing narrower facts unless an
  API contract forwards them. Add declaration-side compatibility reporting
  when a new overload makes signature-free references ambiguous, with pass/fail
  canaries covering both callback binders and domain routes.

  The declaration and admission slice is implemented. Syntax, resolved, and
  typed trees retain a discriminated structural-or-nominal contract; nominal
  binders normalize to one exact trait/requirement symbol pair through the
  shared signature-free resolver. Selection requires one explicit satisfaction
  row for that exact requirement, rejects structural coincidence and a row for
  another trait, and keeps nominal and structural specializations distinct in
  template identity. Checked-only filesystem pass/fail canaries now pin unique
  and overloaded signature-free paths for both nominal callback binders and
  authored domain routes. Declaration-side compatibility reporting is also
  implemented: after symbols are assigned and before authored paths are
  normalized, one diagnostic names each overloaded declaring-trait family and
  source-ordered diagnostics name every affected nominal binder or domain
  route. The checked identity spine is also implemented:
  every admitted nominal use retains its exact statement/expression site,
  static-machine ordinal, registration operation, selected machine and entry,
  unique satisfaction trait/requirement, and canonical requirement-overload
  identity. Validation captures that authority before specialization consumes
  the authored arguments and again after each fixed-point cloning round, while
  structural machine parameters publish no nominal row. Each row now also
  pins the normalized published-requirement contract identity separately from
  the selected machine's normalized declared contract identity and retains an
  explicit admission-refinement receipt binding those endpoints. Requirement
  capsules now also retain canonical published service reach and synchronous
  invocation rows plus suspension, blocking, termination, and crash ceilings.
  A separate exact-machine realized envelope aggregates effective checked
  reach/invocation, transitive suspension/blocking, checked termination/crash,
  mutation frames, and capability-flow evidence without relabeling any of it as
  public contract identity. Crash evidence is refreshed after path-conditioned
  checked validation, rather than snapshot before that pass. Resource ceilings
  remain independent until their checked representation exists. The
  checked row now also retains an optional nonzero evaluated boundary-calling
  plan fingerprint. Ordinary nominal binders retain no callback placement;
  boundary callback uses gain the exact join key that target lowering must use
  to recover its already-evaluated `BoundaryEntryPlan`, without exposing a
  runtime code address. Both check-only and native orchestration immediately
  consume that key, revalidate the retained target plan, and reject missing,
  duplicate, invalid, or fingerprint-drifted realizations before backend
  lowering. That join now materializes one target-owned callback-placement row
  containing the exact nominal-use site and ordinal, selected machine/entry,
  satisfaction identity, fingerprint, and validated `BoundaryEntryPlan`.
  Checked-only compilation exposes those rows and native compilation retains
  them on `BackendPlan`, so no later thunk pass may replace the recipe with a
  convention oracle or silently discard it. Native backend planning now also
  resolves each selected machine/entry pair to one exact `ControlFlow`
  `StateKey`, rejects a lost entry before instruction selection, and assigns a
  deterministic compiler-private thunk symbol joined by placement-row index.
  That identity includes every source/selected handle generation and duplicate
  private identities reject before instruction selection.
  The symbol is planned object identity only and never an Omega value. A
  callback thunk now also has a distinct machine-function identity bound to
  its placement-row index and selected source entry, so an ordinary source
  function wearing the callback symbol cannot satisfy emission. That role
  survives the existing assigned-operation, machine-instruction, byte, and
  object carriers; object planning preserves its richer placement-derived
  symbol, and final emission requires the encoded identity to equal the exact
  planned callback role. Target-instruction lowering now validates the whole
  assigned function set before selecting any body, rejecting invalid roles or
  two functions that claim one source, wrapper, or callback identity instead
  of deferring ambiguity to object planning. Each internal direct-call target
  must resolve in that same exact assigned identity set, so role, callback
  placement, continuation-generation, or absence drift rejects before
  placeholder encoding. This pins the eventual thunk-to-selected-entry call
  edge but does not synthesize that thunk body. Missing, duplicate, redirected,
  role-drifted, or interval-drifted identities reject, so a plan row cannot be
  mistaken for emitted thunk evidence. Final compiler-function replay now also
  rejects invalid or duplicate identities and fingerprints each exact role,
  continuation handle/generation, segment, and callback placement alongside
  its byte/instruction partition. Role substitution therefore changes final
  derivation evidence even when all byte intervals remain unchanged. The same
  final replay independently resolves each encoded identity through one exact
  object function binding and requires its text-symbol interval to equal the
  encoded byte interval. Missing, duplicate, redirected, non-text/function, or
  interval-drifted bindings therefore reject uniformly for source, wrapper,
  and callback roles; object-local private spelling is not confused with an
  encoded source display name. Final replay independently rederives the target
  entry name and every non-entry source/wrapper private name; linkage renaming
  rejects, and callback identities cannot replace the process entry. The
  shared private-name primitive now binds role, machine/state arena indices
  and generations, and segment, so handle-generation drift changes linkage
  spelling instead of aliasing the earlier generation. The richer callback
  name remains bound by the placement-specific join. Final
  image construction now revalidates its copied function-symbol carrier before
  format emission: the entry handle and every identity-owned function name,
  text classification, interval, and kind must match exactly, while unowned or
  multiply owned function symbols reject. This does not expose an address or
  synthesize a body. After format placement, checked emission also rejoins each
  encoded identity/object symbol to exactly one compiler-function region with
  the same symbol, section offset, address, byte count, and final-byte
  fingerprint. Missing, duplicate, renamed, reclassified, or byte-drifted
  regions reject; import thunks remain a separate region namespace. Before
  consuming those rows, checked emission independently replays the complete
  placed inventory from final text: exact text identity, ordered region spans,
  derived addresses, per-span bytes, complementary gap partition, retained
  origin/footprint metadata, and aggregate inventory identity must agree.
  Stored-summary, overlap/order, origin, address, byte, or gap drift therefore
  rejects without claiming callback-body synthesis or registration-relocation
  placement. Compiler-function evidence now additionally retains the exact
  ordered identity-to-object-handle-to-final-region join, including the
  inventory identity and each region's index, symbol, address, interval, and
  byte fingerprint. Its binding fingerprint participates in the function
  evidence and final text derivation, preventing a validated function
  partition from being paired with another independently valid inventory.
  Boundary-footprint attachment now consumes an exact sealed entry projection
  of that join rather than searching by linkage spelling. The compiler-private
  identity, object symbol handle, region index and final span, inventory
  identity, and whole function-region binding identity participate in its own
  replayed fingerprint; any identity/handle/row/custody drift rejects before
  the inventory is mutated. That mutation now returns a checked custody receipt
  joining the sealed entry projection and whole function-region binding to the
  prior inventory, exact composed footprint, and resulting inventory.
  Boundary-bearing final footprint certificates require and fingerprint this
  receipt; missing, stale, redirected, or pre/post-inventory drift rejects. The
  complete certificate is now constructed and revalidated before executable or
  app-bundle installation, and auxiliary inventory serialization consumes that
  existing certificate rather than discovering a semantic failure only after
  executable bytes become visible. Publication now additionally seals that
  certificate to the exact emitted image evidence, final-text/inventory pair,
  output name and format, and complete container-byte identity. Flat executable
  and app-bundle installation consume only this validated view, so certificate,
  container, or output-identity drift rejects before either byte copy is
  published. Each executable copy now also replays the complete staged file
  byte-for-byte before its atomic rename and returns an exact installation
  receipt binding the publication identity, output path, byte count, and
  container identity. A redirected name or changed/partial staged file is
  removed and rejects before becoming visible. The compile report now retains
  that exact native-executable receipt through the orchestration return
  boundary, including certificate, inventory, publication, path, container,
  and installation identities. Check-only and object-container fallback paths
  retain no such receipt; it remains artifact custody, not runtime loading
  authority. Receipt minting now occurs only after the renamed destination is
  independently read and compared byte-for-byte with the sealed container.
  Missing or changed destination bytes are removed and reject before the
  orchestration return can expose a receipt or path. Mach-O GUI builds now
  retain the optional app-bundle executable receipt separately from the flat
  executable receipt; both bind the same publication/container identity but
  their exact destination paths cannot substitute for one another. Other
  targets retain no bundle receipt. Before returning the report, orchestration
  independently validates the pair: a bundle receipt requires one flat
  receipt, equal certificate/inventory/publication/container identities and
  output leaf, plus distinct paths and installation identities. Missing,
  substituted, or self-aliased pairs reject atomically. The compile report now
  also retains the exact output category. `NativeExecutable` requires the flat
  receipt, `ObjectContainer` requires both executable receipts absent, and
  `CheckOnly` requires no output or receipt. A dropped native receipt can no
  longer masquerade as a legitimate object-container fallback. Each receipt
  additionally retains its exact destination role. The native flat slot accepts
  only `FlatOutput`, the optional bundle slot accepts only `MacOsAppBundle`, and
  the role tag participates in installation identity; swapped otherwise-matching
  receipts reject. The bundle slot also rederives the canonical
  `<build>/<sanitized-project>.app/Contents/MacOS/<executable>` path from the
  report root and flat output, so a same-leaf receipt under another directory
  cannot substitute. The report root is now outwardly read-only because it
  participates in that derivation; a caller cannot redirect it after final
  validation and thereby change the canonical bundle identity. Immediately
  before either outward receipt is minted,
  installation replays the renamed destination bytes once more against the
  sealed container; interval drift removes the changed file and rejects instead
  of returning stale custody. The validated output flag, category, flat receipt,
  and optional bundle receipt are now outwardly read-only, so a report consumer
  cannot rearrange or drop one component after the compiler's final consistency
  check. Both early check-only and backend reports now use one checked
  constructor, which rejects an inconsistent output/category/receipt tuple
  before it can cross the orchestration return boundary. The constructor also
  rejoins the optional program-storage entry binding to its native bridge: both
  are absent together or the retained binding must equal the bridge's exact
  binding, while a dropped, unpaired, or redirected row rejects before return.
  Both retained fields are now outwardly read-only, so a consumer cannot mutate
  one side into a post-validation mismatch. Report construction also joins
  bridge phase to output category: check-only retains a pending bridge without
  final wrapper evidence, native executable output requires that evidence, and
  object-container fallback cannot carry a program-storage bridge. A native
  bridge's final wrapper evidence must also name the same executable-region
  inventory fingerprint as the flat publication receipt, preventing evidence
  from another valid final image from accompanying the published container.
  Receipts now additionally retain the sealed compiler-text derivation and
  compiler-function evidence fingerprints; flat/bundle copies and native
  wrapper evidence must rejoin the same pair rather than inventory alone. The
  receipt also retains the certificate's optional boundary-contract
  fingerprint; flat/bundle copies agree and native program-storage arrival
  evidence must name that same concrete contract. Report validation now also
  rejoins the retained selected entry binding's boundary-contract fingerprint
  directly to the native flat receipt. Check-only retains that binding without
  publication, while object-container output cannot carry it; matching arrival
  evidence therefore cannot conceal a redirected selected binding. Each
  receipt's installation seal is now independently recomputed from its exact
  destination role, publication identity, output path, and container byte
  identity. Flat-only and flat-plus-bundle reports reject a stale or
  substituted seal without relying on pairwise inequality alone. The private
  written-output handoff now also requires a native output path to equal its
  flat receipt's installed path before auxiliary reporting or report
  construction; object output carries no executable receipts, and check-only
  cannot masquerade as a written output. Before that handoff is consumed, its
  optional bundle receipt must also satisfy the same canonical root-derived
  path, role, shared publication/certificate/container identity, valid
  installation seal, and distinct-installation checks that final report
  construction independently replays. Native execution consumers no longer
  reconstruct an executable leaf from the build directory: the report returns
  its exact flat receipt path only after replaying complete publication and
  program-storage custody, and `omega-run` consumes only that checked path. The
  shared report-and-capability native runner now likewise consumes the checked
  report for all ten of its executions rather than reconstructing a name from
  the build directory; bundle-path tampering is pinned to expose no executable.
  The exact-native source index accepts that form only for an exact report-local
  binding plus literal exit status, adding seven unique rooted owners (795
  total); the twice-owned linear-transfer fixture remains fail-closed and
  unelided. The five authored-root value/type-check executions now also launch
  through their checked reports rather than reconstructing `out/<executable>`;
  their literal-status exact-owner identities remain unique, so the 795 pin is
  unchanged. Output-kind tampering is pinned to expose no native path.
  The first five authored-root value-call/dispatch executions likewise consume
  only the exact checked-report path; the 795 unique-owner pin remains stable,
  and compiler-function fingerprint drift between the flat and bundle copies
  exposes no executable path.
  The next five authored-root dispatch executions through the mixed return-type
  probe now use the same receipt-only launch boundary; the exact-owner pin stays
  795, and flat/bundle boundary-contract drift exposes no executable path.
  The following five value-call executions through the post-splice mutation
  probe also use the exact report receipt; the 795 pin remains unchanged, and
  flat/bundle executable-inventory drift exposes no executable path.
  Five further runtime executions—called-machine loop search, looping
  value/cast returns, the slice-length guard, and sleep—now use receipt-only
  launch. The 795 owner pin stays stable, and flat/bundle compiler-text
  validation drift exposes no executable path. Five additional authored-root
  native executions—write without newline, runtime exit code, borrow-carrying
  data-field access, and u8/i8 field arithmetic—now launch only from their
  exact checked-report receipts. The 795 exact-owner pin remains stable, and
  flat/bundle publication-evidence drift exposes no executable path. The first
  five authored-root range/storage executions likewise launch only from their
  exact checked-report receipts while retaining literal statuses 1, 1, 15, 7,
  and 9 and the guarded-binary cross-target check. The 795 exact-owner pin
  remains stable, and flat/bundle container-byte-count drift exposes no
  executable path. Five more authored-root range/arithmetic executions—guarded
  copy narrowing, ranged divide/modulo, ranged bitwise masking, and declared-
  range index read/write—now launch only from exact checked-report receipts
  while retaining literal statuses 7, 4, 3, 30, and 30. The 795 owner pin
  remains stable, and flat/bundle container-fingerprint drift exposes no
  executable path. Five additional authored-root range/indexed-structure
  executions—constant-expression range bounds, indexed struct-field read-
  modify-write and operand use, and machine-indexed scalar/struct-field
  arguments—now launch only from exact checked-report receipts while retaining
  literal statuses 40, 1, 1, 1, and 1. The 795 owner pin remains stable, and
  reused flat/bundle installation evidence exposes no executable path.
  Forty further authored-root indexed/slice executions now cross the same
  checked report boundary in eight exact cohorts. The first five cover by-value
  parameter/local indexed access; machine/frame read, write, RMW, dual-frame,
  operand, and argument use; nested and runtime-middle indexing; aggregate and
  cross-region indexed copies; and constant/computed index guards, all retaining
  literal status 1. Three later cohorts cover constructor/slice/member use
  (statuses 70, 1, 1, 1, 1), subslice/loop/post-clause delivery (3, 1, 1, 1,
  1), and slice-length/descriptor shrinking (5, 6, 3, 3, 3). The 795-owner pin
  remains stable. Certificate drift, flat/bundle substitution or omission,
  swapped destination roles, a dropped native-output flag, and related
  receipt-cardinality drift each expose no executable path.
  Five further authored-root subslice/carrier executions—runtime-end subslice
  element access, fixed-array length guarding, runtime-bounded subslice
  argument delivery, owned bounded-carrier concatenation, and borrowed bounded-
  carrier alias concatenation—now launch only from exact checked-report
  receipts while retaining literal statuses 20, 7, 3, 70, and 70. The 795
  exact-owner pin remains stable, and the existing receipt-drift matrix
  continues to expose no executable path.
  Five further authored-root carrier/control executions—frame-local bounded-
  carrier concatenation, slice-view carrier guarding, slice-view element
  argument delivery, linear-search early exit, and unary entry-result
  delivery—now launch only from exact checked-report receipts while retaining
  literal statuses 70, 70, 70, 70, and 1. The unary fixture retains its
  `linux_arm64` cross-target emission check, the 795 exact-owner pin remains
  stable, and the existing receipt-drift matrix continues to expose no
  executable path.
  Five further authored-root entry/control executions—computed entry result,
  widened cast result, nested-binary result, free-standing helper result, and
  iterative loop patterns—now launch only from exact checked-report receipts
  while retaining literal statuses 200, 70, 70, 7, and 70. The computed-result
  fixture retains Full emission for its boundary-footprint assertion, the cast
  fixture retains its `linux_arm64` output check, and the 795 exact-owner pin
  remains stable.
  Five further authored-root control/carrier executions—composite-initializer
  argument forwarding, captured-local preservation across source-field
  mutation, bounded-carrier pointee guarding, bounded-carrier slice-field
  writing, and Utf8 return-view equality—now launch only from exact checked-
  report receipts while retaining literal status 70 for every row. Stdout-
  bearing carrier probes remain unchanged, and the 795 exact-owner pin remains
  stable.
  Five further authored-root output/operator executions—bounded-carrier
  `write_line`, cross-state nested-carrier text building, shift operators,
  bitwise operators, and the popcount loop—now launch only from exact checked-
  report receipts while retaining literal statuses 70, 0, 70, 70, and 70. Both
  output probes retain their exact `Room A1` stdout assertions, and the 795
  exact-owner pin remains stable.
  Five further authored-root operator/value-call executions—xorshift PRNG
  composition, bitwise guard subjects, suffixed integer literals, value-
  position branching calls, and free-machine value calls—now launch only from
  exact checked-report receipts while retaining literal status 70 for every
  row. The 795 exact-owner pin remains stable, and the existing receipt-drift
  matrix continues to expose no executable path.
  Five further authored-root by-value machine executions—free-machine struct
  arguments, case-bearing parameter self-write, attached-machine struct
  arguments, record forwarding across a nested statement call, and free-
  machine struct returns—now launch only from exact checked-report receipts
  while retaining literal status 70 for every row. The 795 exact-owner pin
  remains stable.
  Five further authored-root machine/integer executions—free-machine mutable-
  argument value calls, looping free-machine value calls, widened integer
  comparisons, widened bitwise operations, and 16-bit cast roundtrips—now
  launch only from exact checked-report receipts while retaining literal status
  70 for every row. The 795 exact-owner pin remains stable.
  Five further authored-root versioning/equality executions—explicit version
  migration, two-era and three-era lineage matching, scalar `Equatable`
  equality/inequality guarding, and mixed-shape case membership—now launch
  only from exact checked-report receipts while retaining literal status 70
  for every row. The 795 exact-owner pin remains stable.
  Five further authored-root wire executions—max-one repeated-field roundtrip,
  honest Utf8 roundtrip, Utf8 edge-class validation, invalid-Utf8 refusal, and
  numbered schema-as-value use—now launch only from exact checked-report
  receipts while retaining literal status 70 for every row. Five more wire/
  comptime executions—decoded-field let comparison, repeated-then-string
  encoding, nested-plus-repeated roundtrip, transitive const-array length, and
  parenthesized bare-call-arm const-array length—use the same receipt-only
  launch boundary and statuses. The 795 exact-owner pin remains stable.
  The two-row scalar-operation entry-result probe now launches its builtin and
  comparison results only from exact checked-report receipts while retaining
  literal statuses 70 and 1. The 795 exact-owner pin remains stable. All
  four remaining authored-root numeric/float executions in this module—mixed
  numeric casts, float place comparison, float comparison guards, and float
  arithmetic—now also launch only from exact checked-report receipts while
  retaining literal status 70. The operator target-row regression is repaired,
  the 795 exact-owner pin remains stable, and this module no longer reconstructs
  any native executable from a build-directory/name convention.
  All five conventional native launches in the entry/ABI canary module—entry
  run-args, Utf16 literal delivery, case-array element writes, policy-authored
  wire plans, and nested policy-authored wire plans—now consume exact checked-
  report receipts while retaining literal statuses 5, 70, 36, 70, and 70. The
  run-args fixture retains Full footprint inspection, the nested policy fixture
  retains both cross-target checks, and the 795 exact-owner pin remains stable.
  All three conventional native launches in the artifact-footprint canary
  module—shared reference-parameter copy, pointee-pair copy, and record-view
  place addressing—now consume exact checked-report receipts while retaining
  literal statuses 42, 42, and 70. Artifact-producing cross-target tests remain
  unchanged, and the 795 exact-owner pin remains stable.
  The final conventional native launch in the reports/capabilities canary
  module—the linear obligation spanning a dispatched-call continuation—now
  consumes its exact checked-report receipt while retaining literal status 7,
  Full backend-report emission, and the complete permission-realization/event
  assertions. The 795 exact-owner pin remains stable.
  Five authored-root atomic executions—load/store ordering, fetch-add, fetch-
  sub, fetch-xor, and fetch-or—now launch only from exact checked-report
  receipts while retaining literal statuses 70, 70, 70, 70, and 75. Existing
  `linux_arm64` cross-target checks remain unchanged, and the 795 exact-owner
  pin remains stable.
  Five authored-root host/target executions—stdin command echo, qualified-case
  values, single-target internal filtering, target-machine gating, and ring-
  requirement conformance—now launch only from exact checked-report receipts
  while retaining literal statuses 0, 70, 70, 70, and 70. The stdin probe
  retains its exact `look\n` stdout assertion, and the 795 exact-owner pin
  remains stable. Five authored-root layout/generic executions—plan-laid value
  fields, erased plan-laid fields, distinct closed erased sums, mixed closed
  generic erasure, and exact generic call/return contexts—now launch only from
  exact checked-report receipts while retaining literal status 70. Existing
  semantic-layout and interpreter assertions remain unchanged; the 795 rooted
  and 3 legacy exact-owner pins remain stable. Five authored-root ABI/runtime-
  value executions—entry-field-write value calls, post-entry-state lets,
  runtime-local and constant self-array indexing, and a deep post-entry chain—
  now launch only from exact checked-report receipts while retaining literal
  statuses 70, 24, 99, 99, and 30. Existing interpreter assertions remain
  unchanged, and the 795 rooted/3 legacy exact-owner pins remain stable. Five
  further authored-root ABI/value-call executions—chained post-entry lets,
  cross-callee division, same-named cross-callee lets, nested value-call guards,
  and two-site struct results—now launch only from exact checked-report
  receipts while retaining literal statuses 2, 70, 70, 70, and 70. Five more—
  same-callee multi-site results, guarded and straight-line transition
  arguments, straight-line shared-slot results, and enum-self methods—retain
  literal statuses 70, 70, 12, 22, and 70 through the same receipt-only launch
  boundary. Five further ABI/dispatch executions—dispatch-bodied results,
  literal-length arm guards, value-call guard subjects, effectful guard/local
  and self-terminal delivery, and guarded effectful transition arguments—also
  launch only from exact receipts while retaining literal status 70. Existing
  interpreter and diagnostic-status assertions remain unchanged, and the 795
  rooted/3 legacy exact-owner pins remain stable. Four further authored-root
  ABI/value-call executions—nested-entry value calls, shared-name variant
  payload delivery, struct-payload cast fields, and branch-leaf multiple named
  conversions—now launch only from exact checked-report receipts while
  retaining literal status 70. Five further ABI/process-control executions—
  entry-host-state payload delivery, a contained health loop, sequential stdin
  buffering, Full artifact-backed text storage, and stderr writing—use the same
  receipt-only launch boundary while retaining literal statuses 70/75, 0, 0,
  0, and 70. All interpreter, stdin/stdout/stderr, and backend-report assertions
  remain unchanged, and the 795 rooted/3 legacy exact-owner pins remain stable.
  Fifteen further authored-root ABI text executions now use exact checked-
  report receipts: LF/CRLF line reads and indexed slice-string guards retain
  statuses 0, 0, 77, 70, and 72; string places across machine fields, local
  arrays, slices, and pointees retain 70, 70, 89, 70, and 70; mutable parameter,
  wrapped write-line, and struct-field concatenation retain 77, 77, 77, 77, and
  188. Five more string-assembly rows—stored suffix, lookup/large-frame/room
  lookup concatenation, and a call-argument slice alias—retain 193, 190, 192,
  200, and 77. Exact interpreter, stdout, and content-comparison assertions
  remain unchanged, and the 795 rooted/3 legacy exact-owner pins remain stable.
  Five further authored-root ABI/string-storage executions—mutable struct
  string-field copy/concat/write-line, machine-owned indexed integer writes,
  fixed- and runtime-indexed struct copies, and nested indexed exit writes—now
  launch only from exact checked-report receipts while retaining literal
  statuses 77, 79, 83, 85, and 89. Five further authored-root ABI/ordered-
  dispatch executions—direct, after-call, game-shape, and large-machine room
  dispatch plus guarded inline leaf-arm skipping—now use the same receipt-only
  boundary while retaining literal statuses 73, 83, 93, 103, and 70. Exact
  interpreter/stdout and existing diagnostic assertions remain unchanged, and
  the 795 rooted/3 legacy exact-owner pins remain stable.
  Four further ABI/dungeon executions—ordered-room dispatch and real-show-state
  stdin loops, the threaded mutable-argument interrupt soak, and nested value-
  call caller-local guarding—now launch only from exact checked-report receipts
  while retaining literal statuses 135, 145, 70, and 70. Five further authored-
  root domain/control executions—copy-then-read, full-width i64 operations,
  chained bounded-text append, descriptor append-in-place, and two-field
  bounded-text concatenation—use the same receipt-only boundary while retaining
  literal status 70. Existing stdin, interpreter, and diagnostic assertions
  remain unchanged, and the 795 rooted/3 legacy exact-owner pins remain stable.
  Five further authored-root domain/control executions—machine bounded-text
  append, local string-field copying through mutable parameters, bounded-
  carrier call returns, min-call result arithmetic, and direct Boolean
  conjunction dispatch—now launch only from exact checked-report receipts while
  retaining literal statuses 70, 70, 70, 70, and 21. Existing interpreter
  assertions remain unchanged, and the 795 rooted/3 legacy exact-owner pins
  remain stable.
  Five further authored-root executable-domain executions—local and imported
  membership expressions, imported membership guarding, and imported
  intersection/union guards—now launch only from exact checked-report receipts
  while retaining literal statuses 81, 91, 81, 219, and 217. Existing
  diagnostic assertions remain unchanged, and the 795 rooted/3 legacy exact-
  owner pins remain stable.
  Ten further authored-root executable-domain executions now use exact checked-
  report receipts: local intersection/union guards, local union/intersection
  values, and imported union values retain statuses 231, 241, 205, 233, and
  215; imported intersection values, local Boolean-or values, straight-line
  terminal local and field readback, and negated Boolean-place guards retain
  217, 251, 70, 70, and 73. Existing diagnostic assertions remain unchanged,
  and the 795 rooted/3 legacy exact-owner pins remain stable.
  Ten further authored-root control executions now use exact checked-report
  receipts. Local Boolean conjunction, scalar comparison, string comparison,
  Boolean-or guarding, and direct Boolean transition arguments retain statuses
  74, 76, 78, 71, and 211. Local Boolean transition arguments, Boolean
  transition arguments after string guards, machine-owned indexed nested-room
  copies, negated comparison guards, and case-member dispatch retain 201, 247,
  87, 75, and 70. Existing diagnostics remain unchanged, and the 795 rooted/3
  legacy exact-owner pins remain stable.
  Fifteen further authored-root case/data/control executions now use exact
  checked-report receipts. Case-payload construction, record field-value
  patterns, case-payload guard reads, case-membership values, and exhaustive-
  by-cases dispatch retain status 70 and every decoy-sensitive diagnostic.
  Exhaustive case-union domains, case-membership union guards, case
  reassignment, mixed-shape data, and array-literal String fields likewise
  retain status 70 and every wrong-arm/stale-data diagnostic. Struct-literal
  String fields, immutable parameter-domain forwarding, case-payload domain
  forwarding, tuple transitions, and room-use reentry retain statuses 70, 70,
  70, 22, and 41; both independent interpreter oracles remain unchanged. The
  795 rooted/3 legacy exact-owner pins remain stable.
  Five further authored-root dungeon/storage executions—enemy-clear reentry,
  clear/carve/render String fields, full-level wrapper String lookup, multi-room
  reentry, and mutable-slice element writes—now launch only from exact checked-
  report receipts while retaining literal statuses 51, 198, 202, 63, and 21.
  Both dungeon interpreter oracles and all diagnostics remain unchanged, and
  the 795 rooted/3 legacy exact-owner pins remain stable. Five further authored-
  root indexed-storage executions—straight-line and dispatched mutable-slice
  writes, runtime array indexed reads, indexed struct-field writes, and
  particle integration—now launch only from exact checked-report receipts
  while retaining literal statuses 70, 31, 70, 70, and 70. Existing alias,
  stale-fold, and self-check diagnostics remain unchanged, and the 795 rooted/3
  legacy exact-owner pins remain stable. Five further authored-root
  construction/call-identity executions—nested-struct construction, cross-
  machine substate-name resolution, value-call array-element writes, computed
  transition arguments, and by-value struct parameters—now launch only from
  exact checked-report receipts while retaining literal status 70 and every
  regression-specific diagnostic. The 795 rooted/3 legacy exact-owner pins
  remain stable.
  Five further authored-root value/result executions—value-call composition,
  struct-returning calls, Option-returning calls, Result matching, and entity-
  component state—now launch only from exact checked-report receipts while
  retaining literal status 70 and every pipeline, sum/error, and nested-field
  diagnostic. Five further structured-value executions—nested-struct state,
  array-element struct copies, deep nested value semantics, struct-array
  literals, and struct-valued enum payloads—use the same receipt-only boundary
  while retaining status 70 and every copy/layout diagnostic.
  Five further authored-root enum/nested/indexed-state executions—enum
  classification and dispatch, nested-field accumulation, indexed-write/
  constant-read, indexed temporary RMW, and indexed writes beside adjacent
  fields—now launch only from exact checked-report receipts while retaining
  literal status 70 and every dispatch, stale-constant, and out-of-bounds
  diagnostic. Five further bounds/index executions—join-meet bound propagation,
  dual indexed comparisons, array min/max reduction, indexed guard subjects,
  and nested payload range narrowing—use the same boundary while retaining
  status 70 and every bound/element-selection diagnostic; the paced host-timer
  legacy launch remains intentionally untouched.
  Five further authored-root arithmetic-policy executions—saturating wide
  boundaries, saturating parameter carry, saturating expression operands,
  wrapping guard operands, and signed MIN/-1 divide guards—now launch only from
  exact checked-report receipts while retaining literal status 70 and every
  policy-specific diagnostic. Across these cohorts the 795 rooted/3 legacy
  exact-owner pins remain stable.
  Five further authored-root operand-carrier executions—nested unsigned
  arithmetic, local indexed call operands, machine-indexed fused call
  arguments, saturating indexed guard operands, and nested float operands—now
  launch only from exact checked-report receipts while retaining literal status
  70 and every signedness, register-custody, and domain diagnostic. Five further
  shift-policy executions—shift-count domain resolution, guarded Exact shifts,
  at-width wrapping left and right shifts, and indexed shift targets—use the
  same receipt-only boundary while retaining status 70 and every policy
  assertion. Five further saturating-value executions—nested operands, unsigned
  one-direction clamps, the signed MIN idiom, saturating left shift, and 32-bit
  shift value overflow—likewise retain status 70 and every clamp/domain
  assertion. Across these cohorts the 795 rooted/3 legacy exact-owner pins
  remain stable.
  Five further authored-root conversion/float-policy executions—subword masked
  shifts, saturating float-to-int, unsigned/narrow saturating float-to-int,
  saturating float overflow, and direct trapping float overflow—now launch only
  from exact checked-report receipts while retaining every literal status,
  abnormal-exit check, and interpreter reason assertion. The two custom-ranking
  recursive-delivery executions and the u64-magnitude transition-delivery plus
  proven-range Exact shift-count executions likewise use exact receipts while
  retaining status 70 and all terminal-delivery/diagnostic assertions. Slow
  float-policy, helper-driven trapping, platform-gated, timer, and multi-fixture
  owners remain outside fast follow-up cohorts; the 795 rooted/3 legacy exact-
  owner pins remain stable. Profiling one float-policy owner attributes 3.263 of
  3.523 measured native-compile seconds (92.61%) to Stage 05 checked-tree
  construction, with samples concentrated in checked-fact and recursive call-
  frame write-demand summarization. The independent interpreter oracle repeats
  that frontend work, while backend emission is only 1.5 ms and `OutputOnly`
  already fences auxiliary reports.
  A broader test-topology audit built every `omega-compiler` test binary in
  4.61s wall, confirming that Rust test compilation is not the long pole. The
  canary umbrella already runs independent compiles with bounded outer
  parallelism (eight jobs by default), one inner backend worker, deterministic
  source-ordered result collection, and exact-native duplicate elision; ordinary
  helpers already disable auxiliary HTML/report emission through `OutputOnly`.
  Current measurements therefore do not justify an Arena-to-PagedArena rewrite
  or deleting report viewers for speed. Further work should target repeated
  Stage 05 semantic compilation/search and reuse checked-report receipts where
  one owner currently recompiles the same frontend.
  Five further authored-root lifetime/wire executions—method-view writes after
  last use, chained view-of-view writes, shrinking-slice recursion, primitive
  wire encoding, and wire era discrimination—now launch only from exact checked-
  report receipts while retaining literal status 70 and every alias, recursion,
  and byte-level diagnostic. The 795 rooted/3 legacy exact-owner pins remain
  stable.
  Five further authored-root wire-decoder executions—primitive roundtrip,
  ranged scalar and repeated fields, canonical Boolean enforcement, and
  canonical varint enforcement—now launch only from exact checked-report
  receipts while retaining literal status 70 and every hostile-input
  preservation/byte-canonicality diagnostic. The 795 rooted/3 legacy exact-
  owner pins remain stable.
  Five additional authored-root wire executions—scalar-width overflow
  rejection, nested-message roundtrip and malformed-length rejection, plus
  repeated-field roundtrip and overflow rejection—now launch exclusively
  through checked-report executable receipts while preserving literal status
  70 and all byte-shape diagnostics. Exact-owner inventory pins remain
  unchanged.
  Five further authored-root wire owners—wrong-era rejection, exact String and
  byte-slice encoding, zero-copy byte-slice decoding, and decoded-slice
  indexing—now execute solely from checked-report receipts while preserving
  status 70 and all byte-canonicality assertions. The adjacent auxiliary-report
  consumer remains on its report-bearing path, and exact-owner pins remain
  unchanged.
  Five further fast authored-root executions—decoded byte-slice length access,
  call-result binary composition, multi-arm value selection, unsigned value
  guards, and compile-time-sized array execution—now launch solely from checked-
  report receipts while preserving literal status 70 and their original
  failure diagnostics. Report-bearing and float/cast owners remain deliberately
  outside this cohort; exact-owner pins remain unchanged.
  Five further fast authored-root executions—fixed-vector roundtrip, eager
  combination of distinct value-call results, signed i64 arithmetic, high-bit
  bitwise operations, and unsigned high-value comparisons—now launch
  exclusively through checked-report receipts while preserving status 70 and
  all behavioral diagnostics. Report-bearing and known slow float/cast/policy
  owners remain deliberately excluded; exact-owner pins remain stable.
  Five further authored-root algorithm executions—Euclidean GCD, RPN stack
  evaluation, greedy activity selection, maze pathfinding, and graph BFS
  traversal—now launch exclusively through checked-report receipts while
  preserving status 70 and each result-specific diagnostic. Report-bearing and
  known slow float/cast/policy owners remain excluded; exact-owner pins remain
  stable.
  Five further authored-root collection executions—coin-change dynamic
  programming, open-addressed hashing, matrix multiplication, ring-buffer
  queuing, and bubble sorting—now launch solely through checked-report receipts
  while preserving status 70 and their exact result diagnostics. Report-
  bearing, known slow float/cast/policy, and exceptional historical-hang owners
  remain excluded; exact-owner pins remain stable.
  Five further authored-root indexed/container executions—2D transpose, guarded
  indexed access, binary search, two-pointer palindrome checking, and nested
  struct-array field access—now launch solely through checked-report receipts
  while preserving status 70 and exact result diagnostics. Exceptional
  historical-hang, report-bearing, float/cast, and policy owners remain
  excluded; exact-owner pins remain stable.
  Five further authored-root struct/index executions—enum-grid scanning, dual
  indexed reads, struct-field temporary arithmetic, runtime-indexed whole-
  struct writes, and indexed-read guard evaluation—now launch solely through
  checked-report receipts while preserving status 70 and exact regression
  diagnostics. Exceptional historical-hang, report-bearing, float/cast, and
  policy owners remain excluded; exact-owner pins remain stable.
  Five further authored-root aggregate executions—runtime-row/constant-column
  writes, nested-array constant indexing, whole-array and whole-struct value
  copies, and fixed-array field guards—now launch solely through checked-report
  receipts while preserving status 70 and exact data-flow diagnostics.
  Exceptional, report-bearing, float/cast, policy, and automaton owners remain
  excluded; exact-owner pins remain stable.
  The final three eligible fast owners in the wire/algorithm module—standard
  Optional matching, fixed-array field-value access, and fixed-array element
  guards—now launch solely through checked-report receipts while preserving
  status 70 and exact diagnostics. Its remaining conventional launches are
  deliberately retained exceptions: auxiliary-report consumers, known slow
  float/cast/policy cases, the historical-hang owner, and the automaton owner.
  Exact-owner pins remain stable.
  Three atomic authored-root executions—fetch-and, swap, and compare-exchange—
  plus Dutch-flag partitioning now launch natively solely through checked-
  report receipts while preserving their literal statuses, detailed
  diagnostics, and Linux ARM64 cross-target compilation assertions. The
  interactive two-mode console owner and all previously fenced exceptional,
  report, float/cast, and policy owners remain excluded; exact-owner pins
  remain stable.
  Five further authored-root UTF-8/content executions—parameter length-field
  access, regular-call literal length, literal and view content equality, and
  declared-domain field reads—now launch solely through checked-report
  receipts while preserving status 70 and exact content/domain diagnostics.
  Exceptional, interactive, report-bearing, float/cast, policy, and automaton
  owners remain excluded; exact-owner pins remain stable.
  Five further authored-root domain/carrier executions—domain field write/read,
  bounded-carrier content roundtrip, carrier length as both host argument and
  stored field, and carrier byte indexing—now launch solely through checked-
  report receipts while preserving literal statuses 73/10/70 and exact domain/
  content diagnostics. Exceptional, interactive, report-bearing, float/cast,
  policy, and automaton owners remain excluded; exact-owner pins remain stable.
  Five further authored-root byte-carrier executions—runtime-indexed reads and
  writes, indexed reads as value operands, the carrier cipher loop, and
  constant-byte writes at runtime indices—now launch solely through checked-
  report receipts while preserving status 70 and exact byte-level diagnostics.
  Numeric-conversion and all exceptional, interactive, report-bearing, float/
  cast, policy, and automaton owners remain excluded; exact-owner pins remain
  stable.
  Five further authored-root carrier algorithms—length guarding, FNV-1a
  hashing, CRC32, Base64 encoding, and run-length encoding—now launch solely
  through checked-report receipts while preserving status 70 and exact hash/
  encoding diagnostics. Numeric-conversion, rendering, exceptional,
  interactive, report-bearing, float/cast, policy, and automaton owners remain
  excluded; exact-owner pins remain stable.
  Five further authored-root text/byte executions—binary formatting, substring
  search, string palindrome checking, bounded-carrier byte writes, and slice-
  length field access—now launch solely through checked-report receipts while
  preserving literal statuses 70/5 and exact formatting/search/content
  diagnostics. Numeric-conversion, rendering, coercion, exceptional,
  interactive, report-bearing, float/cast, policy, and automaton owners remain
  excluded; exact-owner pins remain stable.
  The final four eligible fast owners in the content/carrier module—unary
  negation, UTF-8 literal length, user-domain literal grants, and bodyless-
  domain declaration spellings—now launch solely through checked-report
  receipts while preserving status 70 and exact arithmetic/domain diagnostics.
  Its remaining conventional launches are deliberately retained numeric-
  conversion, rendering, or coercion exceptions; exact-owner pins remain
  stable.
  Five further authored-root layout/value executions—plan-laid by-value
  parameters, fixed-array record and mutable views, nested fixed-array mutable
  views, and sequential value-call result slots—now launch solely through
  checked-report receipts while preserving status 70, interpreter parity, and
  Windows x64/Linux ARM64 cross-target assertions. The 2.88-second plain
  record-view owner joins the retained slow exceptions; all other fenced
  exception classes remain unchanged and exact-owner pins remain stable.
  Five further authored-root layout/value executions—nested-record, fixed-
  record-array, and ordinary mutable record views, sequential self-capture value
  calls, and nested local state arguments—now launch solely through checked-
  report receipts while preserving status 70, interpreter parity, and existing
  Windows x64/Linux ARM64 cross-target assertions. The 2.88-second plain record-
  view owner remains retained for a dedicated profiled migration; all other
  fenced exception classes remain unchanged and exact-owner pins remain stable.
  Final
  replay now also retains an exact
  selected-instruction-to-function-symbol owner map. Duplicate selected
  instruction identities, redirected instruction relocation origins, and
  instruction-origin rows without a retained owner reject, while semantic and
  materialization origins remain in their separate namespaces. Final
  emission also rejoins every validated placement row to exactly one private
  thunk plan. Missing,
  duplicate, or out-of-range placement indices, selected-entry drift, and
  repeated private thunk identities reject before encoded-function/object
  evidence is accepted. This does not
  materialize the registration relocation. Private-symbol derivation is now
  one shared backend-plan primitive, and final emission recomputes it from the
  exact site kind/index/generation, static ordinal, selected machine/entry
  handles, and evaluated calling-plan fingerprint. Symbol drift rejects even
  when forged encoded-function and object rows agree with each other. The
  remaining slices are
  resource-ceiling aggregation, multi-entry/re-entrant target instruction
  lowering, and the
  private registration relocation (whose binding placement is design-blocked
  on `OWNER_QUESTIONS.md` Q5),
  registration leases/unregister,
  and cross-target registered-callback canaries.
- Implement the narrow Windows `user32` canary without exposing a raw code
  address. Derive `Atomic::interruption_fence` same-context evidence from the
  installed external-root route and reject it elsewhere.

Acceptance: changing a normalized plan changes lowering or rejects; forbidden
state introduced anywhere in final executable text rejects; a registered
callback cannot outlive its registration/code lease or smuggle application
state through a raw address.

### P5 — Cathedral bring-up over general Omega primitives

#### BUMP-ALLOCATOR-CANARY — package allocator

- After P1 conservation is source-usable, implement a package-level bump
  allocator over one qualified `Extent`. Two allocations must coexist; release
  cleans and returns the exact subextent without restoring bump-tail capacity;
  reset succeeds only after full recomposition; finish returns the original
  backing.
- Implement owned `Vec<T>` and then `Vec<u8>::Utf8` only after choosing the
  allocator contract needed for cleanup, authority return, and capacity reuse.

#### Address translation

- Build Cathedral's page-table hierarchy, validation states, installation, and
  teardown in Omega source using `source/drivers/facts/x86_page_table_entry.omg`.
- Use pre-reserved storage for the fixed bootstrap table; dynamic hierarchy
  allocation waits for the package allocator. Cathedral now represents one
  512-entry page candidate and validates its complete exact-zero starting
  state with a checked bounded scan. Physical backing, address-space-profile
  hierarchy, mappings, installation, and teardown remain. Do not restore a
  compiler-owned page-table model.

#### Exception roots and first timer

- Materialize fatal/diagnostic entries for every architectural exception before
  enabling interrupts. Cathedral now has a checked fixed-work internal leaf
  that records one normalized 0–31 vector in preallocated atomic state,
  publishes its validity, and unconditionally aborts. Generated per-vector
  stubs, admitted internal-state binding, physical entry plans, stacks, gates,
  and IDT installation remain.
- Provision dedicated per-CPU double-fault/NMI/machine-check stacks and one
  non-nesting maskable-IRQ stack class; preserve the selected `StatePlan`.
  Cathedral now authors and validates the complete four-role class/IST policy;
  WCSU-derived byte sizing, a source-level `StackLease`, storage provisioning,
  and installed-root binding remain.
- Bring up PIT+PIC first and LAPIC as the production provider. The hard root only
  acknowledges, records time, publishes a coalesced wake, and returns; fan-out
  runs in an ordinary task.
- **BOUNDED-INSTALLATION-REACH-ROWS.** Implement bounded installation reach
  rows spelled `reaches <= Bound` on installation-bound boundary requirements.
  Give each row the exact normalized
  requirement path as identity, retain its `+`-union upper bound and internal
  dependency closure, reject escape through ordinary callable package or
  component contracts, expose unresolved rows and bounds in preselection
  manifests, substitute the selected provider row throughout the root closure,
  and reject final admission with any unresolved row. Do not add effect
  negation, subtraction, lower bounds, exclusive-or, named row variables, or
  cross-requirement correlation.
- Migrate both `InterruptEntry::enter` and
  `InterruptAcknowledgement::complete` from the temporary hardcoded `PortIo`
  ceiling to distinct bounded installation rows beneath
  `MachineControl + PortIo`. Bind entry/completion coherence through the exact
  installed provider execution, acknowledgement policy, operation, and token
  lineage rather than row equality. PIC completion resolves to `PortIo`;
  LAPIC/x2APIC completion resolves to `MachineControl`. Checked and terminal
  artifacts retain the selected provider, operation, bound, resolved row, and
  refinement evidence without granting authority from reach.

Acceptance: QEMU installs Cathedral-owned memory/interrupt structures, reports
timer ticks over owned serial output, and halts between ticks. No customer-shaped
compiler concept is introduced.

## Parallel compiler and language lanes

### Frames, reach, and trust

- **R5:** continue exact inferred may-write summaries and relational candidates.
  Exact frames compose through transparent returns/helpers, caller-isolated
  scratch locals, statement/value positions, stable mutable aliases, and direct
  alias replacement; rebinding leaves earlier reborrows intact. The bounded
  non-reference direct-call expression class is complete through depth two,
  including member projection and one or more independently bounded indexes;
  typed non-reference assignment-value call trees extend through depth four.
  A direct primitive scalar assignment value may wrap complete caller-isolated
  call producers in up to two unary, binary, primitive-cast, member-projection,
  or indexing shells without widening that call budget.
  One top-level concrete primitive-only record or selected-case literal may
  likewise contain an independently bounded non-reference call tree in each
  direct common or payload field while publishing every write. One direct
  field may instead contain a second concrete primitive-only record or
  selected-case literal whose direct fields obey the same rule; this aggregate
  depth-two rail does not widen the depth-four call budget. A declared
  primitive field at either level may wrap independently bounded call operands
  in up to two nested scalar-computation shells made from unary/binary
  operators, primitive value casts, member projections, or indexing without
  widening that budget. Literal-length caller-isolated fixed-array assignment
  values preserve the same relation through one nested array level; every
  element retains the same call and primitive-computation budgets. Within that
  same two-level aggregate budget, fixed arrays may contain concrete record or
  selected-case literals, and concrete record or selected-case fields may
  contain literal fixed arrays. A primitive scalar assignment value may also
  select one direct member from a concrete caller-isolated record or
  selected-case literal whose effectful primitive fields are bounded
  direct-call trees or use one scalar-computation shell around those calls;
  every field publishes its writes. One additional outer scalar shell is
  admitted only when the fields do not consume that remaining shared
  computation-depth-two budget. The literal receiver may use the existing
  two-level aggregate budget while carrying that reduced computation budget
  unchanged; a third aggregate level remains fenced. The same primitive
  assignment may directly index a fixed-array literal whose eagerly evaluated
  elements contain bounded calls or use the one remaining scalar-computation
  shell; all element and index writes publish even when the selected index is
  constant. One outer scalar shell may instead consume that remaining budget;
  combining both remains a third-shell fence.
  Indexing irreversibly coarsens to the nearest backing collection while
  preserving independent index-call writes. Finite named-state SCCs accept only
  bijective write-capable parameter permutations. Primitive-only concrete
  record/sum locals remain isolated through nested fixed arrays.

  Continue with representable relational candidates. Boundary,
  beyond-per-position-budget, binding-reborrow, reference-valued/opaque,
  escaped, non-bijective, generic, recursive or reference-bearing aggregate
  literals, third aggregate or computed shells, other computed field shapes,
  and out-of-isolated-root shapes remain conservative fences. Do not restore
  authored `stores` clauses or treat lifetime elision as evidence; Git carries
  individual evidence cohorts.
- **STR/EFX:** finish independent normalization/publication of machine supply,
  service reach, suspension, blocking, termination, mutation, and trust. The
  state graph and checked-tree visualization now consume suspension and blocking
  independently from exact flow-state and machine-contract facts while service
  reach stays on its dedicated facts. Provider approval now consumes exact
  checked-flow call coordinates directly and no longer replays the operational
  umbrella. Static-machine selection validation now projects transient
  inference into separate machine-keyed suspension and blocking rows before
  threading callable-shape judgments; service reach is inferred separately and
  neither operational axis can supply the other's fallback. Checked fact
  construction likewise projects separate machine-keyed rows before publishing
  the suspension and blocking roots, including mixed-axis checked bodies. The
  published checked operational root is retired; its plan remains only as a
  transient validation and independent-fact construction input.
  Continue removing umbrella carriers after their remaining consumers migrate.
- **TPR4/TPR6 — design blocked on owner Q4.** Choose how an ordinary domain or
  routed requirement is classified and attached as a progress premise before
  connecting progress-profile grants and receipts. Generic routed/domain
  requirements must not be treated as progress merely because they are
  predicate-free or provider-backed; private ranking witnesses remain outside
  public identity.
- **GR6:** finish qualification/trust consumers and their artifact rows. The
  retained selected-provider rows already bind exact plan, overload, grant,
  subject, authority-flow, semantic-domain, carry, predicate, and root-selector
  identity across lock/report/runtime admission. Continue with consumers that
  still lack exact blast-radius rows. Selected schemas, adapter dispatch, and
  calling-plan lookup require nonempty overload identities; name-only singleton
  matching remains forbidden.

Acceptance: contract axes normalize independently, wrappers cannot launder
reach or trust, and private proof improvements do not change public identity.

### Multiplicity, tasks, and execution

- **CML4:** construct the complete `EdgeCleanupPlan` after outgoing-value
  materialization and transfer-map commitment. Current Unit/scalar and bounded
  acyclic slices retain reverse-declaration cleanup, partial-record transfer of
  prefix-disjoint all-field paths, maximal-residual disposal, nominal helper
  calls, shared targets, edge/action ownership, and direct-Boolean contextual
  obligations through terminal verification, interpretation, fuel, and all
  native artifact paths. Nominal scalar cleanup admits finite continuation
  chains whose stages contain arbitrarily nested finite short-circuit Boolean
  decisions. One finite parameter/constant decision tree, including Boolean
  equality against a constant, can instead feed a typed shared terminal-Psi
  convergence value and one native cleanup tail. Extend contextual
  cleanup beyond the current receiver-independent Boolean subset, finite
  continuation trees, and that narrow shared-convergence shape; add
  wider structural partial values, repeated-cycle resource composition, and
  conservation/backend-ledger reporting. This is not yet a general conditional
  CFG, complete cleanup plan, or conservation witness.
- **TR3-TR8:** finish whole-call-graph WCSU derivation, bind exact `StackPlan`
  evidence, reserve fixed nonmoving `StackLease`s, validate preservation and
  cancellation conformances, transfer arguments transactionally, lower
  park/resume, and implement the suspension-safe-loan subset. Current Unit,
  scalar, and acyclic conditional shapes retain exact frame/link/temporary,
  call, crash-terminal, and target-generated division-diamond evidence from
  instruction selection through decoded installation and artifact-wide closure
  composition. One depth-independent conditional-tree carrier accounts nested
  decisions and mutually exclusive source-distributed convergence calls.
  One bounded Boolean carrier additionally accounts ordered actual
  unconditional native join branches plus the final fallthrough into one
  affine-cleanup tail. Extend that accounting to general shared native joins
  and general affine cleanup rather than claiming convergence from duplicated
  leaves.
  Provider-sized external adapter/arrival state is design-blocked on
  `OWNER_QUESTIONS.md` Q3: stack-domain ownership across interrupted and
  switched entry must be settled before this can become a complete root
  `StackPlan`. Zero-byte internal closures remain inadmissible until that
  adapter demand exists.
- **BLOCKEXEC:** implement an ordinary package-level blocking executor with
  bounded queues, moved custody, linear completion claims, suspension, and
  provider selection. A hung in-process worker cannot be killed safely;
  bounded recovery requires process isolation.

Acceptance: linear debt cannot disappear through cleanup or aggregation;
CPU/thread-restricted activations require selected preservation evidence; task
and allocation handles expose no compiler-owned stack/control storage.

### Propositions, quotients, and mathematics

Owner: `wiki/design_briefs/law_bearing_relations_and_quotients.md`.

Remaining N6/N8 work:

- **SELECTED-WITNESS-EVIDENCE.** Bind a privately selected named conformance to
  one carrierless term at named `ensures`; consume its normalized requirement
  map. Named `requires` terms are positional erased inputs, passed explicitly
  after `;` and projected as `term.member`. Never infer evidence from visible
  facts or attached state names.

  The accepted front half carries exact erased terms through resolved, typed,
  checked, call/transition, and finite-state definite-assignment paths. It
  supports positional input lanes, forwarding, concrete subjectless producer
  selection, normalized direct/inherited requirement rows, and exact generic
  evidence interfaces. Terminal Psi retains canonical term declarations,
  requires/ensures lanes, public proof-output selectors, opaque member projections,
  and separate producer provenance; codec and verification reject identity,
  interface, lane, field-name, row, producer, and orphan drift. The detailed
  accepted carrier is stated in
  [`law_bearing_relations_and_quotients.md`](wiki/design_briefs/law_bearing_relations_and_quotients.md).

  Replace the implemented immediate generated-output-package syntax with the
  settled proof-output lane. Ordinary results retain their declared Type;
  `let (value; public_slot: local_term) = call()` captures selected named
  guarantees, while omitted selectors contribute facts but mint no local term.
  Evidence-only binding keeps the empty Type lane. Checked and terminal Psi
  retain the exact call, callee lane, public selector, caller-local term,
  proposition/interface, outcome guard, and producer provenance without any
  generated package identity. Proposition terms remain copyable and add no
  runtime work, cleanup, or fuel.

  Generic producer conformances use the same nested static application form as
  other name-owned telescopes. Type, const, and static-machine arguments are
  explicit; only ordinary lifetime elision applies. Runtime Type results retain
  their own multiplicity independently of the proof lane.
  Keep proposition, evidence-term, and provenance identities separate; neither
  provenance nor display spelling is a term identity oracle.
- Finish generic conformance instantiation and explicit binders. The declaration
  front half now parses `Name<Telescope>: [Subject] satisfies Trait { ... }`,
  retains lifetime/type/const/static-machine parameters through resolved and
  typed Psi, resolves its contracts and trait arguments in that name-owned
  scope, and gives every named conformance a package-scoped symbol. Machine
  telescopes retain a distinct proof-static `Evidence: Subject satisfies Trait`
  binder with its own lexical symbol. A concrete call now binds the exact named
  closed conformance, validates its instantiated subject/trait shape, exposes
  direct and inherited requirements in the binder scope, substitutes the
  selected normalized rows, and commits the map identity separately from
  callable static-machine arguments. Implement nested call-site applications
  such as `SequenceEncoding<u8, Message>` inside the enclosing machine
  telescope. Require every type, const, and static-machine argument owned by
  the conformance; apply only the ordinary lifetime-elision rules, rejecting an
  ambiguous or otherwise unresolved lifetime. Expected subject/trait shape
  validates the closed application but never fills its non-lifetime arguments.
  Nested generic calls already forward an exact closed evidence binder bare
  through specialization. Identity retains declared name, complete normalized
  telescope including resolved elided lifetimes, optional subject, instantiated
  trait, and normalized rows. No visibility-, priority-, specificity-, or
  expected-shape-based selection.

  The nested-application closure slice is implemented. Static applications
  remain recursively delimited through syntax, resolved, and typed Psi; checked
  specialization requires every non-lifetime argument, rejects a bare generic
  conformance name, validates the instantiated subject/trait shape, and retains
  an argument-sensitive identity containing the exact declaration, explicit
  lifetime/type/const/static-machine lanes, subject, trait application, and
  closed row map. Different members of one conformance family now produce
  different specialization identities. Inline and default realization machines
  now close over the conformance telescope, and binder-member rewriting carries
  the selected application's type arguments into the ordinary fixed-point
  monomorphizer. Distinct family applications produce distinct executable row
  instances. Literal const and concrete static-machine arguments now enter the
  same specialization tuple, including calls through a captured static-machine
  parameter inside the row body. Specializing an enclosing generic machine now
  recursively substitutes type, const, and static-machine arguments inside a
  forwarded conformance application before the next fixed-point pass. Generic
  carriers retain their full instantiated subject identity (for example
  `Box<Card>`) while executable attached-row lookup uses the carrier's declared
  base namespace. Terminal Psi now retains each used closed application under
  its concrete terminal machine ID as one ordered, category-tagged telescope,
  exact declaration/subject/trait identities, normalized row map, and canonical
  application commitment. Format-13/vocabulary-18 codec round trips preserve the
  table, and a dedicated verifier module rejects unknown owners, malformed or
  duplicate applications, and redirected rows. Erased conformance lifetimes now
  resolve only from the enclosing call's parameter-position ordinary borrow
  constraints. The closer retains the resolved region in checked and terminal
  identity, rejects missing or conflicting constraints, and checks an explicitly
  supplied region against the same call-site constraint; declaration arity alone
  never selects a lifetime.
- Add `Respects` over compiler-derived positional call telescopes, deriving its
  dependent domain, pointwise input relations, and lifted result relation.
  Add the sealed `Quotient::lift<F, Respect>` wrapper and
  `Quotient::define<F, Respect>` faithful-definition operations. Both select one
  exact named conformance in the quotient owner's ordinary body; `lift` checks
  public-precondition implication, while `define` checks equivalence, exact
  position-preserving runtime argument correspondence, and unchanged result
  flow over normalized IR. Retain the representative machine application,
  derived `RA`/`RR`, complete conformance application, correspondence,
  lift/define kind, and contract proof in checked and terminal identity. Add no
  `lifts` clause, quotient operation map, visibility discovery, or per-call
  selection.

  The first post-ruling representation rung is implemented without granting
  admission. Typed calls recognize only the sealed
  `Quotient::lift<F, Respect>` / `Quotient::define<F, Respect>` spelling and
  retain the exact resolved representative entry, exact named conformance
  application, and operation kind. Wrong-category, unresolved, wrong-arity,
  and shadowed selections reject; structural proof-machine discovery is not a
  fallback. The request remains deliberately non-executable until formation,
  compiler-derived `RA`/`RR`, contract correspondence, and (for `define`)
  positional/result-flow checks are implemented. The former bare
  free/attached-call pilot and its implicit quotient return retagging are
  retired rather than treated as a compatibility path.

  Quotient formation is now migrated to the exact static selection. `%`
  requires a proposition relation plus the declaration's one explicitly named,
  subjectless, closed, nongeneric `Equivalence<C, R>` conformance. The selected
  sealed toolchain interface and exact Reflexive/Symmetric/Transitive row
  contracts are checked without Boolean or structural fallback; strengthened
  or vacuous premises reject. Anti-axiom provenance follows checked row
  dependencies transitively through contracts, values, statements, guards,
  transitions, and continuations. Generic relation applications retain exact
  binder symbols, categories, and order. Formation and congruence canaries cover
  ordinary, independently indexed generic, and static-machine carrier families;
  authored lookalikes, missing selections, Boolean relations, admitted nested
  dependencies, strengthened laws, and swapped generic binders reject. The
  retained lift/define request remains deliberately non-executable until
  compiler-derived `RA`/`RR`, contract correspondence, positional arguments,
  and normalized result flow are independently certified.
  The first relation-planning prerequisite is now implemented without granting
  admission. A monomorphic request at the exact root of one state's final
  expression statement
  derives a non-authoritative plan whose `RA` retains every operand: exact
  quotient type and relation identity for quotient positions, and exact typed
  equality for ordinary positions. `RR` retains the exact quotient result type
  and relation. Untyped or adapted arguments and nonquotient results reject;
  indexed relation applications wait for the fully instantiated representative
  operation telescope rather than guessing binder applications from the
  quotient type. The same plan now resolves exactly one representative
  machine/state and retains its ordered runtime telescope (including attached
  `self`, excluding proof-static `const` binders), parameter symbols/types/
  mutability, result type, and machine/state contract spans. Missing or
  duplicate state identities, open generic/static applications, and unresolved
  results reject. Closed type, literal-`const`, and static-machine applications
  retain exact parameter/argument bindings on the telescope. The 2,422-line
  relation/result-flow parent delegates closed static-application validation
  and immutable declaration-type substitution to a focused 355-line child. Its
  structural matcher substitutes retained bindings through representative
  runtime parameter and result types, including const-parametric array lengths,
  without mutating the checked type arena; unsupported constrained/dynamic
  shapes fail closed unless already canonically identical. The general
  contract-entailment root now delegates structural proof-term construction,
  constant unfolding, selected-machine application identity, and the occurs
  check to a focused 301-line child without changing the proof language or
  judgment order. A direct `define`
  request now also requires exact
  positional runtime correspondence: public parameter symbols in order,
  one-to-one unique runtime identities, quotient-carrier/representative-type or
  ordinary exact-type agreement, mutable/borrow mode and multiplicity
  preservation, and an exact representative-result carrier match. Attached
  representative `self` participates positionally without requiring a public
  parameter spelled `self`; reorder, duplication, locals/literals, arity,
  borrowed-shell, type, and result drift reject. Owner static/const and
  contract-fact substitution remain explicitly unresolved. For exact direct
  `define` correspondence, quotient-facing machine/state `requires` facts are
  partitioned into `Q` versus fixed facts, while representative machine/state
  `requires` facts are partitioned into `P` versus fixed facts. Both use complete
  expression dependency on the corresponding quotient-bearing runtime
  positions; exact side/owner/contract/fact coordinates are retained and
  unresolved value identities reject. The first exact equivalence rung now
  alpha-renames both runtime telescopes by their retained position map and
  requires a bijection between the corresponding dependent facts. Fixed ambient
  facts remain ordinary call obligations outside `Q` and `P`. The plan retains
  both exact fact coordinates; missing, duplicated, category-drifted, or
  redirected facts reject. This proves only exact normalized fact identity, not
  general entailment or any `Respects` clause. The relation-plan coordinator
  now delegates `Q`/`P` dependency partitioning, exact fact lookup, positional
  alpha-renaming, and bijection matching to a focused 416-line precondition
  judgment child. Exact public-parameter order, mode, quotient-carrier matching,
  and representative static substitution now form a separate 212-line runtime-
  correspondence child. Exact representative entry lookup, runtime telescope,
  and the existing whole-call-graph purity and unconditional-termination
  certificates form a separate 213-line representative child. None of these
  extractions duplicate or broaden the proof language. The
  exact final-expression call
  additionally retains one unchanged state-fallthrough result edge. The same
  single edge may
  now pass through a complete straight-line chain of exact immutable,
  result-typed local aliases when the sealed request is the first initializer
  and the state's final expression is the last local. Mutable/type-drifted,
  nested, interrupted, reassigned, cyclic, transition, and state-forwarded
  shapes cannot claim that edge. When the owner machine and state identities
  are each exact-unique, the machine has exactly that one state, and its body
  contains no transition, the plan now records that this fallthrough path
  exhausts the owner's normal result exits. Duplicate identities, another
  state, or any transition retain no single-state coverage claim. A finite
  sibling-state graph now also proves complete coverage when every non-result
  state contains exactly one unconditional ordinary named transition, every
  target is a unique state in the same machine, every path reaches the result
  state, and that state retains the unchanged transition-free fallthrough root.
  Conditional/crash transitions, continuations, extra statements, foreign or
  missing targets, cycles, and duplicate state identities reject. This is one
  bounded graph walk, not a hop-count permutation ladder. These result-flow
  certificates prove no effect, contract, or `Respects` obligation. The
  relation-plan parent now delegates the whole alias/finite-graph judgment to a
  focused 289-line child instead of accumulating hop-specific verification
  functions. Its 1,850-line regression harness now lives in a test-only child,
  leaving the production relation-plan coordinator at 449 lines without
  changing judgment ownership.
  Every request still rejects as non-executable until all remaining obligations
  are checked and retained in checked/terminal identity. Executable admission
  is blocked on one concrete vocabulary decision: the source/core surface has
  no sealed `Respects` declaration or compiler evidence carrier capable of the
  ruled variadic positional telescope. Authored empty lookalikes cannot supply
  that authority, and an arity-indexed `Respects1`/`Respects2` ladder is
  explicitly rejected.
- Suppress every synthesized representation observer on quotient formation.
  Resolved-to-typed lowering now rejects runtime `==`/`!=`, a direct
  `Equatable` conformance, and synthesized container equality through a quotient
  field. It also rejects proof-contract `zero_value<Quotient>()`; a retained
  representative is not a compiler-verified canonical default. Build-time
  layout/access schema reflection rejects a quotient directly and refuses to
  derive a zero-byte nested record layout for one. Record and arm destructuring
  reject quotient subjects before field/case analysis, so an empty or rest
  pattern cannot become a representation observer. Struct and case literals
  likewise cannot forge a quotient value; casting an exact carrier instance
  with `as Quotient` is the sole construction path. That nominal fence runs
  before generic field-shape deferral, so a parameterized quotient head cannot
  bypass it. Logical proof-position equality remains raw for the exact
  quotient-congruence judge; it never lowers to representative bytes. Add
  quotient-owned executable equality through an ordinary lifted operation with
  `DecidesEquivalence`; derive its `Respects` proof, and bind its optional `==`
  token only through the settled fixed-operator declaration head.
  Keep ordering, canonicalization, hashing, and later observer roles on
  explicit role-correctness contracts until each earns a named interface.
- Enforce the initial quotient integration fences: lifted representative
  machines are pure and terminating, and quotient carriers contain no
  affine/linear `Type` content or owned/routed custody. Effectful lifting waits
  for a complete observable-behavior relation; custody-bearing quotients wait
  for exact occurrence-preservation machinery. Formation now walks the exact
  recursive proof-carrier graph once and rejects any contained ordinary Type
  whose multiplicity is not unrestricted, as well as references, slices,
  dynamic traits, const-expression type shells, and explicitly carried data.
  Recursive proof-only nodes and contained structurally copy data remain
  admissible. The direct operation plan now also retains the exact
  representative machine/state when its locally checked termination summary is
  unconditional. A missing guarantee or one requiring progress-profile
  premises retains the termination fence rather than treating those premises
  as discharged. The same validation now consumes the shared whole-call-graph
  operational and service-reach fixed points once on the rejecting quotient
  path. It retains the exact representative machine/state as pure only when
  recursive service reach, suspension, blocking, mutable/out parameters, and
  unresolved concrete call targets are all absent. It does not run a second
  expression-local effect inference. Exact owned/routed custody occurrence
  detection remains open.
- Add exact-pair-selected heterogeneous constructor lifts. Dependent records
  lift in order and generate checked transport obligations for coarser earlier
  fields. Extend R6 carrier-family binders for reusable proposition-valued
  relators; add no global carrier role or default relator.
- Gate runtime deciders whose lifted relation depends on erased `Type` content;
  require determination by the runtime projection or report the component.
- Continue total specification arithmetic. Concrete and abstract Prop owners
  reject direct Trapping arithmetic and conversion while preserving total
  comparison, bitwise, classification, Wrapping, and Saturating terms. Same-
  carrier policy erasure retains Exact formation against prior facts without
  self-justification. The arithmetic-domain regression cases now live in a
  219-line test-only child instead of sharing the 5,729-line production
  analysis module. Its closed dependent-subtraction refinement and conservative
  machine/state field-preservation bridge now live in a separate 313-line
  `dependent_relations` child. The closed bounded-product recognizer and its
  exact unsigned-widening/coupling gates form a separate 260-line
  `dependent_products` child. Flow-sensitive interval/fact state and its
  merge/invalidation rules now live in a 424-line `value_environment` child;
  concrete and abstract total-proposition formation lives in a 976-line
  `total_specification` child; and the recursive operand-domain/interval walk
  lives in a 789-line `expression_analysis` child. Entry-precondition,
  branch/fall-through, out-parameter, and incoming-edge fact construction now
  lives in a 1,207-line `guard_narrowing` child with two sibling-only query
  seams. The production parent is now 1,814 lines, down from 5,166 before these
  responsibility splits. No
  accepted relation, call-write fence, interval, judgment, or diagnostic
  ordering changed. Fixed-width
  integer and address
  `embed` returns proof `Int` and contributes exact source-carrier range facts;
  proof `Int as Nat` requires nonnegativity. Ordinary `Nat - Nat` is now an
  Exact fixed operator whose `right <= left` contract is discharged at
  formation; the independently selected structural `Nat::less_or_equal`
  operator supplies the public order spelling, and missing order custody
  rejects rather than selecting clamping. Checked software conformance now
  applies the existing supported contract-entailment gate to ordinary as well
  as boundary operators, allowing the recursive total provider to implement
  the exact partial surface without weakening either call form. The named
  monus operation and its complete dependent
  order/metric/Cauchy/Rat corpus now use `Nat::saturating_sub`; measured-recursion
  recognition and affected pass/fail canaries consume the same final identity.
  `Granted::content` and the normalized content-projection examples now embed
  fixed unsigned/address fields uniformly into proof `Int`, spell the exact
  `as Nat` conversions explicitly, and retain `IntervalSet<Nat>` as their
  public nonnegative algebra. Canonical content normalization erases only that
  exact proof conversion around an otherwise admitted scalar term; signed
  runtime embeddings still reject at the closed projection boundary. Add the
  remaining integer-policy bridge integrations and the remaining compiler/
  Terminal bindings for the `FloatMeaning` projection rules described by
  [`total_specification_arithmetic.md`](wiki/design_briefs/total_specification_arithmetic.md).
  The shared numeric-semantics layer now owns a closed integer-policy bridge
  catalog for add, subtract, multiply, divide, and left/right shift across
  Exact, Wrapping, Saturating, and Trapping. Each row separately names its
  result law, formation conditions, primitive-specific trap predicates, and
  shift-count law; division retains zero-divisor and signed-minimum/-1 as
  distinct traps, and Trapping shifts do not collapse count failure into
  generic overflow. Concrete and abstract specification shift-count validation
  now consume those rows instead of duplicating the policy match. General
  expression analysis now also consumes the catalog's exact-result formation
  and Trapping result-overflow predicates for add, subtract, multiply, and left
  shift while preserving its established interval refinements, warnings, and
  diagnostics. Exact division remains in its dedicated specification-
  definedness lane. The bounded checked-operation producer, proof-bearing
  Terminal rows, structural-crash verifier, and dedicated Exact-division
  consumers described above now consume the settled catalog. Remainder stays
  explicitly outside it until a remainder primitive is ruled; other catalog
  integrations remain open only where their owning operation surface exists.
  A separate shared float-projection catalog now owns the exact `meaning32`
  and `meaning64` rows. Each row fixes its source format, exact nonzero-rational
  finite projection, signed-zero and signed-infinity preservation, and NaN-
  payload erasure; cross-format projection rejects. The checked interpreter's
  central scalar float paths consume these rows rather than choosing f32/f64
  projection ad hoc. Payload-distinct NaNs collapse while signed zeros and
  infinities remain distinct in focused tests, and the runtime float-operation
  canary remains green. Source-operator binding now consumes the catalog's
  complete `Float::meaning32`/`Float::meaning64` identities and rejects a
  drifted canonical declaration unless it is the exact ordinary tokenless
  `f32`/`f64` to `FloatMeaning` signature, without lifetime/type parameters or
  receiver qualification. Leaf spelling alone grants no projection semantics.
  Terminal and independent-verifier binding to the catalog remains open. The
  current checked/Terminal proposition and operation vocabularies have no
  source-handle-free proof-only `FloatMeaning` carrier or projection fact, so
  this cannot be completed as a name-table join. The next slice must first add
  that erased carrier and exact format-specific projection operation, keeping
  it separate from Terminal runtime float evaluation; only then can producer
  emission and independent verifier reconstruction bind to the catalog.
- Then migrate suffix law discovery to propositions plus explicit conformances,
  and expand the checked `Nat`/`Int`/`Rat`/Cauchy/approximation corpus. `Real`
  remains proof-only and core-level.

Acceptance: an admitted axiom cannot license quotient formation; selected
Reflexive/Symmetric/Transitive evidence and every `Respects` proof are explicit;
different witnesses establish one stable proposition identity and eliminate
through its declared interface; quotient operations select their exact proof
in the quotient owner's body; canonical definitions cannot hide wrappers; and
no structural observer, effect, or custody occurrence crosses the quotient
boundary without its corresponding checked law.

### Fixed operator surface

- The fixed declaration-head syntax is implemented for ordinary and boundary
  operators. One optional compiler-owned token is parsed immediately after
  `operator`, retained by the existing syntax/resolved/typed/checked and public
  compatibility carriers, and calls continue to retain the exact resolved
  declaration. The provisional trailing `spelling` clause is rejected. Core,
  standard-library, canary, architecture, and embedded pipeline fixtures now use
  the declaration head; tokenless named operators remain valid. Parser coverage
  pins `+`, `[]`, `[..]`, unknown punctuation, token lists, and the retired
  clause, while checked and canary coverage pins exact selection and duplicate
  normalized operand rejection. The compiler-owned token/fixity table remains
  closed; no custom-token or alias facility was added.
- Trait-owned operator requirements now parse the same optional fixed-token
  head and retain it through syntax, symbol-resolved, typed, checked-container,
  and serialized snapshot surfaces. The ordinary trait requirement record is
  the sole carrier; conformances continue to implement the named requirement
  without a second token-binding field. Parser, resolved, typed-snapshot, and
  checked-pipeline tests pin this round trip. One shared operand-telescope
  normalizer now places an attached receiver at position zero and otherwise
  preserves the explicit parameter order; use-site matching, top-level overlap
  identity, and trait-owned overlap identity consume that same key. A trait
  rejects a second binding of one token over the same alpha-normalized operand
  telescope while distinct operand telescopes remain legal. A trait-backed token
  use still needs to consume one exact conformance already selected by a proof-static
  binder, rejects with none or several, and never searches visible conformances.
  A concrete direct wrapper may crown only one token meaning per normalized
  operand signature; alternative conformances remain named explicit calls.

Acceptance: `operator + Vec2::add`, `boundary operator [] Slice::index`, and a
trait-owned operator round trip through the semantic pipeline; named calls and
token calls resolve to one declaration identity; adding an unrelated visible
conformance cannot change token meaning; and no accepted source contains a
`spelling` clause.

### Boundary realization and nominal binding identity

- The parallel boundary-primitive registry is retired. The top-level
  `provider Name : Category;` item and operator-local `provider Name` clause no
  longer parse or survive in syntax identity/snapshots; `ProviderCategory`, its
  whitelist validator/collector, registry report rows, architecture coverage,
  and registry-only canaries are deleted. Core slice, pointer, array, and vector
  declarations are provider-agnostic exact boundary requirements, while
  allocation and host-ABI realizations use the ordinary satisfier/external-leaf
  path. Authored provider fields on ordinary data remain unrelated payload.
- Boundary declarations are provider-agnostic. Candidate `ProviderPlan` rows
  derive from exact satisfier identities and complete normalized contracts;
  target defaults, authorized `build.omg` provider slots, or installation-owned
  bindings select complete candidates without constructing rows. The selected
  canonical fact set survives checked lowering and retains exact requirement,
  realization, target applicability, artifact, admission, and receipt identity
  through provider execution, Terminal installation records, manifests, trust
  reports, and diagnostics. Missing, duplicate, partial, name-only, or
  identity-drifting selections reject.
- Replace string-backed intrinsic identity. `Binding::CompilerIntrinsic` has no
  authored payload; its exact resolved realization-machine symbol, normalized
  signature, and selected target key the sealed compiler lowering catalog.
  Rendered-binding reparsing and the write-only rendering interner are deleted;
  symbol resolution interns a closed structural binding key whose mechanism and
  field boundaries remain identity-bearing without text reconstruction.
  External leaves now retain a closed mechanism tag in their supply mode;
  validation, public snapshots, and contract fingerprints consume that tag
  rather than classifying a `Binding::Case(...)` prefix. Resolved and typed
  conformance rows retain the exact `ExternalBindingId` instead of a copied
  rendering; the complete structural binding table now survives into typed
  trees and exposes a fail-closed identity lookup, so consumers can migrate
  away from syntax rescans without reintroducing a text parser. Typed public
  snapshots publish each table id with its closed structural mechanism and
  fields; a payloadless intrinsic snapshot therefore cannot invent a machine
  name. Provider-plan
  candidate derivation now walks only typed machines, exact conformance/supply
  identities, and that table; its API accepts no syntax tree that could
  redirect a retained binding.
  Selected external ABI rows now derive only from the immutable selected
  `ProviderPlan` carrier produced from that identity; backend preparation no
  longer rescans `via` syntax as a second binding authority. Other consumers
  distinguish external from checked satisfiers by identity presence, and the
  interner exposes no rendering lookup. The source
  binding case and its normalized structural value are now payloadless; the
  exact normalized realization-machine identity remains on the enclosing
  provider row instead of being copied into the binding value. The
  core/std/canary corpus uses that form, and the seven obsolete authored-name
  mismatch canaries are deleted in favor of one parser regression for the
  retired payload. Transitional provider rows
  now retain the compiler-derived normalized realization-machine overload
  identity through ABI planning, trust reports, and executable-TCB identity.
  Provider rows no longer retain a separate catalog/display string. A single
  diagnostic-only renderer derives readable float labels on demand from the
  exact checked operator namespace, requirement, types, and arithmetic policy;
  it is never binding identity or dispatch input. Float execution resolves
  directly from that checked structure and selected target, while the old
  display-string catalog matcher and its permutation test are deleted.
- Replace string-backed foreign binding identity with nominal `LibraryId`,
  `SymbolId`, `CallingPlanId`, firmware/table, vtable, and mechanism-specific
  values. Raw object-format library/symbol bytes may exist only in sealed
  target/link metadata and must never serve as an Omega symbol, requirement key,
  provider selection, or checked-artifact identity.
- The console and target-float corpus has migrated mechanically to payloadless
  `CompilerIntrinsic`; legacy payload syntax rejects and is not compatibility
  parsed. Complete the nominal provider/catalog carrier described above; no
  authored intrinsic string may reappear in semantic lowering.

Acceptance: the same boundary requirement can select a checked test provider or
a target intrinsic without editing its declaration; final artifacts contain no
primitive-provider registry or provider categories; an intrinsic lowering is
selected only by exact realization symbol/signature/target; and changing raw
foreign linker bytes preserves the nominal Omega symbol while changing pinned
target/artifact identity and forcing fresh admission.

### Float providers

Owner: `wiki/design_briefs/float_semantics.md`.

Remaining F7 work:

- provide feature-qualified x86-64 FMA or a checked binary32/binary64 software
  implementation, then select the generic x86 FMA slots;
- retain equally target-specific semantic-edge evidence for every other
  admitted hardware realization; and
- complete the wider proof/`Real` connection under N6/N8.

Checked-result float/integer conversion remains blocked on the separate
checked-result arithmetic decision listed below.

### Lifetimes, dynamic traits, and build-time evaluation

- Finish outlives constraints, persistent/parameter-backed owners, aggregate
  borrow propagation, runtime-index expressions beyond exact immutable
  local/state-parameter forwarding, loan-root rebasing, and exact R5 facts.
  Direct helper-call results and moved/projected borrow-carrying aggregates now
  retain exact source loans, enclosing field/fixed-index paths, and polarity
  when nested inside another literal. Same-carrier denotation-preserving value
  casts also retain those loans at root and nested positions across moved,
  helper-produced, and literal operands. Validated shared/mutable recasts over
  whole name/member places now publish the exact source loan too; indexed
  byte-region recasts remain conservative until their complete target footprint
  can enter overlap facts. Remaining computed aggregate expression forms still
  need the same propagation law.
- Materialize dynamic descriptors for pass-through, rebound, and escaping
  borrows from the retained exact conformance rows and declaring-trait symbol.
  Bodyless/bare requirements do not license `dyn`; ambiguous same-carrier
  boundaries name the exact complete conformance.
- Complete hermetic evaluation with crash refinement, target capsule, separate
  result/usage identities, deterministic progress, and runtime equivalence.
  Publish `Hermetic | Receipted | Volatile` ceilings and realized provenance.
- Finish member reflection (`Self::fields` and field/case splices), constant
  positions, and proof checking of generator-expanded bodies.
- Complete the ordinary `Build` API/executor with exact dependency aliases,
  package-scoped providers, no ambient filesystem escape, and generated-source
  rechecking under consumer ceilings.
- Harden resolution with content/revision checks, archive containment, limits,
  scoped writes, receipts, and one dependency/build/trust lock. Any imported
  claim-set diff invalidates root acceptance; release providers are hermetic or
  receipted, and volatile observations cannot pass source-rebuildable release.

### Components and executable trust

- **FFIVAL:** run the narrow Windows `user32` boundary-coherence slice after
  ENT4, using existing activation, custody, registration, stack, and reach
  machinery.
- Extend component artifacts with stack needs, mapping cohorts, two-sided
  import/export checks, boundary multiplicity, custody receipts, and enumerable
  roots. Drain/coexistence, scheduling, and provisioning remain runtime work.
- Implement serialized capability attenuation/revocation only after the
  component carrier and custody rules are complete.

### Atomic ordering and device protocols

- **ATOMIC-EVENT-MODEL — design blocked.** Define the formal portable event
  model and x86-64/AArch64 refinement before enabling general protocol
  verification or global-order fences. Placed atomic accessors, checked ISA
  barriers, and installed-root same-context evidence do not wait for it.
- Add sealed provider requirements for DMA publication/acquisition, cache
  maintenance, MMIO notification, and posted-write completion. Every emitted
  requirement must be discharged or reject.
- Bind publication evidence to exact range/write state so intersecting writes
  invalidate it. Acquisition consumes request- and instance-bound completion
  evidence. Terminal Psi retains the actual ordering event; erased proof values
  and generic call effects are not lowering barriers.

### Wire runtime and executable installation

- Extend repeated encode/decode to `Vec<T>` after allocator obligations land.
  Packed scalar decode into `&[T]` remains unsupported because variable-width
  encodings cannot form a zero-copy scalar view.
- The retained selected provider plan, sealed provider execution, exact
  installed entry, and post-handoff writer context now join to the matching AOT
  fragment. That bound invocation now consumes one exact activated mapping plus
  a provider receipt for nonempty write rights, pinning, and non-publication,
  and returns a written-but-still-unpublished destination while failed linear
  transitions return every input. Implement consumer semantic validation and
  publication, physical AOT invocation, trusted/PCC and final-footprint
  validators, target W^X/coherence reporting, and uninstall/replacement joins.
- Keep arbitrary runtime bytes-to-code, JIT, and raw executable addresses
  unsupported.

Acceptance: only an admitted reusable artifact plus consumed placement authority
can produce installed code; validation binds exact final bytes and placement.

## Blocked index

These are pointers to the owning question or open design item, not duplicate
specifications:

- **EXTERNAL-ENTRY-STACK-DOMAIN:** owner Q3.
- **UEFI-PHYSICAL-SEMANTIC-ENTRY-COMPOSITION:** owner Q1.
- **SUM-MATERIALIZATION:** tagged-case placement vocabulary in
  `wiki/language_guide/appendix_open_questions.md`.
- **ATOMIC-EVENT-MODEL:** portable atomic axioms and target refinement choices
  in `wiki/language_guide/appendix_open_questions.md`.
- **CHECKED-RESULT-ARITHMETIC:** public carrier ruling for failure-returning
  checked arithmetic.
- **IMPORTED-CRASH-CAPSULES:** realization/import/certificate identity in
  `wiki/language_guide/appendix_open_questions.md`.
- **NATIVE-LOGICAL-FUEL-METERING:** owner Q2.
- **PROGRESS-PROFILE-CLASSIFICATION:** owner Q4.
- **QUOTIENT-RESPECTS-CARRIER:** owner propositions/quotients; define the sealed
  compiler-derived positional-telescope interface and its checked/terminal
  evidence identity without an arity ladder or authored adapter.

## Platform-gated verification

- Run the Linux host/time/filesystem and `IntegerAt` metadata paths natively on
  AArch64. x86-64 WSL and cross-target structural coverage already exist; do
  not claim runtime verification without the host.
- Build and run the Windows GUI callback canary through ENT4; do not pass a raw
  code address or add a Win32-only escape.
- Keep unavailable hosts structurally tested and report the missing runtime
  leg explicitly.

## Deferred until a real customer

- fault-tolerant component restart: define closed-custody component closure,
  explicit owner-death protocols for shared resources, external device reset or
  transaction obligations, and target-supplied isolation evidence together;
  abandonment-frontier reports alone must never license survivors;
- concurrent whole-system composition proofs for deadlock, starvation, memory,
  and response bounds;
- richer measured-recursion guards and multi-subject lexicographic cycles;
- reduced-rational divisibility theory beyond current quotient work;
- asynchronous extent revocation beyond provider quiescence;
- non-blocking executable-visibility tokens;
- runtime-generated host code, JIT, and arbitrary self-modifying code;
- independent final-byte CFI certificates and optional
  CET/PAC/shadow-stack hardening;
- universe levels before a full math-library replay goal;
- reusable fragmented allocation until a growable-container/backend customer
  states its retirement, authority-return, and immediate-reuse demands; and
- an optimizing SSA/register-allocation/SIMD backend beyond current correctness
  requirements.
