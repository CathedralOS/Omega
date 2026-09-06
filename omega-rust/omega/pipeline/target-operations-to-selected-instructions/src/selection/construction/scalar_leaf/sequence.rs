//! A free scalar sequence uses ABI inputs and the ordinary return constraint.

use crate::selection::constraints::{fixed_input_constraint, instruction, row};
use crate::selection::shared::*;
use legalized_operations::{LegalizedIntegerStep, LegalizedScalarLeafFunction};

pub(super) fn build(
    function: usize,
    source: &LegalizedScalarLeafFunction,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    let SourceLeafValue::ExactIntegerSequence(sequence) = &source.leaf.value else {
        return Err(invalid());
    };
    let mut registers = Vec::new();
    let mut inputs = Vec::new();
    for (index, parameter) in source.abi.parameters.iter().enumerate() {
        if parameter.value != source.leaf.source_value && !sequence.steps.iter().any(|step| {
            matches!(step, LegalizedIntegerStep::ExactBinary(binary) if binary.left == parameter.value || binary.right == parameter.value)
        }) { continue; }
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
        if Some(fixed.fixed_view) != environment.fixed_register_view(*register) {
            return Err(invalid());
        }
        let view = physical
            .model()
            .views
            .iter()
            .find(|view| view.id == fixed.fixed_view)
            .ok_or_else(invalid)?;
        let id = VirtualRegisterId(u32::try_from(registers.len()).map_err(|_| invalid())?);
        inputs.push((parameter.value, id));
        registers.push(VirtualRegister {
            id,
            scalar_type: ScalarType::Integer(parameter.scalar_type),
            class: view.class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: parameter.value,
                parameter_index: index,
            },
            definition_site: ValueDefinitionSite::FunctionParameter(
                u32::try_from(index).map_err(|_| invalid())?,
            ),
            entry_fixed_view: Some(fixed.fixed_view),
        });
    }
    let (mut instructions, mut result) = super::super::integer_sequence::build(
        function,
        sequence,
        source.leaf.source_value,
        &inputs,
        0,
        &mut registers,
        &constraints.keys,
        catalog,
    )?;
    if registers[result.0 as usize].entry_fixed_view.is_some() {
        let source_register = registers[result.0 as usize].clone();
        let copy_row = row(catalog, constraints.keys.copy_i64)?;
        let [_, output] = copy_row.operands.as_slice() else {
            return Err(invalid());
        };
        let destination = VirtualRegisterId(u32::try_from(registers.len()).map_err(|_| invalid())?);
        let id = SelectedInstructionId(u32::try_from(instructions.len()).map_err(|_| invalid())?);
        registers.push(VirtualRegister {
            id: destination,
            scalar_type: source_register.scalar_type,
            class: output.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: id,
                source_value: source.leaf.source_value,
            },
            definition_site: source_register.definition_site,
            entry_fixed_view: None,
        });
        instructions.push(instruction(
            id,
            SelectedInstructionKind::CopyI64,
            constraints.keys.copy_i64,
            &[result, destination],
            SelectedInstructionProvenance {
                values: vec![source.leaf.source_value],
                ..Default::default()
            },
            catalog,
        )?);
        result = destination;
    }
    let terminator = SelectedTerminator::Return {
        instruction: instruction(
            SelectedInstructionId(u32::try_from(instructions.len()).map_err(|_| invalid())?),
            SelectedInstructionKind::ReturnI64,
            constraints.keys.return_i64,
            &[result],
            SelectedInstructionProvenance {
                values: vec![source.leaf.source_value],
                edges: vec![source.leaf.return_edge],
                fuel: source.leaf.return_fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?,
        psi_return_edge: source.leaf.return_edge,
    };
    Ok(SelectedFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
        entry_block: SelectedBlockId(0),
        virtual_registers: registers,
        blocks: vec![SelectedBlock {
            id: SelectedBlockId(0),
            source_block: source.entry_block,
            instructions,
            terminator,
        }],
    })
}
