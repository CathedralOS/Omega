# Design Brief: Calling And Machine-State Plans

Current as of 2026-07-28. Boundary conventions are normalized policy artifacts;
Omega's internal calling convention remains compiler-sovereign. This brief now
includes inbound machine-state preservation, which ordinary calls do not expose.
Engineering is incomplete. The normalized compiler model, initial built-in
policy evaluators, recursive public fixed-array/record signature graphs, direct
source-policy evaluation, publication of the evaluated identity, and
relationship-span diagnostics are implemented through the current
`Calling<C>` surface. Migration to the explicit named-evidence
`Calling<C, Policy>` surface and authoritative lowering remain.

The live source policy ABI uses `u64` for every nonnegative size, alignment,
count, graph index, register ordinal, immediate, stack offset, stack class, and
preemption depth. These are integer quantities rather than addresses, so they
do not use `addr`. Build-time evaluation preserves the complete 64-bit source
value; normalization narrows to the closed compiler model's `u8`/`u16`/`u32`
fields only with explicit range checks.

## One boundary entry plan, two independent facets

An ordinary ABI is a layout over registers and a stack. It does not, by itself,
describe hardware entering while another activation is live.

```omega
data CallPlan {
    params: [Placement];
    result: Placement;
    ordinary_clobbers: RegisterSet;
    stack_align: count;
    shadow_bytes: count;
    entry_control: EntryControl;
}

data StatePlan {
    initial_regime: MachineRegime;
    interrupted_state: MachineStateSet;
    saved_state: MachineStateSet;
    restored_state: MachineStateSet;
    permitted_transitive_use: MachineStateSet;
}

data BoundaryEntryPlan {
    call: CallPlan;
    state: StatePlan;
}
```

`CallPlan` owns parameter/result placement and ordinary ABI behavior.
`StatePlan` owns the state that already belonged to an interrupted activation,
the state an entry stub preserves, and the state the handler and its callees may
use. Their projections coincide for many ordinary calls; that is not a reason to
fuse their identities.

## Policies are ordinary trait relationships

A boundary requirement pins a target policy through ordinary trait composition.
The source relationship mirrors `Layout::plan`: authorship is open, while the
compiler owns the normalized input/output vocabulary and validator.

```omega
data BoundaryPlanResult {
    case Accepted(plan: BoundaryEntryPlan);
    case Rejected(reason: CallingPolicyRejection);
}

trait CallingPolicy {
    machine Self::plan(
        signature: BoundarySignature,
    ) -> BoundaryPlanResult;
}

trait Calling<C, Policy: C satisfies CallingPolicy>
{
}

data X86InterruptConvention;

X86InterruptPolicy:
    X86InterruptConvention satisfies CallingPolicy
{
    machine plan(signature: BoundarySignature) -> BoundaryPlanResult {
        ...
    }
}

boundary trait TimerInterrupt:
    Calling<X86InterruptConvention, X86InterruptPolicy>
{
    machine handle(frame: &mut X86InterruptFrame, ack: LapicAck);
}
```

`C` is the calling-convention subject, and `Policy` is the exact named
conformance supplying `CallingPolicy`. `Policy::plan` is compile-time-only,
deterministic, terminating, and build-time-admissible. It receives the
normalized requirement signature and
either returns an accepted plan or a structured rejection. Rejection produces a
diagnostic at the `Calling<C, Policy>` relationship and contributes no contract identity:
there is no boundary requirement to fingerprint until policy evaluation accepts
the signature. The rejection value identifies the incompatible signature feature
(for example, a forbidden result, frame parameter, or return control) so the
diagnostic does not degrade into a later generic "invalid plan" failure.
Hardware conventions are often validators first: an interrupt policy rejects a
return value, frame shape, parameter, or entry-control form the hardware cannot
honor instead of manufacturing a deliberately invalid plan.

