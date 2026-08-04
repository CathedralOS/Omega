# Design Brief: Calling And Machine-State Plans

Current as of 2026-07-28. Boundary conventions are normalized policy artifacts;
Omega's internal calling convention remains compiler-sovereign. This brief now
includes inbound machine-state preservation, which ordinary calls do not expose.
Engineering is incomplete. The normalized compiler model, initial built-in
policy evaluators, recursive public fixed-array/record signature graphs, direct
source-policy evaluation, concrete and generic `Calling<C>` discovery,
publication of the evaluated identity, and relationship-span diagnostics are
implemented. Authoritative lowering remains.

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

trait Calling<C>
where
    C satisfies CallingPolicy
{
}

data X86InterruptConvention;

machine X86InterruptConvention::plan(
    signature: BoundarySignature,
) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    ...
}

X86InterruptConvention satisfies CallingPolicy;

boundary trait TimerInterrupt:
    Calling<X86InterruptConvention>
{
    machine handle(frame: &mut X86InterruptFrame, ack: LapicAck);
}
```

`C` is a calling-policy type, not the frame data type or a friendly target name.
Its `plan` machine is compile-time-only, deterministic, terminating, and
build-time-admissible. It receives the normalized requirement signature and
either returns an accepted plan or a structured rejection. Rejection produces a
diagnostic at the `Calling<C>` relationship and contributes no contract identity:
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

Using a policy type is a semantic choice, not a workaround for missing static
machine parameters. Compile-time machine parameters can select and directly
invoke an authored machine symbol, but `Calling<C>` names the policy
relationship whose normalized result is part of the requirement. It also leaves
the policy free to contain several ordinary machines without turning one helper
symbol into the public ABI name. Neither mechanism reifies a machine as a
runtime value or exposes its code address.

The capability audit is therefore graded rather than binary:

- compile-time machine selection and direct specialized invocation are live;
- policy evaluation such as `Calling<C>` is live and does not require machine
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
boundary trait Console<C>: Calling<C>
where
    C satisfies CallingPolicy
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

The canonical evidence boundary is one self-describing, versioned footprint
certificate bound to exact final bytes, placements, and complete executable-
region inventory. Its normalized instruction and region rows are replayed by
the admission checker against the closed target instruction specifications;
the checker proves exact byte coverage before composing the realized footprint.
This is certificate checking, not trust in the producer. A second independent
whole-image decoder that produces a competing admission result is not another
supported path. Admitted leaves join only through explicit accepted rows with
their own provider provenance. Static and dynamically loaded artifacts use the
same certificate and checker boundary.

The first v1 envelope is a typed `omega-image` value rather than a report-only
JSON convention. Its closed class vocabulary, normalized coverage rows,
completeness flags, final placement binding, compiler-text derivation, and
region inventory all enter one replayed identity. The current producer remains
explicitly partial while compiler-body footprint decoding and admitted-leaf
rows are missing; serialization occurs only after internal identity validation.

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

Checked Psi now retains the first descriptor-selection input: a direct bare
place coercion succeeds only for one unique complete nominal conformance and
records its exact source-data, target-trait, and optional stable conformance
symbol. Missing or ambiguous selections fail before Omega lowering. An exact
`dyn Type::Conformance` target is now retained through typed identity, derives
its dispatch trait from the declared edge, and selects that stable child symbol
even when sibling conformances exist; unknown and wrong-carrier paths reject.
The private table and adapter realization remain.

The dynamic requirement's operational envelope accounts for the complete
dispatch path:

```text
descriptor dispatch
+ table adapter
+ erased physical call shape
+ selected satisfier demand
```

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
    Calling<MicrosoftX64>
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
authored `Calling<C>` policy may publish `InterruptReturn`, a stack class,
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

## Engineering order

Generic trait-parent composition used by `Calling<C>` is implemented. Header
parents and body-level `requires` share one validated graph; boundary parents
contribute service reach and ordinary policy parents do not.

The `omega-calling-conventions` foundation now owns normalized
`CallPlan`, `StatePlan`, and `BoundaryEntryPlan` compiler records. It evaluates
MS-x64, SysV-x64, AAPCS64, x86-64 Linux-syscall, and AArch64 Linux-syscall
policies for the currently classified scalar/HFA shapes. Validation rejects
unclassified aggregates, incomplete or overlapping placements, incompatible
regimes, unsaved permitted state, and footprints above the state ceiling.
Register use derives its machine-state class, so evidence cannot hide SIMD use
by omitting a self-reported class. Contract and evidence fingerprints are
separate by type.

Source-authored policy discovery and build-time evaluation are live, but the
bundled standard policy evaluators remain substantially compiler-built Rust
bootstraps. The current `BoundarySignature` also supplies a preclassified
`ValueClass` rather than enough normalized recursive structure for policy code
to classify every fixed array and record itself. Migration must enrich that
input, move the standard target rules into readable Omega policy packages, and
differential-check them against the bootstrap until retirement. This does not
move validation or instruction emission out of the compiler.

Existing compatibility bindings select this normalized policy as an independent
oracle. The process-entry prologue now evaluates the target's normalized native
policy and carries every classified scalar argument's exact register or
ABI-relative stack offset and width through abstract operations, target
operations, layout, and x86-64 or AArch64 emission. Target encoders alone add
the entry return-address or function-frame bias. This removes both the former
backend convention that interpreted an abstract argument index as the
Microsoft x64 register sequence and the global four-register entry limit.
Incoming scalar `f32`/`f64` parameters follow XMM/V locations as well.
Integer process-entry terminal values likewise carry the normalized plan's
exact result register through abstract/target operations and x86-64/AArch64
emission; ISA encoders no longer invent `rax` or `x0`. On AAPCS64, flat records
of one to four contiguous, same-width `f32` or `f64` members classify as HFAs
from the normalized data layout and arrive through the plan-selected vector
register fragments. Fixed non-HFA AAPCS64 records up to 16 bytes now follow the
normalized consecutive-`x`/whole-value-stack rule, including even-register
rounding
for 16-byte alignment; normalized small-aggregate result plans use `x0`/`x1`.
`Binding`-authored outbound calls preserve the same small non-HFA record as one
by-value operand: emission loads every planned consecutive `x` fragment or
copies the whole value into the plan's aligned outgoing stack fragments.
Small aggregate entry results and outbound result stores consume their
plan-selected fragments. Aggregates above 16 bytes use the normalized indirect
argument and caller-owned `x8` result-destination conventions. Generic Linux
syscall leaves are the first outbound path to make the normalized plan
authoritative: emission evaluates the x86-64 or AArch64 syscall policy for the
operand signature, then passes its exact parameter registers, number register,
and supervisor-call immediate to the ISA encoder. The legacy binding's
`number_register`/`supervisor_call` fields no longer select those facts on that
path. The normalization seam now also rejects incompatible policy,
supervisor-call control, stack alignment, shadow space, and encoder-scratch
clobber ceilings. Runtime-storage operands on x86-64 stage through the
plan-clobbered `r11`/`rax` pair rather than silently overwriting callee-saved
`r15`. AArch64 large-offset marshalling reuses the plan-selected `x8` number
register before loading the call number, and the evaluated plan explicitly
includes `x8` in its realized clobber set. Composite runtime-text byte and line
syscalls now consume the binding-retained three-argument/result plan in both
emission and layout rather than re-evaluating it from the architecture. Byte
read, byte write, and bounded-carrier, fixed-array, or string-descriptor line
read compatibility sequences have an exact byte/width differential lock on
x86-64 and AArch64. AArch64 emits the plan-selected registers and supervisor-call
immediate; the current fixed x86-64 sequences fail closed when asked to realize
a different normalized plan rather than silently overriding it. On the direct
Darwin import route, the same composites validate the retained native
three-word/result AAPCS64 signature against their fixed `x0`/`x1`/`x2`/`x0`
sequence and layout fails closed with emission when placement drifts. Windows
runtime byte/line imports remain composite adapters over their separate
GetStdHandle and ReadFile/WriteFile subcalls. Production layout and emission
take a structurally complete plan set from the retained catalog bindings: a
direct call carries exactly one plan, while the Windows file adapter carries
both GetStdHandle and ReadFile/WriteFile plans. A partial adapter is therefore
not representable at that boundary; a mismatched variant or incompatible plan
still rejects. The direct byte/line encoder and width APIs require their one
plan directly, and the ISA-level Windows-pair validator likewise requires both
plans; independent normalization is exposed only by explicitly named no-plan
differential oracles. The outer semantic binding plan is not substituted for
either internal signature.
The base Linux binding rows make that retention literal: `Stdin::read`,
`Stdout::write`, and `Stderr::write` each carry the exact three-word/result
syscall plan, while `Process::exit_group` carries its one-word/no-result plan.
Machine layout and emission reject any selected syscall binding that loses
that plan; architecture-derived syscall placement remains only a differential
oracle. No composite or terminal process path needs to rediscover those fixed
external signatures from the target architecture. The direct Darwin libc rows retain
the corresponding three-word/result AAPCS64 plans for `_read` and `_write`,
and the exact I32/no-result plan for `_exit`; unlike Windows' composite adapter,
these are the actual external calls made by the runtime-text and process paths.
Darwin's scalar libm cohort also carries typed plans: `_lround` retains its
F64-to-I64 signature, while `_sqrt`, `_hypot`, and `_fma` retain one, two, and
three F64 parameters respectively with an F64 result. Their vector argument and
result placement is selected once at binding construction. The Darwin time
adapters retain exact scalar plans as well: `poll(NULL, 0, milliseconds)` has
three word parameters and no consumed result, while each calibrated
`clock_gettime_nsec_np(clockid)` read has one word parameter and one word result.
The older Darwin `tick_count` alias uses the same typed row and now consumes its
`ConstantArgument(8)` through the data-driven clock-read operand path. Windows
TickCount keeps the same path with no constant row data and therefore no call
parameter.
The scalar Objective-C runtime cohort is binding-planned too. Class/selector
lookup, the two-, three-, and six-word `objc_msgSend` forms, the runtime byte
string send, and autorelease-pool push/pop retain their exact AAPCS64 word
signatures. The source-level `pool_pop` scratch result remains part of its
selected call shape even though the foreign function itself returns void.
The mixed macOS GUI rows are typed rather than reduced to parameter counts.
Rectangle and image-size message sends retain their ordered word/F64 parameter
shapes and word result, while the CGRect max-X/max-Y functions retain four F64
parameters and an F64 result. The AAPCS64 planner derives the independent X/V
register streams from those exact selected shapes. The remaining scalar Core
Graphics rows—color-space creation, bitmap-context creation/snapshot, image
width, release calls, and event-source key state—retain their exact zero-,
one-, two-, or seven-word parameter plans and word/source-scratch results.
The no-argument Darwin `___error()` row retains an I32 stored-result plan for
its pointer-dereference adapter. Every Darwin filesystem import now retains its
typed libc signature. Integer literals adopt the selected parameter width at
the final ABI seam, while an oversized compiler scratch slot is only result
capacity: the retained result shape selects the actual return-register store
width. The creating `open(path, flags, mode)` row retains one concrete Apple
variadic plan, including the fixed/anonymous boundary and the anonymous I32
mode at outgoing stack offset zero; flattening it into `x2` rejects.
On Microsoft x64, the parameter-free `GetTickCount64` and
`GetForegroundWindow` rows retain word-result plans, while `_errno` and
`GetLastError` retain I32 stored-result plans. These rows need no compatibility
with an untyped synthesized argument, so their selected plans are authoritative.
The rest of the built-in Windows import catalog now retains its concrete native
Microsoft x64 plan at the same construction seam. Composite console and time
rows retain the actual GetStdHandle, ReadFile/WriteFile, QPC/QPF, or FILETIME
subcall signature rather than pretending the adapter's outer semantic operands
are the imported function. Contextual integer literals and compiler-derived
byte counts adopt that retained parameter width during validation, so a DWORD
plan is not replaced by an eight-byte compiler scratch classification. The
direct GetStdHandle, ReadFile/WriteFile, key-state, and time-out-pointer
encoders and their x86-64 relocation walks validate those retained concrete
plans before using their fixed adapter layouts; changing an outer semantic
shape cannot silently replace the native subcall contract.
Register-resident AArch64 C/import emission now also evaluates AAPCS64 from the
selected operand shapes and passes the exact planned X/V argument and result
registers to the ISA encoder. Scalar stack arguments and flat HFA arguments and
results consume normalized stack or fragmented vector placements. Darwin
variadic `open_create` evaluates one concrete fixed-plus-anonymous signature;
its named arguments, stack mode, and result all consume that complete plan. The
general Microsoft
x64 import encoder now receives its policy from the concrete target,
evaluates selected scalar/pointer operand shapes, and consumes the plan's exact
RCX/RDX/R8/R9, shadow-relative stack, and RAX-result placements. A non-Microsoft
x86 target fails closed at this Win64 compatibility encoder. The normalization
seam also validates call/return control, 16-byte stack alignment, 32-byte shadow
space, and the required volatile `rax`/`r10`/`r11` scratch ceiling. General
argument staging, vtable and service-table dispatch, and result stores use those
plan-clobbered registers rather than leaking into callee-saved `r15`.
Microsoft x64 vtable and firmware service-table calls now reuse the same plan-driven
marshaller; receiver arguments remain on the wire, dispatch-only table pointers
do not, and result-bearing field calls validate the plan-selected RAX placement
before storage. `GetStdHandle`, `ExitProcess`, and `Sleep` now route through the
same plan-driven general marshaller without changing bytes or relocation sites.
`GetAsyncKeyState` likewise consumes planned RCX/RAX placements while retaining
its compatibility-specific 16-bit zero-extension transform. The composite
Windows time calls now plan their actual native one-pointer signatures: QPC/QPF
also carry an ignored RAX `BOOL` result, while `GetSystemTimePreciseAsFileTime`
is void. Their temporary out slot remains an encoder materialization detail.
Composite `ReadFile`/`WriteFile` sequences now evaluate their actual five-value
native signature and ignored RAX `BOOL` result. Their four register arguments,
shadow-relative fifth argument, and scratch-slot reservation consume that plan;
the scratch slot itself remains an encoder materialization detail. Dedicated
runtime line/byte Windows sequences now reuse the same file layout and consume
the retained one-DWORD/RAX `GetStdHandle` plan alongside the retained
five-parameter file plan without changing their fixed bytes or relocation
sites. Supplying only one native subcall plan fails closed in layout, emission,
and the independent object-relocation walk; that walk no longer silently omits
a missing GetStdHandle record or accepts catalog-shaped offsets without
validating both plans.

AArch64 `VtableSlot` and `VtableField` compatibility calls now consume the
same normalized AAPCS64 argument and stack placements as direct imports. The
receiver must be the plan's full-width `x0` argument; emission reads the slot
or layout-resolved field into caller-saved `x16` and uses `blr x16`, with no
import relocation. A field call may carry a separate leading result place;
plan-selected scalar GPR/vector and flat HFA results use matching stores, with
layout and page-fixup offsets accounting for each result tail.

AArch64 `TableFunction` compatibility calls use the same plan consumer while
excluding their dispatch-only table pointer from the AAPCS64 signature. The
declared arguments therefore begin in `x0`/`v0`; after marshalling, emission
loads the table from its relocated runtime scalar, reads the layout-resolved
function into `x16`, and uses `blr x16`. Scalar GPR/vector and flat HFA result
stores, layout, and argument/table/result page-fixup offsets share the same
accounting.

Scalar AAPCS64 outbound stack arguments now consume normalized stack offsets:
the encoder reserves a 16-byte-aligned outgoing area, materializes integer,
pointer, and float values through caller-saved X/V scratch registers, stores
them at the planned offset, and restores SP after `BL`. Width calculation plus
call/data relocation walkers consume the same stack-prefix/store/restore
accounting. A flat two-to-four-member AArch64 HFA argument is now preserved as
one by-value aggregate operand from selection onward. Its grouped normalized
placement drives one load per member into the exact selected vector register,
and width plus both relocation walkers account by source value rather than
mistaking fragments for independent arguments. If the vector bank is exhausted,
the encoder instead copies every member into the plan's one contiguous,
16-byte-aligned outgoing stack area. An authored flat HFA result remains one
selected aggregate result place while the evaluated result placement supplies
each exact vector-register fragment; emission spills those fragments through
one relocated storage base, with width and relocation accounting remaining per
source value. The AArch64 import normalization seam now validates the rest of
the plan contract it can realize as well: AAPCS64 policy, call/return entry
control, 16-byte stack alignment, zero shadow space, and an ordinary-clobber
ceiling containing every fixed caller-saved X/V scratch register used by the
encoder family. A future policy or evaluator change therefore fails before
emission instead of silently preserving only the placement projection.

`Binding`-authored scalar-float imports retain their float result shape,
consume the plan-selected `v` result register, and move its raw bits through a
relocated scalar store. The float result operand's ordinary width already
equals that store tail, while the result page fixup explicitly accounts for
the intervening `fmov`.

Ordinary process and firmware entries now evaluate a complete validated
`BoundaryEntryPlan`, not a detached call layout. Their concrete `StatePlan`
records that no activation was interrupted, requires no save/restore, uses the
provider-selected entry stack, marks preemption inapplicable, and limits
transitive machine-state use to the general/vector classes named by the ABI's
volatile register set. The current ordinary AArch64 hosted projection records
EL0; that reversible target default can be refined when higher-EL roots land.

The bundled `std::calling` module now supplies the closed source vocabulary.
For a concrete boundary `Calling<C>` relationship, both compiler entry paths
materialize every inherited and declared method signature, purity-gate and run
`C::plan`, validate/canonicalize acceptance, report authored rejection, and
publish only the evaluated plan fingerprint through provider requirement
identity. Policy type names and source bodies do not enter that fingerprint;
boundaries without a calling policy retain their prior identities. The authored
relationship span survives typed lowering and is attached to evaluation,
rejection, invalid-plan, and signature-materialization diagnostics.
Generic boundary declarations are inert until a standalone conformance supplies
a concrete trait argument tuple. Each such instance resolves its policy and
forwarded signature types independently; provider schemas recover that same
tuple, while only the evaluated plan fingerprint enters their public identity.
The canonical call plan itself remains internal lowering evidence. Provider
selection now carries it through authored `via` leaves into the host binding
without adding policy source identity to the schema. On x86-64,
authored imports use the retained plan directly for emission, width, and both
call and data relocation layout; supplied operand shapes are checked again at
that seam. Thus a source-selected SysV placement can govern an import in a PE
image instead of being silently replaced by Microsoft x64. AArch64 source-plan
consumption now crosses the same authored-import seam: the retained plan is
revalidated against lowered operand shapes, then supplies exact parameter and
result placements to emission, width, and both relocation walks. Register and
outgoing-stack canaries prevent a target-derived AAPCS64 plan from replacing it.
The same retained-plan path now covers indirect compatibility bindings.
`VtableSlot`/`VtableField` keep the receiver as the first wire parameter, while
`TableFunction` excludes its dispatch-only table storage before revalidating
the source plan. Emission, layout, and data relocation consume that one plan on
x86-64 and AArch64. Consequently a source-selected SysV indirect call remains
SysV even in a PE image, and AArch64 field calls preserve exact non-receiver
register and outgoing-stack placements.
`Binding`-authored syscall leaves likewise consume their retained Linux
syscall plan rather than re-evaluating one from the CPU architecture. The
encoder rechecks the word signature and syscall contract, then uses the exact
parameter registers, number register, and supervisor-call immediate on x86-64
or AArch64; layout measures those same emitted bytes.
When a compatibility external leaf has no explicit `Calling<C>` relationship,
binding construction evaluates the selected target's native policy once from
the declared recursive boundary signature and retains that complete plan.
Explicit source-authored policy evidence always wins. Compatibility syscall
leaves use the corresponding Linux full-word syscall policy at that same seam;
they do not inherit the C policy or ask emission to reconstruct one.

Compatibility providers may adapt a syscall's concrete result shape without
publishing it as Omega ABI. Linux `clock_gettime` is the first composite
customer: its retained boundary plan is the real two-word syscall signature
(`clockid_t`, caller-owned `timespec*`, status result), while the semantic
`Clock` operation remains argument-free and returns nanoseconds. Selection
injects the plan-owned clock id; target emission owns the 16-byte temporary,
traps if the fixed valid inputs nevertheless return an error, and computes
`tv_sec * 1_000_000_000 + tv_nsec`. Width and result relocation are derived
from that exact sequence on x86-64 and AArch64. The internal `timespec` never
becomes a universal language representation, and calibration values remain
per-target constant-result rows with no call boundary.
Linux `nanosleep` uses the symmetric argument adapter: the semantic operation
continues to accept one millisecond scalar while its retained boundary plan is
the real two-pointer syscall signature. Target emission converts milliseconds
to a private two-word request, passes a null remainder pointer, and derives
width plus operand relocation from that same sequence. An interrupted sleep
returns early rather than hiding an unbounded retry loop inside the provider.

Value-returning Linux syscalls now use one general companion path rather than
being forced through the non-returning console encoder. The leading Omega
result place is excluded from the syscall parameters, the retained plan
selects the exact argument/result registers, and target emission stores the
kernel result only after the supervisor call. Width and every argument/result
relocation consume that same sequence. The first filesystem rows use this path
for `read`, `write`, positioned I/O, descriptor lifecycle, seeking, sync,
permissions, duplication, locking, ownership, truncation, and descriptor
metadata (`fstat`) on both x86-64 and AArch64. Semantic `open` and
`open_create` remain common while the Linux target injects `AT_FDCWD` and
lowers both through the architecture's `openat`; that compatibility argument
is plan data, not public filesystem ABI. Path creation and permission changes
use the same plan-owned adaptation through `mkdirat` and `fchmodat`; semantic
`create_dir`, `create_dir_name`, and `set_permissions` therefore keep their
portable path-plus-mode shape while Linux injects `AT_FDCWD`. The already
directory-relative `unlink_at` seam maps directly to `unlinkat`, while
`read_link` keeps its portable path-buffer-count shape and receives the same
plan-owned prefix through `readlinkat`. Plain-path `remove`/`remove_dir` and
their trusted-name twins also lower through `unlinkat`; plan data injects both
`AT_FDCWD` and the target-specific trailing flag (`0` or Linux
`AT_REMOVEDIR = 512`). Two-path plan data likewise places the portable path
pair into `renameat`, `linkat`, or `symlinkat`, injecting both directory
descriptors and `linkat`'s trailing flags where the selected syscall requires
them. Path and descriptor metadata use the settled closed integer-placement
extension: the real Linux ABIs may store one semantic integer field at
different signed or unsigned widths, and projection widens it into the
portable carrier. The known Darwin-shaped Linux `StatLayout` placeholder must
not be treated as native coverage while that placement is implemented. Linux
directory reads now retain the real three-argument
`getdents64` plan: selection omits the portable seam's Darwin-only cursor, and
the x86-64/AArch64 target packages decode `d_reclen` at 16, `d_type` at 18, and
the NUL-terminated name at 19. Direct syscall failures remain `-errno` at the
raw seam and flow explicitly into target-package classification; Linux never
binds the libc-shaped zero-argument `FilesystemHost::errno` operation or gains
a hidden last-error slot. The common wrappers now retain the failed i32/i64
result through their error arms, while the selected target normalizes its
native code set (including Linux EAGAIN/EWOULDBLOCK 11). POSIX
directory count, stats, indexed lookup, and fd-relative lookup now hold the
descriptor cursor across repeated complete-record fills until EOF; the
interpreter providers paginate the same record stream.

The compiler's retained source-policy identity carries the complete canonical
`BoundaryEntryPlan` through checked lowering. Public provider schemas still
publish only its contract fingerprint. Outbound binding construction projects
the `CallPlan`; inbound stub construction can therefore recover the associated
`StatePlan` without re-evaluating policy source or trying to infer state
obligations from the fingerprint. The selected `ExternalBindingRow` and backend
`HostBinding` retain that complete plan too. `HostBinding` is structurally
plan-bearing; optionality exists only on a pre-selection external row, which
must either select an existing compiler intrinsic or acquire an evaluated plan
before entering the host ABI plan. Existing emission, layout, and
relocation consumers borrow only its call half, so the selected state policy
reaches the backend without creating a parallel lowering table. External
bindings resolve through the same complete-plan API and project their outbound
call plan from it; source-selected complete plans are revalidated as a unit at
that seam. The first reusable inbound consumer derives parameter-register,
incoming-stack, indirect-parameter, and hidden-result-pointer storage writes
from that complete plan. Process entry uses the same derivation, and state
validation precedes instruction production so future provider stubs do not
need a separate placement path. Its matching exit derivation returns the
canonical result fragments and call/return control from the same validated
plan; process-entry result lowering uses that shared consumer as well.
Inbound derivation also retains one exact semantic-parameter, placement,
destination, and generated-write-range row. The admitted external-root
sidecar joins that row to the matching live invocation occurrence and checked
parameter fact without creating a detachable receipt; the eventual concrete
provider entry executor must require this borrowed handoff before dispatching
the checked adapter body.

Fixed generated call-return frames now make `ControlState` part of their exact
machine footprint. They save the caller's complete MXCSR/FPCR, establish
Omega's canonical semantic controls before checked execution, and restore the
caller on return; footprint validation permits that state only for prescribed
`CallReturn` mechanics. This supplies the concrete callback entry/exit seam.
Returning imports and indirect vtable/table calls now take the conservative
save/restore path automatically. A target-neutral mechanism classification
adds an aligned MXCSR/FPCR envelope around the existing target call program;
direct syscalls receive no envelope because they do not execute a returning
user-space counterparty. Relocation planning rebases the unchanged inner
program by the exact target prefix width, so specialized argument/result
layouts remain the sole relocation oracle. A future admitted preservation
proof may select the zero-envelope optimization without changing call layout.

Syscall bindings now retain only syscall identity and number. Register
placement and supervisor-call control come exclusively from the normalized
plan on both x86-64 and AArch64; the historical duplicate register-slot and
immediate fields are retired, so reports and encoders cannot treat mechanism
metadata as a second placement oracle.

The vtable-slot, vtable-field, and service-table compatibility shapes have an
exact differential lock: on Microsoft x64, SysV AMD64, and AAPCS64, normalized
compatibility selection must emit the same bytes as supplying the independently
evaluated native `CallPlan`. Result-bearing field and table calls also require
identical planned widths. Their mechanism records no longer carry the former
declared-parameter-count compatibility field; the authoritative plan and the
dispatch recipe now determine the complete call shape. The production x86-64
ISA encoder, width, and
field-call data-relocation APIs now require that authoritative plan directly;
normalized Win64 reconstruction is a test-only oracle, and the unused SysV
no-plan entry points are retired. The AArch64 vtable and service-table plan
normalizers and their data-relocation consumers likewise require the retained
AAPCS64 plan; no field-call relocation path synthesizes one from operands.
Scalar
authored imports carry the same byte/width
lock on Microsoft x64 and both AAPCS64 targets. The x86-64 compatibility host
encoder has no SysV authored-import path, so that target instead proves it
fails closed without a plan and succeeds with the explicit SysV plan.
Ordinary import encoding, width, AArch64 placement, and x86-64 call/data-site
APIs make that split structural: their `with_plan` surfaces require a concrete
plan reference, while the separately named no-plan entry points exist only for
the differential oracle. Object-level external-call and data-address offset
helpers use the same required-plan/named-no-plan split. Production emission or
relocation accounting cannot pass `None` through an apparently plan-aware API.
Linux statement, value-result, timespec-result, and timespec-argument syscall
families likewise compare compatibility selection with an independently
evaluated x86-64/AArch64 plan. Their emitted bytes and planned widths must
match; result and argument relocation sites must remain identical as well.
Their `with_plan` encoding and width APIs require a concrete plan, while the
value, timespec-result, and timespec-argument relocation helpers expose the
same mandatory-plan/named-no-plan split. Separately named compatibility
functions own the no-plan differential side. Production syscall emission and
relocation accounting therefore cannot request a plan-aware operation while
silently omitting its plan.

Ordinary non-variadic built-in imports now retain the selected binding plan
through machine emission, layout, and x86-64 relocation accounting instead of
reconstructing placement at those consumers. Direct scalar Windows x64 and
macOS arm64 calls have the same byte/width differential lock as authored
imports, and the Windows x64 comparison includes the call, argument, and result
relocation sites. A selected built-in import cannot represent an absent plan;
unresolved external rows reject before host binding construction, and the
no-plan route remains only an explicit differential oracle. The lock now
also covers void imports, the errno-style
pointer-result dereference tail, Windows key-state postprocessing, and AAPCS64
scalar-float arguments/results. The obsolete semantic-operation classifier for
Windows clock out-pointers is retired; the composite time encoder's concrete
subcall plan is the sole owner of that shape. On authoritative AArch64 built-in
paths, result presence and scalar-float class now come from the retained plan in
emission plus call/data relocation accounting; operation-key classification
remains only as the no-plan compatibility oracle. Void AArch64 imports likewise
consume retained argument placements in both relocation walks, so an
outgoing-stack placement cannot leave emission and relocation accounting on
different plans. Composite adapter-owned native calls continue to consume
their separately normalized concrete subcall signatures; an outer source
boundary plan is not a replacement for the adapter's internal ABI call. Darwin
anonymous-variadic `open_create` is now covered through its concrete adapter
signature rather than forcing that internal fixed/anonymous boundary into the
outer source binding plan.

The object relocation walk independently sends every ordinary selected import
and its exact operands through that retained plan's encoder before accepting
call or data offsets. An incompatible import plan therefore rejects relocation
planning instead of letting offset helpers reconstruct catalog- or
architecture-shaped placement; absence is excluded by the selected binding
carrier itself. Compiler-materialized host constants remain
outside this rule because they have no foreign call or selected binding. Their
sole result-place relocation uses dedicated fixed non-boundary geometry rather
than entering the compatibility ABI oracle.

The unused x86-64 call/data relocation wrappers that silently selected
Microsoft x64 are now retired. The object relocation walk also no longer makes
a redundant second no-plan data-site query after its plan-aware query. The
remaining x86-64 relocation consumers therefore either name the policy being
tested as a compatibility oracle or supply the retained authoritative plan.

Concrete promoted variadic calls now have a normalized signature that retains
the fixed/anonymous parameter boundary. The first Darwin AAPCS64 evaluator
keeps fixed parameters on the ordinary plan and places promoted anonymous
scalars in the outgoing stack area; a focused `open(path, flags, mode)` plan
pins `mode` at stack offset zero. The open-create encoder, layout width, call
relocation, and data relocation now consume that complete plan. Their former
manual `+8`/`+12` stack accounting and trailing-mode operation classifier are
retired; the operation key only selects the concrete adapter subcall.

Final footprint certificate format v19 now retains an exact
function-to-instruction partition in the encoded carrier. Checked image
emission replays every contiguous function
and instruction boundary over relocated final bytes, rejects gaps, overlaps,
and unowned instruction rows, and fingerprints the complete partition into the
versioned footprint certificate. This is boundary enumeration evidence, not an
instruction footprint assertion: ordinary instruction rows still need their
closed target-spec footprint replay before the certificate can mark compiler
body footprint decoding complete. The retained first and last rows of every
compiler function do already replay the exact architecture-owned entry and
return byte programs. This replaces the old entry-symbol-only prefix/suffix
check and binds the validated mechanics count and fingerprint into the same
certificate. Final validation also derives their architecture-owned
register/machine-state union, requires equality with the `StatePlan`-validated
`CallReturnMechanics` fragment, and binds that footprint plus the same boundary
contract identity into the certificate.
The replay boundary is the compiler-authored prefix, not the entire executable
`.text` section. Format-owned import-thunk tails appended by Mach-O or PE stay
outside compiler-function enumeration and are validated by their separate
closed thunk specifications.
The first middle-row target-spec replay covers dispatch-loop entry, case entry,
state writes and termination, forward arm skips, and case leaves. Their
normalized indices and branch distances survive emission; final validation
regenerates the x86-64/AArch64 program and fingerprints the matching placed
bytes. The certificate names this as a body-specification subset and continues
to list complete compiler-body footprint decoding as missing. Storage-backed
static guards are included by retaining the comparison operator, storage
region, operand geometry, immediate, float mode, and branch distance. Their
zero-address target program is checked before relocation; exact storage-symbol
relocation rows and unchanged non-relocation bits bind that recipe to final
placed bytes. Place-pair and place-vs-immediate guards retain canonical `Place`
operands and replay the same x86 materializer or admitted direct AArch64
encoder, including every place/index storage relocation site. Final validation
independently derives the dispatch-scaffold, static-guard, and place-guard
register/machine-state unions from the replayed rows, requires
them to equal the semantic fragments already admitted under the boundary's
`StatePlan`, and includes the resulting footprint fingerprint and boundary
contract identity in the certificate. Certificate construction rejects a
different public ceiling. Unreplayed body classes remain outside that partial
proof.
Direct storage-pair `CopyPlaces` operations, direct-storage copies targeting a
frame-held pointee, frame-held pointee sources landing in direct storage, and
frame-held pointee-to-pointee copies are the first ordinary write rows in that
subset. Single runtime-indexed sources with frame-held descriptor/index slots
and a direct frame or machine target are included too. Their `Ordinary` role
remains distinct from hidden-result copies; final validation replays the exact
target encoder and relocation set and matches the derived scratch union to the
retained `CompilerBodyPlaceCopy` fragment. Other ordinary copy/write shapes and
calls remain unreplayed. Pointee-pair selection resolves both reference
operands before flat storage so the source pointer is never copied as field
data.

Remaining order:

1. Complete plan-driven outbound calls and their results;
   differential-check every supported compatibility encoder against the plan,
   add the concrete firmware/interrupt state policies, and make the plan
   authoritative.
2. Derive outbound encoders and inbound stubs from the same plan.
3. Add state-ceiling-aware instruction selection/register allocation and
   contextual specialization.
4. Emit object-level footprint evidence and validate the final artifact.
5. Add external-root reporting and the x86 interrupt vertical slice.

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
