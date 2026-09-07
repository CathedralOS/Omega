//! Exact immediate-u64 eligibility, definition, and future-use reconstruction.

use register_model::RegisterOperandAccess;
use selected_instructions::{
    SelectedInstruction, SelectedInstructionKind, SelectedTerminator, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerSign, ScalarType};

use crate::{
    NoAdmittedRecoveryReason, RecoveryClassification, RecoveryClassificationError,
    RecoveryFutureUse, VirtualFixedConstraintSite,
};

pub(super) fn classify(
    function: usize,
    selected: &selected_instructions::SelectedFunction,
    ranges: &crate::FunctionLiveRanges,
    choice: &crate::SpillChoice,
    victim: &selected_instructions::VirtualRegister,
    range: &crate::VirtualLiveRange,
) -> Result<RecoveryClassification, RecoveryClassificationError> {
    if !is_fixed_unsigned_u64(victim.scalar_type) {
        return no_recovery(NoAdmittedRecoveryReason::UnsupportedScalarType);
    }
    let (defining_id, source_value) = match victim.origin {
        VirtualRegisterOrigin::BlockParameter { .. } => {
            return no_recovery(NoAdmittedRecoveryReason::UnsupportedRangeShape);
        }
        VirtualRegisterOrigin::EntryParameter { .. } => {
            return no_recovery(NoAdmittedRecoveryReason::EntryParameter);
        }
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        }
        | VirtualRegisterOrigin::LegalizationTemporary {
            instruction,
            source_value,
            ..
        } => (instruction, source_value),
    };
    if crate::analyses::liveness::edge_values::has_edge_use(selected, victim.id)
        || range.fragments.len() != 1
        || range.fragments[0].block != choice.block
        || !range.edge_connectors.is_empty()
    {
        return no_recovery(NoAdmittedRecoveryReason::UnsupportedRangeShape);
    }
    if range.fixed_constraints.iter().any(|fixed| {
        matches!(
            fixed.site,
            VirtualFixedConstraintSite::Operand { point, .. } if point >= choice.point
        )
    }) {
        return no_recovery(NoAdmittedRecoveryReason::FutureFixedUse);
    }
    let defining = unique_definition(function, selected, victim.id, defining_id)?;
    match defining.kind {
        SelectedInstructionKind::ExactAddI64 { .. }
        | SelectedInstructionKind::ExactAddI64Immediate { .. }
        | SelectedInstructionKind::ExactSubtractI64 { .. } => {
            return no_recovery(NoAdmittedRecoveryReason::ProofBearingDefinition);
        }
        SelectedInstructionKind::MaterializeI64 { .. } => {}
        _ => return no_recovery(NoAdmittedRecoveryReason::NonMaterializeDefinition),
    }
    let SelectedInstructionKind::MaterializeI64 { value } = defining.kind else {
        unreachable!("materialize kind established above")
    };
    if defining.operands.len() != 1
        || defining.operands[0].virtual_register != victim.id
        || defining.operands[0].access != RegisterOperandAccess::Def
        || defining.provenance.values.as_slice() != [source_value]
        || defining.provenance.operations.len() != 1
        || !defining.provenance.edges.is_empty()
        || !defining.provenance.obligations.is_empty()
        || defining.provenance.fuel.is_empty()
        || !defining.provenance.fuel.iter().all(|fuel| {
            fuel.site
                == optimization_unit::PsiProvenance::Operation(defining.provenance.operations[0])
        })
        || !matches!(victim.scalar_type, ScalarType::Integer(integer) if integer.admits(value))
    {
        return Err(RecoveryClassificationError::VictimMismatch {
            function,
            register: victim.id.0,
        });
    }
    let future_uses = future_uses(function, selected, ranges, choice, victim.id, range)?;
    if future_uses.is_empty() {
        return no_recovery(NoAdmittedRecoveryReason::NoFutureUse);
    }
    Ok(
        RecoveryClassification::ImmediateU64RematerializationCandidate {
            defining_instruction: defining.id,
            source_value,
            value,
            provenance: defining.provenance.clone(),
            future_uses,
        },
    )
}

fn unique_definition(
    function: usize,
    selected: &selected_instructions::SelectedFunction,
    victim: VirtualRegisterId,
    expected: selected_instructions::SelectedInstructionId,
) -> Result<&SelectedInstruction, RecoveryClassificationError> {
    let mut definitions = Vec::new();
    for block in &selected.blocks {
        for instruction in block_instructions(block) {
            if instruction.operands.iter().any(|operand| {
                operand.virtual_register == victim
                    && matches!(
                        operand.access,
                        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                    )
            }) {
                definitions.push(instruction);
            }
        }
    }
    if definitions.len() != 1 || definitions[0].id != expected {
        return Err(RecoveryClassificationError::VictimMismatch {
            function,
            register: victim.0,
        });
    }
    Ok(definitions[0])
}

fn future_uses(
    function: usize,
    selected: &selected_instructions::SelectedFunction,
    ranges: &crate::FunctionLiveRanges,
    choice: &crate::SpillChoice,
    victim: VirtualRegisterId,
    range: &crate::VirtualLiveRange,
) -> Result<Vec<RecoveryFutureUse>, RecoveryClassificationError> {
    let mut uses = Vec::new();
    for occurrence in &range.occurrences {
        if occurrence.point < choice.point || occurrence.access != RegisterOperandAccess::Use {
            continue;
        }
        let block = ranges
            .block_domains
            .iter()
            .find(|domain| domain.start <= occurrence.point && occurrence.point < domain.end)
            .ok_or(RecoveryClassificationError::VictimMismatch {
                function,
                register: victim.0,
            })?;
        if block.block != choice.block
            || !selected.blocks.iter().any(|candidate| {
                candidate.id == block.block
                    && block_instructions(candidate)
                        .into_iter()
                        .any(|instruction| {
                            instruction.id == occurrence.instruction
                                && instruction.operands.iter().any(|operand| {
                                    operand.operand == occurrence.operand
                                        && operand.virtual_register == victim
                                        && operand.access == RegisterOperandAccess::Use
                                        && operand.fixed_view.is_none()
                                })
                        })
            })
        {
            return Err(RecoveryClassificationError::VictimMismatch {
                function,
                register: victim.0,
            });
        }
        uses.push(RecoveryFutureUse {
            block: block.block,
            point: occurrence.point,
            instruction: occurrence.instruction,
            operand: occurrence.operand,
        });
    }
    uses.sort_unstable();
    uses.dedup();
    Ok(uses)
}

fn block_instructions(block: &selected_instructions::SelectedBlock) -> Vec<&SelectedInstruction> {
    let terminator = match &block.terminator {
        SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    };
    block
        .instructions
        .iter()
        .chain(std::iter::once(terminator))
        .collect()
}

fn is_fixed_unsigned_u64(scalar: ScalarType) -> bool {
    matches!(
        scalar,
        ScalarType::Integer(integer)
            if !integer.is_address()
                && integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 64
    )
}

fn no_recovery(
    reason: NoAdmittedRecoveryReason,
) -> Result<RecoveryClassification, RecoveryClassificationError> {
    Ok(RecoveryClassification::NoAdmittedRecovery { reason })
}
