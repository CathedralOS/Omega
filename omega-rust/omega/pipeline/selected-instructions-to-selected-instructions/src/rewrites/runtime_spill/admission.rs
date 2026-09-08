use optimization_core::OptimizationWorkBudget;
use optimization_unit::ValueDefinitionSite;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedOperand, SelectedTerminator,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};

use super::RuntimeSpillError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub victim: &'source VirtualRegister,
    pub source_value: ValueId,
    pub address_scalar_type: ScalarType,
    pub definition: SelectedInstructionId,
    pub slot: LocalStorageSlotId,
    pub first_instruction: u32,
    pub first_register: u32,
    pub address: &'source RegisterInstructionConstraint,
    pub load: &'source RegisterInstructionConstraint,
    pub store: &'source RegisterInstructionConstraint,
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, RuntimeSpillError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(RuntimeSpillError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(RuntimeSpillError::SourceMismatch)?;
    let [block] = function.blocks.as_slice() else {
        return Err(RuntimeSpillError::UnsupportedControlFlow);
    };
    let SelectedTerminator::Return {
        instruction: terminal,
        ..
    } = &block.terminator
    else {
        return Err(RuntimeSpillError::UnsupportedControlFlow);
    };
    if block.id != function.entry_block || function.ranked.is_some() {
        return Err(RuntimeSpillError::UnsupportedControlFlow);
    }
    let victim = function
        .virtual_registers
        .iter()
        .find(|value| value.id == register)
        .ok_or(RuntimeSpillError::UnsupportedValue)?;
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 64)
        .map_err(|_| RuntimeSpillError::UnsupportedValue)?;
    let VirtualRegisterOrigin::InstructionResult {
        instruction: definition,
        source_value,
    } = victim.origin
    else {
        return Err(RuntimeSpillError::UnsupportedValue);
    };
    if (victim.scalar_type != ScalarType::Integer(unsigned)
        && !matches!(victim.scalar_type, ScalarType::IeeeFloat(_)))
        || victim.entry_fixed_view.is_some()
        || !matches!(
            victim.definition_site,
            Some(ValueDefinitionSite::FunctionParameter(_))
                | Some(ValueDefinitionSite::Node { .. })
        )
    {
        return Err(RuntimeSpillError::UnsupportedValue);
    }
    let slot = LocalStorageSlotId::Spill { register };
    if function
        .local_storage_slots
        .iter()
        .any(|storage| storage.id == slot)
        || terminal
            .operands
            .iter()
            .any(|operand| operand.virtual_register == register)
    {
        return Err(RuntimeSpillError::UnsupportedUse);
    }
    let mut defined = false;
    let mut uses = 0usize;
    for instruction in &block.instructions {
        for operand in &instruction.operands {
            if operand.virtual_register != register {
                continue;
            }
            match operand.access {
                RegisterOperandAccess::Def if !defined && instruction.id == definition => {
                    if matches!(
                        instruction.kind,
                        SelectedInstructionKind::FrameAddress { .. }
                            | SelectedInstructionKind::AddressOffset { .. }
                            | SelectedInstructionKind::ByteViewAddress
                    ) {
                        return Err(RuntimeSpillError::UnsupportedValue);
                    }
                    defined = true;
                }
                RegisterOperandAccess::Use
                    if defined
                        && instruction.id != definition
                        && operand.fixed_view.is_none()
                        && operand.tied_to.is_none()
                        && !operand.early_clobber
                        && operand.class == victim.class =>
                {
                    // An output tied to this use would extend the reload's value identity.
                    if instruction
                        .operands
                        .iter()
                        .any(|other| other.tied_to == Some(operand.operand))
                    {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    }
                    uses = uses
                        .checked_add(1)
                        .ok_or(RuntimeSpillError::IdentityOverflow)?;
                }
                _ => return Err(RuntimeSpillError::UnsupportedUse),
            }
        }
    }
    if !defined || uses == 0 {
        return Err(RuntimeSpillError::UnsupportedValue);
    }
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())
            })
        })
        .and_then(|total| total.checked_add(uses.checked_mul(4)?))
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| RuntimeSpillError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(RuntimeSpillError::WorkBudgetExceeded);
    }
    let keys = environment.selected_keys();
    let address = environment
        .constraint(
            keys.frame_address
                .ok_or(RuntimeSpillError::ConstraintMismatch)?,
        )
        .ok_or(RuntimeSpillError::ConstraintMismatch)?;
    let load = environment
        .constraint(keys.load64.ok_or(RuntimeSpillError::ConstraintMismatch)?)
        .ok_or(RuntimeSpillError::ConstraintMismatch)?;
    let store = environment
        .constraint(keys.store64.ok_or(RuntimeSpillError::ConstraintMismatch)?)
        .ok_or(RuntimeSpillError::ConstraintMismatch)?;
    // Frame accesses retain the validated target row's stack-pointer reads.
    // Ordinary pointer loads have only their explicit address operand.
    if !load.implicit_uses.is_empty() {
        return Err(RuntimeSpillError::ConstraintMismatch);
    }
    for (row, accesses) in [
        (address, &[RegisterOperandAccess::Def][..]),
        (
            load,
            &[RegisterOperandAccess::Use, RegisterOperandAccess::Def][..],
        ),
        (store, &[RegisterOperandAccess::Use][..]),
    ] {
        if row.operands.len() != accesses.len()
            || !row.implicit_defs.is_empty()
            || !row.clobbers.is_empty()
        {
            return Err(RuntimeSpillError::ConstraintMismatch);
        }
        for (ordinal, (operand, access)) in row.operands.iter().zip(accesses).enumerate() {
            if usize::from(operand.operand) != ordinal
                || operand.access != *access
                || operand.class != victim.class
                || operand.fixed_view.is_some()
                || operand.tied_to.is_some()
                || operand.early_clobber
            {
                return Err(RuntimeSpillError::ConstraintMismatch);
            }
        }
    }
    let first_instruction = block
        .instructions
        .iter()
        .map(|instruction| instruction.id.0)
        .chain([terminal.id.0])
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    let first_register = function
        .virtual_registers
        .iter()
        .map(|value| value.id.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    Ok(Admission {
        function,
        victim,
        source_value,
        address_scalar_type: ScalarType::Integer(unsigned),
        definition,
        slot,
        first_instruction,
        first_register,
        address,
        load,
        store,
    })
}

pub(super) fn fresh(next: &mut u32) -> Result<u32, RuntimeSpillError> {
    let result = *next;
    *next = next
        .checked_add(1)
        .ok_or(RuntimeSpillError::IdentityOverflow)?;
    Ok(result)
}

/// Target rows supply the complete operand/effect interface, not guessed ISA conventions.
pub(super) fn instruction(
    id: SelectedInstructionId,
    kind: SelectedInstructionKind,
    row: &RegisterInstructionConstraint,
    registers: &[VirtualRegisterId],
) -> SelectedInstruction {
    SelectedInstruction {
        id,
        kind,
        constraint: row.key,
        operands: row
            .operands
            .iter()
            .zip(registers)
            .map(|(operand, register)| SelectedOperand {
                operand: operand.operand,
                virtual_register: *register,
                access: operand.access,
                class: operand.class,
                fixed_view: operand.fixed_view,
                tied_to: operand.tied_to,
                early_clobber: operand.early_clobber,
            })
            .collect(),
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: Default::default(),
    }
}

pub(super) fn frame(slot: LocalStorageSlotId) -> FrameStorageSlotId {
    FrameStorageSlotId::Local(slot)
}
