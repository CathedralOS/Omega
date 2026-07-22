# Design Brief: OS Memory And Hardware Foundation

Current direction as of 2026-07-18. The primitive taxonomy and security model
are settled enough to guide implementation. Exact source types, several plan
vocabularies, and backend validators remain open and are listed explicitly
below.

This brief is the common foundation for MMIO, page tables, DMA, shared-memory
IPC, descriptor tables, interrupt entry, admitted executable installation, and
early multicore boot. These are not separate language features.

## Governing invariant

An address is data, not authority. Computing or copying an `addr` grants no
right to read memory, write memory, install a mapping, execute code, or transfer
control.

Every operation that affects hardware, arbitrary memory, or external control
flow must be attributable to one of three sources:

1. checked Omega operations over storage and authority the checker can track;
2. compiler-understood low-level operations with complete instruction or plan
   contracts; or
3. an explicitly admitted provider whose unprovable commitments are represented
   by grants and receipts.

There is no raw fourth route. Wrapping an operation does not erase its service
reach, authority requirement, clobbers, trust expenditure, or installed-root
status.

## The reusable pieces

| Piece | Meaning | Primary customers |
|---|---|---|
| `Extent` capability | authority over one concrete address range with rights, provenance, address space, and lifetime | mappings, MMIO, DMA, IPC, allocators |
| `LayoutPlan` | physical geometry: offsets, alignment, overlays, bit and fragment placement, endianness | foreign records, IDT/GDT, page tables, protocols |
| `AccessPlan` | permitted primitive access: read/write/atomic, width, observation, ordering contract, RMW permission, service reach | MMIO and shared storage views |
| placed view | a checked `Extent + LayoutPlan + AccessPlan` interpretation | registers, framebuffers, IPC pages |
| parsed checked assembly | target instructions whose contracts emit effects, authority, clobbers, state changes, and exits | control registers, port I/O, fences, mode changes |
| boundary entry plan | one normalized contract containing a `CallPlan` and a `StatePlan` | firmware entry, interrupts, exceptions, syscalls, callbacks |
| symbolic materialization | toolchain-resolved identities placed into structures at the last legal phase | IDT targets, image symbols, callbacks |
| executable-artifact installation | validate and place immutable admitted code under scoped authority; never convert arbitrary bytes to code | boot images, components, AP trampolines |
| external-root ledger | all installed inbound roots plus their effects, trust, stack domains, preemption relations, and version pins | interrupts, callbacks, runtime entries |
| external loan | a linear token standing in for a borrower the checker cannot observe | DMA and device ownership transfer |
| carry/runtime contracts | value demands joined with scheduler/storage behavior at admission | suspension, migration, CPU/thread affinity, address stability |

These pieces reuse existing declaration forms: `data`, `machine`, `trait`,
`domain`, `boundary`, ordinary contracts, linearity, capabilities, and plan
policies. No interrupt DSL, `volatile` qualifier, external-satisfier keyword,
instruction-wrapper keyword, or parallel admission system is introduced.

## Extents are not allocators

`Arena` is bounded allocation authority: it permits drawing storage from a
resource under capacity and lifetime rules. A borrow-backed Arena is affine; an
owned-backing wrapper derives linearity from its Extent. The returned
`Allocation<T>` borrows its Arena and carries typed establishment/ownership; it
is not itself the allocator or a fresh root authority. See
[`allocator_story.md`](allocator_story.md).

A placed view instead needs authority over an
already-existing range that was not allocated by the program, such as a UART
register block. That is an `Extent`.

The public carrier is one opaque linear declaration with no public constructor:

```omega
data Extent [linear] {
    // Provider-owned representation.
}
```

Address space, rights, provenance, and mapping era are sealed domain facts on
that carrier, not nominal carrier types or generic parameters. Physical,
virtual, I/O-port, and provider-defined spaces share the same range algebra.
An operation requiring `Physical` statically rejects an extent carrying only
`Virtual`; unproven facts gate rather than cast. Rights such as `Readable` and
`Writable` are grant-established facts: provider evidence or a conservative
derivation may establish them, but address bits and structural observation
never do.

An extent records privately at least:

