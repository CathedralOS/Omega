//! Independent replay for the closed attached-Unit scalar-call selection.

use super::integrity::{validate_block_constraints, validate_def_use};
use crate::selection::constraints::row;
use crate::selection::shared::*;

pub(super) fn validate(
    function_index: usize,
    source: &SourceScalarCallUnitFunction,
    selected: &SelectedFunction,
    constraints: &SelectedSelectionConstraints,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let call_key = constraints.keys.call_i64_2_u64_to_u64.ok_or(
        SelectedInstructionError::UnsupportedSourceShape {
            function: function_index,
        },
    )?;
    let call_row = row(catalog, call_key)?;
    if call_row.operands.len() != 3
        || call_row.operands[0].access != RegisterOperandAccess::Use
        || call_row.operands[1].access != RegisterOperandAccess::Use
        || call_row.operands[2].access != RegisterOperandAccess::Def
        || call_row
            .operands
            .iter()
            .any(|operand| operand.fixed_view.is_none())
    {
        return Err(SelectedInstructionError::MissingConstraint(call_key));
    }
    if selected.machine != source.machine
        || selected.attachment != Some(source.attachment)
        || selected.provenance != source.provenance
        || selected.entry_block != SelectedBlockId(0)
        || selected.virtual_registers.len() != 14
        || selected.blocks.len() != 1
        || selected.blocks[0].id != SelectedBlockId(0)
        || selected.blocks[0].source_block != source.entry_block
        || selected.blocks[0].instructions.len() != 14
    {
        return Err(SelectedInstructionError::FunctionProjectionMismatch {
            function: function_index,
        });
    }
    let block = &selected.blocks[0];
    let u64_type = ScalarType::Integer(
        psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 scalar type"),
    );
    if selected
        .virtual_registers
        .iter()
        .enumerate()
        .any(|(index, register)| {
            register.id.0 as usize != index
                || register.scalar_type != u64_type
                || register.entry_fixed_view.is_some()
        })
        || block
            .instructions
            .iter()
            .enumerate()
            .any(|(index, instruction)| instruction.id.0 as usize != index)
    {
        return Err(SelectedInstructionError::NonCanonicalVirtualRegisters {
            function: function_index,
        });
    }
    for index in 0..2 {
        let constant = &source.constants[index];
        let instruction = &block.instructions[index];
        let register = &selected.virtual_registers[index];
        if instruction.kind
            != (SelectedInstructionKind::MaterializeI64 {
                value: constant.value,
            })
            || instruction.constraint != constraints.keys.materialize_i64
            || instruction.provenance
                != (SelectedInstructionProvenance {
                    operations: vec![constant.operation],
                    values: vec![constant.result],
                    fuel: constant.fuel.clone(),
                    ..Default::default()
                })
            || register.origin
                != (VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(index as u32),
                    source_value: constant.result,
                })
            || register.definition_site != constant.definition_site
        {
            return Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: function_index,
                instruction: index as u32,
            });
        }
    }
    let mut durable = [VirtualRegisterId(0), VirtualRegisterId(1)];
    for call_index in 0..3 {
        let source_call = &source.calls[call_index];
        let base = 2 + call_index * 4;
        let inputs = if call_index < 2 {
            [VirtualRegisterId(0), VirtualRegisterId(1)]
        } else {
            durable
        };
        for argument_index in 0..2 {
            let instruction = &block.instructions[base + argument_index];
            let value = source_call.arguments[argument_index].source.source_value();
            let register = &selected.virtual_registers[base + argument_index];
            if instruction.kind != SelectedInstructionKind::CopyI64
                || instruction.constraint != constraints.keys.copy_i64
                || instruction
                    .operands
                    .iter()
                    .map(|operand| operand.virtual_register)
                    .ne([
                        inputs[argument_index],
                        VirtualRegisterId((base + argument_index) as u32),
                    ])
                || instruction.provenance
                    != (SelectedInstructionProvenance {
                        values: vec![value],
                        ..Default::default()
                    })
                || register.origin
                    != (VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId((base + argument_index) as u32),
                        source_value: value,
                    })
                || register.definition_site != source_definition_site(source, value)
            {
                return Err(SelectedInstructionError::InstructionProjectionMismatch {
                    function: function_index,
                    instruction: instruction.id.0,
                });
            }
        }
        let call = &block.instructions[base + 2];
        let short_result = VirtualRegisterId((base + 2) as u32);
        if call.kind
            != (SelectedInstructionKind::CallI64 {
                callee: source_call.callee,
            })
            || call.constraint != call_key
            || call
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
                .ne([
                    VirtualRegisterId(base as u32),
                    VirtualRegisterId((base + 1) as u32),
                    short_result,
                ])
            || call.provenance
                != (SelectedInstructionProvenance {
                    operations: vec![source_call.operation],
                    values: vec![
                        source_call.arguments[0].source.source_value(),
                        source_call.arguments[1].source.source_value(),
                        source_call.result_home.source_value,
                    ],
                    obligations: source_call.requirement_obligations.clone(),
                    fuel: source_call.fuel.clone(),
                    ..Default::default()
                })
        {
            return Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: function_index,
                instruction: call.id.0,
            });
        }
        let copy_out = &block.instructions[base + 3];
        let durable_result = VirtualRegisterId((base + 3) as u32);
        if copy_out.kind != SelectedInstructionKind::CopyI64
            || copy_out.constraint != constraints.keys.copy_i64
            || copy_out
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
                .ne([short_result, durable_result])
            || copy_out.provenance
                != (SelectedInstructionProvenance {
                    values: vec![source_call.result_home.source_value],
                    ..Default::default()
                })
        {
            return Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: function_index,
                instruction: copy_out.id.0,
            });
        }
        for (register_id, instruction_id) in [(base + 2, base + 2), (base + 3, base + 3)] {
            let register = &selected.virtual_registers[register_id];
            if register.origin
                != (VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(instruction_id as u32),
                    source_value: source_call.result_home.source_value,
                })
                || register.definition_site != source_call.result_definition_site
            {
                return Err(
                    SelectedInstructionError::VirtualRegisterProjectionMismatch {
                        function: function_index,
                        register: register_id as u32,
                    },
                );
            }
        }
        if call_index < 2 {
            durable[call_index] = durable_result;
        }
    }
    let SelectedTerminator::Return {
        instruction: returned,
        psi_return_edge,
    } = &block.terminator
    else {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: 0,
        });
    };
    if returned.id != SelectedInstructionId(14)
        || returned.kind != SelectedInstructionKind::ReturnUnit
        || returned.constraint != constraints.keys.return_unit
        || returned.provenance
            != (SelectedInstructionProvenance {
                edges: vec![source.return_edge],
                fuel: source.return_fuel.clone(),
                ..Default::default()
            })
        || *psi_return_edge != source.return_edge
    {
        return Err(SelectedInstructionError::InstructionProjectionMismatch {
            function: function_index,
            instruction: returned.id.0,
        });
    }
    validate_block_constraints(function_index, block, selected, catalog)?;
    validate_def_use(function_index, selected, catalog)
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
