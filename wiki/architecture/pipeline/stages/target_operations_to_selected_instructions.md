# Target Operations To Selected Instructions

[Pipeline](../pipeline.md) | Optimizer design: [Optimizer Architecture](../../../design_briefs/optimizer_architecture.md)

This stage selects a typed virtual-register instruction CFG. It runs after
independently validated optimized abstract lowering and before liveness or
physical register assignment. The remaining direct-assignment path is an
alternate implementation still scheduled for removal by the ownership cleanup.

## Stage Contract

Input: one complete `ValidatedOptimizedTargetOperations` custody carrier plus
the exact independently validated target register environment.

Output: `StagedOptimizedSelectedInstructions`, which owns that input custody,
the validated selected plan, and identities binding the Terminal Psi program,
optimization unit, fuel schedule, optimized projection, target, exact joined
register environment, and selection.

Primary responsibility: turn admitted target operations into typed virtual
register instructions with exact register constraints, machine-state effects,
Psi provenance, and path-specific logical-fuel placement. It does not compute
liveness itself, choose physical homes, emit machine instructions, or authorize
publication. Downstream liveness and allocation consume independently admitted
current selected data.

## Current Admitted Shape

One ordinary graph represents admitted scalar and Unit functions. It retains
ordered instructions, explicit returns, conditional branches, jumps and block
parameters. Supported work includes constants, Boolean entry conditions and
negation, integer comparisons, proof-bearing exact arithmetic and widening,
and ordered U64 register calls. Calls and arithmetic can occur in the same
function and in acyclic branching graphs. Source block-roster order does not
stand in for control-flow order. Function-shape recipes and a separate
conditional-function carrier no longer select these programs.

Each selected function owns its virtual registers and blocks. Instructions
retain exact operation, obligation, value, edge, definition and fuel provenance.
Successor bindings retain both semantic identities and explicit virtual-register
transfers; duplicate ABI transport copies do not make source identity a register
lookup key. Fixed ABI views are constraints, not assigned homes.
x86-64 obtains RFLAGS/RIP effects and AArch64 obtains NZCV/PC effects from their
independently validated ISA-owned catalog rows. Unsupported inputs reject
rather than falling back after failed selection.

The bounded Microsoft-x64 owned-indirect-pair Unit family uses this same graph.
Its structural signature and ordered call metadata retain parameter places,
ownership, source and destination ABI placements, provider conformance and
claim transfers. Place-backed pointer snapshots become ordinary virtual
registers. `Load64`, `Store64`, `FrameAddress` and `CallUnit` express the argument
copies and call, with exact outgoing-slot and memory-access records. Selection
does not assign stack addresses or emit one atomic structural call template.
Claim-completion operations retain their evidence at selected block/instruction
positions without inventing executable instructions or scalar results.
Downstream frame construction reserves shadow and outgoing-copy storage before
preservation storage; ordinary encoding resolves addresses against that exact
frame. Fragment, text, object and installation publication use the common
route with empty or selected optimizations; frame-free leaves do not acquire
storage merely because their parameters are structural.

One separate atomic plan family admits the exact two-function owned-linear
projected structural call/return closure already authenticated by target
legalization. It creates no scalar virtual register or selected machine
instruction. Instead it retains eight direct 8-byte integer fragment
placements, the exact ordinary-call and return register-constraint rows,
fixed views/classes/access, complete implicit uses/defs/clobbers, and the three
required transfers. X86-64 transfers with the complete `copy_i64` row when ABI
views differ; AArch64 records an explicit same-view/no-copy transfer. Liveness
and pre-allocation machine-effect analysis reject every nonempty instance, so
no later physical authority follows from this selection boundary.

## Semantic Custody

- Compiler-introduced condition tests, ABI copies and structural address work
  do not invent Psi operations or logical-fuel charges. Authored comparisons
  and calls retain their original provenance and charges.
- The branch retains both source edges, polarity, source targets, ordered
  bindings, and separate successor fuel. Only the taken successor settles its
  edge.
- Each materialization retains the exact source constant operation, value,
  definition site, and operation fuel.
- Each return retains its exact Unit or scalar role, return edge, applicable
  fixed result constraint and edge fuel.
- Structural calls preserve the two original referents by copying into their
  distinct ABI temporaries. Source kind and completion custody remain explicit
  evidence, not an alternative executable representation.
- A canonical selected-plan content identity covers every retained field. The
  independent validator reconstructs the projection from the optimized unit,
  abstract plan, target plan, and target register catalog rather than trusting
  the producer.

## Implementation Map

- `representations/selected-instructions` owns current program data, structural
  contracts and derived analysis data, without selection admission authority.
- `pipeline/target-operations-to-selected-instructions` owns
  production, independent validation, source-custody joins, and content
  identity construction.
- `legalization/source` and `legalization/replay` independently join the current
  source, target ABI and optimized unit into the ordinary legalized graph.
  Their structural grammar leaves retain the bounded parameter, provider and
  completion contracts; they do not produce a separate structural program.
- `selection/construction/mod.rs` owns the complete ordinary function roster
  and the separately retained projected structural call/return roster.
- `selection/construction/scalar_graph.rs` builds ordinary register transport
  and control flow. Its `structural.rs` leaf handles place-backed snapshots,
  outgoing copies, calls and zero-instruction settlements.
- `selection/validation/scalar_graph.rs` independently checks the proposed
  instruction stream and all call, slot, access and settlement records. Its
  structural replay leaf does not call construction helpers.
- `selection/construction/projected_structural_call_return/mod.rs` coordinates
  the bounded atomic closure through named projection, constraint, and transfer
  leaves; its independent validator descends through separate source and target
  replay.
- `pipeline/target-operations-to-selected-instructions/src/optimized/mod.rs` owns the opaque
  cross-stage carrier. The preceding `backend/register-environment`
  stage injects exact ISA/ABI constraints and binds the physical
  model, constraint catalog, active reservation profile, and selected keys into
  one environment identity.
- `isa-x86_64` and `isa-aarch64` own the mapping
  from target machine registers to validated physical register views.

## Known Gaps

The ordinary graph does not yet cover loops, general memory and structural
operations, cleanup, suspension or the complete proof-bearing vocabulary.
Narrow/Boolean call transport, read-qualified arguments and outgoing stack arguments remain
outside the admitted register-call contract. Structural Unit functions retain
the exact owned-indirect-pair ABI and bounded per-function call/settlement
grammar, although closed acyclic rosters no longer require one or two functions.
Other structural ABIs and executable-entry provisioning remain unfinished.
The separate `projected_structural_call_returns` carrier still stops before
liveness and machine effects; its selection is not physical publication
authority. Liveness, live ranges, allocation, recovery and frame validation
already serve the admitted ordinary graphs, but do not establish support for
those remaining inputs. The assigned-operation route is transitional and is
not evidence that this stage or the overall cleanup is complete.