- base and length;
- address-space identity (physical, virtual, I/O, or provider-defined);
- read/write/execute or more specific rights;
- minting provenance, parent grant, and authority-origin/split ancestry;
- lifetime or mapping era; and
- ownership sufficient to split, attenuate, borrow, release, or revoke it.

Admitted suppliers mint root extents: boot handoff, an address-space mapper, a
parent allocator's backing store, or a device provider. Ordinary checked code
may derive children but never mint fresh authority. Bare `addr` values never do.

Splitting consumes one owned extent and returns disjoint owned children whose
ranges exactly cover it. Attenuation may only remove rights. Merge consumes
contiguous compatible descendants of the same authority origin; numeric
adjacency alone is insufficient because adjacent ranges may have different
grants, rights, provenance, or eras. The ordinary case is rejoining what one
split separated. Combining unrelated adjacent grants, if needed, is an explicit
provider operation that establishes the combined authority.

Subrange loans are borrow-carrying values. Their polarity follows the parent
borrow: shared loans permit only shared operations; exclusive loans permit
ordinary mutation. Loans are not linear cleanup obligations. The owned extent,
DMA tokens, shootdown tokens, and similar authority/debt values remain linear.

The normalized conservation model is live in `omega-extents`. Its opaque Rust
carrier is non-clonable; an admitted one-shot root grant is the only mint;
space, provenance, era, and lineage identities are normalized; rights are an
open set of normalized identities rather than a compiler-blessed enumeration;
and split, attenuation, sibling merge, and bounded shared/exclusive loans are
validated. Failed consuming operations return every input authority. The
remaining work is connection to the Omega `[linear]` carrier and sealed domain
facts, followed by provider mapping and reclamation.

### Mapping and reclamation

Mapping requires authority on both sides. A requested `addr` may be a placement
hint, but it never authorizes occupation of a virtual range. Fixed placement
therefore consumes an owned extent in the virtual space. Automatic placement
draws that destination from a caller-supplied virtual-space allocator whose
own authority descends from the destination Extent.

The source relationship is independent: a mapping may consume an owned physical
extent or borrow one. A mapped virtual extent retains that relationship, so a
borrowed source cannot be reclaimed while the mapping lives. Conceptually:

```omega
machine map_owned(source: Extent, destination: Extent) -> Extent
    requires source in Extent::Physical
    requires destination in Extent::Virtual
    ensures result in Extent::Virtual
    ensures result in Extent::Mapped;

machine map_borrowed(source: &Extent, destination: Extent) -> Extent
    requires source in Extent::Physical
    requires destination in Extent::Virtual
    ensures result in Extent::Virtual
    ensures result in Extent::Mapped;
```

The exact overload spelling is engineering; the ownership distinction is law.
Unmapping consumes the mapped extent, returns the reusable destination range,
and either returns an owned source or ends its source loan. On targets requiring
cross-core invalidation, reuse remains gated by a linear shootdown/quiescence
token. Its completion operation carries the provider's ordinary `Suspend` or
`Block` ceiling, so an interrupt root cannot hide an illegal wait.

V1 has no per-access generation probe. Reclamation requires exclusive ownership
back and therefore no live in-language views. Forced asynchronous revocation of
unreclaimed loans is deferred to provider quiescence/lifecycle machinery when a
customer requires it; page-table edits, shootdowns, and process teardown remain
ordinary runtime provider work.

The provider-neutral mapping lifecycle is live in `omega-extents`. An admitted
mapping grant pins source custody, source/destination spaces and required
rights, provider-established mapped facts, and an open set of translation
release facts. Fixed mapping consumes the destination Extent and independently
owns, shared-borrows, or exclusive-borrows its source. Shared source custody
cannot expose mutable mapped loans. `begin_unmap` retains every authority until
an exact provider receipt establishes that stale translations are released and
all target completion facts hold; only then are the destination and any owned
source returned. Provider page-table operations, `Suspend`/`Block` reach, the
Omega source carrier, and automatic destination allocation remain.

Zero-filled storage does not establish a linear extent and creates no
must-consume obligation. A zero-usable pool uses the ordinary debt-free sum:

```omega
data ExtentSlot {
    case Empty;
    case Live(extent: Extent);
}
```

This preserves the useful ZII distinction: zero bytes remain physically
representable, while establishment decides whether those bytes are accessible
as an authority-bearing value. Bulk structures should retain meaningful zero
states where honest; authority and foreign validity must never be minted by
zero-fill.

