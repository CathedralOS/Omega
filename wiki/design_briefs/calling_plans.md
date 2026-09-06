# Design Brief: Calling And Machine-State Plans

Current as of 2026-08-24. Boundary conventions are normalized policy artifacts;
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

## Structural parameter access and reference identity

The [structural borrow identity ruling](core_multiplicity_and_linearity.md#structural-borrow-identity-at-native-calls)
separates referent layout from parameter passing. `Owned` retains shape-selected
value ABI treatment; shared, mutable, and write-only borrows use the existing
`ValueClass::BorrowedReference` contract to preserve original storage. An owned
aggregate may itself require physical indirection without becoming a borrow.

Structural signatures are derived from parameter declarations through one
exhaustive access classifier, not paired afterward with an independently built
shape list. Structural producers must require that derived result; generic
scalar/low-level ABI construction remains available. Caller argument preparation
still checks the actual referent and permitted access, and receiving plan
validation/replay independently rejects inconsistent or substituted access,
shape, and placement. Staging the reference pointer is valid; silently staging
the object as a replacement for that reference is not. Copy-based optimization
requires occurrence-specific observational equivalence, including relevant exit
paths. Complete enforcement remains implementation work, not a new ABI meaning
to choose in each backend.

## One boundary entry plan, two independent facets

An ordinary ABI is a layout over registers and a stack. It does not, by itself,
describe hardware entering while another activation is live.

```omega
data CallPlan {
    params: [Placement];
    result: Placement;
    callback_materializations: [CallbackMaterialization];
    ordinary_clobbers: RegisterSet;
    stack_align: count;
    shadow_bytes: count;
    entry_control: EntryControl;
}

data NativePlace {
    case Parameter(parameter: NativeParameterId);
    case Field(
        parameter: NativeParameterId,
        layout: LayoutPlanId,
        field_path: [LayoutSlotId],
    );
}

data NativeParameterSource {
    case SemanticFormal(formal: ParameterId);
    case PrivateCallback(
        binder: StaticMachineBinderId,
        requirement: CallbackRequirementId,
    );
}

data NativeParameterApplication {
    parameter: NativeParameterId;
    source: NativeParameterSource;
    shape: AbiValueShape;
    placement: Placement;
}

data CallbackMaterialization {
    binder: StaticMachineBinderId;
    destination: NativePlace;
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

data BoundaryPlanApplication {
    requirement: BoundaryRequirementId;
    native_parameters: [NativeParameterApplication]; // authored ABI order
    plan: BoundaryEntryPlan;
}
```

`CallPlan` owns parameter/result placement, private callback materialization,
and ordinary ABI behavior.
`StatePlan` owns the state that already belonged to an interrupted activation,
the state an entry stub preserves, and the state the handler and its callees may
use. Their projections coincide for many ordinary calls; that is not a reason to
fuse their identities.

`NativeParameterId` names one entry in a single native-parameter identity
space. An ordinary source formal contributes a `SemanticFormal` entry. A
direct callback contributes a `PrivateCallback` entry that has no Omega runtime
value. `NativePlace::Parameter` names the whole entry;
`NativePlace::Field.parameter` names an ordinary entry whose native layout owns
the selected private field. Declaration order fixes ABI position, while the
declared name fixes nominal identity. A multi-register aggregate remains one
entry with a multi-location placement.

The physical `CallPlan` fingerprint is reusable across declarations with the
same ABI recipe. The boundary-plan application fingerprint is stricter: it
includes the exact requirement, ordered native telescope, every nominal
parameter-to-placement row, callback materializations, and physical plan.
Consequently swapping two equally shaped parameters rejects even when the raw
register sequence is unchanged.

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
canonical evaluated result—not `C`'s symbol, source body, or unnormalized
construction order—enters public contract identity. The complete canonical
public contract now has a domain-separated SHA-256 commitment; its historical
64-bit fingerprint is compatibility/report metadata only. Checked plans,
realized envelopes, Terminal handoff, and nominal provider selection replay the
strong commitment. Refactoring a policy machine without changing its normalized
output therefore preserves ABI identity; changing an observable placement or
state commitment changes it.

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

A runtime opaque `boundary data` value crossing by value is such a choice. The
build must close one exact representation application before any `Calling<C>`
policy evaluates. That application contributes a compiler-derived bounded
`ValueShape` graph or sealed ABI leaf plus the carrier's physical movement and
finalization. The calling policy may classify and place that closed shape; it
may not inspect private carrier fields, invent a shape, or select a
representation.

The application is lazy. An opaque pointee used only through references needs
no by-value representation, and a proof-erased denotation contributes no
runtime shape. Compiler-owned target families such as `Ptr<T>` close from
`TargetSemantics`; provider-owned carriers close through the exact named
representation conformance selected by build composition. Every producer and
consumer of the opaque value must resolve the same application. This is one
selection per opaque declaration in the active compilation, not a unification
of selections made when dependency packages were independently reviewed as
roots. Future independently compiled artifacts compare the strong application
commitment at each actual by-value composition edge.

The exact opaque declaration, representation source, target-semantics version,
closed shape, physical movement, role-tagged lifecycle disposition, and
evidence origin enter the
`BoundarySignature` and `CallPlan` application fingerprints. Representation
closure therefore precedes calling-policy evaluation whenever an opaque
by-value demand exists; replay against another carrier rejects before placement
or lowering.

Fixed arrays and fixed records are structurally determined aggregates. Their
element/member types, count, size, alignment, and public layout are available to
the policy, which may classify or reject them under the target's actual
aggregate rules. A `[f32; 4]` may therefore become an HFA under AAPCS64, use an
SSE aggregate class under SysV AMD64, or be indirect under a policy that requires
that result. Classification is recursive and semantic; equal byte size alone
does not imply equal ABI class.

An `[erased]` record field remains part of the semantic type and proof identity,
but contributes no public ABI field, offset, size, alignment, register/stack
fragment, or transfer. The normalized policy graph and native classifiers omit
such fields recursively for fixed non-generic records, while terminal Psi keeps
an opaque row containing the exact erased type identity. A record whose fields
are all erased has no by-value ABI carrier and rejects. This rule does not extend
the graph vocabulary: case-bearing data and unresolved generic aggregates remain
unclassifiable until their own public ABI shapes are specified.

`BoundarySignature` presents that structure as a bounded flat graph rather than
as recursively embedded values: semantic parameter and result roots index
`ValueShape` nodes; fixed-array nodes name their element root and count; record
nodes name a contiguous `ValueField` range whose entries carry child roots and
exact byte offsets. A separate ordered native telescope projects those semantic
formals and interleaves any declared native-only callback entries. Each entry
retains nominal identity, origin, and shape; a private callback receives the
selected target's function-pointer shape without acquiring a semantic graph
node. The policy returns a separate `AbiValueShape` and placement for each
native entry. The compiler checks classification against both the semantic
graph and native telescope before accepting and fingerprinting the application.

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

The current envelope is a typed `image` value rather than a report-only
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

Plans exist only at boundaries. Most callers do not name one: the satisfied
requirement and exact selected realization determine the pinned policy. An
external `Binding` contributes only when the realization needs an
undiscoverable locator or selector payload. Explicit policy identity is authored
on the requirement, never inferred from a DLL name, syscall number, or friendly
target string.

The compiler derives provider-plan rows from `satisfies` and, when present, a
payload-bearing `via`; build-time machines may compute policy results or select
among declared candidates, but do not imperatively assemble a second plan
table. A builder that restates
requirement-to-implementation edges would duplicate `satisfies` and remains
rejected independently of machine-parameter implementation progress.

Provider plans remain derived from explicit `satisfies` declarations. A
bodyless boundary satisfier is an external leaf; `via` is retained only when it
carries binding data that exact declaration, signature, and target identity do
not already determine. Admission proves or accepts that a realization refines
the complete boundary plan. See
[`extern_boundary_and_format_domains.md`](extern_boundary_and_format_domains.md).

When a provider explicitly conforms to a descendant boundary trait that selects
an inherited requirement's checked calling application, that descendant owns
the selected schema while the row keeps its declaring requirement identity.
This applies to ordinary scalar signatures as well as routed entry claims;
calling-policy selection does not require an unrelated authority-bearing input.

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

The first physical erased adapter admits an immutable bare-dynamic parameter
and calls one exact table slot. Its retained normalized requirement identity,
slot byte offset, and every candidate realization are validated before the
caller plan is accepted. Each candidate's realization signature must evaluate
to the same structural `CallPlan`; a representative candidate cannot define the
erased ABI by coincidence. That one plan owns the descriptor receiver,
arguments, and result transport. Runtime-body lowering therefore excludes the
direct/spliced terminal-result replay used by inline calls. The private call's
closed validation identity also distinguishes it from an authored foreign
table call, whose mechanism independently requires the floating-control
envelope.

Standalone private realizations do not acquire separate boundary contracts.
Their complete prologue/body/result/return footprints are independently derived
and composed by normalized union into the enclosing root certificate, while
their internal call plan remains authoritative for the call bytes. Missing
instruction spans and root-ceiling mismatches reject with diagnostics; neither
can silently omit a private fragment or panic during selection.

The first mutable-local version of that adapter remains deliberately narrower
than general erased storage. One local may be initialized and reassigned by
exact direct-place casts only when carrier, trait, named conformance, and the
complete normalized row map are unchanged. Checked facts retain both statement
versions; checked-to-state replay reconstructs the assignment and prior row
identity; the forwarded argument selects the latest prior version. Runtime
lowering then replaces both words in the exact existing local slot with the new
instance address and unchanged selected-table address. The same exact local may
now dispatch one requirement directly after that rebind. State-call planning
retains the unanimous binding and latest selection statement as a closed
rebound-local receiver, rejects version collisions without falling through to
devirtualization, and owns the selected table demand independently of forwarded
argument evidence. Instruction selection rejoins the latest checked selection,
sole exact conformance candidate, existing two-word local slot, common
normalized table row, and its authoritative `CallPlan` before emitting the
ordinary private table-slot call. A changed or inferred conformance, non-cast
assignment, control-flow join, aggregate storage, return, and component
crossing gain no authority from this rung.

The first checked control-flow join is narrower than general descriptor
storage. Exactly two syntactic calls may feed one bare dynamic state parameter,
and both calls must already have complete descriptor-transfer custody. The
joined outgoing transfer retains two distinct paths: each path owns its exact
root conformance selection and the complete ordered call-edge chain through
the join. Even when both roots select the same conformance, their source places
remain distinct; when they select different conformances, neither may stand in
as a representative for the other. An unrecognized third predecessor, a
missing edge, or a substituted selection or coordinate publishes no joined
transfer. An acyclic chain of subsequent transparent parameter-to-parameter
calls may carry the joined descriptor onward, provided every transfer retains
both complete predecessor paths. A second join that combines an already-joined
value with another predecessor remains fenced. Terminal Psi already has the required
runtime form: each predecessor call supplies its own selection-sourced
descriptor argument to one shared callee descriptor parameter. That parameter
is the runtime phi; no representative conformance and no synthetic joined table
exists. Verification, canonical encoding, and interpretation exercise two
branch-local arguments with distinct source places and distinct closed
conformances. The checked producer now groups the first three-state Boolean
source shape into one joined-call plan containing the exact guard, both
successor edges, both complete branch-local calls, and the two exact root
transfers into the shared parameter. This avoids presenting the backend with
two rival plans for one source machine. Checked-to-Terminal lowering replays
that plan and emits one three-block conditional caller, two selection-sourced
descriptor arguments, one shared helper descriptor parameter, and the two
independent closed applications and realization machines. Canonical
verification, codec replay, source-call occurrence accounting, and
interpretation cover both branches. Promotion retains the structural carrier
shapes needed by the joined plan after consuming the direct candidates.
Target-neutral lowering then preserves the conditional and both descriptor-
bearing calls, including their distinct selection/application sources and one
shared helper parameter dispatch. Optimization reconstruction independently
validates that graph and rejects replacing either predecessor with the other.
Target lowering preserves that same graph without a joined-table form. Its
attached-Unit ABI keeps the entry guard as an exact Boolean parameter, and one
parameter-conditional operation binds both successor ordinals and nominal
return edges to the two descriptor-bearing leaves. Each leaf still owns its
distinct selection/application custody and the same helper target. Physical
assignment rejoins the guard with the complete Unit call plan, selects its
exact register or incoming-stack coordinate, and assigns both leaf descriptor
arguments independently; guard identity or placement drift fails closed.
Native encoding emits that Boolean split and both leaf calls with one shared
Unit epilogue: the true-arm return jumps over the false leaf into the common
frame-release/return sequence. Object construction independently decodes the
conditional and convergence targets, rejoins the five semantic intervals and
the two exact descriptor sources, and rejects branch-byte or source collapse
before final-image replay. Canonical installation format 69 carries the same
general Unit scalar ABI plus each forwarded call's full structural source
(root, path, and access), then rejoins the Boolean ABI, both source paths, call
spans/results, and five semantic intervals without introducing a join-specific
record. Transparent forwarding after the join uses that same ordinary
descriptor-parameter ABI: each additional helper forwards its incoming
parameter, the final helper alone dispatches, and source-to-installation replay
retains both alternative root paths on x86-64 and AArch64. Aggregate storage
beyond the bounded single-field local form below, a second join over an
already-joined value, component crossing, and wider control graphs remain
fenced.

The first Terminal aggregate-storage rung accepts only the checked immutable
one-field record initialized from one earlier exact shared-borrow selection.
Terminal custody preserves one dense aggregate-local ordinal, the normalized
aggregate and field identities, the selected closed interface/application, and
the portable descriptor word roles `{ instance: 0, table: 1 }`. A scalar
indirect operation consuming that descriptor is the exact field reload; it is
not rewritten as a direct call. Verification rejoins the stored row to its sole
selection, application, requirement row, callable, and operation, and rejects
word-role, interface, or coordinate substitution. Terminal format 73 /
vocabulary 76 encodes this target-neutral row. It does not claim physical field
offsets or native replay; mutable, multi-field, nested, joined, returned, and
component-crossing storage remain outside this rung.

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

For a nested native destination, the independently evaluated layout declares
the stable typed slot before the registrar can supply it:

```omega
WndClassWindowProcedureSlot:
    WndClassLayout satisfies
        PrivateCallbackSlot<WindowProcedure::call>;

machine WndClassLayout::plan(schema: Schema) -> Plan
    satisfies Layout::plan
{
    ...
    Plan::place_private<WndClassWindowProcedureSlot>(
        plan,
        window_procedure_offset
    )
}
```

The plan operation explicitly names the conformance; the compiler never scans
for conformances attached to `WndClassLayout`. The conformance is inert until
cited, so ordinary third-party named-conformance rules need no owner or orphan
exception. Its subject and static requirement argument form the exact typed
pair needed for wrong-layout and wrong-requirement rejection. The layout owns
the physical offset. That offset is target-dependent geometry, never slot
identity and never an outbound calling-plan coordinate.

The registration operation binds its static machine parameter to the exact
callback requirement with the nominal `where machine` form:

```omega
boundary trait WindowRegistrar:
    Calling<MicrosoftX64, MicrosoftX64Policy>
{
    machine register<machine Selected>(
        specification: &WindowClassSpecification
    ) -> Registration
    where machine Selected satisfies WindowProcedure::call;
}
```

The registrar requirement supplies its ordinary evaluated outbound calling
plan. The concrete realization supplies only the external locator binding; no
callback-specific declaration keyword is added:

```omega
windows_x86_64 machine User32Bindings::register_window_procedure()
    -> Binding<10, 16, 0>
{
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "user32.dll",
            export: "RegisterClassExW",
        },
    }
}

boundary machine User32::register_window_procedure<machine Selected>(
    specification: &WindowClassSpecification
) -> Registration
where machine Selected satisfies WindowProcedure::call
satisfies WindowRegistrar::register
via User32Bindings::register_window_procedure();
```

That plan maps the nominal binder slot to one native place, conceptually:

```omega
CallbackMaterialization {
    binder: RegisterClassSignature::Selected,
    destination: NativePlace::Field(
        RegisterClassNativeParameters::class,
        WndClassLayout,
        [WndClassWindowProcedureSlot],
    ),
}
```

The source name resolves to the binder slot in the registrar's normalized
static-machine telescope, not to the machine later substituted at a call.
Nested placement is only half the native surface. A direct callback parameter
is declared in the registrar requirement's native telescope, interleaved at
its actual foreign-ABI position:

```omega
boundary trait HookRegistrar: Calling<MicrosoftX64, MicrosoftX64Policy> {
    machine install<machine Handler>(
        hook: HookKind,
        native callback procedure from Handler,
        module: ModuleHandle,
        thread: ThreadId,
    ) -> Registration
    where machine Handler satisfies HookProcedure::call;
}

HookRegistrar::install<ApplicationHook>(hook, module, thread)
```

`procedure` contributes no Omega runtime argument and the source call omits it.
It is nevertheless an explicitly authored native parameter, not a trailing
argument invented during lowering. Its declaration binds the exact `Handler`
binder and therefore the exact `HookProcedure::call` requirement. The compiler
mints its nominal `NativeParameterId`, adds its target-closed function-pointer
shape to the ordered native telescope, and publishes the corresponding demand
as `NativePlace::Parameter`. The calling policy assigns declared entries to
registers or stack locations; it cannot create, reorder, or retarget them.

Native parameter and layout-slot identities are nominal plan values. Native
position is separately the authored telescope order. Raw native ordinals,
field byte offsets, inferred binder order, address-typed callback values, and
undeclared hidden parameters are not placement forms. One declaration is
sufficient for a direct parameter because the registrar requirement owns both
the parameter and the telescope. A nested field keeps the separate named-
conformance citation because its independently owned layout must authorize the
demand.

The fail-closed matrix exercises those distinctions at both source admission
and independent retained-product replay. It rejects an authored `addr`
substitute, omitted or inferred `from`, duplicate declaration names,
duplicate/wrong binders or requirements, a locally coherent invented native
parameter, a policy-returned extra telescope entry, the ordinal-derived v1
parameter identity, and a self-consistent historical application-v2 envelope.
The current application-v3 identity is recomputed from the exact retained
signature and plan; no older coordinate or commitment is translated into it.

Downstream direct-parameter lowering currently closes one address-free custody
cohort: exactly one direct `NativePlace::Parameter` callback on the ordinary
unoptimized normalized-import path for supported ELF/Mach-O targets, with only
the established fixed-width integer semantic arguments and result. The cohort
requires exactly one binder, demand, and materialization, a target-pointer-
shaped application, and one complete register or stack placement. Its retained
occurrence rejoins one Terminal registrar operation by exact `OperationId`.
Target lowering retains its exact nominal parameter, authored native ordinal,
target-closed application, and selected callback-thunk
`MachineFunctionIdentity` beside the ordinary source scalar arguments.
Physical assignment then binds that same entry to the policy-selected register
or stack destination without synthesizing an Omega value or inferring a
parameter position. The retained thunk identity is not a function address,
pointer value, or emitted symbol. Field destinations, multiple callbacks,
thunk-body or symbol synthesis, address loads, call emission, relocations,
source-free artifact replay, and executable registrar settlement remain
outside this cohort.
The application-v3 commitment retained on this carrier remains a compiler-
origin provenance projection. This rung does not claim to rederive it from the
reduced plan/context tuple: the complete authored nominal telescope required by
the canonical application identity is no longer present. Independent
commitment authentication remains required before artifact publication.

The requirement supplies the complete callable signature, contracts,
operational ceilings, and evaluated calling/entry plan; the binder does not
repeat them.
`WindowRegistrar::register<ApplicationWindow::dispatch>(&specification)`
selects the authored satisfaction row for that exact requirement. A matching
signature or uniquely visible conformance establishes nothing.

`WindowProcedure::call` is a signature-free requirement reference and must
resolve to one exact overload at the binder declaration. Ambiguity rejects.
This is the same rule used by domain `established by` clauses and other
signature-free
requirement paths. There is no callback-local expanded-signature workaround,
and `as Name` remains either the exact-edge satisfier-set label or the complete
conformance selector dictated by its grammar position, never an overload
selector. Adding an overload to an existing requirement name is therefore a
breaking change for all such references and must be reported at the declaring
trait as well as the affected uses.

That compatibility report is live in the shared pre-normalization resolver.
After symbol assignment, it groups ambiguity by exact declaring-trait symbol
and requirement name, emits one declaration-anchored diagnostic per overloaded
family, then emits source-ordered diagnostics for every affected nominal binder
and authored domain route. Running before either consumer rewrites its authored
path prevents the first ambiguous use from masking the rest of the breakage.

The normalized registrar plan retains the registration-operation identity,
exact binder-slot identity, destination, and complete evaluated-plan
fingerprint independently of any selected callback. Each checked callback use
separately retains the call site, static-machine argument ordinal, selected
machine, selected satisfaction row, exact canonical requirement overload, and
evaluated target entry recipe. Lowering joins those rows; substituting another
valid callback changes the per-use/thunk identity but not the registrar's ABI
fingerprint. The callback's published requirement envelope and selected
machine's actual envelope remain separate, with an explicit refinement proof
from actual to published. The foreign protocol relies on the published
envelope; installation, resource, reach, and crash reasoning may use the
narrower actual envelope. Callback admission rejoins both the published report
coordinate and strong commitment to the exact requirement capsule, while the
actual commitment rejoins the selected machine contract. Target closure also
updates a checked callback placement only when its pre-closure report
coordinate and strong calling-plan commitment both match; compact equality
alone cannot transfer the placement to a newly closed plan.

That evaluated identity is a boundary-plan application, not merely the
reusable physical `CallPlan`. It fingerprints the requirement's complete
ordered native telescope and each nominal `NativeParameterId` paired with its
validated `ValuePlacement`. Hashing only the ordered placements is
insufficient: two pointer-shaped parameters could trade positions without
changing the register sequence. Parameter identity is derived from the owning
requirement plus the declared parameter symbol; authored order is retained as
separate ABI metadata. Changing either invalidates replay for the appropriate
reason.

The earlier ordinal-derived parameter IDs and callback-placement fingerprint
domain are not reinterpreted. Moving to nominal IDs and the application
fingerprint is an explicit format-version migration: affected plans,
callback-placement receipts, and downstream artifacts are reissued, and rows
from the two versions never compare or translate heuristically.

The first private-relocation planner remains deliberately address-free. It
selects the callback handler's inbound plan only from the exact satisfaction
trait/requirement and independently selects the registrar's outbound
realization by the exact registration-operation symbol, static binder ordinal,
and satisfaction row. One retained materialization binds the complete ordered
registrar context and validated plan/fingerprint to its exact binder,
requirement, and `NativePlace`. Backend planning joins that receipt one-to-one
and in placement order with the emitted thunk and callback-root schedule.
Replay rejects missing, duplicated, reordered, substituted-operation, plan,
context, row, thunk, or schedule identities. The resulting
`CallbackPrivateRelocationDemand` has no target operation, physical offset,
bytes, object relocation, runtime storage, native address, registration
authority, or lease; those remain later lowering and lifetime steps.

### Retired pre-Terminal callback carrier

The following host-call, assigned-operand, object-store, and installation-
manifest sequence records a removed custom/unknown host-operation prototype.
It is historical context, not current implementation or reusable authority.
The canonical replacement starts at the checked identity spine below and its
current frontier is stated in the direct-source section later in this brief.

The prototype retained the registrar occurrence through the ordinary
host-call and abstract-boundary spine without granting any of those later
authorities. One outbound `HostCallPlan` row records its exact authored
statement or expression site, resolved registration-operation symbol,
canonical registrar overload, state/statement/call ordinal, and exact platform
lowering identity. Its native telescope retains ordered nominal
`NativeParameterId` rows and, for semantic-formal entries, the exact source
formal identity; a private callback has no formal ordinal. A synthetic result-
place operand is explicitly outside that list. Abstract lowering reconstructs
and replays one occurrence row and one ordered native-parameter span, and every
host-operation edge points back to that occurrence. A source boundary edge may
link only when its resolved target symbol agrees in addition to the ordinary
state/statement/call coordinates. The target semantic summary preserves these
rows unchanged. They remain identity-only: they select no `NativePlace`, target
operand, byte offset, object relocation, address, registration authority, or
callback lifetime.

At the first backend-plan point where both catalogs coexist, one
`CallbackRegistrarArgumentBinding` joins each ordered private-relocation demand
to exactly one retained registrar occurrence and one ordered native-parameter
row.
Independent replay first revalidates the complete placement/thunk/demand
catalog, then checks the authored site, resolved registrar target, canonical
overload, state/statement/call coordinates, platform-lowering identity,
authored native order, and declaration-derived `NativeParameterId`. A direct
destination selects
that parameter row; a field destination selects the same root while preserving
its complete nominal layout identity and ordered field path. Multiple field
demands may share one root but remain distinct through those paths. Missing,
duplicate, reordered, handle-, identity-, layout-, or path-drifted rows reject.
The binding remains address-free and grants no target operand, physical offset,
object relocation, runtime storage, address, registration authority, or
callback lifetime.

The nominal callback IDs in this catalog are compact report discriminators,
not standalone authority. Before policy evaluation, the compiler reconstructs
each binder's exact canonical requirement from its retained trait and machine
symbols, rejects distinct requirements sharing one compact ID, and rejects
native-parameter ID reuse between distinct exact registrar parameters. Layout
and slot IDs remain joined to the exact data symbols, layout rows, field paths,
and private-slot demands described below; compact equality never bypasses that
replay.

The prototype's target-closed placement recipe bound that exact argument to
the outbound registrar `CallPlan` parameter's `ValuePlacement`. A direct
`Field` retains exactly one authoritative native-layout demand row, including
layout, slot, requirement, data symbol, offset, pointer extent, and alignment.
The bounded nested form retains one exact rooted path containing a nominal root
layout, one domain-separated inline named-record field identity plus its exact
`FieldLayout` edge and child layout, and one child-owned terminal private-slot
demand. The compiler preserves the terminal one-slot layout identity and gives
the rooted layout a separate data- and policy-subject-bound identity, so equal
physical layouts cannot alias roots and existing one-slot plans do not drift.
Checked calling signatures also retain a named callback-layout catalog: exact
semantic/native formal positions, root layout, optional inline field and child
layout, complete typed terminal slot application, and target-closed geometry.
Each entry stays paired with its native demand while sorting, and replay
requires complete, ordered, exact field-demand joins. Direct native callback
parameters contribute no invented layout field. The signature is exposed by
borrowed access so callers cannot substitute an unhashed catalog. These
compiler-owned records support later receipt-free policy projection; their
arena handles and compact native IDs are not an accepted-lock encoding.
The package-policy projector checks those exact compiler associations before
translating them to package-qualified declarations and ordinal references into
the retained native and static telescopes. Callback catalogs are ordered by
their semantic applications, with destination indices rebound after ordering;
compact-ID sort order does not become policy identity. Layout geometry remains
the pointee geometry for a reference parameter, distinct from the pointer's ABI
placement. Bounded typed recovery checks these relationships but does not
reconstruct a checked calling plan or a native materialization receipt.
Layout closure and independent replay both prove the field belongs to the root
record, rejoin the exact child record, and checked-compose the two relative
offsets while validating the terminal extent inside the child and the final
extent/alignment inside the root. Missing, duplicate, colliding, reordered,
short, overlong, reference-indirected, array, variant, or deeper paths reject.
The composed offset remains layout evidence and never becomes a materialization
identity. Direct parameter placement is covered only by synthetic compiler
tests pending the settled source/native-telescope implementation. The recipe
grants no selected or assigned operation, object
symbol, relocation, bytes, runtime address, registration authority, or callback
lifetime.

The selected/assigned registrar-operand prerequisite was completed only for the
closed custom/unknown outbound host-operation branch. Instruction selection
retained the exact source-call arena identity, call and operation ordinals, and
an ordered `NativeParameterId` to abstract-operand map with semantic-formal
identity only where one exists; result-storage pseudo-arguments are excluded.
Target lowering resolves that source handle to
exactly one registrar occurrence and boundary edge and carries exact abstract
and target operand handles. At backend-plan coexistence, one
`CallbackRegistrarAssignedOperandBinding` joins the prior physical destination
to its abstract, target, and assigned instruction/operand identities. Replay
rejects same-coordinate call collisions, missing or duplicated edges and
operations, native-telescope/cardinality drift, stale handles, and operand-
shape drift.
Generic host operations remain outside this opt-in carrier. The row still owns
no object symbol, relocation, bytes, runtime address, registration authority,
or callback lifetime.

The prototype's object-relative request closed only the one-slot
`Field` plus exact `RuntimeStorageAddress` shape. One
`CallbackPrivateObjectStoreRequest` preserves the complete assigned binding,
runtime-storage region and base, target-closed slot/destination geometry, the
canonical BSS owner symbol snapshot, and the exact private callback text-symbol
snapshot. Construction and replay rejoin every preceding catalog and reject
missing/duplicate symbols, wrong section or kind, bounds/alignment drift,
`DataAddress`, and the not-yet-implemented direct-parameter form. These symbols
are
identity evidence rather than resolved-address authority. Its closed compiler
rung inserted one exact `WriteFunctionAddressToRuntimeStorage`
operation immediately before its registrar host call, preserving the registrar
source coordinate and containing function. The operation survives
abstract-to-target-to-assigned lowering, is encoded on both ISAs with symbolic
function and BSS bases, and produces exactly two x86-64 `Absolute64` records or
two AArch64 `Page21`/`PageOffset12` pairs. Final replay binds the source through
`MachineFunctionIdentity`, the destination through the canonical BSS symbol,
and rechecks record origin/cardinality plus all non-relocation bits. The root
boundary certificate is extended with the complete compiler-body address-write
footprint; missing or insufficient root authority rejects. This grants no
resolved runtime address, registration, invocation, callback lifetime/lease,
or publication authority. `DataAddress`, direct parameters, and the bounded
two-hop path remain fenced.

The prototype carried that closed evidence through one ordered non-Clone
installation manifest. Its deployment layer bound each field/BSS entry to an
installed-code occurrence and retained attribution through pending/live
callback states. Both the manifest and those callback-specific deployment
states were removed; this historical sequence supplies no authority or
implementation to the current direct-parameter path.

### Current callback identity and custody spine

The checked identity spine is live. Admission records the exact statement or
expression handle, argument ordinal, registration operation, selected machine
and entry, unique satisfaction trait/requirement pair, and canonical overload
identity before specialization erases the static argument. The specialization
fixed point repeats admission after cloning so a forwarded generic selection
receives its own durable call-site row. Exact repeated observations collapse;
conflicting identities for one site and ordinal reject. Structural machine
parameters deliberately produce no nominal-use row. Published/actual envelope
projection and refinement evidence are described below. Callback uses now also
retain the nonzero report fingerprint of their exact evaluated boundary calling
plan; ordinary nominal uses retain no callback placement. This is the target-
owned plan cache/join coordinate, not authority or a source-visible address.
Check-only and native orchestration consume it before handoff, revalidate the
retained plan, and require one exact trait/requirement/fingerprint realization.
The validated payloadless thunk and its address-free relocation demand are
retained at this rung. The direct-parameter cohort now completes target/object
relocation and final-image replay as described below; field materialization and
runtime registration remain later lowering slices.

Provider planning carries the complete validated inbound plan in the bound
placement and the complete registrar plan in any private materialization. The
thunk's structural placement identity now retains the inbound plan itself as
well as the compact report coordinate. Root-schedule replay compares that exact
identity and independently revalidates the plan before consulting later compact
summaries, so a collision-equal plan substitution rejects. The compact inbound,
registrar, and ordered-thunk fingerprints remain compatibility/report data;
they do not authorize a callback, relocation, or installed entry.

The canonical checked-to-Terminal producer accepts the retained callback-
placement sidecar as an opaque by-value custody input and returns it unchanged
and in the same order on success or rejection. The compiler's Terminal product
retains that sidecar beside the source-free artifact, exposes borrowed
observation, and permits only a consuming transfer of both parts together.
Checked provenance and canonical row order come from the private driver route;
the report carrier validates each row but does not attempt to reconstruct its
origin. The canonical native-realization seam also has a generic opaque by-value
adapter that returns the exact sidecar beside successful native realization or
diagnostic rejection without interpreting its contents. The neutral source-
free `NativeArtifact` does not absorb those checked-derived rows. Separately,
the compiler source-evaluated-import re-entry consumes one admitted direct row
through target lowering and physical assignment, then rejects at callback
emission. Check-only compilation remains valid. These custody routes grant no
registration, invocation, address, lifetime, or publication authority.

The identity row additionally pins two separate normalized public-contract
endpoints: the callback requirement capsule and the selected machine's declared
contract plan. A validated-refinement receipt explicitly binds those two
fingerprints, rather than asking consumers to infer a relationship from
adjacent identities. The requirement capsule retains its canonical published
reach, direct invocation, suspension, blocking, termination, and crash axes.
The selected exact-machine envelope separately aggregates effective checked
reach/invocations, transitive suspension/blocking, checked termination/crash,
mutation frames, and capability flows without promoting them into caller facts
or changing either fingerprint. That realized envelope now also owns one
checked resource-derivation anchor per concrete entry. The anchor binds the
exact machine, entry, and realized contract endpoint independently to stack,
logical structural-work/fuel, and machine-state obligations. A boundary
callback use now retains a separate receipt over that exact anchor beside the
target calling-plan key. Provider planning independently rejoins the receipt
to the current checked roster, then the bound placement and complete placement
identity carry it through thunk/root/manifest replay; the compact callback
identity summary also folds it. The anchor and receipt carry no numeric demand,
ceiling, target footprint, provider receipt, or installation authority, and
their fingerprints are compilation-local join summaries only. Callback
resource admission must next join each axis to its independently derived
Terminal/target/backend evidence; the target calling-plan fingerprint cannot
substitute for any of them.

Lowering alone materializes the thunk relocation at the plan's exact native
argument or nested field. A nested destination must name a typed private demand
from the validated layout plan, and complete outbound-plan validation requires
each demand and callback binder to participate in one compatible, nonoverlapping
row. Neither the static machine argument nor its address
becomes an Omega runtime value. Compiler-private function identity remains
intact through assigned target operations, machine instructions, encoded
bytes, and object planning. A callback thunk has a distinct function role
bound to its placement-row index and selected source entry; it cannot satisfy
emission by relabeling the ordinary source-entry function. Object planning
preserves the richer site/fingerprint-bound private symbol carried by that
role, while final emission independently recomputes it from the placement.
Target-instruction lowering validates the complete assigned function set
before selecting any body: an invalid role or two functions claiming one role
reject rather than surviving until object planning.
Every retained internal direct-call target must also resolve in that exact
assigned identity set. Role, placement, continuation-generation, or absence
drift therefore rejects before placeholder encoding; this pins the call edge a
future callback body will use without claiming that the body exists yet.
Native image emission rejects a planned callback unless that exact callback
identity names one encoded function and one matching private text symbol.
The final compiler-function replay independently rejects invalid or duplicate
function identities and folds each role, continuation handle/generation,
segment, and callback placement into its validation fingerprint. Function-role
substitution therefore changes final derivation evidence even when byte
intervals and instruction rows are otherwise unchanged. That replay also
resolves every encoded identity through the object's exact-unique function
binding and requires the bound text-symbol interval to equal the encoded byte
interval. Missing, duplicate, redirected, non-function, non-text, and interval-
drifted bindings therefore reject at the final image boundary for source,
wrapper, and callback roles alike. Object-local symbol spelling is deliberately
not compared to the encoded source display name; callback private-symbol
recomputation remains the stronger callback-specific check described above.
Final replay nevertheless rederives the canonical public entry name for the
one object entry and the canonical private name for every non-entry source or
wrapper identity. Renaming either rejects, and a callback identity cannot own
the process entry. This keeps authored display spelling separate from linkage
identity without treating arbitrary object spelling as trusted.
The shared non-entry private-name primitive encodes the role, machine arena
index and generation, state arena index and generation, and segment index.
Generation drift can therefore neither preserve linkage spelling nor collide
with a live canonical source/wrapper name. Callback spelling remains excluded
from this primitive because its placement and plan fingerprint are additional
identity inputs.
The common object-to-final-image copy is also checked before any format writer
runs. The final carrier must retain the exact entry handle and a one-to-one,
identity-owned copy of every function symbol's name, text classification,
offset, size, and kind; an unowned extra function symbol or an aliased binding
rejects. This is a carrier-retention check only and does not make symbol-table
spelling a source-level address or synthesize a callback body.
After format-specific placement, checked emission rejoins each encoded function
and exact object symbol to one compiler-function region. Symbol, section
offset, placed address, byte count, and final-byte fingerprint must all match;
missing, duplicated, renamed, reclassified, redirected, or byte-drifted
regions reject. Import thunks remain in their separate executable-region
namespace and cannot satisfy a compiler-function row. Before that join can
consume the inventory, checked emission independently replays the inventory
from final text: the text size and fingerprint, ordered region intervals,
derived addresses, per-region byte fingerprints, exact complementary gap
partition, origin/footprint-bearing rows, and aggregate inventory fingerprint
must agree. Thus a forged summary, reordered or overlapping span, altered
origin, or missing/changed gap rejects rather than becoming final placement
authority. This replay retains identity only; it neither synthesizes callback
bytes nor chooses registration-relocation placement. The checked compiler-
function evidence then retains the exact ordered join from each private
function identity through its object-symbol handle to that final region,
including the inventory identity and the region's symbol, index, address,
interval, and byte fingerprint. That binding fingerprint is part of both the
function evidence and final text derivation, so later certificates cannot mix
a validated function partition with a different, individually valid placed
inventory. Entry boundary-footprint attachment consumes a separately sealed
projection of that join: the exact compiler-private entry identity, object
symbol handle, final-region index/symbol/address/interval/bytes, inventory
identity, and complete function-region binding identity. The projection has
its own replayed fingerprint, so identity, handle, row, or custody drift
rejects before mutating the inventory; entry evidence is no longer attached by
linkage spelling alone. The mutation itself returns a checked custody receipt
binding that sealed entry projection and the complete function-region join to
the prior inventory identity, exact composed footprint, and resulting
inventory identity. Final footprint certification requires that receipt
whenever a boundary contract is present and folds it into both placement and
certificate identity. A certificate can therefore neither pair pre-attachment
function evidence with an unrelated post-attachment inventory nor omit the
only authorized post-validation inventory mutation.
The encoded compiler-text bytes, relocated compiler-text prefix, and canonical
ordered relocation envelope now also carry separate domain-framed SHA-256
digests. A fourth derivation digest joins those commitments to the retained
instruction/count report fields. Their historical FNV values remain compact
report compatibility only. Final-footprint construction and replay reject a
substituted strong derivation, while installation format 42 serializes all four
digests and rejects digest/compact-field drift before accepting the canonical
record.
The compiler constructs and revalidates this complete footprint certificate
before installing executable bytes. Auxiliary inventory
serialization consumes that already validated certificate instead of creating
authority after publication; report I/O may fail later, but no semantic
certificate failure can occur only after executable visibility.
Publication then seals the certificate to the exact emitted image evidence,
replayed final-text/inventory pair, output name and format, and full container
byte identity. Executable publication consumes that validated view. This is
non-serialized orchestration custody, not a new semantic footprint class: it
prevents a valid certificate from being
paired with another container or output identity before publication.
Certificate, publication, container, compiler-text, and installed-destination
joins use separate domain-framed SHA-256 digest types. Compact callback,
inventory, and function-validation fingerprints remain report coordinates
inside those exact semantic/container commitments; none is the sole
authority. Compiler-function validation additionally has its own domain-framed
SHA-256 commitment over the complete normalized validation summary. Final-
footprint identity incorporates that commitment, and publication receipts
retain and replay it separately from the compatibility fingerprint, so a
compact-collision-equal substitute rejects before executable custody is
accepted; the compact fingerprint is never a publication or installation key.
Executable installation writes that sealed container to a temporary file,
reads it back, and compares every byte before the atomic rename. The resulting
non-serialized receipt binds the publication identity, exact output path, byte
count, and container identity. A redirected output leaf or changed/partial
staged file rejects and is removed before it can become visible. The ordinary
object-container fallback remains outside this executable-footprint custody.
The compiler report retains the exact native-executable receipt across the
orchestration return boundary: certificate, inventory, publication, output
path, container, and checked installation identities remain joined instead of
collapsing back to a bare path. Check-only and object-container fallback
reports retain no receipt. This compiler-publication evidence does not grant
runtime loading or installation authority.
After atomic rename, the compiler independently reads the destination and
compares every byte with the sealed container before minting or returning the
installation receipt. A missing or changed destination is removed and rejects,
so the outward report cannot attest merely to a validated temporary file.
The [macOS application publication contract](macos_application_publication.md)
supersedes the former flat-plus-optional-bundle-copy design. Current publication
writes a flat executable even for macOS GUI output; std requests activation.
The optional bundle-copy carrier has no producer and is scheduled for removal,
not restoration as evidence of a complete application. Its flat-receipt and
general report checks remain required. Flat installation v1 retains its fixed
`0` destination tag and byte-identical digests after the enum is removed.

The replacement publishes one complete `.app` for a selected macOS GUI
application, using the post-compilation product-publication owner. It validates
the executable, generated plist, directory shape, and agreement between retained,
signed, and plist application identifiers. The package root and inner executable
have separate checked accessors with one validated structural relationship.
No separate flat copy is a required deliverable. This is a settled contract with
implementation outstanding, not a claim that the present writer emits bundles.
The compilation root remains read-only information about the reported build;
that property no longer depends on bundle-name derivation.

Immediately before an outward executable receipt is minted, installation
replays the renamed destination bytes once more against the sealed container. Destination
drift in the interval after the installation check removes the changed file and
rejects instead of returning stale custody.
The report also retains the exact orchestration output category. A native
executable requires checked executable custody, an object-container fallback
forbids executable receipts, and a check-only result forbids output and receipts.
Thus lost native custody cannot be reclassified as a valid fallback merely
because both use the older `wrote_output` boolean.
The validated output flag, category, and publication records are exposed only
through read-only report accessors. Callers can inspect that
custody tuple but cannot rearrange, replace, or drop one component after the
compiler's final consistency check.
Production orchestration now constructs both early check-only and backend
reports through the same checked constructor. The constructor rejects an
inconsistent output/category/receipt tuple before it can cross the return
boundary; raw custody-field construction remains confined to its unit tests.
That constructor also rejoins the optional program-storage entry binding to its
native bridge. Both must be absent together or the retained binding must equal
the bridge's exact binding; a dropped, unpaired, or redirected binding rejects
before report return.
The retained binding and bridge are outwardly read-only after that check.
Consumers can inspect or clone their evidence through accessors, but cannot
mutate one side of the report pair into a post-validation mismatch.
Report construction also joins bridge phase to output category. A check-only
bridge must remain pending without final wrapper evidence, a native-executable
bridge must retain that evidence, and object-container fallback cannot carry a
program-storage bridge. Missing or premature evidence therefore rejects at the
same return boundary.
For a native bridge, its final wrapper evidence must additionally name the same
executable-region inventory fingerprint as the executable publication receipt.
Evidence from another otherwise valid final image cannot accompany the
published container.
The receipt now also retains the compiler-text derivation and compiler-function
evidence fingerprints already present in the sealed certificate. Native wrapper
evidence must rejoin both rather than relying on inventory identity alone.
The receipt additionally retains the certificate's optional boundary-contract
fingerprint. A native program-storage arrival must name that same concrete
contract; absent or redirected contract
custody rejects before report return.
Final relocation replay also builds one exact owner map from every retained
selected-instruction identity to that function symbol. A selected instruction
retained twice, an instruction relocation naming another function, or an
instruction-origin row with no retained owner rejects before the image can be
accepted. Semantic-operation, semantic-edge, and materialization origins keep
their separate non-instruction identity namespaces.
Emission also rejoins every validated
placement row to exactly one thunk plan and rejects a missing, duplicate, or
out-of-range placement index, selected-entry drift, and repeated private thunk
identity. Retaining a thunk plan is not itself emission evidence, and these
checks do not materialize a registration relocation. Planning and final
emission share one canonical private-symbol derivation over the exact site
kind/index/generation, static ordinal, selected machine/entry handles, and
evaluated calling-plan fingerprint. Emission recomputes that identity, so a
stored-symbol substitution rejects even when forged encoded-function and
object rows agree with it. A retained `Registration` keeps the exact selected
identity in occurrence provenance and owns the code/component lease,
but ownership does not automatically import that narrower envelope into a
caller's proof context. A public API that exposes those facts forwards them in
its own contract.

A durable registration returns an ordinary linear package value. That value
owns the protocol registration and, when code unloading is possible, the
artifact or component lease. Its explicit terminal operation unregisters the
foreign entry before releasing those obligations. Call-scoped callback
parameters instead remain ordinary borrows and produce no durable registration.

The registrar is an ordinary runtime boundary operation. `build.omg` selects
and admits its realization and resource profile; it does not execute the
registration. Success establishes the future external root and moves any
finite live-registration capacity into `Registration`; rejection establishes
no root and returns that authority unchanged. Successful unregister ends the
root and returns the same capacity occurrence. A consumable lifetime budget is
a different authority. Many live registrations may share one compatible
statically emitted thunk, so capacity bounds runtime registration state rather
than code size.

The earlier custom object-store/installation-manifest implementation and its
callback registration modules were removed. The replacement runtime bridge
joins exact compiler-private entry attribution to the complete installed-code
occurrence and exact entry of an admitted external root, and retains that
attribution beside the provider's reclaimable registration through unregister
and root quiescence. Its retained-era split borrows the installed occurrence,
independent root ledger, and component lifecycle together. That bridge derives
the era and entry contract, acquires and validates the exact non-clone
component-era lease, and lowers the successful provider registration to the
package-visible linear `Registration`. Admission and teardown rejection
preserve their full inputs for retry. Typestate permits lease release only
after provider unregister and exact-root quiescence; the intermediate state
keeps returned capacity and root-slot authority inaccessible, and a failed or
cross-component release retains the completed callback and lease intact.

The direct-parameter path reaches canonical installation custody.
Installation format 51 retains the exact compiler-private thunk identity,
source-Psi identity, artifact-local machine, fixed-integer ABI, and final text
interval and rejoins that row to the executable image after encode/decode. A
first deployment prerequisite now accepts a caller-supplied, already-admitted
entry identity and binds it to that unique row's exact final-text start, bytes,
and opaque installed occurrence. It does not derive an entry identity, expose
an executable address, publish code, admit an external root, or establish
registrar success by itself. The runtime bridge performs the exact admitted
root/provider-result join and preserves retry custody through provider failure,
unregister, and quiescence. The retained-era bridge then binds that live
registration to the exact component-era lease and lowers the source-visible
linear `Registration` runtime carrier. Installed-entry attribution and the
lower-level callback carrier grant neither the lease nor lowering authority by
themselves. Capacity still means simultaneous live registrations, not emitted
thunk count or a consumable lifetime budget.

Callback materialization records only binder slot and destination. Whether the
destination is a direct argument, call-scoped temporary, or part of retained
stable storage is the ordinary native-parameter lifetime/custody disposition
of the outbound plan. It is not duplicated on the callback row. Foreign-owned
internal registration tables are provider state; an API that retains
caller-supplied storage must satisfy the general foreign-retained-storage rules.

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

`EntryStack` selects the execution-stack disposition; it is not a complete
account of external-entry storage. Root admission separately consumes a
normalized `EntryStackRealization`. That realization contains a finite,
validated set of admissible arrival contexts and one finite epoch sequence per
context, conceptually:

```text
EntryStackRealization {
    contexts: [ArrivalContextRealization]
}

ArrivalContextRealization {
    context: ArrivalContextId
    epochs: [EntryStackEpoch]
}

EntryStackEpoch {
    stage: Enter | Body | Exit
    active_domain: StackDomainRef
    occupancy_by_domain: [StackOccupancy]
    nesting: PhaseNestingAllowance
}
```

An epoch is a maximal interval with one active stack-domain interpretation and
one nesting allowance. There is exactly one `Body` epoch. A no-switch entry
normally has one enter epoch; a hardware-atomic switch starts directly in the
new domain; a software switch ends one epoch and begins another. Multiple or
conditional adapter transitions therefore require no new vocabulary.

For domain `d`, the composed demand is the maximum over every admissible
context and epoch of the epoch's occupancy on `d`, plus Terminal-Psi body WCSU
only when that epoch is `Body` and `d` is its execution domain, plus nesting
that the epoch permits on `d`. Context alternatives take their maximum rather
than their sum. A nested root's relative `Interrupted` domain resolves to the
parent epoch's `active_domain`; it is not one global stack identity. Concurrent
live use adds with alignment, sequential alternatives take their maximum, and
declared finite nesting depth bounds repeated occurrences. For stack resource
accounting, `Nestable(maximum_depth)` counts concurrently live occurrences on
one root lineage, including the current occurrence; zero rejects. `Masked`
admits no nested occurrence covered by that policy, and `ProviderDefined` must
resolve to equivalent finite evidence before bounded admission.

The complete admissible context set comes from validated installation facts,
not provider omission. A sealed target arrival rule applied to those facts
derives architectural arrival epochs, including privilege-conditional and
hardware-atomic switching. Generated adapter epochs derive from exact emitted
stub instructions. A direct compiler-emitted Terminal entry with no adapter
prologue is the degenerate generated case: its exact installed Terminal stack
closure derives one body epoch and requires no provider receipt. An opaque
adapter instead carries an admitted complete arrival-context set bound to the
target, exact installed entry, boundary plan, root, provider, and validation
receipt. That set must equal the contexts in its domain/epoch realization;
neither a bare receipt nor a structurally valid subset establishes
completeness. Generated and opaque origins remain distinct in artifact identity
and reports. A bare byte count establishes nothing.

The installed-entry and opaque-arrival carriers bind that boundary plan with
its domain-separated commitment. Their historical 64-bit plan, target-rule,
domain, and realization values are report/cache coordinates over retained exact
facts; a compact-equal boundary-plan substitute cannot settle either the target
arrival rule or the external-root stack join.

The compiler's x86-64 rule derives the architectural frame from the exact
installed vector, arrival mechanism, interrupted/entry privilege pair, and
hardware stack selection. Same-privilege arrival retains three machine words;
a privilege or IST switch retains five; the sealed exception-vector table adds
the architectural error-code word. Installation metadata cannot supply the
word count or an error-code Boolean. The bound realization records target
arrival, adapter, and Terminal-body provenance independently, because those
parts may have different authorities even when they compose into one epoch
sequence.

The production x86 installation join does not accept that hardware stack
selection as an independent row. It reconstructs `Current`, privilege-change,
or IST selection from the validated gate's IST field, the exact installed TSS
slot/class maps, and each complete target-profile arrival context. The exact
boundary plan supplies the single stack/preemption policy, while the installed
code supplies the artifact, symbolic entry identity, and entry offset. The
public gate/TSS context details must exactly equal an opaque roster established
at the table/profile validation seam; its complete context set, exact boundary
commitment, validation receipt, and `InstalledCodeContext` remain retained in
the locally produced target carrier and are rechecked by the binder. Omitted or
padded contexts, compact-equal installed occurrences, missing or repeated TSS
selections, descriptor-entry drift, a non-interrupt boundary, and a derived
stack that disagrees with a fixed public disposition all reject before the
target rule can enter epoch composition.
`ProviderSelected` must close in every admissible context to the interrupted
domain or one exact provisioned domain before final composition. The selected
stack may differ across contexts when a sealed target rule proves
conditional hardware switching. Unknown arrival contexts or unresolved domains
reject; when no narrower phase-specific nesting fact is proven, the root's
declared nesting policy applies conservatively to every epoch.

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
`external-roots`.
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

The target-neutral realization carriers and pure epoch composer are live. They
validate and fingerprint the finite context/epoch shape, resolve relative
`Interrupted` per path, join body WCSU only at the body epoch, compose aligned
concurrent demand, maximize sequential epochs and contexts, and close repeated
nesting through the declared finite depth. Opaque adapters have an
evidence-bound admission path: orchestration binds the validated realization
and body demand to the exact installed entry, installed-code context, artifact,
boundary-plan fingerprint, target architecture, exact
context-to-body-domain closure, public preemption ceiling, and provider
receipt. Fixed public stack dispositions must agree in every context;
`ProviderSelected` may close to a different concrete body domain per context
but may not remain unresolved. The external-root resource column, provider
execution, installation ledger, and canonical artifact report consume that
bound epoch composition directly and retain its exact inputs behind the compact
fingerprint; the earlier scalar admitted composer is no longer an admission
path. Emitter-derived body evidence also retains the compact report coordinates
and strong commitments for admitted same-stack leaves; those premises enter the
bound fingerprint and canonical report, so a strong substitution cannot hide
behind equal numeric demand or an equal compact report. The report includes
per-domain demand, body-domain closure, and complete normalized context/epoch
rows without exposing code addresses. Hardware arrival
and its domain closure still need derivation from sealed target facts, generated
adapter coverage still needs widening beyond the first receiver-free x86
semantic ProgramStorage wrapper, and admitted context sets still need a
completeness proof before this is a complete `StackPlan` for every entry
origin. That first generated wrapper is independently replayed from its
canonical template and resolved private-continuation call. Its exact installed
artifact/entry, boundary ABI, Terminal body, and context closure bind three
ordered epochs; the live 72-byte outgoing frame contributes to `Enter`, remains
live while body WCSU joins `Body`, and remains present until `Exit` releases it.
The evidence retains exact emitted bytes and call coordinates under a generated
origin. A sealed installed-entry interval comparison additionally proves that
those resolved wrapper bytes equal the corresponding bytes in the exact frozen
installed image without projecting the image or an executable address. This
closes installed-stub-byte derivation for that wrapper only. It establishes
stack geometry, not firmware invocation or a physical stack mutation.

A sealed provider-execution binding joins the normalized selected provider
plan, exact entry/boundary/reach, and all three resource realizations into
admission; it cannot be replayed after realization drift, and its identity is
reportable. Exact validated compiler-selected plans survive checked lowering
in one canonical fact set. External-root candidates bind the retained plan
identity before validation; normalized root identity covers it, and execution
inherits it rather than accepting a second plan input. The ledger's
deterministic fingerprint and the `artifacts` `external_roots.json`
projection report these facts and the complete boundary plan without leaking
numeric entry addresses or private ranking/codegen proofs.

An installation-bound requirement may contribute one bounded abstract service
row spelled `reaches <= Bound`. Before provider selection, the root manifest
records the requirement-path row identity, normalized bound, and every internal
dependency on it. Ordinary callable package and component contracts cannot
carry the unresolved row. Provider selection supplies the exact operation row;
installation verifies it is a subset of the bound, substitutes it through the
complete root closure, and rejects final admission if any row remains
unresolved. Entry/completion or other multi-operation coherence is checked from
the exact provider-execution binding and protocol lineage, never inferred from
equal rows.

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
logical-fuel provision/realized maximum logical work/IR proof evidence, and
`StatePlan` ceiling/realized footprint/codegen evidence. Reports retain ceilings,
realized facts, and validation receipts; private rankings and codegen proofs stay
behind the evidence firewall. Maximum logical work proves only a finite admitted
operation path, not target WCET. The current schedule-keyed fixed-fuel
provider-summary composer and logical-fuel provision now use the dependency-light
`semantic-vocabulary` schedule identity directly. Local-evidence rows distinguish
recomputable terminal-Psi entry/segment certificates from admitted opaque-
provider unit claims, and the external-root report retains that distinction.
Whole-entry certificate rows now bind exact relocation-free frozen executable
bytes and selected entry offsets, and root installation rechecks the exact
installed-code context. Segment rows independently replay their exact installed
occurrence, artifact, and entry stub after carriage without becoming
whole-entry authority. They remain the implementation precursor to broader
terminal-Psi maximum-logical-work and safe-point checking in
[`canonical_ir_fuel_and_resource_provisioning.md`](canonical_ir_fuel_and_resource_provisioning.md),
not a general symbolic cost model. Migration of the remaining
provider-authored hard-root rows remains.

Maximum-logical-work summaries and installed-code correspondence are evidence,
not calling-plan inputs. They may support build-time bounds, reports, or WCET
analysis, but they reserve no register, add no hidden ABI position, select no
sponsor, and authorize no transfer or resume path. Calling plans describe the
program's real machine-state and boundary behavior only.

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

`calling-conventions` owns normalized `CallPlan`, `StatePlan`, and
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
incompatible plan fails closed. When an adapter intentionally discards a native
status or byte-count result, that result remains part of plan validation and
footprint accounting but does not manufacture an Omega result storage operand.
The exact compiler-internal root structural passthrough also uses the ordinary
native parameter/result plan for its eight-byte fragments. Its live claim and
optional claim-free affine cleanup are retained in typed artifacts. One exact
empty-record local establishment is semantic/fuel metadata, not another native
parameter; neither it nor cleanup adds a physical ABI word or cleanup
instruction. Wider structural returns remain rejected until their complete
vertical slice exists.

Internal Unit scalar calls may author a parameter order different from the
caller's inbound order. If a destination would overwrite a register that is
still an argument source, the backend derives deterministic compiler-private
snapshot slots beyond the plan-selected outgoing area, accounts them in the
transient call frame, and materializes every argument from the preserved
source. Independent object and installation replay derive the same snapshots
from the authoritative caller sources and callee plan; the snapshots do not
become public ABI positions or semantic parameters.
One exact preceding ordinary fixed-integer scalar call may also supply a later
Unit-call argument from its durable result home. That source is neither an
inbound ABI position nor a constant: checked planning binds the call
coordinate, target state, strong contract commitment, reach, arguments, and
dense result local, while assignment owns the physical home. Emission and
independent object/installation replay rejoin the exact home and
materialization before using it as an argument source.
Within the producing Unit body, that same durable home may also feed the exact
immediately following whole-root fixed-integer primitive store. This is not a
second call plan: assignment rejoins the store source to the earlier scalar
call's result home, and later replay validates the home load and width-specific
store directly.

Registered nominal callback admission likewise joins its checked call-site and
selected-machine identity to the one validated target `BoundaryEntryPlan`.
That target-owned row survives both checked-only compilation and the native
`BackendPlan`; callback thunk lowering must consume it directly and may not
re-evaluate a calling convention. Native planning resolves the selected entry
to an exact control-flow key and assigns one deterministic compiler-private
thunk symbol; a missing selected entry rejects before instruction selection.
Multi-entry/re-entrant instruction emission, the private registration
relocation, and registration lifetime accounting remain the next vertical
slice. The outbound registrar plan now owns the settled binder-slot-to-
`NativePlace` row and private layout-demand closure; neither may be inferred
from the callback's inbound entry plan. The address-free backend demand now
replays that separation against the exact emitted thunk/root schedule, but does
not yet choose a relocation kind, section, offset, encoded bytes, or runtime
lease.

Checked Psi carries a domain-separated SHA-256 commitment to the complete
canonical inbound boundary plan beside its compact report fingerprint. Native
planning recomputes that commitment from the exact validated plan and also
replays its crate-sealed exact-plan copy. Contract-envelope and refinement FNV
values are likewise report coordinates beside their existing normalized
machine-contract commitments. Holding any compact coordinate equal cannot
substitute a different contract endpoint or callback plan.

The checked callback's per-entry resource envelope and receipt also retain the
selected machine-contract commitment. Their axis, envelope, and roster FNV
values are compilation-local reports only; target and backend consumers rejoin
the strong contract endpoint before attaching stack, fuel, or machine-state
evidence.

The first direct-source closure slice is live. A bodyless boundary requirement
may parse and lower an interleaved `native callback procedure from Binder`
entry without adding an Omega runtime parameter or source-call argument.
Target closure reconstructs the exact binder requirement, derives the nominal
parameter identity from the owning requirement and declared parameter name,
adds the target function-pointer shape at the authored native position,
publishes `NativePlace::Parameter`, and validates the resulting telescope
against the physical plan. The application-v3 commitment covers that exact
requirement, ordered nominal telescope, origins and shapes, placements,
callback demands, and reusable physical plan; retained callback placement
replay carries both the strong commitment and its compact report coordinate.
The ordinal-derived v1 identity remains a separate domain and is never
translated heuristically.

An actual source registrar invocation now binds its selected callback through
checked Unit planning to one canonical Terminal boundary-call occurrence.
The checked native callback telescope is admitted only when its binder,
nominal use site, registrar requirement, static ordinal, and satisfaction row
all agree. Checked-to-Terminal lowering temporarily retains the exact authored
site, source coordinate and target beside the emitted `OperationId`; the Omega
product consumes those source handles at the boundary and stores only one
placement-index-to-`BoundaryCall` row in the target-owned native-realization
proposal. That occurrence now also owns the exact target-closed direct native-
parameter application from `NativeParameterId` to authored ordinal, function-
pointer shape, and `ValuePlacement`; retained-product replay compares it
structurally with the placement's registrar plan instead of inferring it from
the ID or ordinal. Missing, duplicate, unreachable, wrong-target, non-boundary,
native-application-drifted, and artifact-drifted rows reject independently.
Native production now carries the first direct callback through the complete
source-evaluated normalized-import route. Target lowering carries exactly one
direct native-only callback application,
keyed by stable Terminal `OperationId`, through target lowering and physical
assignment while retaining the selected thunk identity separately from source
scalar values. The checked callback body is now retained as an isolated
canonical Terminal artifact and independently compiled into a disjoint private
machine-code function. Object and final-image replay bind that function to the
exact placement-derived symbol and text span without merging its artifact-local
`MachineId` into the semantic namespace. Machine emission selects the exact
register-or-stack address load; object construction emits architecture-specific
relocations to that private symbol, and final-image replay decodes the patched
address and requires the private function's final text address. The callback-
closed registrar plan is also rejoined to its exact source-evaluated locator
and explicit receiving-policy row. Canonical installation format 51 preserves
the exact private identity, source-Psi identity, artifact-local machine, fixed-
integer ABI, and final text interval and rejoins the decoded row to the same
image. The removed custom/unknown host-operation prototype is not reusable
evidence of completion. Runtime registration, installed lifetime custody,
entry publication, and executable publication remain engineering work under
the settled v3 application model, not open language-design questions. The
authored-hidden-parameter and stale-v1/v2 negative matrix is complete at source
admission and independent replay.

Compiler-body memory operations likewise retain their exact plan-selected place
and relocation recipes through emission and replay validation. Current
coverage includes scalar/aggregate parameters and results, AAPCS64 HFAs,
indirect large aggregates, runtime-indexed places, string and bounded-buffer
operations, compact bit fields, and the built-in OS/runtime catalogs. Dedicated
no-plan paths exist only as differential oracles.

Remaining work is to derive inbound and outbound machinery from the same plan,
add state-ceiling-aware selection/allocation, and validate composed footprints
at the final artifact. Logical-work reports and installed-code correspondence
remain non-authorizing evidence outside calling-plan and native-runtime
semantics.

## Still open

- register/machine-state vocabulary extensions beyond the implemented x86-64
  and AArch64 foundation;
- object-certificate composition and final-image validation format;
- admitted indirect-call footprint contracts;
- unwind/non-local-exit representation; and
- general quantitative resource/WCET algebra beyond the timer's structural
  maximum-logical-work profile.

These are plan/checker/backend questions. They do not justify reviving
`boundary(<Plan>)`, adding an interrupt machine species, or exposing code
addresses as integers.

The compile report also rejoins a retained program-storage entry binding's
exact boundary-contract fingerprint directly to the native executable publication
receipt. Check-only compilation may retain the selected binding while no
publication exists, and object-container output may not retain the binding at
all. This is an independent custody check: matching wrapper-arrival evidence
cannot conceal a redirected selected binding.

Each retained executable receipt's installation seal is now replayable from
its publication identity, output path, and container byte identity, retaining
the fixed flat-destination tag under the existing v1 digest domain. Report
validation checks that seal without depending on a second receipt. A substituted
path, container, or opaque installation fingerprint therefore rejects. Whole
application-package validation follows the replacement contract above, not the
retired optional-copy relation.

The written-output handoff is also checked before its path is used by
auxiliary reporting or consumed into the compile report. Native output requires
that handoff path to equal the executable receipt's exact installed path;
object-container output carries no executable receipt, and check-only
cannot appear as a written output. The handoff fields are private after this
check, preventing path/receipt drift between installation and report custody.
For the replacement macOS package producer, both handoff and report must retain
the checked package root and inner executable relationship. An executable-copy
seal alone cannot establish whole-package publication.

Native execution consumers no longer reconstruct the executable name from a
build-directory convention. The report exposes its installed executable path only
after replaying both the complete publication graph and optional
program-storage bridge custody; check-only, object-container, or internally
drifted reports return no executable path. The `omega run` probe consumes only
that checked receipt path.

The shared report-and-capability native execution helper now takes the checked
compile report rather than a build directory. Its current ten executions all
resolve the command directly from retained publication custody, so those tests
cannot pass by guessing a conventional executable leaf after losing or
redirecting the receipt.

The one-pass exact-native coverage index recognizes this form only when the
same local is bound by an exact rooted compile helper and passed to the checked
runner with one literal status. That admits seven newly unique stronger owners;
the repeated linear-transfer fixture has two owners and remains deliberately
unelided under the existing ambiguity fence.

The five authored-root native executions in the value/type-check cohort now
use the same checked-report runner boundary. They retain their exact literal
exit-status assertions while removing every conventional `out/<executable>`
reconstruction; the strict source index preserves the same unique-owner count.

Authored-root value-call, dispatch, loop, cast, slice-length, and sleep consumers
use that checked-report launch boundary. Preserve their exact source/status
oracles and the independent flat-receipt tampering checks when deleting mixed
bundle-copy tests. Production callers of a validator do not establish test
coverage by themselves. For bundled output, the same executable-path accessor
must return the verified inner executable, not a guessed flat sibling.
