/*
  Shared instruction-construction, block-edge, operand-probe, and work-budget
  primitives (`build_instruction`, `insert_block_with_terminator`,
  `operand_uses_register`) used by selected-instruction optimizations and
  register-pressure recovery rewrites.
*/

pub mod block_edges {

    pub use selected_instructions::terminator_successors;

    use selected_instructions::{
        LocalStorageSlotId, SelectedBlock, SelectedCasePayloadTransport, SelectedFunction,
        SelectedInstruction, SelectedInstructionId, SelectedStructuralTransport, SelectedSuccessor,
        SelectedValueTransport, VirtualRegisterId, terminator_instruction,
    };

    pub fn member_position(
        function: &SelectedFunction,
        member: SelectedInstructionId,
    ) -> Option<(usize, usize)> {
        function
            .blocks
            .iter()
            .enumerate()
            .find_map(|(block_index, block)| {
                block
                    .instruction_index(member)
                    .map(|member_index| (block_index, member_index))
            })
    }

    pub fn function_at<E>(
        plan: &selected_instructions::SelectedInstructionPlan,
        function_index: usize,
        miss: E,
    ) -> Result<&SelectedFunction, E> {
        plan.functions.get(function_index).ok_or(miss)
    }

    pub fn block_instructions(block: &SelectedBlock) -> impl Iterator<Item = &SelectedInstruction> {
        block
            .instructions
            .iter()
            .chain(std::iter::once(terminator_instruction(&block.terminator)))
    }

    pub fn block_instruction_ids(block: &SelectedBlock) -> impl Iterator<Item = u32> + '_ {
        block_instructions(block).map(|instruction| instruction.id.0)
    }
    pub fn edge_surface(successor: &SelectedSuccessor) -> usize {
        successor.bindings.len()
            + successor.structural_bindings.len()
            + successor
                .structural_case
                .as_ref()
                .map_or(0, |case| case.payloads.len())
    }

    pub fn local_slot_mentions(slot: LocalStorageSlotId, register: VirtualRegisterId) -> bool {
        matches!(slot, LocalStorageSlotId::Spill { register: subject } if subject == register)
    }

    pub fn successor_mentions(successor: &SelectedSuccessor, register: VirtualRegisterId) -> bool {
        let binding_mentions = successor
            .bindings
            .iter()
            .any(|binding| match binding.transport {
                SelectedValueTransport::Registers {
                    argument,
                    parameter,
                } => argument == register || parameter == register,
                SelectedValueTransport::Unused => false,
            });
        let structural_mentions = successor
            .structural_bindings
            .iter()
            .any(|binding| match binding.transport {
                SelectedStructuralTransport::WholeValue {
                    argument,
                    destination,
                    ..
                } => argument == register || local_slot_mentions(destination, register),
                SelectedStructuralTransport::ScalarReference {
                    base, destination, ..
                }
                | SelectedStructuralTransport::Address {
                    base, destination, ..
                } => {
                    local_slot_mentions(destination, register)
                        || match base {
                            selected_instructions::SelectedAddressBase::Register(argument) => {
                                argument == register
                            }
                            selected_instructions::SelectedAddressBase::Local(slot) => {
                                local_slot_mentions(slot, register)
                            }
                        }
                }
                SelectedStructuralTransport::Unused => false,
            });
        let case_mentions = successor.structural_case.as_ref().is_some_and(|case| {
            case.source
                .local_slot()
                .is_some_and(|slot| local_slot_mentions(slot, register))
                || case.payloads.iter().any(|payload| match payload.transport {
                    SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                        parameter == register
                    }
                    SelectedCasePayloadTransport::Registers {
                        argument,
                        parameter,
                    } => argument == register || parameter == register,
                    SelectedCasePayloadTransport::Unused => false,
                })
        });
        binding_mentions || structural_mentions || case_mentions
    }
}
pub mod operand_probes {

    use register_model::RegisterOperandAccess;
    use selected_instructions::{
        MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration,
        MachineMemoryEffect, MachineSemanticKind, MachineTrapBehavior,
        PreparedMachineEffectCatalog, SelectedInstruction, VirtualRegisterId,
    };