## Layout and access are separate plans

`LayoutPlan` answers where bits live. `AccessPlan` answers how a placed range
may be touched. Combining them would force wire formats to carry MMIO concepts
and hardware plans to carry codec concepts.

The layout vocabulary needs name-keyed entries and fragmented placements so one
logical source can occupy several destination ranges. Validation must check both
destination overlap and exact source tiling: every source bit appears exactly
once unless an explicit overlay rule says otherwise.

The access vocabulary stays small and compiler-owned:

- read, write, and atomic transfer classes;
- exact transfer width;
- stable, volatile/external, or atomic observation;
- generic read-modify-write permitted or forbidden;
- provider-private versus exported access; and
- the statically pinned boundary-trait reach of the accessor.

The raw primitive is not a public `volatile_read(extent, arbitrary_offset)`
escape hatch. A provider validates extent provenance, `LayoutPlan`, and
`AccessPlan`, then derives an opaque placed view. Its public primitive surface
is projection plus only the operations authorized for that field:

- `read()` performs one exact-width access and returns a snapshot;
- `write(value)` performs one whole-container write; and
- explicitly atomic fields expose the ordinary checked atomic API (`load`,
  `store`, `compare_exchange`, and authorized atomic RMW operations).

Pure projection narrows authority without performing I/O. Projected field
accessors are ordinary passable borrow-carrying values and cannot outlive the
mapped extent. Whole-view methods may coexist as ergonomic forwarding machines;
projection is the least-authority surface for helpers that need only one field.

Borrow polarity remains authoritative: a shared projection permits reads;
ordinary writes require an exclusive borrow; mutation through a shared borrow
is legal only through an explicitly atomic or protocol-safe field. Placed views
must not smuggle aliased unsynchronized mutation around the ownership checker.

For example:

```omega
let status_register = uart.status;
let status = status_register.read();
```

One container-width read produces one snapshot. Bit projections from that
snapshot are pure. Flow facts attach to the snapshot value, never to storage
that hardware or a peer may change.

Generic RMW is never derived for an ordinary MMIO field. Device behaviors such
as W1C, read-to-clear, FIFO pop, and doorbells remain package machines over
provider-private primitive access. This does not forbid atomic RMW on an
explicitly atomic shared-memory field; those are different access classes.

The normalized `AccessPlan` foundation is now implemented independently of its
future source spelling. It validates name-keyed entries against fixed
`LayoutPlan` geometry, exact whole-byte transfer width, observation/operation
compatibility, provider-private external RMW, and statically pinned reach. Its
operation gate already enforces shared-read/exclusive-write polarity while
allowing explicitly atomic shared mutation. Successful geometry validation
mints a sealed field descriptor containing the plan-derived container offset;
successful operation authorization combines that descriptor with borrow
polarity and the selected operation. Primitive lowering accepts only this
authorized value, never an author-supplied offset. Source-policy evaluation,
source-level borrow-carrying access values, and the exact external/atomic
lowering remain.

The normalized Extent/AccessPlan join is also live. A reusable
provider-admitted placed-view grant pins address space, provenance, required
open-set rights, and permitted static reaches. Derivation checks those facts
and layout size against an actual borrow-carrying Extent loan. Field operation
authorization then derives shared/exclusive polarity from that loan rather
than accepting a caller claim, and its sealed lowering value remains borrowed
from the view. Omega source projection and target-specific primitive emission
remain.

The extent's provenance gates construction of an access capability. The
accessor's normalized contract statically pins service reach. Runtime
provenance never changes a machine's effect row.

## Page tables, IPC, and DMA

Page tables use a hybrid correct-by-construction path. Mapping operations
require frame/mapping authority and preserve provenance incrementally;
`finish()` establishes an `Installable` domain; installation accepts only that
domain. Imported tables may instead be scanned once to establish the same fact.
This avoids rescanning every locally-built table without trusting arbitrary
address bits.

Shared-memory IPC and MMIO share external mutability, not an observation model.
For proved or mutually trusted peers, an atomic protocol may return a linear
lease whose borrow exposes stable payload bytes until explicit `release`.
Against a hostile writable peer, protocol cooperation proves nothing: the
receiver must copy then validate, or a provider must revoke/remap the peer's
write permission and complete required cross-core invalidation before zero-copy
validation.