An accepted result is compiler-validated and canonicalized before use. The
fingerprint of that canonical evaluated result—not `C`'s symbol, source body, or
unnormalized construction order—enters public contract identity. Refactoring a
policy machine without changing its normalized output therefore preserves ABI
identity; changing an observable placement or state commitment changes it.

Selected vtable-slot, vtable-field, service-table, and source-authored import
bindings carry that evaluated plan all the way through layout and emission.
Those consumers do not reselect a native policy or catalog operation from the
output target: a missing plan contributes no layout width and produces an
explicit emission diagnostic, while a changed plan either changes the emitted
placement or rejects as unrealizable.
Field-model mechanisms retain only their dispatch recipe. They do not duplicate
the source parameter count: layout, emission, relocation, and reporting derive
wire arity and result presence from the retained plan, adding the service
table's one dispatch-only operand only where the selected operand topology
requires it.
At the final ABI seam, a checked scalar argument's retained parameter shape is
the foreign type; a wider compiler-owned scratch slot is only storage capacity
and cannot redefine that type. A slot smaller than the retained shape still
rejects.

Ordinary packages may author policy types under the same validation rules as
platform packages. They choose only from the closed placement and machine-state
vocabularies; adding a new primitive placement or state concept still requires a
compiler release. Omega's internal calling convention is never selected through
this surface.

Using a policy conformance is a semantic choice, not a workaround for missing
static machine parameters. Compile-time machine parameters can select and
directly invoke an authored machine symbol, but `Calling<C, Policy>` names the policy
relationship whose normalized result is part of the requirement. It also leaves
the policy free to contain several ordinary machines without turning one helper
symbol into the public ABI name. Neither mechanism reifies a machine as a
runtime value or exposes its code address.

The capability audit is therefore graded rather than binary:

- compile-time machine selection and direct specialized invocation are live;
- policy evaluation such as `Calling<C, Policy>` is live and does not require machine
  identity as a value;
- a foreign binding whose declared parameter is a callback requirement may
  accept one named static machine satisfying that requirement and privately
  materialize the target ABI entry relocation; and
- general runtime function values, captured environments, and user-visible code
  addresses remain separate facilities.

No downstream slice may cite a blanket "machine parameters are unbuilt" fence.
It must name the stronger reification operation it actually requires.

The implementation dependency is therefore explicit: calling-policy work does
not wait on machine-parameter support. Static selection and invocation are
already available. Callback lowering does not require a general source-visible
entry-reference carrier. The registration call supplies the selection context:
one named static machine satisfying the callback requirement becomes the native
entry expected by that exact parameter, and the compiler emits its plan-driven
thunk and relocation privately.

A target-specific requirement may layer over a portable semantic service trait.
When the boundary declaration itself is reusable across conventions, make the
policy an ordinary type parameter:

```omega
boundary trait Console<C, Policy: C satisfies CallingPolicy>:
    Calling<C, Policy>
{
    machine write(bytes: &[u8]);
}
```

`Console<MicrosoftX64>` and `Console<SysVAMD64>` reuse one semantic declaration
but have distinct boundary contract identities. One concrete requirement never
lets individual providers choose different conventions; mixed conventions use
separate requirement instantiations or composed boundary facets.

Boundary-trait parents and policy parents have different established meanings:
boundary service parents contribute service reach; ordinary core policy parents
contribute contract identity and no reach.

## Boundary shapes: determine or declare

A calling policy may classify a semantic type directly only when that type's
public normalized structure determines every ABI fact the policy needs. If
ABI-relevant facts remain choices, the native leaf must declare the concrete
foreign shape that supplies them. A private compiler lowering never establishes
a public ABI merely because code generation knows its byte shape.

Fixed arrays and fixed records are structurally determined aggregates. Their
element/member types, count, size, alignment, and public layout are available to
the policy, which may classify or reject them under the target's actual
aggregate rules. A `[f32; 4]` may therefore become an HFA under AAPCS64, use an
SSE aggregate class under SysV AMD64, or be indirect under a policy that requires
that result. Classification is recursive and semantic; equal byte size alone
does not imply equal ABI class.

