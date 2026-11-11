# Native representation ownership

These are current program representations, not a substitute portable boundary.
[Terminal Psi](../../../../wiki/spec/terminal-psi/product.md) is the portable
boundary. Pipeline crates transform these programs; target backends own ISA,
ABI, object-format, and relocation details.

## Program families and pipeline boundaries

The six top-level folders expose program families. Each contains an independent
representation package; pipeline crates consume its data without owning its
public program types. The table names current program data, not the admission
tokens or replay evidence that a stage retains alongside it.

| Family | Current program root | Producing and consuming operations |
| --- | --- | --- |
| `abstract-operations/` | [`AbstractOperationPlan`](abstract-operations/src/lib.rs) | [Omega 00](../pipeline/00_terminal-psi-to-abstract-operations/src/artifact_admission.rs) admits Terminal Psi and lowers it; [Omega 01](../pipeline/01_abstract-operations-to-abstract-operations/src/lib.rs) optimizes the abstract program without changing its public form; Omega 02 consumes it. |
| `target-operations/` | [`TargetOperationPlan`](target-operations/src/lib.rs) | [Omega 02](../pipeline/02_abstract-operations-to-target-operations/src/lib.rs) lowers abstract operations with target and provider settlements; Omega 03 consumes the resulting target program. |
| `selected-instructions/` | [`SelectedInstructionPlan`](selected-instructions/src/lib.rs) | [Omega 03](../pipeline/03_target-operations-to-selected-instructions/src/optimized.rs) legalizes target operations and selects instructions; [Omega 04](../pipeline/04_selected-instructions-to-selected-instructions/src/selected_optimization.rs) rewrites that same program form; Omega 05 assigns register homes. |
| `register-homes/` | [`AllocatedProgram`](register-homes/src/lib.rs), including [`RegisterHomePlan`](register-homes/src/lib.rs) | [Omega 05](../pipeline/05_selected-instructions-to-register-homes/src/register_allocation.rs) consumes the selected program and publishes its current allocation; Omega 06 consumes the replayed allocation. |
| `post-allocation-machine/` | [`PostAllocationMachinePlan`](post-allocation-machine/src/lib.rs) | [Omega 06](../pipeline/06_register-homes-to-post-allocation-machine/src/lib.rs) derives physical machine instructions from the allocation; Omega 07 encodes them. The Cargo package is still named `physical-instructions`. |
| `machine-code/` | Encoding, resolved-layout, and emitted-code roots listed below | Omega 07–09 and backend emission share this family while retaining distinct program forms. |

Three additional representation packages belong beneath their program families:

- [`abstract-operations/verified-input/`](abstract-operations/verified-input/src/lib.rs)
  owns `VerifiedPsiOptimizationInput`, the admitted handoff from Omega 00 to
  Omega 01: the lowered `AbstractOperationPlan`, its Terminal module, and the
  target, provider, callback, interrupt and evidence selections admitted beside
  it. Omega 00 produces it; Omega 01 and realization read it, so Omega 01 does
  not depend on Omega 00.
- [`abstract-operations/optimization-unit/`](abstract-operations/optimization-unit/src/lib.rs)
  owns `PsiOptimizationUnit`, the reconstructible control-flow, value, ownership,
  and fact representation used for abstract-program rewrites and replay.
  [Omega 01 builds it](../pipeline/01_abstract-operations-to-abstract-operations/src/verified_unit.rs)
  from the verified input, [publishes an `AbstractOperationPlan`](../pipeline/01_abstract-operations-to-abstract-operations/src/publication.rs)
  and retains the unit for replay. It is a derived form, not another numbered
  boundary after abstract operations. Its
  [independent semantic checker](abstract-operations/optimization-unit/semantics/src/lib.rs)
  has its own manifest; neither representation package depends on that checker.
- [`target-operations/legalized-operations/`](target-operations/legalized-operations/src/lib.rs)
  owns `LegalizedOperationPlan`. Omega 03 first
  [legalizes the target program](../pipeline/03_target-operations-to-selected-instructions/src/legalization/mod.rs),
  then [selects instructions from that checked intermediate](../pipeline/03_target-operations-to-selected-instructions/src/selection/mod.rs).
  This is a distinct resolution level inside the stage, with its own package
  and independent replay; it is not an extra numbered pipeline stage.

Nesting preserves every package boundary. It tells the reader which program
family an intermediate belongs to; it does not turn the package into a module
of its parent or its producing transform.

### Encoding, resolved layout, and emitted code

`machine-code/` currently owns several real program forms. Follow their type
roots rather than treating its name as one undifferentiated IR:

| Program form | Root and operation |
| --- | --- |
| Layout-independent selected-form encoding | [`SelectedFormEncoding`](machine-code/src/encoding.rs), produced by [Omega 07](../pipeline/07_post-allocation-machine-to-selected-form-encoding/src/lib.rs) from the post-allocation machine and selected-program roots. |
| Function-relative resolved layout | [`ResolvedMachineLayout`](machine-code/src/layout.rs), produced by [Omega 08](../pipeline/08_selected-form-encoding-to-resolved-layout/src/lib.rs) from encoded forms. [Omega 09](../pipeline/09_resolved-layout-to-resolved-layout/src/lib.rs) optimizes the same layout type. |
| Emitted machine-code program | [`FunctionFragmentEmissionPlan`](machine-code/src/fragments.rs) placed into one [`RelocationFreeTextSectionPlacement`](machine-code/src/text_section.rs), containing target fragments and their executable bytes. [Backend machine emission](../backend/machine-emission/src/lib.rs) owns function realization, fragment emission, frame application, and text placement; object and image owners consume their results. |

[`ResolvedMachineProgram`](machine-code/src/layout_program.rs)
retains shared selected, allocated, physical, encoding, layout, and frame inputs
for fragment emission. It joins original representation artifacts; it does not
embed producer-stage objects or replace the distinct current roots above.

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
These remain directly consumable library packages; moving them out of
`representations/` does not make callers depend on a coordinator or grant their
raw records authority.

## Shared facts and real lowering boundaries

Represent a semantic concept once at each resolution level. Share vocabulary
across stages that do not change its meaning; move genuinely shared concepts to
their lowest appropriate owner, not merely to whichever stage first needed them.
Do not merge distinct lowering levels into a single representation.

An annotation-only stage retains its current input and adds annotations rather
than cloning its program or redeclaring every operation. The
[allocated program](register-homes/src/lib.rs) shares selected
instructions and register homes through separate immutable artifacts; its
[borrowed view](register-homes/src/lib.rs) exposes those same
artifacts. The [physical program](post-allocation-machine/src/lib.rs)
is a genuinely different resolution level with chosen machine alternatives and
physical operands. Neither representation embeds a chain of transformation
objects. Replay evidence is not another executable program.

Unchanged structural declarations live in one shared
[type catalog](abstract-operations/src/structural_type_catalog.rs).
Function records and later native representations retain that catalog instead of
deep-copying its declarations. Editing a revision uses explicit copy-on-write;
replay and encoded identity still compare contents, never allocation identity.

Keep addressing compositional: operations consume places and value sources,
not a Cartesian family of source/destination-specific opcodes.
[Target structural values](target-operations/src/values.rs)
retain place identity, complete projection paths, access, qualifications, shapes,
and placements. [Scalar homes](target-operations/src/storage.rs)
retain their scalar types and source identities. Resolve these semantic facts
from the checked representation and carry them; native consumers must not infer
them from source spelling, pointer shape, or a guessed storage slot.

Independent replay must still reconstruct and check the retained facts. This is
not the redundant semantic discovery that the ownership rule prohibits.
Unsupported lowering fails explicitly; a missing selection must not discard a
write, substitute a slot, or fabricate a result. The
[lowering owner](../pipeline/02_abstract-operations-to-target-operations/README.md)
describes current structural access and argument-replay boundaries.

## Encoding and artifact scope

Encoder output is authoritative for final instruction size. Lay out executable
bytes and relocations from the sequence actually emitted, not a separately
maintained instruction-width mirror. The
[text-section representation](machine-code/src/text_section.rs)
retains actual bytes, function/block/instruction spans, and resolved internal-call
coordinates. The [object handoff](../backend/images/image-emission/src/function_fragments.rs)
projects admitted fragments into the image publisher and independently validates
that projection. Its relocation-free lane does not imply arbitrary external
symbol resolution or independent component publication.

Representation migrations must preserve exact places, widths, call results,
argument transfers, control flow, and byte/relocation correspondence. Use the
affected corpus cases under `mbx xtask corpus --both` (interpreter
against native) and x86-64/AArch64 encoder checks as acceptance evidence. Extending calls or frames requires exact stack ownership
and disjointness evidence, not an assumption that whole-image addresses remain
valid across a component boundary. Existing frame and spill work owns wider
physical support; historical selection failures do not by themselves establish
frame overlap.

Component descriptions and installation evidence record artifact and deployment
requirements separately from language meaning; a runtime package or Cathedral
owns live deployment and reaccess policy.