DMA is an external borrow. A linear transfer token is the checker's proxy for
the invisible device:

- device-read is a shared loan: CPU mutation is excluded;
- device-write is an exclusive loan: CPU reads and writes are excluded;
- bidirectional sharing requires an explicit atomic/coherence protocol; and
- completion consumes the token, performs the required cache/fence contract,
  and returns the loan.

The token may remain live across suspension; waiting for completion is normal.
The provider receipt is the accepted claim that completion really means the
device has stopped using the range.

This conservation model is live in `omega-extents`. A reusable admitted grant
pins the borrower, direction, space, provenance, required open-set rights, and
an open set of completion facts (including target fence/cache facts where
needed). Starting a transfer accepts an actual Extent loan and
derives CPU exclusion from its polarity. The non-clonable proxy holds that
borrow until a matching provider receipt establishes completion and every
required ordering/coherence fact; failed starts and completions return their
borrow-carrying inputs. Omega `[linear]` integration, permission-context events,
provider execution, and the DMA vertical slice remain.

## Checked assembly is the low-level operation surface

`asm { ... }` is parsed target assembly, not an opaque byte blob. Every accepted
instruction has a compiler-owned contract covering applicable machine regime,
required authority and facts, service reach, register/flag/memory changes,
ordering, and control exits.

Direct checked assembly and a higher-level boundary operation must contribute
the same normalized reach. For example, `asm { wrmsr }` cannot be the quiet way
around a `MachineControl` ceiling. Unknown instructions and raw byte emission
are rejected. Prebuilt foreign code enters through provider admission instead.

The catalog distinguishes user-available checked instructions from deriver-only
entry/exit instructions such as `iretq` or `sysret`; user code cannot manufacture
an unmodeled control exit. Regime-changing instructions state their transition
directly: require regime R, establish regime R'.

The former `Binding::Instruction` duplication is retired; parsed checked
assembly is the only source-level instruction surface.

## Boundary entry plans

An ordinary boundary call often makes ABI placement and machine-state policy
look identical. Interrupts prove they are independent.

```text
CallPlan
  parameter and result placement
  stack alignment and ordinary ABI clobbers
  entry/return control shape

StatePlan
  initial machine regime
  interrupted state set
  state saved and restored by the stub
  permitted transitive machine-state use
```

A normalized boundary-entry plan carries both. The requirement cites the
calling policy through ordinary trait composition (`Calling<C>`). `C` satisfies
`CallingPolicy`; its build-time-admissible `plan` machine evaluates the
normalized signature to `Accepted(BoundaryEntryPlan)` or a structured rejection.
Accepted plans are compiler-validated and canonicalized. The canonical evaluated
plans, not the policy symbol, machine source, or construction order, enter the
published contract identity. Policy authorship is open to ordinary packages,
while the plan vocabulary and validator remain compiler-owned and closed.

Implementation evidence is firewalled from that identity. A provider artifact
contains its emitted transitive state/register footprint and validation result.
Changing register allocation or choosing a different legal implementation does
not change caller contract identity.

State ceilings constrain instruction selection and register allocation before
code is emitted. Codegen may clone a machine under a different state ceiling;
this is contextual specialization, not generic type monomorphization, although
the backend may share cloning and cache infrastructure. The final realized
artifact is validated after inlining, relaxation, veneers/thunks, generated
stubs, and admitted indirect leaves:

```text
actual_transitive_footprint subset_of StatePlan.permitted_state
actual_clobbers intersect unsaved_interrupted_state = empty
```

Checked Omega code produces checkable footprint evidence. Admitted leaves
supply accepted footprint claims under receipt. The validator, not backend
optimism, decides acceptance.

