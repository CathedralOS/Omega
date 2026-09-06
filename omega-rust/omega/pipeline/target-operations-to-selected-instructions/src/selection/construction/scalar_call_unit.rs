//! Ordered attached-Unit calls using the actual native U64 register ABI.

use crate::selection::constraints::{instruction, row};
use crate::selection::shared::*;

#[cfg(test)]
mod tests;

pub(super) fn build(
    function: usize,
    source: &SourceScalarCallUnitFunction,
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
    let materialize = row(catalog, constraints.keys.materialize_i64)?;
    let copy = row(catalog, constraints.keys.copy_i64)?;
    if materialize.operands.len() != 1 || copy.operands.len() != 2 {
        return Err(invalid());
    }
    let class = materialize.operands[0].class;
    if copy.operands.iter().any(|operand| operand.class != class) {
        return Err(invalid());
    }
    let u64_type = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 scalar type"),
    );
    let mut registers = Vec::new();
    let mut instructions = Vec::new();
    let mut definitions = Vec::new();

    for operation in &source.operations {
        let base = u32::try_from(instructions.len())
            .map_err(|_| SelectedInstructionError::UnsupportedSourceShape { function })?;
        match operation {
            LegalizedScalarCallUnitOperation::Constant(constant) => {
                let instruction_id = base;
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
                definitions.push((
                    constant.result,
                    VirtualRegisterId(register_id),
                    constant.definition_site,
                ));
            }

            LegalizedScalarCallUnitOperation::Call(selected_call) => {
                let argument_count =
                    u32::try_from(selected_call.arguments.len()).map_err(|_| invalid())?;
                base.checked_add(argument_count)
                    .and_then(|value| value.checked_add(2))
                    .ok_or_else(invalid)?;
                let call_key = constraints
                    .keys
                    .call_i64
                    .get(selected_call.arguments.len())
                    .copied()
                    .ok_or_else(invalid)?;
                let call = row(catalog, call_key)?;
                crate::selection::scalar_call_abi::validate(
                    function,
                    selected_call,
                    call_key,
                    call,
                    &environment,
                )?;
                if call.operands.iter().any(|operand| operand.class != class) {
                    return Err(invalid());
                }
                let base_instruction = base;
                let base_register = base;
                let input_registers = selected_call
                    .arguments
                    .iter()
                    .map(|argument| {
                        definitions
                            .iter()
                            .find(|(value, _, _)| *value == argument.source.source_value())
                            .map(|(_, register, _)| *register)
                            .ok_or_else(invalid)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
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
                        definitions
                            .iter()
                            .find(|(value, _, _)| *value == source_value)
                            .map(|(_, _, site)| *site)
                            .expect("validated preceding definition"),
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
                let short_result = VirtualRegisterId(base_register + argument_count);
                registers.push(result_register(
                    short_result.0,
                    base_instruction + argument_count,
                    selected_call.result_home.source_value,
                    selected_call.result_definition_site,
                    u64_type,
                    class,
                ));
                instructions.push(instruction(
                    SelectedInstructionId(base_instruction + argument_count),
                    SelectedInstructionKind::CallI64 {
                        callee: selected_call.callee,
                    },
                    call_key,
                    &(0..=argument_count)
                        .map(|index| VirtualRegisterId(base_register + index))
                        .collect::<Vec<_>>(),
                    SelectedInstructionProvenance {
                        operations: vec![selected_call.operation],
                        values: selected_call
                            .arguments
                            .iter()
                            .map(|argument| argument.source.source_value())
                            .chain(std::iter::once(selected_call.result_home.source_value))
                            .collect(),
                        obligations: selected_call.requirement_obligations.clone(),
                        fuel: selected_call.fuel.clone(),
                        ..Default::default()
                    },
                    catalog,
                )?);
                let durable_result = VirtualRegisterId(base_register + argument_count + 1);
                registers.push(result_register(
                    durable_result.0,
                    base_instruction + argument_count + 1,
                    selected_call.result_home.source_value,
                    selected_call.result_definition_site,
                    u64_type,
                    class,
                ));
                instructions.push(instruction(
                    SelectedInstructionId(base_instruction + argument_count + 1),
                    SelectedInstructionKind::CopyI64,
                    constraints.keys.copy_i64,
                    &[short_result, durable_result],
                    SelectedInstructionProvenance {
                        values: vec![selected_call.result_home.source_value],
                        ..Default::default()
                    },
                    catalog,
                )?);
                definitions.push((
                    selected_call.result_home.source_value,
                    durable_result,
                    selected_call.result_definition_site,
                ));
            }
        }
    }
    let return_instruction = instruction(
        SelectedInstructionId(
            u32::try_from(instructions.len())
                .map_err(|_| SelectedInstructionError::UnsupportedSourceShape { function })?,
        ),
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
