//! Projected-access folding through declarative pairs.
//!
//! The instruction-pair descriptors under `rewrites/selected_lowering`
//! declare a materialized literal folded into a consumer operand — a
//! `Use`-position constant substitution — and the flag-reading families
//! under `peepholes` declare consumers whose whole input arrives through
//! implicit physical units. Between those vocabularies sits the pair an
//! address projection forms with the access that consumes it: the
//! `AddressOffset` producer publishes an ordinary `Def` register — no
//! literal to substitute — and the consumer absorbs the projection's
//! displacement into the `byte_offset` its own kind already carries, so the
//! rewritten form is the consumer's *same* constraint row reading the
//! producer's base register. The
//! [`ProjectedAccessRule`](pair::ProjectedAccessRule) table under
//! `projected_access/pair` declares that relationship: each rule pairs the
//! `AddressOffset` producer with one dereferencing consumer kind —
//! `Load8`, `Load16`, `Load32`, `Load64`, or `Store` — and the axes the
//! relationship must satisfy: `ProjectedAccessOperandShape` for the
//! pointer-operand grammar, `ProjectedAccessDisplacement` for the combined
//! immediate's scaled bound, `ProjectedAccessUnitFlow` for the
//! register-only unit surface, and `ProjectedAccessEffects` for the
//! catalog-declared machine-effect relationship.
//!
//! The declared family is the projected-access fold — the same
//! transformation `rewrites::address_fold` performs non-declaratively,
//! restated with the catalog binding the bespoke rewrite never consulted.
//! An access whose operand-0 pointer is the block's last definition of an
//! `AddressOffset` result is admitted when the producer is a clean `[use
//! base, def pointer]` projection, no instruction in the open interval
//! redefines the base, the register classes agree through the bound
//! constraint rows, and the summed displacement stays inside the
//! access-scaled unsigned immediate every target's forms share (`a
//! multiple of the access width, quotient at most 4095`). The pointer
//! already reads `base + producer_offset`, so the folded access observes
//! the identical bytes at `base + combined`.
//!
//! Under `ProjectedAccessEffects::AccessFaultRetained` the rewrite carries
//! the trap-*preserving* relationship the `FaultDischargedBy*` vocabulary
//! cannot name: the folded form performs the consumer's dereference
//! unchanged, so the bound catalog must declare the access row — the
//! consumer's and the rewritten form's alike — with its own memory effect
//! over operand 0 (`ReadPointerV1` at the access's byte width for a load,
//! `WritePointerV1` for the store) and the `MayArchitecturalFaultV1`
//! surface the dereference retains, fall-through and stack-unchanged on
//! every alternative, while the retained `AddressOffset` producer is fully
//! effect-isolated. The `Store` consumer makes this the first declared
//! pair whose consumer writes memory; a target whose projection or access
//! rows encode anything else cannot admit the pair.
//!
//! The rewritten record keeps the consumer's identity, provenance,
//! constraint row, operand-1 tail — the load's `Def` result or the store's
//! `Use` value — and implicit surfaces verbatim; only the kind's
//! displacement and operand zero's register change. The producer stays:
//! its projected register remains published for every other reader the
//! function still holds, and producer elimination stays with the
//! producer-elimination rules.
//!
//! The descriptor declares admissibility; it never certifies it. The
//! producer consults its own rule and axes in `admission`; the independent
//! replay in `replay` re-derives the consumer grammar, the last-definition
//! and interval scans, the combined-displacement bound, and the catalog
//! surfaces from the instruction records alone — never consulting the pair
//! table — and restores the complete source by content.

pub(super) fn retained_access(
    declaration: &MachineEffectDeclaration,
    consumer: MachineSemanticKind,
    width: u32,
) -> bool {
    let (memory, encoded_memory, reads, writes) = match consumer {
        MachineSemanticKind::Store => (
            MachineMemoryEffect::WritePointerV1,
            MachineEncodedMemoryEffect::WritePointerV1 { pointer_operand: 0 },
            vec![0, 1],
            Vec::new(),
        ),
        _ => (
            MachineMemoryEffect::ReadPointerV1,
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: width as u16,
            },
            vec![0],
            vec![1],
        ),
    };
    declaration.memory == memory
        && declaration.trap == MachineTrapBehavior::MayArchitecturalFaultV1
        && declaration.barrier == MachineBarrier::None
        && declaration.call == MachineCallEffect::NoneV1
        && declaration.cleanup == MachineCleanupEffect::NoneV1
        && declaration
            .alternatives
            .iter()
            .all(|alternative| access_alternative(alternative, encoded_memory, &reads, &writes))
}

