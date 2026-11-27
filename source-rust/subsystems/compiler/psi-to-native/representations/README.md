# Native pipeline representations

The numbered transformations exchange the program families described here. The native [body driver](../driver/compile_body.rs) runs the stages for each selected body. Stage 06 computes [allocation analyses](../pipeline/06_selected-instructions-to-selected-instructions/analyses) and hands [the selected program and its results](../pipeline/06_selected-instructions-to-selected-instructions/analyses/src/allocation_input.rs) to stage 07.

## Program families and pipeline boundaries

Each program family owns its public types in a representation package.
Pipeline crates consume those types. The table names program data only, not the admission
tokens or replay evidence a stage keeps beside it.

| Family | Current program root | Producing and consuming operations |
| --- | --- | --- |
| `abstract-operations/` | [`AbstractOperationPlan`](abstract-operations/src/lib.rs) | [stage 02](../pipeline/02_target-bound-program-to-abstract-operations/src/selected_input.rs) lowers the target-bound Psi program; [stage 04](../pipeline/04_abstract-operations-to-target-operations/src/abstract_input.rs) consumes it unchanged. |
| `target-operations/` | [`TargetOperationPlan`](target-operations/src/lib.rs) | [stage 04](../pipeline/04_abstract-operations-to-target-operations/src/lib.rs) lowers abstract operations with target and provider settlements; stage 05 consumes the resulting target program. |
| `selected-instructions/` | [`SelectedInstructionPlan`](selected-instructions/src/lib.rs) | [stage 05](../pipeline/05_target-operations-to-selected-instructions/src/lib.rs) legalizes target operations and selects instructions; [stage 06](../pipeline/06_selected-instructions-to-selected-instructions/src/selected_rewrites/selected_optimization.rs) rewrites that same program form and hands stage 07 one [`SelectedAllocationInput`](../pipeline/06_selected-instructions-to-selected-instructions/analyses/src/lib.rs): the current program, the analyses it established, and its `SelectedOptimizationRecord`; stage 07 assigns register homes. stage 06's completed run is kept only as validator replay evidence. |
| `register-homes/` | [`AllocatedProgram`](register-homes/src/lib.rs), including [`RegisterHomePlan`](register-homes/src/lib.rs) | [stage 07](../pipeline/07_selected-instructions-to-register-homes/src/allocation/register_allocation.rs) consumes the selected program stage 06 hands it and publishes its current allocation; stage 08 consumes that current allocation's facts. |
| `post-allocation-machine/` | [`PostAllocationMachineProgram`](post-allocation-machine/src/program.rs), including physical choices in [`PostAllocationMachinePlan`](post-allocation-machine/src/lib.rs) | [stage 08](../pipeline/08_register-homes-to-post-allocation-machine/src/lib.rs) derives physical machine instructions and carries settled preservation requirements; frame planning and stage 11 read that current form. |
| `machine-code/` | Encoding, resolved-layout, and emitted-code roots listed below | stages 11–13 and backend emission share this family while retaining distinct program forms. |

Other packages separate shared records and preparation from program roots:

- [Target-bound program reach](target-bound-program/bound_reach.rs) records which machines, boundaries, callbacks and scalar applications stage 01 selected for an entry. Stage 02 reads it to lower that body. Direct provider matching and object publication need only the [reached IDs](target-bound-program/execution-reach/reach.rs), so their crates can read those without compiling artifact decoding or layout admission.
- [Operation evidence](operation-evidence/src/lib.rs) identifies where a row came
  from and preserves its charged work and claim sources.
  Later forms can name this evidence without opening a function.
- [Program entry contracts](program-entry-contracts/program_entry.rs) carry the selected source signature, physical arrival and storage plan from stage 01 through emission and requested verification.
- [Operation operands](operation-operands/src/lib.rs) hold the call targets,
  value bindings, integer operation policies and structural storage operands first produced during abstract lowering.
  Target and machine representations carry the same operands through later stages.
