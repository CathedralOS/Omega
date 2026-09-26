use std::collections::BTreeSet;

use selected_instructions_to_register_homes::FunctionRegisterHomes;
use target_operations_to_selected_instructions::register_model::{
    RegisterOperandAccess, ValidatedPhysicalRegisterModel,
};
use target_operations_to_selected_instructions::{
    MachineAlternativeApplicability, SelectedBlock, SelectedInstruction,
};

use crate::PostAllocationMachineError;
use crate::physical_instructions::{PhysicalOperandFootprint, PostAllocationMachineInstruction};
use target_operations_to_selected_instructions::InstructionMachineEffects;

pub(super) fn reconstruct_instruction(
    function_index: usize,
    selected: &SelectedInstruction,
    effects: &InstructionMachineEffects,
    homes: &FunctionRegisterHomes,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<PostAllocationMachineInstruction, PostAllocationMachineError> {
    if effects.instruction != selected.id || effects.kind != selected.kind {
        return Err(PostAllocationMachineError::InstructionMismatch {
            function: function_index,
            instruction: selected.id.0,
        });
    }
    let mut operands = Vec::with_capacity(selected.operands.len());
    for operand in &selected.operands {
        let home = homes
            .assignments
            .iter()
            .find(|home| home.virtual_register == operand.virtual_register)
            .ok_or(PostAllocationMachineError::MissingHome {
                function: function_index,
                register: operand.virtual_register.0,
            })?;
        let view = physical
            .model()
            .views
            .iter()
            .find(|view| view.id == home.view)
            .ok_or(PostAllocationMachineError::UnknownView {
                function: function_index,
                register: operand.virtual_register.0,
                view: home.view.0,
            })?;
        if home.class != operand.class
            || view.class != operand.class
            || operand.fixed_view.is_some_and(|fixed| fixed != home.view)
        {
            return Err(PostAllocationMachineError::HomeClassMismatch {
                function: function_index,
                register: operand.virtual_register.0,
            });
        }
        let reads = reads(operand.access);
        let writes = writes(operand.access);
        operands.push(PhysicalOperandFootprint {
            operand: operand.operand,
            virtual_register: operand.virtual_register,
            class: operand.class,
            view: home.view,
            access: operand.access,
            storage_units: view.units.clone(),
            read_units: if reads {
                view.units.clone()
            } else {
                Vec::new()
            },
            write_units: if writes {
                view.write_units.clone()
            } else {
                Vec::new()
            },
            write_semantics: writes.then_some(view.write_semantics),
        });
    }
    let mut chosen = None;
    for alternative in &effects.alternatives {
        if is_applicable(
            selected.id.0,
            &operands,
            alternative.applicability,
            physical,
        )? && chosen.replace(alternative.clone()).is_some()
        {
            return Err(
                PostAllocationMachineError::AmbiguousApplicableAlternatives {
                    instruction: selected.id.0,
                },
            );
        }
    }
    let alternative = chosen.ok_or(PostAllocationMachineError::NoApplicableAlternative {
        instruction: selected.id.0,
    })?;
    let mut unit_uses = BTreeSet::from_iter(effects.unit_uses.iter().copied());
    let mut unit_defs = BTreeSet::from_iter(effects.unit_defs.iter().copied());
    for operand in &operands {
        unit_uses.extend(&operand.read_units);
        unit_defs.extend(&operand.write_units);
    }
    Ok(PostAllocationMachineInstruction {
        instruction: selected.id,
        alternative,
        operands,
        address: match selected.kind {
            target_operations_to_selected_instructions::SelectedInstructionKind::Load64 { byte_offset } => {
                Some(crate::physical_instructions::PhysicalAddressOperation::Load64 {
                    base_operand: 0,
                    byte_offset,
                })
            }
            target_operations_to_selected_instructions::SelectedInstructionKind::LoadPacked { byte_offset, width } => {
                Some(
                    crate::physical_instructions::PhysicalAddressOperation::LoadPacked {
                        base_operand: 0,
                        byte_offset,
                        width,
                    },
                )
            }
            target_operations_to_selected_instructions::SelectedInstructionKind::StorePacked { byte_offset, width } => {
                Some(
                    crate::physical_instructions::PhysicalAddressOperation::StorePacked {
                        base_operand: 0,
                        byte_offset,
                        width,
                    },
                )
            }
            target_operations_to_selected_instructions::SelectedInstructionKind::Load8 { byte_offset } => {
                Some(crate::physical_instructions::PhysicalAddressOperation::Load8 {
                    base_operand: 0,
                    byte_offset,
                })
            }
            target_operations_to_selected_instructions::SelectedInstructionKind::Load16 { byte_offset } => {
                Some(crate::physical_instructions::PhysicalAddressOperation::Load16 {
                    base_operand: 0,
                    byte_offset,
                })
            }
            target_operations_to_selected_instructions::SelectedInstructionKind::Load32 { byte_offset } => {
                Some(crate::physical_instructions::PhysicalAddressOperation::Load32 {
                    base_operand: 0,
                    byte_offset,
                })
            }
            target_operations_to_selected_instructions::SelectedInstructionKind::Load8Indexed => Some(
                crate::physical_instructions::PhysicalAddressOperation::Load8Indexed {
                    base_operand: 0,
                    index_operand: 1,
                },
            ),
            target_operations_to_selected_instructions::SelectedInstructionKind::HostedReadByte { slot } => {
                Some(crate::physical_instructions::PhysicalAddressOperation::HostedReadByte { slot })
            }
            target_operations_to_selected_instructions::SelectedInstructionKind::SaveFloatingControl { slot } => {
                Some(crate::physical_instructions::PhysicalAddressOperation::SaveFloatingControl { slot })
            }
            target_operations_to_selected_instructions::SelectedInstructionKind::RestoreFloatingControl { slot } => {
                Some(
                    crate::physical_instructions::PhysicalAddressOperation::RestoreFloatingControl {
                        slot,
                    },
                )
            }
            target_operations_to_selected_instructions::SelectedInstructionKind::HostedWriteByteI32 { slot } => {
                Some(crate::physical_instructions::PhysicalAddressOperation::HostedWriteByteI32 { slot })
            }
            target_operations_to_selected_instructions::SelectedInstructionKind::Store {
                byte_offset,
                byte_size,
            } => Some(crate::physical_instructions::PhysicalAddressOperation::Store {
                base_operand: 0,
                byte_offset,
                byte_size,
            }),
            target_operations_to_selected_instructions::SelectedInstructionKind::AddressOffset { byte_offset } => Some(
                crate::physical_instructions::PhysicalAddressOperation::AddressOffset {
                    base_operand: 0,
                    byte_offset,
                },
            ),
            target_operations_to_selected_instructions::SelectedInstructionKind::Store64 { slot, byte_offset } => {
                Some(crate::physical_instructions::PhysicalAddressOperation::Store64 { slot, byte_offset })
            }
            target_operations_to_selected_instructions::SelectedInstructionKind::FrameAddress { slot, byte_offset } => {
                Some(
                    crate::physical_instructions::PhysicalAddressOperation::FrameAddress {
                        slot,
                        byte_offset,
                    },
                )
            }
            _ => None,
        },
        implicit_unit_uses: effects.unit_uses.clone(),
        implicit_unit_defs: effects.unit_defs.clone(),
        implicit_unit_clobbers: effects.unit_clobbers.clone(),
        unit_uses: unit_uses.into_iter().collect(),
        unit_defs: unit_defs.into_iter().collect(),
        unit_clobbers: effects.unit_clobbers.clone(),
    })
}

fn is_applicable(
    instruction: u32,
    operands: &[PhysicalOperandFootprint],
    applicability: MachineAlternativeApplicability,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<bool, PostAllocationMachineError> {
    let view = |number| {
        operands
            .iter()
            .find(|operand| operand.operand == number)
            .map(|operand| operand.view)
            .ok_or(PostAllocationMachineError::MissingApplicabilityOperand {
                instruction,
                operand: number,
            })
    };
    let aliases = |left, right| physical.model().aliases(left, right);
    Ok(match applicability {
        MachineAlternativeApplicability::Always => true,
        MachineAlternativeApplicability::ResultAliasesOperand { result, operand } => {
            aliases(view(result)?, view(operand)?)
        }
        MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result,
            aliased_operand,
            distinct_operand,
        } => {
            let result = view(result)?;
            aliases(result, view(aliased_operand)?) && !aliases(result, view(distinct_operand)?)
        }
        MachineAlternativeApplicability::ResultAliasesOperands {
            result,
            left,
            right,
        } => {
            let result = view(result)?;
            aliases(result, view(left)?) && aliases(result, view(right)?)
        }
        MachineAlternativeApplicability::ResultDistinctFromOperands {
            result,
            left,
            right,
        } => {
            let result = view(result)?;
            !aliases(result, view(left)?) && !aliases(result, view(right)?)
        }
        MachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
            left,
            right,
            excluded_view,
        } => !aliases(view(left)?, excluded_view) || !aliases(view(right)?, excluded_view),
    })
}

pub(super) fn selected_instructions(
    block: &SelectedBlock,
) -> impl Iterator<Item = &SelectedInstruction> {
    let terminator = match &block.terminator {
        target_operations_to_selected_instructions::SelectedTerminator::ConditionalBranch { instruction, .. }
        | target_operations_to_selected_instructions::SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            ..
        }
        | target_operations_to_selected_instructions::SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            ..
        }
        | target_operations_to_selected_instructions::SelectedTerminator::Jump { instruction, .. }
        | target_operations_to_selected_instructions::SelectedTerminator::Return { instruction, .. }
        | target_operations_to_selected_instructions::SelectedTerminator::Crash { instruction, .. }
        | target_operations_to_selected_instructions::SelectedTerminator::HostedExitProcess { instruction, .. } => {
            instruction
        }
    };
    block.instructions.iter().chain(std::iter::once(terminator))
}

const fn reads(access: RegisterOperandAccess) -> bool {
    matches!(
        access,
        RegisterOperandAccess::Use | RegisterOperandAccess::UseDef
    )
}

const fn writes(access: RegisterOperandAccess) -> bool {
    matches!(
        access,
        RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
    )
}