`BoundarySignature` presents that structure as a bounded flat graph rather than
as recursively embedded values: parameter and result roots index `ValueShape`
nodes; fixed-array nodes name their element root and count; record nodes name a
contiguous `ValueField` range whose entries carry child roots and exact byte
offsets. The policy returns a separate `AbiValueShape` for each placement. The
compiler checks that classification against the semantic graph and selected
convention before accepting and fingerprinting the plan.

Omega never performs C source-level array decay. A native function that receives
a fixed aggregate by value is declared with `[T; N]`. A C declaration such as
`f(uint8_t bytes[16])` has a pointer parameter at the ABI boundary, so its Omega
native leaf declares the corresponding pointer/reference contract instead.
Changing `[T; N]` to `&[T; N]` changes the boundary signature; the compiler does
not guess between them.

Safe slices, text views, vectors, and bounded text carriers do not determine one
native ABI. `&[u8]` intentionally omits choices such as the foreign length type,
nullability, retention, ownership, and whether the counterparty expects separate
parameters, a terminator, or a descriptor record. Their private pointer/length
or pointer/length/capacity carriers are not stable public ABI. The default native
policies therefore reject a bare slice, text view, vector, or bounded-text
carrier at a leaf.

The binding declares the foreign API's actual shape instead:

```omega
boundary machine NativeConsole::write(
    bytes: Ptr<u8>,
    length: u64,
) -> i32;
```

An API that takes a null-terminated pointer uses that contract. An API that
really takes a descriptor record declares that record. Two parameters must not
be replaced with a superficially equivalent `{pointer, length}` record: policies
such as Microsoft x64 can pass those shapes differently.

A checked adapter presents the safe Omega surface and lowers it to the native
leaf. For a synchronous non-retaining call, it scopes the foreign
`borrowed-out` contract on `Ptr<u8>` to the call duration. A foreign operation
that retains storage is a different contract and requires an explicit pinned
loan, ownership transfer, or registration protocol; it cannot reuse the
borrowed-out adapter.

The fixed-array pointer-form import canary pins that distinction in the current
checker: `&[u8; 16]` borrows its owner for the synchronous leaf call, and an
ordinary owner mutation is legal immediately after return. No calling-plan bit
extends that loan.

Text has no special ABI. Outbound conversion forgets the `Utf8` domain fact and
passes bytes through the declared foreign shape. Inbound conversion validates
the returned bytes before establishing `Utf8`. The foreign side never receives
Omega's proof object.

A custom calling policy may deliberately define a canonical slice or text ABI.
That is an explicit policy contract, not a default inferred from the compiler's
current private carrier.

Process entry is pinned by the selected process-entry requirement and returns
the platform exit-status scalar. This rule is attached to that requirement, not
to the friendly source name `main`. Firmware entries and other callable boundary
requirements may admit aggregate results when their selected policy does.

The exported machine remains ordinary and keeps `boundary` bare:

```omega
boundary machine Kernel::on_timer(
    frame: &mut X86InterruptFrame,
    ack: LapicAck,
) satisfies TimerInterrupt::handle {
    ...
}
```

The old `boundary(InterruptFrame)` / `boundary(MsX64)` modifier spelling is
retired. It fused the boundary marker with deployment policy and duplicated the
requirement's identity.

## Plan derivation and validation

One evaluated plan drives both directions:

```text
calling policy + signature
          |
          v
validated BoundaryEntryPlan
       /             \
outbound encoder   inbound entry/exit stub
```

Plan validation checks, at minimum:

- every parameter and result is placed exactly once and compatibly;
- stack ranges, alignment, shadow space, and register classes are coherent;
- ordinary clobbers match the stable ABI regime;
- saved/restored state covers the `StatePlan` commitment;
- entry/exit control is valid for the initial regime; and
- target and provider applicability match the requirement.

