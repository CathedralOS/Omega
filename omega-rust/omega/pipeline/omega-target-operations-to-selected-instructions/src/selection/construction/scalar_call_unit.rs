//! Closed attached-Unit `U64, U64 -> U64` three-call selection.

use crate::selection::constraints::{instruction, row};
use crate::selection::shared::*;

pub(super) fn build(
    function: usize,
    source: &SourceScalarCallUnitFunction,
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let call_key = constraints
        .keys
        .call_i64_2_u64_to_u64
        .ok_or(SelectedInstructionError::UnsupportedSourceShape { function })?;
    let materialize = row(catalog, constraints.keys.materialize_i64)?;
    let copy = row(catalog, constraints.keys.copy_i64)?;
    let call = row(catalog, call_key)?;
    if materialize.operands.len() != 1
        || copy.operands.len() != 2
        || call.operands.len() != 3
        || call
            .operands
            .iter()
            .any(|operand| operand.fixed_view.is_none())
    {
        return Err(SelectedInstructionError::MissingConstraint(call_key));
    }
    let class = materialize.operands[0].class;
    if copy.operands.iter().any(|operand| operand.class != class)
        || call.operands.iter().any(|operand| operand.class != class)
    {
        return Err(SelectedInstructionError::MissingConstraint(call_key));
    }
    let u64_type = ScalarType::Integer(
        psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 scalar type"),
    );
    let mut registers = Vec::with_capacity(14);
    let mut instructions = Vec::with_capacity(14);

    for (index, constant) in source.constants.iter().enumerate() {
        let instruction_id = u32::try_from(index).expect("two constants fit u32");
        let register_id = instruction_id;
        registers.push(result_register(
            register_id,
            instruction_id,
            constant.result,
            constant.definition_site,
            u64_type,
            class,
        ));
        instructions.push(instruction(
            SelectedInstructionId(instruction_id),
            SelectedInstructionKind::MaterializeI64 {
                value: constant.value,
            },
            constraints.keys.materialize_i64,
            &[VirtualRegisterId(register_id)],
            SelectedInstructionProvenance {
                operations: vec![constant.operation],
                values: vec![constant.result],
                fuel: constant.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?);
    }

    let mut durable_registers = [VirtualRegisterId(0), VirtualRegisterId(1)];
    for call_index in 0..3 {
        let selected_call = &source.calls[call_index];
        let base_instruction = 2 + u32::try_from(call_index).expect("three calls fit u32") * 4;
        let base_register = base_instruction;
        let input_registers = if call_index < 2 {
            [VirtualRegisterId(0), VirtualRegisterId(1)]
        } else {
            durable_registers
        };
        for (argument_index, input_register) in input_registers.into_iter().enumerate() {
            let instruction_id = base_instruction + u32::try_from(argument_index).unwrap();
            let register_id = base_register + u32::try_from(argument_index).unwrap();
            let source_value = selected_call.arguments[argument_index]
                .source
                .source_value();
            registers.push(result_register(
                register_id,
                instruction_id,
                source_value,
                source_definition_site(source, source_value),
                u64_type,
                class,
            ));
            instructions.push(instruction(
                SelectedInstructionId(instruction_id),
                SelectedInstructionKind::CopyI64,
                constraints.keys.copy_i64,
                &[input_register, VirtualRegisterId(register_id)],
                SelectedInstructionProvenance {
                    values: vec![source_value],
                    ..Default::default()
                },
                catalog,
            )?);
        }
        let short_result = VirtualRegisterId(base_register + 2);
        registers.push(result_register(
            short_result.0,
            base_instruction + 2,
            selected_call.result_home.source_value,
            selected_call.result_definition_site,
            u64_type,
            class,
        ));
        instructions.push(instruction(
            SelectedInstructionId(base_instruction + 2),
            SelectedInstructionKind::CallI64 {
                callee: selected_call.callee,
            },
            call_key,
            &[
                VirtualRegisterId(base_register),
                VirtualRegisterId(base_register + 1),
                short_result,
            ],
            SelectedInstructionProvenance {
                operations: vec![selected_call.operation],
                values: vec![
                    selected_call.arguments[0].source.source_value(),
                    selected_call.arguments[1].source.source_value(),
                    selected_call.result_home.source_value,
                ],
                obligations: selected_call.requirement_obligations.clone(),
                fuel: selected_call.fuel.clone(),
                ..Default::default()
            },
            catalog,
        )?);
        let durable_result = VirtualRegisterId(base_register + 3);
        registers.push(result_register(
            durable_result.0,
            base_instruction + 3,
            selected_call.result_home.source_value,
            selected_call.result_definition_site,
            u64_type,
            class,
        ));
        instructions.push(instruction(
            SelectedInstructionId(base_instruction + 3),
            SelectedInstructionKind::CopyI64,
            constraints.keys.copy_i64,
            &[short_result, durable_result],
            SelectedInstructionProvenance {
                values: vec![selected_call.result_home.source_value],
                ..Default::default()
            },
            catalog,
        )?);
        if call_index < 2 {
            durable_registers[call_index] = durable_result;
        }
    }
    let return_instruction = instruction(
        SelectedInstructionId(14),
        SelectedInstructionKind::ReturnUnit,
        constraints.keys.return_unit,
        &[],
        SelectedInstructionProvenance {
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )?;
    Ok(SelectedFunction {
        machine: source.machine,
        attachment: Some(source.attachment),
        provenance: source.provenance.clone(),
        entry_block: SelectedBlockId(0),
        virtual_registers: registers,
        blocks: vec![SelectedBlock {
            id: SelectedBlockId(0),
            source_block: source.entry_block,
            instructions,
            terminator: SelectedTerminator::Return {
                instruction: return_instruction,
                psi_return_edge: source.return_edge,
            },
        }],
    })
}

fn result_register(
    id: u32,
    instruction: u32,
    source_value: ValueId,
    definition_site: ValueDefinitionSite,
    scalar_type: ScalarType,
    class: RegisterClassId,
) -> VirtualRegister {
    VirtualRegister {
        id: VirtualRegisterId(id),
        scalar_type,
        class,
        origin: VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(instruction),
            source_value,
        },
        definition_site,
        entry_fixed_view: None,
    }
}

fn source_definition_site(
    source: &SourceScalarCallUnitFunction,
    value: ValueId,
) -> ValueDefinitionSite {
    source
        .constants
        .iter()
        .find(|constant| constant.result == value)
        .map(|constant| constant.definition_site)
        .or_else(|| {
            source
                .calls
                .iter()
                .find(|call| call.result_home.source_value == value)
                .map(|call| call.result_definition_site)
        })
        .expect("validated scalar-call source owns every argument value")
}
