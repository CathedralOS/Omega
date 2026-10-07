//! Dynamic control adds one dispatch register to the ordinary call ABI row.
//!
//! The data placements and result definitions stay exactly those of the
//! requirement. A dedicated volatile register keeps selection disjoint from
//! every parameter/result placement, including hidden result pointers. The
//! target-owned call clobbers already cover the address-load scratch register.

use crate::{
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
    MachineEncodedTrapBehavior, MachineMemoryEffect, MachineSizeKnowledge, MachineTrapBehavior,
    SelectedCallTarget, SelectedFunction, SelectedInstruction,
};
use register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
    ValidatedPhysicalRegisterModel,
};

pub fn call_record<'function>(
    function: &'function SelectedFunction,
    instruction: &SelectedInstruction,
) -> Option<&'function crate::SelectedCallContract> {
    let target = instruction.kind.call_target()?;
    let mut records = function
        .calls
        .iter()
        .filter(|record| record.instruction == instruction.id);
    let record = records.next()?;
    (records.next().is_none() && SelectedCallTarget::from_native(record.call.target) == target)
        .then_some(record)
}

pub fn instantiate_constraint(
    base: &RegisterInstructionConstraint,
    physical: &ValidatedPhysicalRegisterModel,
) -> Option<RegisterInstructionConstraint> {
    let name = match physical.model().architecture {
        target::Architecture::Aarch64 => "x16",
        target::Architecture::X86_64 => "r11",
    };
    let selector = physical.model().view_named(name)?;
    if base
        .operands
        .iter()
        .any(|operand| operand.fixed_view == Some(selector.id))
    {
        return None;
    }
    let mut row = base.clone();
    let position = row
        .operands
        .iter()
        .take_while(|operand| operand.access == RegisterOperandAccess::Use)
        .count();
    row.operands.insert(
        position,
        RegisterOperandConstraint {
            operand: position.try_into().ok()?,
            access: RegisterOperandAccess::Use,
            class: selector.class,
            fixed_view: Some(selector.id),
            tied_to: None,
            early_clobber: false,
        },
    );
    for (position, operand) in row.operands.iter_mut().enumerate() {
        operand.operand = position.try_into().ok()?;
    }
    Some(row)
}

pub fn instantiate_declaration(
    base: &MachineEffectDeclaration,
    constraint: &RegisterInstructionConstraint,
    physical: &ValidatedPhysicalRegisterModel,
) -> Option<MachineEffectDeclaration> {
    let mut declaration = base.clone();
    let inputs = constraint
        .operands
        .iter()
        .take_while(|operand| operand.access == RegisterOperandAccess::Use)
        .count();
    inputs.checked_sub(1)?;
    declaration.memory = MachineMemoryEffect::ReadPointerV1;
    declaration.trap = MachineTrapBehavior::MayArchitecturalFaultV1;
    for alternative in &mut declaration.alternatives {
        alternative.key.variant = 1;
        alternative.size = MachineSizeKnowledge::ExactBytes(match physical.model().architecture {
            target::Architecture::Aarch64 => 36,
            target::Architecture::X86_64 => 24,
        });
        alternative.encoded = encoded_effects(&alternative.encoded, physical.model().architecture)?;
    }
    Some(declaration)
}

pub fn encoded_effects(
    base: &crate::MachineEncodedEffects,
    architecture: target::Architecture,
) -> Option<crate::MachineEncodedEffects> {
    let position = u16::try_from(base.external_operand_reads.len()).ok()?;
    let mut effects = base.clone();
    effects.external_operand_reads.push(position);
    for output in &mut effects.external_operand_writes {
        *output = output.checked_add(1)?;
    }
    effects.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
        pointer_operand: position,
        byte_count: 8,
    };
    effects.control = MachineEncodedControlEffect::IndirectTableCallOrTrapV1;
    effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
    let _ = architecture;
    Some(effects)
}