    pub fn effect_declaration(
        catalog: &PreparedMachineEffectCatalog,
        semantic: MachineSemanticKind,
        constraint: register_model::RegisterConstraintKey,
    ) -> Option<&MachineEffectDeclaration> {
        let mut matches = catalog.catalog().declarations.iter().filter(|declaration| {
            declaration.semantic == semantic && declaration.constraint == constraint
        });
        let declaration = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(declaration)
    }

    pub fn last_definition_before(
        instructions: &[SelectedInstruction],
        position: usize,
        register: VirtualRegisterId,
    ) -> Option<usize> {
        instructions[..position].iter().rposition(|instruction| {
            instruction.operands.iter().any(|operand| {
                operand.access != RegisterOperandAccess::Use && operand.virtual_register == register
            })
        })
    }

    pub fn isolated_declaration(declaration: &MachineEffectDeclaration) -> bool {
        declaration.memory == MachineMemoryEffect::NoneV1
            && declaration.trap == MachineTrapBehavior::NeverV1
            && declaration.barrier == MachineBarrier::None
            && declaration.call == MachineCallEffect::NoneV1
            && declaration.cleanup == MachineCleanupEffect::NoneV1
    }
}
pub mod selected_operands {
    use selected_instructions::{SelectedInstruction, SelectedOperand, VirtualRegisterId};

    pub fn selected_operand(
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

    pub fn copy_shaped(
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
}

use crate::rewriting::block_edges::function_at;
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_model::{RegisterClassId, RegisterInstructionConstraint, RegisterOperandAccess};
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlan, SelectedOperand, VirtualRegisterId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAllocationRuleTargetApplicability {
    TargetIndependent,
}

pub fn rewrite_scan(plan: &SelectedInstructionPlan, function: &SelectedFunction) -> Option<usize> {
    function_scan(function)?.checked_add(plan.functions.len())
}

pub fn required_rewrite_budget(
    plan: &SelectedInstructionPlan,
    function: &SelectedFunction,
    budget: OptimizationWorkBudget,
) -> Option<OptimizationWorkBudget> {
    let operands = function.blocks.total_operand_count()?;
    let extent = rewrite_scan(plan, function)?
        .checked_add(operands)?
        .checked_add(function.memory_accesses.len())?
        .checked_add(function_transports(function)?)?;
    Some(budget.for_extent(u64::try_from(extent).ok()?))
}

pub fn function_transports(function: &SelectedFunction) -> Option<usize> {
    function.blocks.total_transport_count()
}

pub fn function_scan(function: &SelectedFunction) -> Option<usize> {
    function.blocks.iter().try_fold(0usize, |total, block| {
        total.checked_add(block.instructions.len())?.checked_add(1)
    })
}

pub fn charge_steps<E>(
    steps: usize,
    budget: OptimizationWorkBudget,
    overflow: impl FnOnce() -> E,
    exceeded: impl FnOnce() -> E,
) -> Result<(), E> {
    if u64::try_from(steps).map_err(|_| overflow())? > budget.validation_steps() {
        return Err(exceeded());
    }
    Ok(())
}

pub fn integer_copy_row<E>(
    environment: &ValidatedTargetRegisterEnvironment,
    value_class: RegisterClassId,
    output_class: RegisterClassId,
    mismatch: impl Fn() -> E,
) -> Result<&RegisterInstructionConstraint, E> {
    let Some(row) = environment.constraint(environment.selected_keys().copy_i64) else {
        return Err(mismatch());
    };
    if !declares_pair(row, RegisterOperandAccess::Use, RegisterOperandAccess::Def)
        || row.operands[0].class != value_class
        || row.operands[1].class != output_class
        || !row.implicit_uses.is_empty()
        || !row.implicit_defs.is_empty()
        || !row.clobbers.is_empty()
        || row.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return Err(mismatch());
    }
    Ok(row)
}

pub fn declares_pair(
    row: &RegisterInstructionConstraint,
    first_access: RegisterOperandAccess,
    second_access: RegisterOperandAccess,
) -> bool {
    let [first, second] = row.operands.as_slice() else {
        return false;
    };
    first.operand == 0
        && first.access == first_access
        && second.operand == 1
        && second.access == second_access
}

pub fn carrier_pair(
    instruction: &SelectedInstruction,
    second_access: RegisterOperandAccess,
) -> Option<(&SelectedOperand, &SelectedOperand)> {
    let [first, second] = instruction.operands.as_slice() else {
        return None;
    };
    if !instruction.implicit_uses.is_empty()
        || !instruction.implicit_defs.is_empty()
        || !instruction.clobbers.is_empty()
        || first.operand != 0
        || first.access != RegisterOperandAccess::Use
        || second.operand != 1
        || second.access != second_access
        || instruction.operands.iter().any(|operand| {
            operand.fixed_view.is_some() || operand.tied_to.is_some() || operand.early_clobber
        })
    {
        return None;
    }
    Some((first, second))
}

pub struct ConePropagation {
    pub function_scan: usize,
    pub edge_count: usize,
    pub elements: usize,
    pub per_unit_base: usize,
    pub walk_setup_base: usize,
}

pub fn cone_propagation<E>(
    function: &SelectedFunction,
    successors: &[Vec<usize>],
    overflow: impl Fn() -> E,
) -> Result<ConePropagation, E> {
    let function_scan = function_scan(function).ok_or_else(&overflow)?;
    let edge_count = successors
        .iter()
        .try_fold(0usize, |total, targets| total.checked_add(targets.len()))
        .ok_or_else(&overflow)?;
    let block_count = function.blocks.len();
    let elements = function_scan.checked_add(1).ok_or_else(&overflow)?;
    let pops = block_count
        .checked_mul(elements.checked_add(1).ok_or_else(&overflow)?)
        .ok_or_else(&overflow)?;
    let widest_out = successors
        .iter()
        .map(|targets| targets.len())
        .max()
        .unwrap_or(0);
    let per_unit_base = function_scan
        .checked_add(block_count)
        .and_then(|total| total.checked_add(pops))
        .and_then(|total| total.checked_add(pops.checked_mul(widest_out)?.checked_mul(elements)?))
        .ok_or_else(&overflow)?;
    let walk_setup_base = edge_count
        .checked_mul(block_count.checked_add(1).ok_or_else(&overflow)?)
        .and_then(|total| total.checked_add(block_count))
        .and_then(|total| total.checked_add(edge_count))
        .ok_or_else(overflow)?;
    Ok(ConePropagation {
        function_scan,
        edge_count,
        elements,
        per_unit_base,
        walk_setup_base,
    })
}

pub fn plan_function<'source, E>(
    source: &'source impl crate::SelectedProgramView,
    function_index: usize,
    environment: &ValidatedTargetRegisterEnvironment,
    mismatch: impl Fn() -> E + Copy,
) -> Result<(&'source SelectedInstructionPlan, &'source SelectedFunction), E> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(mismatch());
    }
    Ok((plan, function_at(plan, function_index, mismatch())?))
}

