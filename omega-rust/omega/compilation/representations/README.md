# Native representation ownership

These crates hold native program forms; [Terminal Psi](../../../../wiki/spec/terminal-psi/product.md)
remains the portable form. Pipeline crates transform these programs; target backends own
ISA, ABI, object-format and relocation details.

## Program families and pipeline boundaries

Each of the six top-level folders is a program family with its own
representation package. Pipeline crates read that data but do not own its
public program types. The table names program data only, not the admission
tokens or replay evidence a stage keeps beside it.

| Family | Current program root | Producing and consuming operations |
| --- | --- | --- |
| `abstract-operations/` | [`AbstractOperationPlan`](abstract-operations/src/lib.rs) | [Omega 00](../pipeline/00_terminal-psi-to-abstract-operations/src/artifact_admission.rs) admits Terminal Psi and lowers it; [Omega 01](../pipeline/01_abstract-operations-to-abstract-operations/src/lib.rs) optimizes the abstract program without changing its public form; Omega 02 consumes it. |
| `target-operations/` | [`TargetOperationPlan`](target-operations/src/lib.rs) | [Omega 02](../pipeline/02_abstract-operations-to-target-operations/src/lib.rs) lowers abstract operations with target and provider settlements; Omega 03 consumes the resulting target program. |
| `selected-instructions/` | [`SelectedInstructionPlan`](selected-instructions/src/lib.rs) | [Omega 03](../pipeline/03_target-operations-to-selected-instructions/src/optimized.rs) legalizes target operations and selects instructions; [Omega 04](../pipeline/04_selected-instructions-to-selected-instructions/src/selected_optimization.rs) rewrites that same program form and hands Omega 05 one [`SelectedAllocationInput`](selected-instructions/analyses/src/lib.rs): the current program, the analyses it established, and its `SelectedOptimizationRecord`; Omega 05 assigns register homes. Omega 04's completed run is kept only as validator replay evidence. |
| `register-homes/` | [`AllocatedProgram`](register-homes/src/lib.rs), including [`RegisterHomePlan`](register-homes/src/lib.rs) | [Omega 05](../pipeline/05_selected-instructions-to-register-homes/src/register_allocation.rs) consumes the selected program Omega 04 hands it and publishes its current allocation; Omega 06 consumes that current allocation's facts. |
| `post-allocation-machine/` | [`PostAllocationMachineProgram`](post-allocation-machine/src/program.rs), including physical choices in [`PostAllocationMachinePlan`](post-allocation-machine/src/lib.rs) | [Omega 06](../pipeline/06_register-homes-to-post-allocation-machine/src/lib.rs) derives physical machine instructions and carries settled preservation requirements; frame planning and Omega 07 read that current form. |
| `machine-code/` | Encoding, resolved-layout, and emitted-code roots listed below | Omega 07–09 and backend emission share this family while retaining distinct program forms. |

Other packages separate shared records and preparation from program roots:

- [`abstract-operations/verified-optimization-input/`](abstract-operations/verified-optimization-input/src/lib.rs)
  owns `VerifiedPsiOptimizationInput`, what Omega 00 passes to Omega 01: the
  lowered `AbstractOperationPlan`, its Terminal module, and the target,
  provider, callback, interrupt and evidence selections admitted with it.
  Omega 00 produces it; Omega 01 and realization read it, so Omega 01 does not
  depend on Omega 00.
- [`abstract-operations/optimization-unit/`](abstract-operations/optimization-unit/src/lib.rs)
  owns `PsiOptimizationUnit`, a rebuildable form of control flow, values,
  ownership and facts used to rewrite and replay abstract programs.
  [Omega 01 builds it](../pipeline/01_abstract-operations-to-abstract-operations/src/verified_unit.rs)
  from the verified input, [publishes an `AbstractOperationPlan`](../pipeline/01_abstract-operations-to-abstract-operations/src/publication.rs)
  and keeps the unit for replay. It is derived from abstract operations, not
  another numbered stage after them. Its
  [independent semantic checker](abstract-operations/optimization-unit-semantics/src/lib.rs)
  has its own manifest; neither representation package depends on it.