Canonicalization is part of validation: unordered register/clobber sets are
sorted, equivalent encodings normalize identically, and only accepted canonical
plans may be fingerprinted. The implementation should reuse the compiler's
general normalized-plan infrastructure while preserving `CallPlan` and
`StatePlan` as their own semantic algebras.

Regime-changing instructions do not turn one calling plan into a multi-mode
blob. Checked instruction contracts require regime R and establish R'. Stable
regions on either side use their own plans.

## Contract identity versus implementation evidence

The requirement-pinned, evaluated `CallPlan + StatePlan` is published contract
identity.
The emitted register/machine-state footprint is provider evidence.

The backend must honor a state ceiling while selecting instructions and
allocating registers, then emit checkable footprint evidence. The final placed
artifact is independently validated after inlining, specialization,
link relaxation, veneers/thunks, generated stubs, and admitted indirect leaves:

```text
actual_transitive_footprint subset_of permitted_transitive_use
actual_clobbers intersect unsaved_interrupted_state = empty
```

A legal change in register allocation or implementation evidence does not alter
caller contract identity. It revalidates the provider artifact only.

The firewall is observational: published promises are identity; realization
evidence is not. Calling placement and the `StatePlan` ceiling are counterparty-
observable promises. The emitted footprint certificate proves one realization
refines that ceiling and may change without changing the promise, just as a
termination witness may change without changing a machine's termination
contract.

Checked Omega leaves produce derived footprint evidence. Raw/admitted leaves
carry accepted footprint claims under receipt. The trust report must distinguish
the two.

The canonical evidence boundary is one self-describing footprint
certificate bound to exact final bytes, placements, and complete executable-
region inventory. Its normalized instruction and region rows are replayed by
the admission checker against the closed target instruction specifications;
the checker proves exact byte coverage before composing the realized footprint.
This is certificate checking, not trust in the producer. A second independent
whole-image decoder that produces a competing admission result is not another
supported path. Admitted leaves join only through explicit accepted rows with
their own provider provenance. Static and dynamically loaded artifacts use the
same certificate and checker boundary.

The current envelope is a typed `omega-image` value rather than a report-only
JSON convention. Its closed class vocabulary, normalized coverage rows,
completeness flags, final placement binding, compiler-text derivation, and
region inventory all enter one replayed identity. The current producer is
complete for its closed executable-region vocabulary: compiler functions and
final-byte-validated import thunks. Relaxation products, veneers, generated
stubs, and in-image admitted leaves are absent by construction; adding any such
origin must also add certificate replay before the vocabulary can remain
complete.
Every byte-bearing compiler instruction already carrying a final-byte
validation identity must also produce a target footprint row; an unsupported
shape rejects rather than disappearing from the union. The checker
independently requires every nonempty retained instruction row to choose
exactly one replay authority (compiler target specification or checked-
assembly catalog), while zero-width scaffolds may choose neither. It binds the
catalog-row count to the independently replayed catalog validation count, then
composes the complete compiler-row union, requires exact equality with the
earlier `StatePlan`-validated semantic union, and binds its normalized
fingerprint into the typed certificate. Checked catalog rows also require every
operand loader in their closed envelope; indexed/addressed loaders and fixed
instruction sequences contribute their independently derived flag, stack, and
control effects. Serialization occurs only after internal identity validation.

Exit realization is a second implementation-evidence axis. The external-root
admission path now checks the realized return-control mechanism against
`CallPlan::entry_control` and the exact restored-state set against `StatePlan`.
An opaque provider must carry that accepted claim under a root-reported trust
receipt or a root-reported adequate-hardware-isolation receipt; absence,
unreported evidence, and either plan mismatch all fail before a provider
execution can be formed.

Independently derived fragment evidence composes by normalized set union before
state validation. Fragment order and duplicate evidence therefore cannot alter
the checked transitive footprint or its implementation-only fingerprint. This
composition is the shared pre-certificate seam; object/final-image validation
must still prove that it received every realized fragment after placement.