#[macro_export]
macro_rules! receipt {
    ($receipt:ident, $source:expr, $proposed:expr $(, $field:ident : $value:expr)* $(,)?) => {
        $receipt {
            source_selected: $source.selected_identity(),
            transformed_selected: selected_instructions::selected_instruction_plan_identity(&$proposed),
            optimization_unit: $source.optimization_unit_identity(),
            work_schedule: $source.work_schedule_identity(),
            $($field: $value,)*
        }
    };
}

#[macro_export]
macro_rules! sealed {
    ($validated:ident, $receipt:ident, $source:expr, $proposed:expr $(, $field:ident : $value:expr)* $(,)?) => {
        $validated {
            receipt: $crate::receipt!($receipt, $source, $proposed $(, $field : $value)*),
            transformed: std::sync::Arc::new($proposed)}
    };
}

pub fn instruction(
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
            .map(|(operand, register)| selected_operands::selected_operand(operand, *register))
            .collect(),
        implicit_uses: row.implicit_uses.clone(),
        implicit_defs: row.implicit_defs.clone(),
        clobbers: row.clobbers.clone(),
        provenance: Default::default(),
    }
}

pub fn register_occurs_once(
    function: &selected_instructions::SelectedFunction,
    register: selected_instructions::VirtualRegisterId,
) -> bool {
    use crate::rewriting::block_edges::terminator_successors;
    use selected_instructions::{SelectedStructuralTransport, terminator_instruction};

    let mut occurrences = 0_usize;
    for block in &function.blocks {
        for instruction in block
            .instructions
            .iter()
            .chain(std::iter::once(terminator_instruction(&block.terminator)))
        {
            occurrences += instruction
                .operands
                .iter()
                .filter(|operand| operand.virtual_register == register)
                .count();
        }
        for successor in terminator_successors(&block.terminator) {
            occurrences += successor
                .structural_bindings
                .iter()
                .filter(|binding| {
                    matches!(
                        binding.transport,
                        SelectedStructuralTransport::WholeValue { argument, .. }
                        | SelectedStructuralTransport::ScalarReference { base: selected_instructions::SelectedAddressBase::Register(argument), .. }
                        | SelectedStructuralTransport::Address { base: selected_instructions::SelectedAddressBase::Register(argument), .. }
                            if argument == register
                    )
                })
                .count();
            if let Some(case) = &successor.structural_case {
                occurrences +=
                    case.payloads
                        .iter()
                        .filter(|payload| {
                            match &payload.transport {
                        selected_instructions::SelectedCasePayloadTransport::Unused => false,
                        selected_instructions::SelectedCasePayloadTransport::Unmaterialized {
                            parameter} => *parameter == register,
                        selected_instructions::SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter} => *argument == register || *parameter == register}
                        })
                        .count();
            }
            occurrences += successor
                .bindings
                .iter()
                .filter(|binding| match &binding.transport {
                    selected_instructions::SelectedValueTransport::Unused => false,
                    selected_instructions::SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } => *argument == register || *parameter == register,
                })
                .count();
        }
    }
    occurrences == 1
}

