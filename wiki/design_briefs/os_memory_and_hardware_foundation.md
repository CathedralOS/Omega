# Design Brief: OS Memory And Hardware Foundation

Current direction as of 2026-07-18. The primitive taxonomy and security model
are settled enough to guide implementation. Exact source types, several plan
vocabularies, and backend validators remain open and are listed explicitly
below.

This brief is the common foundation for MMIO, page tables, DMA, shared-memory
IPC, descriptor tables, interrupt entry, executable publication, and early
multicore boot. These are not separate language features.

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
| external-root ledger | all installed inbound roots plus their effects, trust, stack domains, preemption relations, and version pins | interrupts, callbacks, runtime entries |
| external loan | a linear token standing in for a borrower the checker cannot observe | DMA and device ownership transfer |
| carry/runtime contracts | value demands joined with scheduler/storage behavior at admission | suspension, migration, CPU/thread affinity, address stability |

These pieces reuse existing declaration forms: `data`, `machine`, `trait`,
`domain`, `boundary`, ordinary contracts, linearity, capabilities, and plan
policies. No interrupt DSL, `volatile` qualifier, external-satisfier keyword,
instruction-wrapper keyword, or parallel admission system is introduced.

## Extents are not allocators

`Region` is allocation authority: it permits drawing storage from a resource
under capacity and lifetime rules. A placed view needs authority over an
already-existing range that was not allocated by the program, such as a UART
register block. That is an `Extent`.

An extent records at least:

- base and length;
- address-space identity (physical, virtual, I/O, or provider-defined);
- read/write/execute or more specific rights;
- minting provenance and parent grant;
- lifetime or mapping era; and
- ownership sufficient to split, attenuate, borrow, release, or revoke it.

Suppliers mint extents: boot handoff, an address-space mapper, a parent
allocator's backing store, or an admitted device provider. Bare `addr` values
never do. Splitting conserves the original range and authority; attenuation may
only remove rights. V1 revocation requires exclusive ownership back so no
in-language view can dangle.

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

Device behaviors such as write-one-to-clear, read-to-clear, FIFO pop, and
self-clearing commands are ordinary target-package machines over private
primitive field access. They do not become `AccessPlan` cases.

The raw primitive is not a public `volatile_read(extent, arbitrary_offset)`
escape hatch. A validated plan derives sealed field-access values. Pure
projection narrows authority without performing I/O; calling the field's
operation performs exactly the declared transfer:

```omega
let status_register = uart.status;
let status = status_register.read();
```

One container-width read produces one snapshot. Bit projections from that
snapshot are pure. Flow facts attach to the snapshot value, never to storage
that hardware or a peer may change.

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

`Binding::Instruction` is therefore transitional duplication and retires as
the checked catalog reaches its customers.

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
calling policy through ordinary trait composition (`Calling<C>`); `C` evaluates
against the signature to the complete pair. The evaluated plans, not merely the
policy symbol, enter the published contract identity.

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

## Symbolic materialization and executable publication

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
Placement plans may constrain range, alignment, phase, machine regime, and
executable-memory authority.

Executable memory has a lifecycle:

1. writable, not yet published;
2. finalized and published executable; and
3. replacement of already-published executable code.

First publication performs the target-specific W^X transition, cache
maintenance, ordering, and instruction-fetch synchronization through one
contracted provider operation. AP bringup is first publication followed by a
target boot protocol. Live patching is replacement and requires the component
versioning/quiescence machinery; it is not merely publication with a larger
`CoreSet` argument.

The publication target must record execution/liveness status, not merely a set
of cores. Publishing for a dormant AP that cannot currently fetch the range is
not the same authority or protocol as changing bytes that an executing core may
already be running. The exact evidence/API spelling remains open in
`OWNER_QUESTIONS.md`; the two lifecycle cases must not collapse meanwhile.

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

Runtime providers state independent behavior:

- safe-point or asynchronous preemption;
- migration behavior and available affinity/pinning;
- host-thread behavior; and
- stable or movable continuation storage.

Admission joins demand and behavior. The effect row stays static: a live mask
or affinity token may make a particular call locally inadmissible without
editing or masking the machine's published effects.

Plain transparent data derives permissive carry structurally. Aggregates join
field restrictions. Opacity stops derivation, and opaque/provider-minted
resources are born strict until their constructor/provider contract proves or
accepts a looser policy. Interrupt masking and scheduler-switch suppression are
different linear tokens: the former defers delivery; the latter prevents an
Omega activation switch but cannot prevent a host kernel from preempting its
thread.

The Cathedral reference profile should begin with safe-point scheduling and
stable continuation storage. The language representation keeps the axes
independent so a stricter admitted asynchronous runtime does not require a
redesign.

## Implementation order

1. Implement the already-designed parsed `asm {}` frontend and the first
   complete x86 instruction-contract catalog. No raw-byte shortcut.
2. Implement ordinary generic trait-parent composition needed by `Calling<C>`.
3. Extend programmable layouts to name-keyed fragmented placements and exact
   tiling validation.
4. Define and implement the `Extent` carrier, conservation rules, and mapping
   provenance.
5. Define `AccessPlan`, validation against `LayoutPlan`, sealed field-access
   derivation, and exact volatile/atomic primitives.
6. Split boundary entry planning into `CallPlan + StatePlan`; constrain codegen,
   emit footprint evidence, and validate final artifacts.
7. Add symbolic relocation sources, phase/constraint-aware materialization, and
   executable publication.
8. Add the external-root ledger and IDT/timer vertical slice.
9. Add external loans and DMA/hostile-IPC vertical slices.
10. Add carry/runtime admission and the region-backed Cathedral task profile.

## Gauntlet

The foundation is not complete because this brief says so. It earns confidence
when the same pieces implement, without new customer-shaped syntax:

1. UART MMIO with read-only, write-only, and W1C registers;
2. page-table construction and installation;
3. trusted and hostile shared-page IPC;
4. zero-copy DMA with completion and revocation;
5. IDT, timer interrupt, nesting, and acknowledgement;
6. SMP AP bringup through low-memory trampoline publication and mode changes.

Required negative tests include hidden reach through direct assembly, forged
addresses without authority, stale hostile-peer validation, CPU access during
an external loan, a split relocation consumed too early, and a final-image
veneer/thunk that introduces a register class forbidden by the root's
`StatePlan` despite all earlier per-function checks passing.

## Open decisions

These are the remaining design questions, not permission to invent local
syntax while implementing:

- the exact opaque `Extent` API, address-space representation, parent/child
  lifetime law, and v1 mapping-revocation protocol;
- the minimal normalized `AccessPlan` vocabulary and public field-access value
  shapes;
- the exact source-property vocabulary for carry/affinity/address-stability
  contracts and opaque-type defaults;
- the first-publication evidence/state types and how target boot protocols
  consume them;
- the final artifact-footprint certificate format and validation boundary for
  static, dynamically linked, and runtime-generated code; and
- the concrete x86 interrupt requirement, stack/preemption classes, and IDT
  materialization records used by the timer slice.

Dynamic source-visible entry references, movable continuations, asynchronous
revocation, live patching policy, and rich resource algebra remain deliberately
deferred until their owning customers are implemented.