A no-SIMD interrupt root may require a callee clone compiled under a no-SIMD
state ceiling. This is contextual codegen specialization, not generic type
monomorphization, although both may share backend cloning and cache machinery.

## Selection and bindings

Plans exist only at boundaries. Most callers do not name one: an external
`Binding` and satisfied requirement determine the pinned policy. Explicit policy
identity is authored on the requirement, never inferred from a DLL name,
syscall number, or friendly target string.

The compiler derives provider-plan rows from `satisfies` and `via`; build-time
machines may compute policy results or select among declared candidates, but do
not imperatively assemble a second plan table. A builder that restates
requirement-to-implementation edges would duplicate `satisfies` and remains
rejected independently of machine-parameter implementation progress.

Provider plans remain derived from explicit `satisfies` declarations and `via`
leaves. Admission proves or accepts that a realization refines the complete
boundary plan. See
[`extern_boundary_and_format_domains.md`](extern_boundary_and_format_domains.md).

### Local dynamic dispatch

`dyn Trait` remains within one artifact and does not use a boundary
`CallPlan`. Its descriptor selects one complete conformance and an
artifact-private requirement table. The requirement owns one erased caller
call shape; each selected satisfier supplies a checked adapter into its
physical machine shape.

The selected conformance is one closed
`Name<Telescope>: Type satisfies Trait { ... }` implementation block. Its
normalized map has exactly one row per inherited
`(declaring trait, complete requirement overload)` slot, selecting the block
member, an explicit existing-machine reference, or that conformance's own
per-overload default instantiation. Complete overload identity includes the
normalized parameter signature and dispatch-bearing result-domain set. Typed
dynamic calls carry the resulting exact requirement symbol through checked Psi
and state-call planning, including inherited slots; a backend never recovers
the row from a method spelling. An unqualified call to distinct same-spelled
inherited requirements rejects as ambiguous.
No adapter row is recovered from an attached-state name or a uniquely visible
machine. An independent `machine ... satisfies Trait::requirement` remains a
per-requirement provider/adapter realization and never supplies `dyn` by
itself.

Checked Psi retains the descriptor-selection input from an exact named
conformance coercion and records its source data, target trait, stable
conformance symbol, and normalized rows. A bare place coercion never searches
visible conformances. Unknown names and wrong-carrier selections reject before
Omega lowering.
The selection also retains the original source place. When that coercion and
call remain nonescaping inside the closed artifact, the backend selects the
exact normalized row and calls its realization with the original source place
as the concrete receiver. That call needs no physical descriptor. Private table
materialization and erased-shape adapters remain necessary for dynamic values
that pass onward, are rebound, join, are stored, or otherwise escape this exact
use.

A bare dynamic parameter retains every eligible complete closed conformance as
an exact candidate map. Call-site specialization selects from those maps by the
concrete receiver and routes to each row's retained realization symbol. It does
not enumerate carrier names or recover attached machines by method spelling. A
concrete carrier passed to a bare dynamic parameter must have exactly one such
conformance; a parameter that intends one of several names it exactly in its
dynamic type. Bodyless static conformances have no normalized row map and
therefore never enter this candidate set.

When the physical descriptor remains, the dynamic requirement's operational
envelope accounts for the complete dispatch path:

```text
descriptor dispatch
+ table adapter
+ erased physical call shape
+ selected satisfier demand
```

Whole-artifact devirtualization may discharge the descriptor, table, and erased
adapter terms for an exact call and record the selected realization's direct
call shape instead. It cannot erase resource cost merely because the semantic
target is known; the realized direct call remains in the resource envelope.

Call-shape cost is a resource term, not merely an ABI note. If a
suspension-capable requirement needs a continuation-capable physical shape,
even a nonsuspending satisfier pays that shape's frame and structural work. It
may satisfy the suspension guarantee while still exceeding a caller's stack or
work ceiling.