- [`target-operations/legalized-operations/`](target-operations/legalized-operations/src/lib.rs)
  owns `LegalizedOperationPlan` and its sealed form, `PreparedExecutionPlan`:
  the execution plan Omega 02 publishes. Omega 02
  [prepares it from the target program](../pipeline/02_abstract-operations-to-target-operations/native-preparation/src/legalization.rs)
  as its last step, and Omega 03
  [selects instructions from it](../pipeline/03_target-operations-to-selected-instructions/src/selection.rs)
  without reading anything behind it. The package also holds the layout
  queries both stages ask of the plan's structural declarations.
- [`target-operations/target-vocabulary/`](target-operations/target-vocabulary/src/lib.rs)
  owns the records a target program is made of (call contracts and argument
  sources, boundary realizations, call-site owners, structural homes, value
  expressions), shared structural and function calling contracts, scalar
  operands and their canonical encoders, apart from either target or prepared
  instruction graphs. Both earlier forms re-export the same records. The plan, selected instructions and
  every later form name them from here, and `check-boundary` refuses any path
  from those forms to `target-operations` itself. Selected and later forms
  also cannot reach `legalized-operations`, even through a helper crate.
- [`native-vocabulary/`](native-vocabulary/src/lib.rs) owns the leaf types
  every native form carries unchanged: provenance and work records,
  structural store destinations, access-reference backings, and the pure
  predicates over structural declarations, integer arithmetic policies, call
  targets and formals, completion claim sources, ownership-frontier facts,
  and the canonical scalar and integer type encoders. It holds no program, so
  a later stage can name these without being able to read an earlier one.
  `check-boundary` proves that `target-operations`, `legalized-operations`,
  `selected-instructions` and its analyses cannot reach `abstract-operations`
  through any production dependency.

Nesting keeps each package separate. It shows an intermediate form's family;
it does not make the package a module of its parent or producing stage.

### Encoding, resolved layout, and emitted code

`machine-code/` holds several distinct program forms. Follow each one's root
type; the folder is not one IR:

| Program form | Root and operation |
| --- | --- |
| Framed machine program | [`FramedMachineProgram`](machine-code/src/layout.rs): current physical instructions and function contracts with concrete frame coordinates and protocol. [Backend machine emission](../backend/machine-emission/src/function_realization.rs) computes it from one current physical program; Omega 07 reads it. |
| Layout-independent selected-form encoding | [`EncodedMachineProgram`](machine-code/src/layout.rs), produced by [Omega 07](../pipeline/07_post-allocation-machine-to-selected-form-encoding/src/lib.rs), adds actual encoded rows while retaining the current instruction and frame columns. |
| Function-relative resolved layout | [`ResolvedMachineProgram`](machine-code/src/layout.rs), produced by [Omega 08](../pipeline/08_selected-form-encoding-to-resolved-layout/src/lib.rs), adds exact layout rows. [Omega 09](../pipeline/09_resolved-layout-to-resolved-layout/src/lib.rs) rewrites that layout in the same current form. Final assembly reads the resolved form and checked publication receipts; original stage artifacts remain only in named independent replay custody. |
| Emitted machine-code program | [`FunctionFragmentEmissionPlan`](machine-code/src/fragments.rs) placed into one [`RelocationFreeTextSectionPlacement`](machine-code/src/layout/text_section.rs), containing target fragments and their executable bytes. [Backend machine emission](../backend/machine-emission/src/lib.rs) owns function realization, fragment emission, frame application, and text placement; object and image owners consume their results. Fragment emission reads the resolved form plus [`EmissionFacts`](../backend/machine-emission/src/emission_facts.rs), which the [native driver](../native-compiler/src/pipeline.rs) copies out of Omega 02's output: the admitted lowering facts and the optimized abstract program with its projection identities. Callable reach and the pre-physical root come from the current form's execution contracts. |