The first backend consumer now derives inbound runtime-frame storage writes
from the complete validated boundary plan. Each target encoder publishes the
registers its generated copy fragment overwrites; derivation unions those
clobbers into implementation-only `StateFootprintEvidence`, rejects a selected
input register destroyed before capture, and validates the fragment against the
plan's state ceiling. This evidence covers only inbound storage realization.
The normalized calling-plan foundation also composes any number of fragment
footprints by deterministic register/state union and validates the aggregate
against one retained boundary plan. Ordering and duplicate fragments cannot
perturb that implementation-only evidence fingerprint. Validated inbound
fragments now remain attached to the abstract-operation plan with
`entry_storage` provenance, and `08_boundary_footprints.json` publishes the
fragments plus their composed fingerprint. Its explicit
`enumeration_complete: false` status is a firewall: this retained slice is
checkable implementation evidence, not yet the final certificate.
The final certificate must still aggregate the specialized handler body,
save/restore and exit sequences, relaxation products, veneers/thunks, generated
stubs, and admitted indirect leaves after final placement.

## Symbolic materialization and admitted executable installation

Runtime-known addresses are ordinary `addr` data, used only with separate
authority at mapping, installation, or access. Toolchain-known identities do
not become user-visible code addresses. Plans refer to a closed symbolic source
vocabulary such as:

```text
RelocationTarget = Data(DataSymbolId) | Entry(EntryStubId)
```

The materializer resolves a symbolic source at the last legal phase: fixed
image layout, native relocation, loader relocation, or a generated runtime
writer. Split IDT offsets require the fragment plan above; they must not be
reconstructed by user arithmetic.

Each target also declares its consumption phase. A field the loader consumes
before the first Omega instruction must fit the object format's native
relocation vocabulary. Post-handoff structures may use the generated writer.
The normalized writer program is now derived from the same actions: it
validates the concrete placement, resolves each sealed target once through the
provider, stages every fragment, and publishes only after the entire write can
succeed. Target-machine emission of that program remains engineering work.
Placement plans may constrain range, alignment, phase, machine regime, and
scoped artifact-installation authority. The normalized materialization
foundation now carries those five facts: policy alignment is joined with the
layout's alignment, compiler-issued identities cite regime and installation
scope, and a concrete-site validator checks the complete occupied range before
linker/loader/provider consumption. Propagation through the final artifact
pipeline remains engineering work.

There is no general `ExecutableMemory` capability, arbitrary byte-to-code
conversion, JIT facility, or self-modifying-code path. Executable eligibility
is the sealed `Artifact::AdmittedExecutable` domain fact over a reusable
immutable `Artifact` carrier. Admission binds it to normalized content,
identity, relocation plan, footprint, and placement plan; packages cannot
self-establish it. Mutation destroys that eligibility. Runtime loading borrows
the admitted artifact and consumes linear placement authority; it never
consumes the reusable artifact itself.

The normalized lifecycle is:

```text
ArtifactCandidate
    -- canonical decode + PCC/contract admission -->
Artifact in Artifact::AdmittedExecutable                 reusable

CodePlacement                                            linear, W + NX
    + borrow admitted artifact
    -- materialize declared sections/relocations -->
FrozenPlacement                                          linear, R + NX
    -- validate exact final bytes/footprint -->
ValidatedPlacement                                       linear, R + NX
    -- installer provider + visibility completion -->
InstalledCode                                            linear, R + X
```

These are normalized semantic states, not a promise of literal generic source
types. Content/placement binding prevents transplanting a validation
certificate to different bytes; linear placement ownership prevents spending
one destination twice. The certificate may remain reusable/reportable while
`ValidatedPlacement` is consumed.

This normalized ladder is live in `omega-executable-installation`. Canonically
decoded artifacts are immutable and reusable; exact admission evidence checks
content, contracts, declared footprint, and placement plan before establishing
the executable qualification. A one-shot authority claims an Extent-backed
placement, materialization checks the admitted artifact's real size and freezes
writes, the final certificate is bound to artifact + placement + final bytes +
realized footprint, and installation consumes an authority scoped to that
artifact, admission, placement, scope, and audience. Synchronous visibility and
`HardwareEnforced | ConventionOnly | Unsupported` W^X reporting are checked.
Failed linear transitions return their inputs. Container decode, actual PCC and
final-code validators, provider execution, Omega linear integration, and live
replacement remain.

`CodePlacement` now consumes the existing placement-plan vocabulary rather
than duplicating it. The one-shot authority carries normalized range,
alignment, phase, machine-regime, and installation-scope constraints plus the
provider's concrete site. Claiming the Extent checks its actual base and length
against that site and runs the shared `PlacementConstraints` validator before
materialization. A caller cannot substitute a friendlier placement hint.