- [`abstract-operations/verified-optimization-input/`](abstract-operations/verified-optimization-input/src/lib.rs)
  owns `VerifiedPsiOptimizationInput`, what stage 02 passes to stage 04: the
  lowered `AbstractOperationPlan`, its Terminal module, and the target,
  provider, callback, interrupt and evidence selections admitted with it.
  stage 02 produces it; stage 04 and realization read it, so stage 04 does not
  depend on stage 02.
- [`abstract-operations/optimization-unit/`](abstract-operations/optimization-unit/src/lib.rs)
  owns `PsiOptimizationUnit`, a rebuildable form of control flow, values,
  ownership and facts derived from abstract operations.
  [Stage 04 builds it](../pipeline/04_abstract-operations-to-target-operations/src/abstract_input.rs)
  once from the verified input, checking the artifact's evidence seals as it
  attaches accepted obligation facts, and pairs it with the unchanged plan in an
  [`OptimizedAbstractProgram`](abstract-operations/optimization-unit/src/unoptimized.rs).
  It is derived from abstract operations, not another numbered stage after
  them. Stage 04 and image emission query it through
  [optimization-unit-semantics](abstract-operations/optimization-unit-semantics/src/lib.rs)
  (descriptor and reference graphs, callable values, borrowed storage); neither
  representation package depends on that crate.
- [`target-operations/legalized-operations/`](target-operations/legalized-operations/src/lib.rs)
  owns `LegalizedOperationPlan` and its sealed form, `PreparedExecutionPlan`:
  the execution plan stage 04 publishes. stage 04
  [prepares it from the target program](../pipeline/04_abstract-operations-to-target-operations/src/preparation/legalization.rs)
  as its last step, and stage 05
  [selects instructions from it](../pipeline/05_target-operations-to-selected-instructions/src/instruction_selection/selection.rs)
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
- The [structural catalog](structural-storage/src/lib.rs) holds the declarations and
  view plans carried by native forms. Stages that resolve projections, address joins
  and loans use the [storage queries](structural-storage/queries/src/lib.rs). Input
  preparation measures sequence constructors in the selected machine bodies and
  hands the resulting bounds to the catalog.
- [Native publication facts](target-operations/target-vocabulary/src/publication/emission.rs)
  retain each lowered function's parameters, result, source occurrences, value
  producers and memory obligations. Stage 04 records these facts while lowering
  the function. Frame planning and installation admission read the issued records.

Nesting keeps each package separate. It shows an intermediate form's family;
it does not make the package a module of its parent or producing stage.

### Encoding, resolved layout, and emitted code

Follow the root type for each program form.

