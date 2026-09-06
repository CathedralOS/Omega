//! Straight-line leaf selection uses ordinary virtual values and return rows.

use crate::selection::constraints::{fixed_input_constraint, instruction, row};
use crate::selection::shared::*;
use legalized_operations::LegalizedScalarLeafFunction;

mod sequence;

pub(super) fn build(
    function: usize,
    source: &LegalizedScalarLeafFunction,
    native_target: target::NativeTarget,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    let environment = register_environment::validate_target_register_environment(
        native_target,
        physical.model().clone(),
        catalog.catalog().clone(),
    )
    .map_err(|_| invalid())?;
    let [
        ValueLocation::Register {
            register: result_register,
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
    if return_operand.fixed_view != environment.fixed_register_view(*result_register)
        || return_operand.fixed_view.is_none()
    {
        return Err(invalid());
    }
    if matches!(source.leaf.value, SourceLeafValue::ExactIntegerSequence(_)) {
        return sequence::build(
            function,
            source,
            constraints,
            physical,
            catalog,
            &environment,
        );
    }
    let mut instructions = Vec::new();
    let source_value = source.leaf.source_value;
    let (origin, definition_site, entry_fixed_view, class) = match &source.leaf.value {
        SourceLeafValue::Immediate {
            value,
            constant_operation,
            definition_site,
            constant_fuel,
        } => {
            let materialize_row = row(catalog, constraints.keys.materialize_i64)?;
            let [operand] = materialize_row.operands.as_slice() else {
                return Err(invalid());
            };
            instructions.push(instruction(
                SelectedInstructionId(0),
                SelectedInstructionKind::MaterializeI64 { value: *value },
                constraints.keys.materialize_i64,
                &[VirtualRegisterId(0)],
                SelectedInstructionProvenance {
                    operations: vec![*constant_operation],
                    values: vec![source_value],
                    fuel: constant_fuel.clone(),
                    ..Default::default()
                },
                catalog,
            )?);
            (
                VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(0),
                    source_value,
                },
                *definition_site,
                None,
                operand.class,
            )
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
            if Some(input.fixed_view) != environment.fixed_register_view(*register) {
                return Err(invalid());
            }
            let view = physical
                .model()
                .views
                .iter()
                .find(|view| view.id == input.fixed_view)
                .ok_or_else(invalid)?;
            (
                VirtualRegisterOrigin::EntryParameter {
                    source_value,
                    parameter_index: *parameter_index,
                },
                *definition_site,
                Some(input.fixed_view),
                view.class,
            )
        }
        _ => return Err(invalid()),
    };
    let mut virtual_registers = vec![VirtualRegister {
        id: VirtualRegisterId(0),
        scalar_type: ScalarType::Integer(source.abi.result.scalar_type),
        class,
        origin,
        definition_site,
        entry_fixed_view,
    }];
    let result_register = if entry_fixed_view.is_some() {
        let copy_row = row(catalog, constraints.keys.copy_i64)?;
        let [_, output] = copy_row.operands.as_slice() else {
            return Err(invalid());
        };
        // The ABI input remains precolored. A distinct definition carries the
        // result constraint; one live range cannot be both RCX and RAX.
        virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(1),
            scalar_type: ScalarType::Integer(source.abi.result.scalar_type),
            class: output.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(0),
                source_value,
            },
            definition_site,
            entry_fixed_view: None,
        });
        instructions.push(instruction(
            SelectedInstructionId(0),
            SelectedInstructionKind::CopyI64,
            constraints.keys.copy_i64,
            &[VirtualRegisterId(0), VirtualRegisterId(1)],
            SelectedInstructionProvenance {
                values: vec![source_value],
                ..Default::default()
            },
            catalog,
        )?);
        VirtualRegisterId(1)
    } else {
        VirtualRegisterId(0)
    };
    let terminator = SelectedTerminator::Return {
        instruction: instruction(
            SelectedInstructionId(instructions.len() as u32),
            SelectedInstructionKind::ReturnI64,
            constraints.keys.return_i64,
            &[result_register],
            SelectedInstructionProvenance {
                values: vec![source_value],
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
        virtual_registers,
        blocks: vec![SelectedBlock {
            id: SelectedBlockId(0),
            source_block: source.entry_block,
            instructions,
            terminator,
        }],
    })
}