The normalized retirement path is live as well. It consumes one exact
`InstalledCode` plus authority scoped to its artifact, placement, and scope;
visibility evidence cannot satisfy it. The provider receipt must separately
establish executor quiescence, removal of execute permission, restoration of
write authority, and every open target completion fact. Only then does the
placement return to W+NX for a later admitted artifact. The runtime
quiescence/provider implementation and component-slot orchestration remain.

This invariant covers every route to execute permission. Correct-by-construction
page-table APIs require admitted-artifact provenance before deriving an
executable mapping, and checked assembly emits the same installation authority
and reach obligations rather than exposing a back door. Device firmware and
GPU/NIC programs are device-provider uploads, not host executable artifacts.

Installation performs final post-materialization validation, target-specific
W^X transition, cache maintenance, ordering, and instruction-fetch
synchronization through one contracted provider operation. Its authority is
scoped to the admitted artifact identity, `CodePlacement`, and audience.
`CodePlacement` composes existing physical/virtual `Extent` authority and
placement constraints; it is not a component dispatch slot or a new parallel
authority family. Component-slot binding happens later and separately. AP
bringup installs a compiler-produced low-memory trampoline and
then invokes a target boot protocol; it does not generate host code at runtime.

Audience state has three cases:

1. a dormant/local target needs local installation completion;
2. a future remote fetcher needs a visibility-completion fact before entry; and
3. a possible current executor requires component replacement and quiescence.

Visibility gates future entry; quiescence gates retirement of existing code.
They may share linear-obligation infrastructure but establish different facts
and are not one token algebra. Template patching of already-live code uses
admitted fragments at declared patch sites through the replacement path. The
v1 loader completes visibility synchronously (it may itself suspend); a
non-blocking visibility token waits for a real provider customer. Successful
installation reports `HardwareEnforced` or `ConventionOnly` W^X; an
`Unsupported` provider rejects installation.

Installation prevents code injection. It does not by itself establish
control-flow integrity over already-admitted code. Sealed entry references and
final indirect-branch/return validation are a separate gate recorded in
`OWNER_QUESTIONS.md`.

The component container is a minimal canonical Omega-native artifact, decoded
through checked schema/layout machinery: bounded length-delimited tables,
checked arithmetic, content identity, a closed relocation vocabulary, and
explicit PCC/contract/footprint sections. It has no constructors, scripts,
ambient imports, recursive metadata, or permissive semantic extensions. An
ignorable section is informational only and contributes no admission authority;
anything affecting meaning or trust is required. UEFI may require a thin
PE/COFF boot envelope, but that envelope is not Omega's component format.

The normalized post-decode validator is live in
`omega-executable-installation`. It applies configured total/section/count
bounds, checked range arithmetic, non-overlap, exact presence and identity of
code/relocation/contract/footprint/placement/proof sections, and the required
versus informational unknown-section rule. Its output is only an immutable
`Artifact` candidate; executable qualification still requires the separate
admission receipt. Actual byte decoding through LayoutPlan/schema machinery,
content-identity computation, and closed relocation validation remain.

The boot base case preserves the same discipline:

```text
build validates PCC/CFI and signs an admitted artifact identity
    -> secure boot authenticates and gates entry
    -> measured boot records the entered identity
    -> the boot-admitted installer loads later admitted artifacts
```

Secure boot gates; measured boot records. Measurement alone never establishes
admission.

## External roots and interrupt tables

`boundary machine` already describes an inbound callable. Interrupts add no new
machine species. Installation makes the selected provider's entry stub an
external root because hardware reaches it without an Omega caller.

The root ledger is a normalized artifact, not user-authored prose. Each entry
records package-qualified requirement/provider identities, evaluated boundary
plan, artifact and receipt identities, authority and scope actually granted,
effects, stack domain, preemption/nesting relationships, and version/liveness
pins. Friendly names are presentation only.

The ledger closes three whole-program holes:

- effects and trust reachable only from hardware callbacks remain visible;
- WCSU composes across interrupt nesting and same-stack roots; and
- dynamic install, replacement, and removal are checked against version pins
  and quiescence.

The IDT is consequently a first serious customer, not a special construct:

