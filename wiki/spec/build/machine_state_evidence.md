# Machine-state realization evidence

The evaluated [call and state plans](calling_plans.md) are the published boundary
promise. Actual register and machine-state use is evidence about one provider
artifact, not an additional caller contract.

## Final-artifact obligation

Instruction selection and allocation must honor the state ceiling. Independent
validation checks the complete final artifact after inlining, specialization,
relaxation, thunks/veneers, generated stubs, and admitted indirect leaves:

```text
actual_transitive_footprint ⊆ permitted_transitive_use
actual_clobbers ∩ unsaved_interrupted_state = ∅
```

Changing allocation within those bounds revalidates the implementation without
changing the published promise. Contextual specialization may produce a callee
under a restricted state ceiling; it is not a new generic type application.

## Certificate and replay

One self-describing certificate binds exact final bytes, placements, and the
complete executable-region inventory. Admission replays normalized instruction
and region rows against closed target instruction specifications, proves exact
byte coverage, and then composes the footprint. A second whole-image decoder
does not define a competing admission path.

Checked Omega code supplies derived evidence. Raw/admitted leaves require
explicit accepted claims with provider provenance; the trust report distinguishes
them. Static and dynamically loaded artifacts use the same checking boundary.
Adding an executable-region origin requires its replay before a producer may
claim complete coverage.

Every byte-bearing compiler instruction with final-byte validation also needs
a footprint row. Each nonempty retained instruction chooses exactly one replay
authority: compiler target specification or checked-assembly catalog. Zero-width
scaffolding may choose neither. Unsupported shapes reject rather than disappear
from the union. Catalog replay includes every operand loader and the flag,
stack, and control effects of fixed instruction sequences.

The checker binds catalog-row counts to replayed validation counts, composes
the complete instruction union, compares it with the state-plan-validated union,
and binds the normalized identity into the certificate before serialization.
Fragment order and duplication do not affect normalized set union. Object and
image validation still establish that every realized fragment was supplied.

## Exit realization

Return-control evidence must match `CallPlan::entry_control`; the restored-state
set must match `StatePlan`. An opaque provider supplies the accepted claim under
a root-reported trust receipt or adequate-hardware-isolation receipt. Missing,
unreported, or mismatched evidence rejects before forming provider execution.

These are required guarantees, not a claim that the current native route
implements all of them. See the [image implementation note](../../../omega-rust/omega/backend/images/image/footprint_replay.md).