The exact continuation-capable lowering depends on the final suspension-frame
representation. The architecture does not: one requirement-owned erased shape,
checked adapters, and resource accounting in the per-requirement envelope.

A local table is never shipped across a replaceable component boundary.
Component calls use the boundary requirement's evaluated `CallPlan`,
`StatePlan`, and entry contract. A local proxy may adapt that component
binding back into `dyn Trait` inside the consumer artifact.

### Registered foreign callbacks

A binding package declares the callback as an ordinary boundary requirement
carrying its target calling policy:

```omega
boundary trait WindowProcedure:
    Calling<MicrosoftX64, MicrosoftX64Policy>
{
    machine call(
        hwnd: HWnd,
        message: u32,
        word: WParam,
        long: LParam,
    ) -> LResult;
}

boundary machine ApplicationWindow::dispatch(
    hwnd: HWnd,
    message: u32,
    word: WParam,
    long: LParam,
) -> LResult
    satisfies WindowProcedure::call
{
    ...
}
```

The registration operation expects that callback requirement. Passing
`ApplicationWindow::dispatch` selects its explicit conformance; the compiler
validates the evaluated `CallPlan + StatePlan`, generates the inbound thunk,
and materializes the native code address only inside the binding lowering. The
source program continues to name a machine and a requirement rather than
constructing an address-shaped callback value.

A durable registration returns an ordinary linear package value. That value
owns the protocol registration and, when code unloading is possible, the
artifact or component lease. Its explicit terminal operation unregisters the
foreign entry before releasing those obligations. Call-scoped callback
parameters instead remain ordinary borrows and produce no durable registration.

Calling plans do not describe whether a callback runs. That is ordinary machine
behavior. A bodyful machine infers its direct synchronous boundary invocations;
a bodyless requirement declares them with `invokes`:

```omega
boundary trait EventSource {
    machine register_and_fire(handler: Handler) -> Registration
    invokes handler;
}
```

The clause means the current invocation may enter `handler` before returning.
It automatically contributes the handler trait and the selected conformance's
operational envelope to the current invocation's normalized service reach.
Omission means no synchronous invocation. A returned linear registration
separately establishes a future external root; its root-admission obligation
and compiler-tracked claim metadata retain the concrete conformance and
envelope without widening every registration to the trait ceiling.

The realized synchronous boundary-invocation graph must be acyclic. Cycle
checking consumes direct `invokes` edges rather than the transitive service
row. A mailbox, queue, scheduler handoff, or other new-activation boundary
breaks a cycle structurally; merely inserting another synchronous trait does
not.

Per-instance state does not ride implicitly on a C function pointer. The
binding package uses the protocol's explicit context parameter, a checked
generational handle recoverable from callback arguments, or package-owned
stable state. Ownership remains in Omega; foreign storage carries only the
inert token the protocol requires.

Raw `addr` and `Ptr<T>` carriers likewise grant no memory authority. Core
publishes no `Ptr::read` or `Ptr::write` operation; pointer offset/range
operations transform only the inert representation token. A calling plan may
classify and place that token but cannot manufacture a readable or writable
view.

The target callback-entry plan selects one stack disposition:

- continue on the provider's current stack under its containment contract;
- preflight that stack against the exact Omega WCSU plus the target's reserved
  entry/unwind margin, with a protocol-valid unavailable result; or
- enter a target-supported owned stack whose provision enforces the Omega
  WCSU.

Preflight proves that the predicted Omega segment fits. A hard-limited owned
stack additionally detects an underestimated WCSU at its own boundary. Opaque
foreign frames remain in the provider stack domain; an exact separated-stack
profile returns to that domain before making another foreign call.

Platform packages may normalize a re-entrant native callback protocol into a
safer Omega handler API. The package declares exact `invokes` ceilings on its
bodyless surfaces, infers ordinary application-handler reach locally, handles
synchronous platform queries through restricted handlers, and queues or defers
ordinary application events. This needs no inference over the opaque provider's
internal call graph. A direct raw callback path instead retains the provider's
admitted behavior and resource provenance.