1. ordinary `data` describes the logical gate;
2. an x86 layout policy supplies bit and fragment placements;
3. a target-specific interrupt requirement pins `CallPlan + StatePlan`, stack
   class, acknowledgement protocol, and effect ceiling;
4. build/provider selection chooses a satisfying handler;
5. the materializer resolves its sealed entry-stub identity into gate bits;
6. checked `lidt` installation requires IDT authority and records roots; and
7. a linear acknowledgement token forces exactly-once completion.

Static IDT construction does not require a source-visible first-class entry
reference. The selected plan can retain the entry identity privately. Reified
entry references remain deferred until dynamic callback registration supplies a
real source-level customer.

## Carry contracts and runtimes

Canonical frontend place-liveness is shared infrastructure. WCSU, linear
consumption, permission/external-loan checking, and carry checking consume it
independently; they do not share an algebra.

Values/resources state independent demands:

- may or may not cross suspension;
- same CPU required or not;
- same host thread required or not; and
- stable address required or not.

The settled type-wide source is one compiler-built-in property over the full
product:

```omega
boundary data PerCpuLease [
    linear,
    carry(
        suspension: allowed,
        cpu: same,
        thread: any,
        address: movable,
    ),
];
```

The property lowers directly to normalized compiler IR. It is not ordinary
`omega::core` data, a trait, or the output of a policy machine: the vocabulary
is closed because the compiler must interpret every axis. Transparent data
derives structurally; opaque data with no declaration is maximally strict. An
opaque declaration is only a claim and remains inert until proved or accepted
under admission receipt. Constructor `ensures` may establish sealed per-mint
carry domains, monotonically adding permissions above the type-wide floor.

The old `[send]` placeholder is retired. Cross-activation exclusive transfer is
ordinary ownership plus carry/runtime compatibility; crossing shared references
also requires a sanctioned shared-access contract. Carry and sharing are not
marker traits and do not follow from copyability.

Runtime providers state independent behavior:

- safe-point or asynchronous preemption;
- migration behavior and available affinity/pinning;
- host-thread behavior; and
- stable or movable continuation storage.

Admission joins demand and behavior. The effect row stays static: a live mask
or affinity token may make a particular call locally inadmissible without
editing or masking the machine's published effects.

The normalized join is live in `omega-task-plans`: suspension is rejected
locally against possible park crossings, while provider admission selects a
safe-point or all-instruction migration envelope and checks CPU/thread
affinity, continuation stability, frame provisioning, cancellation, and inline
behavior. Missing opaque-runtime evidence is pessimistic. Compiler liveness
derivation and provider-plan integration remain.

The enforcement sites are deliberately asymmetric. A value that forbids
suspension is checked locally against possible `Suspend` reach; provider
selection cannot erase that ceiling. CPU affinity, host-thread affinity, and
address stability instead join the activation's demands with the runtime's
normalized behavior at admission. Preemption granularity selects which points
need those live-value checks. Runtime behavior is born pessimistic: a checked
provider proves narrower behavior, while an opaque provider needs an admission
receipt authorizing reliance on its narrower claim. The receipt does not change
behavior; it changes what admission may trust.

Structural composition selects the most restrictive live-field demand on each
axis; the axes share traversal, not an algebra. Interrupt masking and
scheduler-switch suppression are different linear tokens: the former defers
delivery; the latter prevents an Omega activation switch but cannot prevent a
host kernel from preempting its thread.

The Cathedral reference profile should begin with safe-point scheduling and
stable continuation storage. The language representation keeps the axes
independent so a stricter admitted asynchronous runtime does not require a
redesign.

Local checking, runtime admission, and future temporal verification are three
consumers of the same facts. Local checking combines canonical liveness with
carry policy at each transition. Admission joins accumulated demands with the
selected runtime. A future TLA-style layer adds interleavings, protocol state,
and liveness hypotheses; it consumes normalized policy and provenance rather
than re-reading source attributes.

## Implementation order

Ordinary generic trait-parent composition for `Calling<C>` is implemented. The
compiler also has the first normalized `CallPlan + StatePlan` model, built-in
evaluators for the currently supported x86-64/AArch64 host and syscall
policies, deterministic contract fingerprints, and a separate validated
footprint-evidence carrier. Concrete and generic `Calling<C>` relationships
are discovered, their policy machines are evaluated through the build-time
interpreter, accepted results are validated and canonicalized, and the
complete plan is retained through checked lowering. Authoritative stub
derivation, state-ceiling-aware codegen, final footprint validation, and the
concrete interrupt state policy remain. Remaining order:

