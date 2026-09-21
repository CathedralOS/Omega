use optimization_core::OptimizationWorkBudget;
use optimization_unit::ValueDefinitionSite;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedOperand,
    SelectedStructuralTransport, SelectedValueTransport, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerValue, ScalarType, ValueId};

use super::RuntimeRematerializationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::runtime_spill::{control, require_dominated_uses};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub use_blocks: Vec<usize>,
    pub victim: &'source VirtualRegister,
    pub source_value: ValueId,
    pub value: IntegerValue,
    pub first_instruction: u32,
    pub first_register: u32,
    pub materialize: &'source RegisterInstructionConstraint,
}

/// Admit a victim only when its one definition is a pure immediate
/// materialization: repeating that instruction at each flexible use is cheaper
/// than private storage and carries no memory dependency. This is the cost
/// decision the recovery coordinator orders before spilling.
pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    register: VirtualRegisterId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, RuntimeRematerializationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(RuntimeRematerializationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(RuntimeRematerializationError::SourceMismatch)?;
    let victim = function
        .virtual_registers
        .iter()
        .find(|value| value.id == register)
        .ok_or(RuntimeRematerializationError::UnsupportedValue)?;
    let VirtualRegisterOrigin::InstructionResult {
        instruction: definition,
        source_value,
    } = victim.origin
    else {
        return Err(RuntimeRematerializationError::UnsupportedValue);
    };
    let block_index = function
        .blocks
        .iter()
        .position(|block| {
            block
                .instructions
                .iter()
                .any(|candidate| candidate.id == definition)
        })
        .ok_or(RuntimeRematerializationError::UnsupportedValue)?;
    // The regenerable family is one nonaddress runtime value produced by a
    // target-materialized immediate. Narrow values keep their exact type on
    // the fresh register; the repeated instruction reproduces all resident
    // bits with no widening or storage interpretation.
    let scalar_payload = match victim.scalar_type {
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => true,
        ScalarType::Integer(integer) => {
            !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64)
        }
    };
    if !scalar_payload
        || victim.entry_fixed_view.is_some()
        || !matches!(
            victim.definition_site,
            Some(ValueDefinitionSite::FunctionParameter(_))
                | Some(ValueDefinitionSite::BlockParameter { .. })
                | Some(ValueDefinitionSite::Node { .. })
        )
    {
        return Err(RuntimeRematerializationError::UnsupportedValue);
    }
    let materialize = environment
        .constraint(environment.selected_keys().materialize_i64)
        .ok_or(RuntimeRematerializationError::ConstraintMismatch)?;
    // The target row must be a single flexible def with no hidden effects:
    // implicit uses would make repetition consume unproven state, implicit
    // defs or clobbers would destroy values the replayed stream does not name.
    if materialize.operands.len() != 1
        || !materialize.implicit_uses.is_empty()
        || !materialize.implicit_defs.is_empty()
        || !materialize.clobbers.is_empty()
    {
        return Err(RuntimeRematerializationError::ConstraintMismatch);
    }
    let row_operand = &materialize.operands[0];
    if row_operand.access != RegisterOperandAccess::Def
        || row_operand.class != victim.class
        || row_operand.fixed_view.is_some()
        || row_operand.tied_to.is_some()
        || row_operand.early_clobber
    {
        return Err(RuntimeRematerializationError::ConstraintMismatch);
    }
    let defined_instruction = function.blocks[block_index]
        .instructions
        .iter()
        .find(|candidate| candidate.id == definition)
        .ok_or(RuntimeRematerializationError::UnsupportedValue)?;
    let SelectedInstructionKind::MaterializeI64 { value } = defined_instruction.kind else {
        return Err(RuntimeRematerializationError::UnsupportedValue);
    };
    // The recorded definition must be the validated target row itself with
    // exactly its one def operand on the victim and no other effects.
    if defined_instruction.constraint != materialize.key
        || defined_instruction.operands.len() != 1
        || defined_instruction.implicit_uses != materialize.implicit_uses
        || defined_instruction.implicit_defs != materialize.implicit_defs
        || defined_instruction.clobbers != materialize.clobbers
    {
        return Err(RuntimeRematerializationError::UnsupportedValue);
    }
    let def_operand = &defined_instruction.operands[0];
    if def_operand.virtual_register != register
        || def_operand.operand != row_operand.operand
        || def_operand.access != RegisterOperandAccess::Def
        || def_operand.class != victim.class
        || def_operand.fixed_view.is_some()
        || def_operand.tied_to.is_some()
        || def_operand.early_clobber
    {
        return Err(RuntimeRematerializationError::UnsupportedValue);
    }
    let mut defined = false;
    let mut uses = 0usize;
    let mut use_blocks = Vec::new();
    for (current_block_index, block) in function.blocks.iter().enumerate() {
        let previous_uses = uses;
        let (terminal, successors) = control(&block.terminator);
        // A terminator operand use executes after every block instruction, so
        // the fresh materialize serves it from the end of the block; the
        // dominance check below already covers that position. A fixed view on
        // the operand stays attached and pins the fresh register to the same
        // physical unit, so ABI-pinned returns and hosted exits stay exact.
        // Tied, early-clobber, and defining references stay rejected.
        for operand in &terminal.operands {
            if operand.virtual_register != register {
                continue;
            }
            match operand.access {
                RegisterOperandAccess::Use
                    if operand.tied_to.is_none()
                        && !operand.early_clobber
                        && operand.class == victim.class =>
                {
                    // An output tied to this use would extend the fresh
                    // register's value identity.
                    if terminal
                        .operands
                        .iter()
                        .any(|other| other.tied_to == Some(operand.operand))
                    {
                        return Err(RuntimeRematerializationError::UnsupportedUse);
                    }
                    uses = uses
                        .checked_add(1)
                        .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
                }
                _ => return Err(RuntimeRematerializationError::UnsupportedUse),
            }
        }
        if successors.into_iter().flatten().any(|successor| {
            successor.bindings.iter().any(|binding| {
                matches!(binding.transport,
                SelectedValueTransport::Registers { argument, parameter }
                    if argument == register || parameter == register)
            }) || successor.structural_bindings.iter().any(|binding| {
                matches!(binding.transport,
                SelectedStructuralTransport::Descriptor { argument, .. } if argument == register)
            }) || successor.structural_case.as_ref().is_some_and(|case| {
                case.payloads.iter().any(|payload| match payload.transport {
                    SelectedCasePayloadTransport::Registers {
                        argument,
                        parameter,
                    } => argument == register || parameter == register,
                    SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                        parameter == register
                    }
                    SelectedCasePayloadTransport::Unused => false,
                })
            })
        }) {
            return Err(RuntimeRematerializationError::UnsupportedUse);
        }
        for instruction in &block.instructions {
            for operand in &instruction.operands {
                if operand.virtual_register != register {
                    continue;
                }
                match operand.access {
                    RegisterOperandAccess::Def
                        if !defined
                            && current_block_index == block_index
                            && instruction.id == definition =>
                    {
                        defined = true;
                    }
                    RegisterOperandAccess::Use
                        if (defined || current_block_index != block_index)
                            && instruction.id != definition
                            && operand.fixed_view.is_none()
                            && operand.tied_to.is_none()
                            && !operand.early_clobber
                            && operand.class == victim.class =>
                    {
                        // An output tied to this use would extend the fresh
                        // register's value identity.
                        if instruction
                            .operands
                            .iter()
                            .any(|other| other.tied_to == Some(operand.operand))
                        {
                            return Err(RuntimeRematerializationError::UnsupportedUse);
                        }
                        uses = uses
                            .checked_add(1)
                            .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
                    }
                    _ => return Err(RuntimeRematerializationError::UnsupportedUse),
                }
            }
        }
        if uses != previous_uses {
            use_blocks.push(current_block_index);
        }
    }
    if !defined || uses == 0 {
        return Err(RuntimeRematerializationError::UnsupportedValue);
    }
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| total.checked_add(uses.checked_mul(2)?))
        .and_then(|total| total.checked_add(function.blocks.len().checked_mul(2)?))
        .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| RuntimeRematerializationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(RuntimeRematerializationError::WorkBudgetExceeded);
    }
    require_dominated_uses(function, block_index, &use_blocks).map_err(|error| match error {
        crate::RuntimeSpillError::SourceMismatch => RuntimeRematerializationError::SourceMismatch,
        _ => RuntimeRematerializationError::UnsupportedUse,
    })?;
    let first_instruction = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.0)
                .chain([control(&block.terminator).0.id.0])
        })
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
    let first_register = function
        .virtual_registers
        .iter()
        .map(|value| value.id.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
    Ok(Admission {
        function,
        use_blocks,
        victim,
        source_value,
        value,
        first_instruction,
        first_register,
        materialize,
    })
}