## External roots

An installed inbound stub whose code or state may be reclaimed is an external
root because it has no Omega caller. Its linear registration owns the
foreign-held edge; a dynamic artifact root ledger records the evaluated
boundary plan, provider/artifact/receipt identities, service reach, stack
domain, nesting/preemption relation, and liveness/version pins. A statically
linked process-lifetime callback needs the same build-time plan and report but
no live replacement ledger.

This ledger is also where WCSU composes same-stack interrupt demand and where a
dynamic installation is checked against the artifact-wide bound. Per-machine
validation alone cannot answer those questions.

The provider-neutral installation ledger is live in the orchestration crate
`omega-external-roots`.
Each admitted record retains the complete evaluated plan and exact
provider/effect/receipt, stack/nesting/acknowledgement, resource, artifact, and
component-version identities. Its stack, structural-work, and machine-state
columns independently retain their ceilings, realized facts, and validation
receipts. Fixed-work provider summaries compose transitively and fail closed on
missing summaries, recursion, invalid multiplicity, or arithmetic overflow.
Installation consumes consumer-supplied publication authority and returns a
linear installed-root handle borrowing the installed-code claim; removal
returns that authority only after exact unreachability and any required
quiescence evidence. Artifact-wide WCSU composition is live: provider-local
demands and the exact nesting relation produce sealed per-root and per-domain
maxima, same-stack paths add with alignment, and installed roots must agree on
the complete composition fingerprint. Cycles, missing endpoints, unknown
nested provider-selected stacks, overflow, and active dedicated-class re-entry
reject.

A sealed provider-execution binding joins the normalized selected provider
plan, exact entry/boundary/reach, and all three resource realizations into
admission; it cannot be replayed after realization drift, and its identity is
reportable. Exact validated compiler-selected plans survive checked lowering
in one canonical fact set. External-root candidates bind the retained plan
identity before validation; normalized root identity covers it, and execution
inherits it rather than accepting a second plan input. The ledger's
deterministic fingerprint and the `omega-artifacts` `external_roots.json`
projection report these facts and the complete boundary plan without leaking
numeric entry addresses or private ranking/codegen proofs.

Hardware-table construction and publication are consumer policy. Omega supplies
generic symbolic materialization, checked instruction contracts, root
accounting, and validation hooks; an OS package supplies its table schema,
publication authority, installation lifecycle, and device protocol. The
compiler must not grow IDT-, vector-, PIC-, LAPIC-, or timer-shaped lifecycle
types merely because an OS uses those generic pieces.

Installation also supplies the only admissible evidence for
`Atomic::interruption_fence`. The retained root route must identify the selected
handler, asynchronous source, execution context, and interrupted-code
relationship strongly enough to derive same-context entry. The operation cannot
assert that relationship and rejects when the installed-root/provider evidence
does not establish it. This relation orders compiler-visible coherent memory
only; device and cross-core ordering remain separate contracts.

The ledger uses one recording discipline across three
independent resource columns: stack ceiling/realized WCSU/derivation evidence,
logical-fuel provision/realized fixed-work ceiling/IR proof evidence, and
`StatePlan` ceiling/realized footprint/codegen evidence. Reports retain ceilings,
realized facts, and validation receipts; private rankings and codegen proofs stay
behind the evidence firewall. Fixed logical work proves only a finite admitted
operation path, not target WCET. The current schedule-keyed fixed-fuel
provider-summary composer and logical-fuel provision now use the dependency-light
`psi-core` schedule identity directly. Local-evidence rows distinguish
recomputable terminal-Psi entry/segment certificates from admitted opaque-
provider unit claims, and the external-root report retains that distinction.
Whole-entry certificate rows now bind exact relocation-free frozen executable
bytes and selected entry offsets, and root installation rechecks the exact
installed-code context. They remain the implementation precursor to broader
terminal-Psi fixed-work and safe-point checking in
[`canonical_ir_fuel_and_resource_provisioning.md`](canonical_ir_fuel_and_resource_provisioning.md),
not a general symbolic cost model. Migration of the remaining
provider-authored hard-root rows remains.