/// The encoded surface a retained-access alternative must carry, restated
/// without the table.
fn access_alternative(
    alternative: &MachineAlternative,
    encoded_memory: MachineEncodedMemoryEffect,
    reads: &[u16],
    writes: &[u16],
) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == encoded_memory
        && encoded.external_operand_reads == reads
        && encoded.external_operand_writes == writes
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::MayArchitecturalFaultV1
        && encoded.control == MachineEncodedControlEffect::FallThroughV1
        && encoded.implicit_unit_uses.is_empty()
        && encoded.implicit_unit_defs.is_empty()
        && encoded.implicit_unit_clobbers.is_empty()
}

use crate::ValidatedSelectedAnalysis;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineTrapBehavior,
};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, ValidatedMachineEffectCatalog,
};

mod admission;
mod pair;
mod replay;
mod rewrite;

use std::sync::Arc;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub(crate) use replay::validate_projected_access_fold;

/// An accepted projected-access fold with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedProjectedAccess {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ProjectedAccessReceipt,
}

validated_transformed_receipt!(ValidatedProjectedAccess, ProjectedAccessReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectedAccessReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(ProjectedAccessReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectedAccessError {
    SourceMismatch,
    /// Not the emitted two-operand dereference the family declares: a body
    /// instruction of a kind no declared consumer names — indexed, slot,
    /// packed, hosted, or register-only forms carry no referent
    /// displacement to absorb — or a declared kind carrying a malformed
    /// operand shape, a `Store` width no target encodes, an operand unit
    /// binding, or implicit unit traffic the rebuilt record would silently
    /// keep.
    UnsupportedConsumer,
    /// The pointer operand's last definition before the consumer is not a
    /// clean `AddressOffset` — another defining instruction, a decorated or
    /// unit-carrying projection, a `[use, def]` shape mismatch, a producer
    /// whose result register is not the consumer's pointer, or a
    /// projection that rewrote its own base.
    UnsupportedProducer,
    /// A dataflow fact the fold cannot reproduce: the base register
    /// redefined inside the producer–consumer interval, a missing roster
    /// entry, or a base register whose class the rebound operand cannot
    /// carry.
    UnsupportedUse,
    /// The combined `byte_offset` does not satisfy the declared bound: the
    /// sum overflowed, or it is not the access-scaled unsigned immediate —
    /// a multiple of the access width with a quotient of at most 4095 —
    /// every target's form encodes.
    UnsupportedDisplacement,
    /// The consumer's or the producer's constraint row is missing from the
    /// bound environment or does not declare the operand shape, classes,
    /// or register-only unit surface the pair requires.
    ConstraintMismatch,
    /// The bound effect catalog does not describe this plan's target,
    /// constraints, and selected keys, does not declare the consumer or
    /// producer form under the record's constraint key, or the
    /// declarations do not satisfy the rule's retained-access axis — the
    /// dereference's own memory effect and `MayArchitecturalFaultV1`
    /// surface preserved on the rewritten row, the retained producer fully
    /// effect-isolated.
    EffectSurfaceMismatch,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ProjectedAccessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid projected access fold: {self:?}")
    }
}

impl std::error::Error for ProjectedAccessError {}

/// The access both sides locate identically: the source plan targets this
/// environment, the bound catalog attests this plan's surfaces, and the
/// access names a body instruction — a terminator's carried instruction is
/// not an access form the family declares.
pub(super) struct LocatedAccess<'source> {
    pub(super) plan: &'source SelectedInstructionPlan,
    pub(super) function: &'source SelectedFunction,
    pub(super) block_index: usize,
    pub(super) position: usize,
    pub(super) consumer: &'source SelectedInstruction,
}

pub(super) fn locate_access<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    access: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    effect_catalog: &ValidatedMachineEffectCatalog,
) -> Result<LocatedAccess<'source>, ProjectedAccessError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ProjectedAccessError::SourceMismatch);
    }
    // The bound catalog must describe this plan's target and this
    // environment's constraint catalog and selected-key inventory: a
    // foreign catalog's declarations cannot attest this program's encoded
    // surfaces even where constraint keys coincide numerically.
    let catalog = effect_catalog.catalog();
    if catalog.target != plan.target
        || catalog.register_constraints != environment.constraints().identity()
        || catalog.selected_keys != environment.selected_keys()
    {
        return Err(ProjectedAccessError::EffectSurfaceMismatch);
    }
    let function = crate::rewrites::block_edges::function_at(
        plan,
        function_index,
        ProjectedAccessError::SourceMismatch,
    )?;
    let (block_index, position, consumer) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .enumerate()
                .find(|(_, instruction)| instruction.id == access)
                .map(|(position, instruction)| (block_index, position, instruction))
        })
        .ok_or(ProjectedAccessError::UnsupportedConsumer)?;
    Ok(LocatedAccess {
        plan,
        function,
        block_index,
        position,
        consumer,
    })
}