pub(super) fn fresh(next: &mut u32) -> Result<u32, RuntimeRematerializationError> {
    let result = *next;
    *next = next
        .checked_add(1)
        .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
    Ok(result)
}

/// The fresh register and materialization inserted for one admitted use, at an
/// instruction operand or at a terminator operand. The proposal builds the
/// pair from this one constructor; validation rebuilds the identical pair
/// inside its own expected stream rather than calling back here. The
/// consumer operand keeps its own access, class, and any fixed ABI view
/// while only the referenced register changes. The instruction repeats the
/// victim's exact immediate and source-value provenance, so value lineage
/// survives regeneration.
pub(super) struct Materialization {
    pub register: VirtualRegister,
    pub instruction: SelectedInstruction,
}

pub(super) fn materialization(
    admitted: &Admission<'_>,
    next_instruction: &mut u32,
    next_register: &mut u32,
) -> Result<Materialization, RuntimeRematerializationError> {
    let instruction = SelectedInstructionId(fresh(next_instruction)?);
    let register = VirtualRegisterId(fresh(next_register)?);
    Ok(Materialization {
        register: VirtualRegister {
            id: register,
            scalar_type: admitted.victim.scalar_type,
            class: admitted.victim.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value: admitted.source_value,
            },
            definition_site: admitted.victim.definition_site,
            entry_fixed_view: None,
        },
        instruction: SelectedInstruction {
            id: instruction,
            kind: SelectedInstructionKind::MaterializeI64 {
                value: admitted.value,
            },
            constraint: admitted.materialize.key,
            operands: vec![SelectedOperand {
                operand: admitted.materialize.operands[0].operand,
                virtual_register: register,
                access: admitted.materialize.operands[0].access,
                class: admitted.materialize.operands[0].class,
                fixed_view: admitted.materialize.operands[0].fixed_view,
                tied_to: admitted.materialize.operands[0].tied_to,
                early_clobber: admitted.materialize.operands[0].early_clobber,
            }],
            implicit_uses: admitted.materialize.implicit_uses.clone(),
            implicit_defs: admitted.materialize.implicit_defs.clone(),
            clobbers: admitted.materialize.clobbers.clone(),
            provenance: SelectedInstructionProvenance {
                values: vec![admitted.source_value],
                ..Default::default()
            },
        },
    })
}