The source-to-checked acceptance path pins the control-state half directly. An
authored `Calling<C, Policy>` policy may publish `InterruptReturn`, a stack class,
preemption behavior, and the exact saved/restored state set; its canonical
fingerprint is retained unchanged by the boundary service schema, selected
provider plan, and external-root bridge. Which interrupt classes, stacks,
masking policy, acknowledgement protocol, and hard-root work profile an OS
chooses is outside this brief.

## Compiler-owned pieces

Policies choose from closed placement and machine-state vocabularies. Compiler
derivers own instruction emission, entry/exit stubs, contextual specialization,
footprint production, and final-artifact validation. Omega's internal convention
is not expressible and may change between releases.

The steady-state authorship split is:

- `std::calling` defines the closed normalized signature, placement,
  machine-state, acceptance, and rejection vocabulary;
- target and platform packages author ordinary compile-time policy machines for
  SysV AMD64, Microsoft x64, AAPCS64, firmware, syscalls, interrupts, and other
  public boundaries;
- the compiler purity-gates and evaluates the selected policy, validates and
  canonicalizes its result, fingerprints the accepted promise, and derives
  target instructions from that plan; and
- the compiler alone may extend primitive register, placement, control, or
  machine-state vocabulary.

Policy code cannot emit instructions, inject relocation bytes, or bypass the
validator. It constructs checked policy data. The compiler remains the
mechanism validator and generic emitter.

## Current implementation

`omega-calling-conventions` owns normalized `CallPlan`, `StatePlan`, and
`BoundaryEntryPlan` records. It evaluates the supported MS x64, SysV x64,
AAPCS64, Linux syscall, firmware, and interrupt-policy slices and rejects
unclassified shapes, invalid placement, incompatible regimes, unsaved state,
and footprints above the declared ceiling. Contract identity remains separate
from implementation evidence.

Source-authored policy discovery and evaluation are live. Standard target
policies are still partly Rust bootstraps, and `BoundarySignature` still
contains preclassified value shapes; the migration is to richer structural
input and readable Omega policies, differential-checked until the bootstraps
can be retired. Plan validation and instruction emission remain compiler-owned.

Normalized plans are authoritative for process entry/results and for the live
outbound syscall, native import, vtable, service-table, and callback paths on
x86-64 and AArch64. Exact argument/result fragments and stack placements survive
through abstract operations, target operations, layout, emission, and object
relocations. Composite adapters retain each actual native subcall plan instead
of substituting their outer semantic signature. A missing, mismatched, or
incompatible plan fails closed.

Compiler-body memory operations likewise retain their exact plan-selected place
and relocation recipes through emission and replay validation. Current
coverage includes scalar/aggregate parameters and results, AAPCS64 HFAs,
indirect large aggregates, runtime-indexed places, string and bounded-buffer
operations, compact bit fields, and the built-in OS/runtime catalogs. Dedicated
no-plan paths exist only as differential oracles.

Remaining work is to derive inbound and outbound machinery from the same plan,
add state-ceiling-aware selection/allocation, and validate composed footprints
at the final artifact.

## Still open

- register/machine-state vocabulary extensions beyond the implemented x86-64
  and AArch64 foundation;
- object-certificate composition and final-image validation format;
- admitted indirect-call footprint contracts;
- unwind/non-local-exit representation; and
- general quantitative resource/WCET algebra beyond the timer's structural
  fixed-work profile.

These are plan/checker/backend questions. They do not justify reviving
`boundary(<Plan>)`, adding an interrupt machine species, or exposing code
addresses as integers.
