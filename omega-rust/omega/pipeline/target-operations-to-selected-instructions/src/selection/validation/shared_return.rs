//! Independently validate common-return selection and its exact transfer graph.

use super::blocks::instruction_projection;
use super::integrity::{validate_block_constraints, validate_def_use};
use crate::selection::constraints::{fixed_input_constraint, row};
use crate::selection::shared::*;
use legalized_operations::LegalizedSharedReturnConditionalFunction;

pub(super) fn validate(
    function: usize,
    source: &LegalizedSharedReturnConditionalFunction,
    selected: &SelectedFunction,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::FunctionProjectionMismatch { function };
    let [entry, true_block, false_block, returned] = selected.blocks.as_slice() else {
        return Err(invalid());
    };
    if selected.machine != source.machine
        || selected.attachment != source.attachment
        || selected.provenance != source.provenance
        || selected.entry_block != SelectedBlockId(0)
        || selected.virtual_registers.len() != 5
        || selected
            .blocks
            .iter()
            .enumerate()
            .any(|(position, block)| block.id != SelectedBlockId(position as u32))
        || entry.source_block != source.entry_block
        || returned.source_block != source.return_block
        || true_block.source_block != source.when_true.block
        || false_block.source_block != source.when_false.block
        || entry.instructions.len() != 1
        || !returned.instructions.is_empty()
    {
        return Err(invalid());
    }
    let (operation, fuel, left, right) = match &source.condition {
        LegalizedCondition::IntegerEqualParametersV1 {
            operation,
            fuel,
            left,
            right,
            ..
        }
        | LegalizedCondition::IntegerLessThanParametersV1 {
            operation,
            fuel,
            left,
            right,
            ..
        }
        | LegalizedCondition::IntegerLessOrEqualParametersV1 {
            operation,
            fuel,
            left,
            right,
            ..
        }
        | LegalizedCondition::I64LessThanParametersV1 {
            operation,
            fuel,
            left,
            right,
            ..
        }
        | LegalizedCondition::I64LessOrEqualParametersV1 {
            operation,
            fuel,
            left,
            right,
            ..
        } => (operation, fuel, left, right),
        _ => return Err(invalid()),
    };
    let (branch, actual_true, actual_false, kind, reversed) =
        match (&source.condition, &entry.terminator) {
            (
                LegalizedCondition::IntegerEqualParametersV1 { .. },
                SelectedTerminator::ConditionalBranch {
                    instruction,
                    when_nonzero,
                    when_zero,
                },
            ) => (
                instruction,
                when_zero,
                when_nonzero,
                SelectedInstructionKind::ConditionalBranchNonZero,
                false,
            ),
            (
                LegalizedCondition::IntegerLessThanParametersV1 { .. },
                SelectedTerminator::ConditionalBranchU64LessThan {
                    instruction,
                    when_less,
                    when_not_less,
                },
            ) => (
                instruction,
                when_less,
                when_not_less,
                SelectedInstructionKind::ConditionalBranchU64LessThan,
                false,
            ),
            (
                LegalizedCondition::IntegerLessOrEqualParametersV1 { .. },
                SelectedTerminator::ConditionalBranchU64LessThan {
                    instruction,
                    when_less,
                    when_not_less,
                },
            ) => (
                instruction,
                when_not_less,
                when_less,
                SelectedInstructionKind::ConditionalBranchU64LessThan,
                true,
            ),
            (
                LegalizedCondition::I64LessThanParametersV1 { .. },
                SelectedTerminator::ConditionalBranchI64LessThan {
                    instruction,
                    when_less,
                    when_not_less,
                },
            ) => (
                instruction,
                when_less,
                when_not_less,
                SelectedInstructionKind::ConditionalBranchI64LessThan,
                false,
            ),
            (
                LegalizedCondition::I64LessOrEqualParametersV1 { .. },
                SelectedTerminator::ConditionalBranchI64LessThan {
                    instruction,
                    when_less,
                    when_not_less,
                },
            ) => (
                instruction,
                when_not_less,
                when_less,
                SelectedInstructionKind::ConditionalBranchI64LessThan,
                true,
            ),
            _ => return Err(invalid()),
        };
    let keys = &constraints.keys;
    instruction_projection::validate(
        function,
        &entry.instructions[0],
        SelectedInstructionId(0),
        SelectedInstructionKind::CompareI64,
        keys.compare_i64,
        &if reversed {
            [VirtualRegisterId(1), VirtualRegisterId(0)]
        } else {
            [VirtualRegisterId(0), VirtualRegisterId(1)]
        },
        &SelectedInstructionProvenance {
            operations: vec![*operation],
            values: vec![
                left.source_value,
                right.source_value,
                source.condition_source,
            ],
            fuel: fuel.clone(),
            ..Default::default()
        },
        catalog,
    )?;
    instruction_projection::validate(
        function,
        branch,
        SelectedInstructionId(1),
        kind,
        keys.conditional_branch,
        &[],
        &SelectedInstructionProvenance {
            values: vec![source.condition_source],
            ..Default::default()
        },
        catalog,
    )?;
    for (position, parameter) in [left, right].into_iter().enumerate() {
        let register = &selected.virtual_registers[position];
        let fixed = fixed_input_constraint(
            source.machine,
            parameter.source_value,
            parameter.parameter_index,
            parameter.register,
            &constraints.fixed_inputs,
        )
        .ok_or_else(invalid)?;
        let view = physical
            .model()
            .views
            .iter()
            .find(|view| view.id == fixed.fixed_view)
            .ok_or_else(invalid)?;
        if register.id != VirtualRegisterId(position as u32)
            || register.origin
                != (VirtualRegisterOrigin::EntryParameter {
                    source_value: parameter.source_value,
                    parameter_index: parameter.parameter_index,
                })
            || register.definition_site != parameter.definition_site
            || register.entry_fixed_view != Some(fixed.fixed_view)
            || register.class != view.class
            || register.scalar_type
                != ScalarType::Integer(source.abi.parameters[parameter.parameter_index].scalar_type)
        {
            return Err(invalid());
        }
    }
    let class = row(catalog, keys.materialize_i64)?.operands[0].class;
    let scalar_type = ScalarType::Integer(source.abi.result.scalar_type);
    for (position, ((arm, block), edge)) in [&source.when_true, &source.when_false]
        .into_iter()
        .zip([true_block, false_block])
        .zip([actual_true, actual_false])
        .enumerate()
    {
        if edge.block != block.id
            || edge.source_target != arm.block
            || edge.psi_edge != arm.branch_edge
            || edge.bindings != arm.branch_bindings
            || edge.fuel != arm.branch_fuel
            || block.instructions.len() != 1
        {
            return Err(invalid());
        }
        // The retained arm parameters have no uses: the only value-producing
        // operation is its constant and the only outgoing value is that result.
        // They retain their semantic binding rows but require no virtual home.
        if arm.parameters.iter().any(|parameter| {
            parameter.value == arm.constant.source_value
                || parameter.value == arm.transfer_binding.argument
        }) || arm.parameters.len() != arm.branch_bindings.len()
            || arm
                .parameters
                .iter()
                .zip(&arm.branch_bindings)
                .any(|(parameter, binding)| {
                    parameter.value != binding.parameter
                        || parameter.scalar_type != binding.scalar_type
                })
        {
            return Err(invalid());
        }
        let virtual_id = VirtualRegisterId(position as u32 + 2);
        let instruction_id = SelectedInstructionId(position as u32 * 2 + 2);
        let register = &selected.virtual_registers[position + 2];
        if register.id != virtual_id
            || register.scalar_type != scalar_type
            || register.class != class
            || register.origin
                != (VirtualRegisterOrigin::InstructionResult {
                    instruction: instruction_id,
                    source_value: arm.constant.source_value,
                })
            || register.definition_site != arm.constant.definition_site
            || register.entry_fixed_view.is_some()
        {
            return Err(invalid());
        }
        instruction_projection::validate(
            function,
            &block.instructions[0],
            instruction_id,
            SelectedInstructionKind::MaterializeI64 {
                value: arm.constant.value,
            },
            keys.materialize_i64,
            &[virtual_id],
            &SelectedInstructionProvenance {
                operations: vec![arm.constant.constant_operation],
                values: vec![arm.constant.source_value],
                fuel: arm.constant.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?;
        let SelectedTerminator::Jump {
            instruction,
            successor,
        } = &block.terminator
        else {
            return Err(invalid());
        };
        instruction_projection::validate(
            function,
            instruction,
            SelectedInstructionId(instruction_id.0 + 1),
            SelectedInstructionKind::Jump,
            keys.jump,
            &[],
            &SelectedInstructionProvenance::default(),
            catalog,
        )?;
        if successor.block != SelectedBlockId(3)
            || successor.source_target != source.return_block
            || successor.psi_edge != arm.transfer_edge
            || successor.bindings != [arm.transfer_binding]
            || successor.fuel != arm.transfer_fuel
            || arm.transfer_binding.argument != arm.constant.source_value
            || arm.transfer_binding.parameter != source.return_parameter.value
        {
            return Err(invalid());
        }
    }
    let register = &selected.virtual_registers[4];
    if register.id != VirtualRegisterId(4)
        || register.scalar_type != scalar_type
        || register.class != class
        || register.origin
            != (VirtualRegisterOrigin::BlockParameter {
                source_value: source.return_parameter.value,
                block: SelectedBlockId(3),
                parameter_index: 0,
            })
        || register.definition_site != source.return_parameter.site
        || register.entry_fixed_view.is_some()
    {
        return Err(invalid());
    }
    let SelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &returned.terminator
    else {
        return Err(invalid());
    };
    if *psi_return_edge != source.return_edge {
        return Err(invalid());
    }
    instruction_projection::validate(
        function,
        instruction,
        SelectedInstructionId(6),
        SelectedInstructionKind::ReturnI64,
        keys.return_i64,
        &[VirtualRegisterId(4)],
        &SelectedInstructionProvenance {
            values: vec![source.return_parameter.value],
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )?;
    for block in &selected.blocks {
        validate_block_constraints(function, block, selected, catalog)?;
    }
    validate_def_use(function, selected, catalog)
}