pub fn rewrite_bytes(
    tag: &'static [u8],
    source_selected: impl AsRef<[u8]>,
    transformed_selected: impl AsRef<[u8]>,
    optimization_unit: impl AsRef<[u8]>,
    tail: impl FnOnce(&mut Vec<u8>),
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(tag);
    bytes.extend_from_slice(source_selected.as_ref());
    bytes.extend_from_slice(transformed_selected.as_ref());
    bytes.extend_from_slice(optimization_unit.as_ref());
    tail(&mut bytes);
    bytes
}

pub fn shifted_boundary_settlements<E>(
    function: &selected_instructions::SelectedFunction,
    block: selected_instructions::SelectedBlockId,
    removed: usize,
    source_mismatch: impl Fn() -> E,
    overflow: impl Fn() -> E,
) -> Result<Vec<selected_instructions::SelectedBoundarySettlement>, E> {
    let body = function
        .block(block)
        .ok_or_else(&source_mismatch)?
        .instructions
        .len();
    let mut shifted = function.boundary_settlements.clone();
    for settlement in &mut shifted {
        if settlement.block != block {
            continue;
        }
        let position = settlement.instruction_index as usize;
        if position > body {
            return Err(source_mismatch());
        }
        if position > removed {
            settlement.instruction_index = u32::try_from(position - 1).map_err(|_| overflow())?;
        }
    }
    Ok(shifted)
}

pub fn constant_fold_steps<E>(
    plan: &selected_instructions::SelectedInstructionPlan,
    function: &selected_instructions::SelectedFunction,
    successors: &[Vec<usize>],
    implicit_uses: usize,
    flag_universe: usize,
    surface: usize,
    defs: usize,
    overflow: impl Fn() -> E + Copy,
) -> Result<usize, E> {
    let cone = crate::rewriting::cone_propagation(function, successors, overflow)?;
    let per_unit = Some(cone.per_unit_base)
        .and_then(|total| total.checked_add(flag_universe.checked_add(surface)?.checked_add(defs)?))
        .ok_or_else(overflow)?;
    let walk_setup = Some(cone.walk_setup_base)
        .and_then(|total| total.checked_add(flag_universe))
        .and_then(|total| total.checked_add(surface))
        .ok_or_else(overflow)?;
    let function_scan = cone.function_scan;
    let reach_scan = implicit_uses
        .checked_mul(per_unit)
        .and_then(|total| total.checked_add(walk_setup))
        .ok_or_else(overflow)?;
    crate::rewriting::rewrite_scan(plan, function)
        .and_then(|total| {
            total
                .checked_add(function_scan.checked_mul(2)?)?
                .checked_add(reach_scan)
        })
        .ok_or_else(overflow)
}