| Program form | Root and operation |
| --- | --- |
| Planned framed machine | [Stage 09](../pipeline/09_post-allocation-machine-to-framed-machine/entry-planning/src/imports.rs) prepares the [function body](selected-instructions/physical-recipes/src/frame_protocol.rs) with its checked frame layout, private references and selected import recipes. |
| Bound framed machine | [Stage 09's installation binding](../pipeline/09_post-allocation-machine-to-framed-machine/entry-planning/src/installation_binding.rs) checks the installation resources against the measured handlers and issues the [bound function body](bound-framed-machine/bound_framed_machine.rs) and [bound entry recipes](bound-framed-machine/entries.rs). [Stage 11](../pipeline/11_bound-framed-machine-to-selected-form-encoding/src/lib.rs) consumes them. The body keeps its publication opaque during encoding. |
| Layout-independent selected-form encoding | [`EncodedMachineProgram`](machine-code/src/layout.rs), produced by [stage 11](../pipeline/11_bound-framed-machine-to-selected-form-encoding/src/lib.rs), adds actual encoded rows while retaining the current instruction and frame columns. |
| Function-relative resolved layout | [`ResolvedMachineProgram`](machine-code/src/layout.rs), produced by [stage 12](../pipeline/12_selected-form-encoding-to-resolved-layout/src/lib.rs), assigns instruction offsets and settles eligible branches under the selected relaxation policy. Final assembly reads the resolved form and checked publication receipts; original stage artifacts remain only in named independent replay custody. |
| Emitted machine-code program | [`FunctionFragmentEmissionPlan`](machine-code/src/fragments.rs) placed into one [`RelocationFreeTextSectionPlacement`](machine-code/src/layout/text_section.rs), containing target fragments and their executable bytes. [Stage 13](../pipeline/13_resolved-layout-to-placed-functions/src/function_realization.rs) produces the placed functions. [Stage 14](../pipeline/14_placed-functions-to-objects/) constructs their objects, which stage 15 consumes for image emission. The [body driver](../driver/compile_body.rs) carries stage 04’s [emission facts](../pipeline/04_abstract-operations-to-target-operations/src/emission_facts.rs) with the lowered program. Callable reach and the pre-physical root come from the current form's execution contracts. |

[`ResolvedMachineProgram`](machine-code/src/layout.rs)
holds current physical instruction/contract, encoding, layout and frame
columns that fragment emission shares. It contains neither earlier program
containers nor a chain of producer-stage objects. Neither does emission:
stage 04's output and the original allocation, encoding and layout inputs
reach the object only when a validator request retains them for
`TargetOperationCorrespondence` or `FixedFrameCorrespondence`.

## Supporting data belongs with its domain

[Callback function identity](function-identity/src/lib.rs) pairs the published Psi
program with its callback placement. Admission, native program forms and object
verification carry that same identity. The [object stage](../pipeline/14_placed-functions-to-objects/src/names.rs)
gives a private callback its linker symbol when building an object.

[Targets](../targets/) holds calling conventions and physical register models. Build retains the [provider plans](../../build/providers/plans/) and the native stages carry [boundary application evidence](operation-evidence/boundary-applications/boundary_applications.rs). [Installation admission](../verification/installation-admission/) checks handler and environment evidence. [Psi layout](../../psi/layouts/geometry/README.md) owns portable layout records and their interpretation.

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
[type catalog](structural-storage/src/structural_type_catalog.rs).
Function records and later native representations point to that catalog
instead of copying its declarations. Editing a revision uses explicit
copy-on-write; replay and encoded identity compare contents, never
allocation identity.

Keep addressing compositional: operations take places and value sources,
not one opcode for every source/destination pair.
[Target structural values](target-operations/target-vocabulary/src/values.rs)
keep place identity, complete projection paths, access, qualifications,
shapes and placements. [Scalar homes](target-operations/target-vocabulary/src/storage.rs)
keep their scalar types and source identities. Resolve these facts from the
checked representation and carry them; native consumers must not
guess them from source spelling, pointer shape or a storage slot.

Independent replay must still rebuild and check the carried facts; that is
not the repeated semantic discovery the ownership rule forbids. Unsupported
lowering fails with an error. A missing selection must not drop a write,
substitute a slot or invent a result. The
[lowering owner](../pipeline/04_abstract-operations-to-target-operations/src/target_lowering/README.md)
describes current structural access and argument-replay boundaries.

## Encoding and artifact scope

The encoder's output decides final instruction size. Lay out executable
bytes and relocations from what was actually emitted, not a separate
instruction-width table. The
[text-section representation](machine-code/src/layout/text_section.rs)
keeps the actual bytes, function/block/instruction spans and resolved
internal-call targets. The [object handoff](../pipeline/15_objects-to-image/image-emission/src/function_fragments.rs)
passes admitted fragments to the image publisher and checks that result
itself. Its relocation-free support does not imply resolving arbitrary
external symbols or publishing components separately.

A representation change must keep exact places, widths, call results,
argument transfers, control flow and byte/relocation correspondence.
Accept it on the affected crates' tests, the affected programs under
the repository sample gate (native and Psi against expected observations), and x86-64/AArch64 encoder
checks. Extending calls or frames needs exact stack ownership and
non-overlap evidence, not an assumption that whole-image addresses stay valid across a
component boundary. Wider physical support belongs to the existing frame and
spill work; past selection failures alone do not prove frames overlap.

Component descriptions and installation evidence record artifact and
deployment requirements apart from language meaning; a runtime package or
Cathedral owns live deployment and re-access policy.