[`ResolvedMachineProgram`](machine-code/src/layout.rs)
holds current physical instruction/contract, encoding, layout and frame
columns that fragment emission shares. It contains neither earlier program
containers nor a chain of producer-stage objects. Neither does emission:
Omega 02's output and the original allocation, encoding and layout inputs
reach the object only when a validator request retains them for
`TargetOperationCorrespondence` or `FixedFrameCorrespondence`.

## Supporting data belongs with its domain

Shared identity and optimization vocabulary lives in
[Omega foundation](../../foundation/README.md). Target descriptions, calling
conventions, and physical register models live under `backend/`; provider-plan
evidence lives in [`backend/plans/effects/`](../backend/plans/effects/src/lib.rs),
and artifact boundary coverage in
[`backend/artifacts/boundary-applications/`](../backend/artifacts/boundary-applications/src/lib.rs).
Installation evidence and task activation plans have separate packages under
`backend/runtime/`. Opaque representation-selection records live beside
[representation planning](../../../psi/compilation/validation/representation-planning/README.md) in its
independent [`selections/` package](../../../psi/compilation/validation/representation-planning/selections/src/lib.rs).
Callers use these library packages directly. Keeping them out of
`representations/` does not route callers through a coordinator, and it gives
their raw records no authority.

## Shared facts and real lowering boundaries

Represent each semantic concept once per level of detail. Stages that keep a
concept's meaning share its vocabulary. Move a truly shared concept
to the lowest owner that fits, not to whichever stage needed it first. Do not
merge different lowering levels into one representation.

A stage that only adds annotations keeps its input and adds them; it does
not clone the program or redeclare every operation. The
[allocated program](register-homes/src/lib.rs) shares selected
instructions and register homes through separate immutable artifacts; its
[borrowed view](register-homes/src/lib.rs) exposes those same
artifacts. The [physical program](post-allocation-machine/src/lib.rs)
is a different level of detail, with chosen machine alternatives and physical
operands. Neither representation embeds a chain of transformation objects.
Replay evidence is not another executable program.

Unchanged structural declarations live in one shared
[type catalog](abstract-operations/src/structural_type_catalog.rs).
Function records and later native representations point to that catalog
instead of copying its declarations. Editing a revision uses explicit
copy-on-write; replay and encoded identity compare contents, never
allocation identity.

Keep addressing compositional: operations take places and value sources,
not one opcode for every source/destination pair.
[Target structural values](target-operations/src/values.rs)
keep place identity, complete projection paths, access, qualifications,
shapes and placements. [Scalar homes](target-operations/src/storage.rs)
keep their scalar types and source identities. Resolve these facts from the
checked representation and carry them; native consumers must not
guess them from source spelling, pointer shape or a storage slot.

Independent replay must still rebuild and check the carried facts; that is
not the repeated semantic discovery the ownership rule forbids. Unsupported
lowering fails with an error. A missing selection must not drop a write,
substitute a slot or invent a result. The
[lowering owner](../pipeline/02_abstract-operations-to-target-operations/README.md)
describes current structural access and argument-replay boundaries.

## Encoding and artifact scope

The encoder's output decides final instruction size. Lay out executable
bytes and relocations from what was actually emitted, not a separate
instruction-width table. The
[text-section representation](machine-code/src/layout/text_section.rs)
keeps the actual bytes, function/block/instruction spans and resolved
internal-call targets. The [object handoff](../backend/images/image-emission/src/function_fragments.rs)
passes admitted fragments to the image publisher and checks that result
itself. Its relocation-free support does not imply resolving arbitrary
external symbols or publishing components separately.

A representation change must keep exact places, widths, call results,
argument transfers, control flow and byte/relocation correspondence.
Accept it on the affected crates' tests, the affected programs under
`omega run --both` (interpreter against native), and x86-64/AArch64 encoder
checks. Extending calls or frames needs exact stack ownership and
non-overlap evidence, not an assumption that whole-image addresses stay valid across a
component boundary. Wider physical support belongs to the existing frame and
spill work; past selection failures alone do not prove frames overlap.

Component descriptions and installation evidence record artifact and
deployment requirements apart from language meaning; a runtime package or
Cathedral owns live deployment and re-access policy.
