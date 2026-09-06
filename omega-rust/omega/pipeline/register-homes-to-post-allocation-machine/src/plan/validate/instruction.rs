use std::collections::BTreeSet;

use register_model::{RegisterOperandAccess, ValidatedPhysicalRegisterModel};
use selected_instructions::{MachineAlternativeApplicability, SelectedBlock, SelectedInstruction};
use selected_instructions_to_register_homes::FunctionRegisterHomes;

use crate::PostAllocationMachineError;
use physical_instructions::{PhysicalOperandFootprint, PostAllocationMachineInstruction};
use selected_instructions::InstructionMachineEffects;

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
        selected_instructions::SelectedTerminator::ConditionalBranch { instruction, .. }
        | selected_instructions::SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            ..
        }
        | selected_instructions::SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            ..
        }
        | selected_instructions::SelectedTerminator::Jump { instruction, .. }
        | selected_instructions::SelectedTerminator::Return { instruction, .. } => instruction,
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
