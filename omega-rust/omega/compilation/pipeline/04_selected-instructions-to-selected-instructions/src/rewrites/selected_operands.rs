//! Constraint-row operand construction shared by producer rewrites.
use selected_instructions::{SelectedInstruction, SelectedOperand, VirtualRegisterId};

pub(super) fn selected_operand(
    constraint: &register_model::RegisterOperandConstraint,
    register: VirtualRegisterId,
) -> SelectedOperand {
    SelectedOperand {
        operand: constraint.operand,
        virtual_register: register,
        access: constraint.access,
        class: constraint.class,
        fixed_view: constraint.fixed_view,
        tied_to: constraint.tied_to,
        early_clobber: constraint.early_clobber,
    }
}

/// The copy-shaped instruction the producer rewrites emit: the copy row's
/// operand grammar rebound to the fold's value and result registers, with
/// the source instruction's roster surface and provenance carried over.
pub(super) fn copy_shaped(
    id: selected_instructions::SelectedInstructionId,
    kind: selected_instructions::SelectedInstructionKind,
    copy: &register_model::RegisterInstructionConstraint,
    value: selected_instructions::VirtualRegisterId,
    output: selected_instructions::VirtualRegisterId,
    provenance: selected_instructions::SelectedInstructionProvenance,
) -> SelectedInstruction {
    SelectedInstruction {
        id,
        kind,
        constraint: copy.key,
        operands: copy
            .operands
            .iter()
            .zip([value, output])
            .map(|(operand, register)| selected_operand(operand, register))
            .collect(),
        implicit_uses: copy.implicit_uses.clone(),
        implicit_defs: copy.implicit_defs.clone(),
        clobbers: copy.clobbers.clone(),
        provenance,
    }
}
