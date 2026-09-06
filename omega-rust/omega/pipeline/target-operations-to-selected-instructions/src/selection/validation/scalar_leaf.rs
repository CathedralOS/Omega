//! Independent leaf admission compares the proposed value, ABI, and instructions.

use super::blocks::instruction_projection;
use super::integrity::{validate_block_constraints, validate_def_use};
use crate::selection::constraints::{fixed_input_constraint, row};
use crate::selection::shared::*;
use legalized_operations::LegalizedScalarLeafFunction;

mod sequence;

pub(super) fn validate(
    function: usize,
    source: &LegalizedScalarLeafFunction,
    selected: &SelectedFunction,
    native_target: target::NativeTarget,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::FunctionProjectionMismatch { function };
    let [block] = selected.blocks.as_slice() else {
        return Err(invalid());
    };
    let Some(value) = selected.virtual_registers.first() else {
        return Err(invalid());
    };
    if selected.machine != source.machine
        || selected.attachment != source.attachment
        || selected.provenance != source.provenance
        || selected.entry_block != SelectedBlockId(0)
        || block.id != SelectedBlockId(0)
        || block.source_block != source.entry_block
        || value.id != VirtualRegisterId(0)
        || value.scalar_type != ScalarType::Integer(source.abi.result.scalar_type)
    {
        return Err(invalid());
    }
    let environment = register_environment::validate_target_register_environment(
        native_target,
        physical.model().clone(),
        catalog.catalog().clone(),
    )
    .map_err(|_| invalid())?;
    let [
        ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: 8,
        },
    ] = source.abi.result.placement.locations.as_slice()
    else {
        return Err(invalid());
    };
    let return_row = row(catalog, constraints.keys.return_i64)?;
    let [return_operand] = return_row.operands.as_slice() else {
        return Err(invalid());
    };
    if return_operand.fixed_view.is_none()
        || return_operand.fixed_view != environment.fixed_register_view(*register)
    {
        return Err(invalid());
    }
    if matches!(source.leaf.value, SourceLeafValue::ExactIntegerSequence(_)) {
        sequence::validate(
            function,
            source,
            selected,
            constraints,
            physical,
            catalog,
            &environment,
        )?;
        validate_block_constraints(function, block, selected, catalog)?;
        return validate_def_use(function, selected, catalog);
    }
    let source_value = source.leaf.source_value;
    let result_register = match &source.leaf.value {
        SourceLeafValue::Immediate {
            value: immediate,
            constant_operation,
            definition_site,
            constant_fuel,
        } => {
            let [materialize] = block.instructions.as_slice() else {
                return Err(invalid());
            };
            let materialize_row = row(catalog, constraints.keys.materialize_i64)?;
            let [operand] = materialize_row.operands.as_slice() else {
                return Err(invalid());
            };
            if selected.virtual_registers.len() != 1
                || value.origin
                    != (VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(0),
                        source_value,
                    })
                || value.definition_site != *definition_site
                || value.entry_fixed_view.is_some()
                || value.class != operand.class
            {
                return Err(invalid());
            }
            instruction_projection::validate(
                function,
                materialize,
                SelectedInstructionId(0),
                SelectedInstructionKind::MaterializeI64 { value: *immediate },
                constraints.keys.materialize_i64,
                &[VirtualRegisterId(0)],
                &SelectedInstructionProvenance {
                    operations: vec![*constant_operation],
                    values: vec![source_value],
                    fuel: constant_fuel.clone(),
                    ..Default::default()
                },
                catalog,
            )?;
            VirtualRegisterId(0)
        }
        SourceLeafValue::EntryParameter {
            parameter_index,
            register,
            definition_site,
        } => {
            let input = fixed_input_constraint(
                source.machine,
                source_value,
                *parameter_index,
                *register,
                &constraints.fixed_inputs,
            )
            .ok_or_else(invalid)?;
            let view = physical
                .model()
                .views
                .iter()
                .find(|view| view.id == input.fixed_view)
                .ok_or_else(invalid)?;
            if Some(input.fixed_view) != environment.fixed_register_view(*register)
                || value.origin
                    != (VirtualRegisterOrigin::EntryParameter {
                        source_value,
                        parameter_index: *parameter_index,
                    })
                || value.definition_site != *definition_site
                || value.entry_fixed_view != Some(input.fixed_view)
                || value.class != view.class
            {
                return Err(invalid());
            }
            let [_, result] = selected.virtual_registers.as_slice() else {
                return Err(invalid());
            };
            let [copy] = block.instructions.as_slice() else {
                return Err(invalid());
            };
            let copy_row = row(catalog, constraints.keys.copy_i64)?;
            let [_, output] = copy_row.operands.as_slice() else {
                return Err(invalid());
            };
            if result.id != VirtualRegisterId(1)
                || result.scalar_type != value.scalar_type
                || result.class != output.class
                || result.origin
                    != (VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(0),
                        source_value,
                    })
                || result.definition_site != *definition_site
                || result.entry_fixed_view.is_some()
            {
                return Err(invalid());
            }
            instruction_projection::validate(
                function,
                copy,
                SelectedInstructionId(0),
                SelectedInstructionKind::CopyI64,
                constraints.keys.copy_i64,
                &[VirtualRegisterId(0), VirtualRegisterId(1)],
                &SelectedInstructionProvenance {
                    values: vec![source_value],
                    ..Default::default()
                },
                catalog,
            )?;
            VirtualRegisterId(1)
        }
        _ => return Err(invalid()),
    };
    let SelectedTerminator::Return {
        instruction,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(invalid());
    };
    if *psi_return_edge != source.leaf.return_edge {
        return Err(invalid());
    }
    instruction_projection::validate(
        function,
        instruction,
        SelectedInstructionId(1),
        SelectedInstructionKind::ReturnI64,
        constraints.keys.return_i64,
        &[result_register],
        &SelectedInstructionProvenance {
            values: vec![source_value],
            edges: vec![source.leaf.return_edge],
            fuel: source.leaf.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )?;
    validate_block_constraints(function, block, selected, catalog)?;
    validate_def_use(function, selected, catalog)
}