1. Complete the checked-assembly instruction-contract catalog needed by the
   entry provider. No raw-byte shortcut.
2. Derive sealed data/entry identities from selected compiler artifacts,
   propagate normalized placement constraints through artifact construction,
   and lower the derived post-handoff writer program to target code. Name-keyed
   fragment placement, exact tiling, phase-aware action derivation,
   fixed-address resolution, early-consumption rejection, section-qualified
   absolute data relocation/rebasing, concrete-site validation, and the atomic
   provider-resolved writer program are already live. Decoded placement
   constraints are bound into artifact admission and rechecked against the exact
   claimed placement before materialization, preventing a provider from
   substituting a weaker range/alignment/phase/regime/scope record behind the
   admitted plan identity. Native symbolic actions
   lower with tagged materialization origin rather than pretending those sites
   came from an instruction. Canonical executable-container v2 now requires a
   validated entry-set section, binds that set's identity into admission, and
   lets only an admitted artifact select sealed `EntryStubId` targets present
   in the set. The exact installed-code state now privately resolves those
   entries against its placement while executing an atomic post-handoff writer;
   foreign entries and data symbols reject before publication, without a
   source-visible numeric-address operation. Target-machine writer emission
   remains.
3. Connect the implemented normalized `Extent` conservation/mapping model to
   the Omega linear carrier and sealed facts, then implement provider execution
   and source APIs. Root admission, split/merge/attenuation, borrow polarity,
   owned-versus-borrowed source custody, destination consumption, and
   receipt-gated translation reclamation are already checked.
4. Connect the implemented normalized `AccessPlan`/Extent join and sealed
   field-operation values to Omega-authored policies and source projections,
   then lower exact external/atomic primitives. Geometry, provenance, space,
   rights, size, static reach, and loan-derived borrow polarity are already
   checked.
5. Finish migrating lowering to the retained normalized boundary plan, derive
   inbound entry/exit stubs, constrain codegen by the selected `StatePlan`, emit
   footprint evidence, and validate final artifacts. Source policy evaluation,
   structured rejection, accepted-plan canonicalization, and evaluated-plan
   contract identity are complete.
6. Connect the live placement constraints to admitted-artifact validation and
   scoped executable installation.
7. Add the external-root ledger and IDT/timer vertical slice.
8. Connect the implemented normalized external-loan proxy to Omega linearity,
   permission contexts, and provider receipts; then build the DMA/hostile-IPC
   vertical slices.
9. Add carry/runtime admission and the Arena-backed Cathedral task profile.

## Gauntlet

The foundation is not complete because this brief says so. It earns confidence
when the same pieces implement, without new customer-shaped syntax:

1. UART MMIO with read-only, write-only, and W1C registers;
2. page-table construction and installation;
3. trusted and hostile shared-page IPC;
4. zero-copy DMA with completion and revocation;
5. IDT, timer interrupt, nesting, and acknowledgement;
6. SMP AP bringup through installation of an admitted low-memory trampoline and
   checked mode changes.

Required negative tests include hidden reach through direct assembly, forged
addresses without authority, stale hostile-peer validation, CPU access during
an external loan, a split relocation consumed too early, and a final-image
veneer/thunk that introduces a register class forbidden by the root's
`StatePlan` despite all earlier per-function checks passing. Extent/access tests
must also reject merging numerically adjacent ranges from different authority
origins and reject ordinary non-atomic writes through two shared projections.

## Open decisions

These are the remaining design questions, not permission to invent local
syntax while implementing:

- the final artifact-footprint certificate format and validation boundary for
  static and dynamically loaded admitted artifacts;
- the protected-return/final CFI contract tracked in `OWNER_QUESTIONS.md`; and
- the concrete x86 interrupt requirement, stack/preemption classes, and IDT
  materialization records used by the timer slice.

Dynamic source-visible entry references, movable continuations, asynchronous
revocation, live patching policy, and rich resource algebra remain deliberately
deferred until their owning customers are implemented.
