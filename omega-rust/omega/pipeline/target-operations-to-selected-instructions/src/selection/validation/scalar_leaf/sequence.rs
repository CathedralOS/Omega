//! Independent ABI-input, sequence, and returned-value projection checks.

use super::super::blocks::instruction_projection;
use crate::selection::constraints::{fixed_input_constraint, row};
use crate::selection::shared::*;
use legalized_operations::{LegalizedIntegerStep, LegalizedScalarLeafFunction};

pub(super) fn validate(
    function: usize,
    source: &LegalizedScalarLeafFunction,
    selected: &SelectedFunction,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::FunctionProjectionMismatch { function };
    let SourceLeafValue::ExactIntegerSequence(sequence) = &source.leaf.value else {
        return Err(invalid());
    };
    let block = &selected.blocks[0];
    let mut inputs = Vec::new();
    for (index, parameter) in source.abi.parameters.iter().enumerate() {
        let used = parameter.value == source.leaf.source_value || sequence.steps.iter().any(|step| {
            matches!(step, LegalizedIntegerStep::ExactBinary(binary) if binary.left == parameter.value || binary.right == parameter.value)
        });
        if !used {
            continue;
        }
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 8,
            },
        ] = parameter.placement.locations.as_slice()
        else {
            return Err(invalid());
        };
        let fixed = fixed_input_constraint(
            source.machine,
            parameter.value,
            index,
            *register,
            &constraints.fixed_inputs,
        )
        .ok_or_else(invalid)?;
        let view = physical
            .model()
            .views
            .iter()
            .find(|view| view.id == fixed.fixed_view)
            .ok_or_else(invalid)?;
        let id = VirtualRegisterId(u32::try_from(inputs.len()).map_err(|_| invalid())?);
        let proposed = selected
            .virtual_registers
            .get(inputs.len())
            .ok_or_else(invalid)?;
        if Some(fixed.fixed_view) != environment.fixed_register_view(*register)
            || proposed.id != id
            || proposed.scalar_type != ScalarType::Integer(parameter.scalar_type)
            || proposed.class != view.class
            || proposed.entry_fixed_view != Some(fixed.fixed_view)
            || proposed.definition_site
                != ValueDefinitionSite::FunctionParameter(
                    u32::try_from(index).map_err(|_| invalid())?,
                )
            || proposed.origin
                != (VirtualRegisterOrigin::EntryParameter {
                    source_value: parameter.value,
                    parameter_index: index,
                })
        {
            return Err(invalid());
        }
        inputs.push((parameter.value, id));
    }
    let copied = inputs
        .iter()
        .any(|(value, _)| *value == source.leaf.source_value);
    let definitions = inputs.len() + sequence.steps.len();
    if selected.virtual_registers.len() != definitions + usize::from(copied)
        || block.instructions.len() != sequence.steps.len() + usize::from(copied)
    {
        return Err(invalid());
    }
    let mut result = super::super::integer_sequence::validate(
        function,
        sequence,
        source.leaf.source_value,
        &inputs,
        0,
        inputs.len(),
        &selected.virtual_registers[..definitions],
        &block.instructions[..sequence.steps.len()],
        constraints.keys,
        catalog,
    )?;
    if copied {
        let input = &selected.virtual_registers[result.0 as usize];
        let output = &selected.virtual_registers[definitions];
        let copy_row = row(catalog, constraints.keys.copy_i64)?;
        let [_, output_constraint] = copy_row.operands.as_slice() else {
            return Err(invalid());
        };
        let id = SelectedInstructionId(sequence.steps.len() as u32);
        let destination = VirtualRegisterId(definitions as u32);
        if output.id != destination
            || output.scalar_type != input.scalar_type
            || output.class != output_constraint.class
            || output.entry_fixed_view.is_some()
            || output.definition_site != input.definition_site
            || output.origin
                != (VirtualRegisterOrigin::InstructionResult {
                    instruction: id,
                    source_value: source.leaf.source_value,
                })
        {
            return Err(invalid());
        }
        instruction_projection::validate(
            function,
            &block.instructions[sequence.steps.len()],
            id,
            SelectedInstructionKind::CopyI64,
            constraints.keys.copy_i64,
            &[result, destination],
            &SelectedInstructionProvenance {
                values: vec![source.leaf.source_value],
                ..Default::default()
            },
            catalog,
        )?;
        result = destination;
    }
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
        SelectedInstructionId(block.instructions.len() as u32),
        SelectedInstructionKind::ReturnI64,
        constraints.keys.return_i64,
        &[result],
        &SelectedInstructionProvenance {
            values: vec![source.leaf.source_value],
            edges: vec![source.leaf.return_edge],
            fuel: source.leaf.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )
}
