//! Scalar instructions share one selection path regardless of the caller's result.

use crate::selection::constraints::{fixed_input_constraint, instruction, row};
use crate::selection::shared::*;
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarInstructionKind};

mod control;
mod zero_compare;

#[cfg(test)]
mod tests;

pub(super) fn build(
    function: usize,
    source: &LegalizedScalarFunction,
    native_target: target::NativeTarget,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    let order = control::block_order(source)?;
    let environment = register_environment::validate_target_register_environment(
        native_target,
        physical.model().clone(),
        catalog.catalog().clone(),
    )
    .map_err(|_| invalid())?;
    let materialize = row(catalog, constraints.keys.materialize_i64)?;
    let [operand] = materialize.operands.as_slice() else {
        return Err(invalid());
    };
    let class = operand.class;
    let mut builder = Builder {
        class,
        constraints,
        catalog,
        registers: Vec::new(),
        instructions: Vec::new(),
        definitions: Vec::new(),
    };
    // Entry ABI precoloring ends at a copy. The semantic parameter may remain
    // live across calls without being pinned to a caller-clobbered register.
    for (index, parameter) in source.parameters.iter().enumerate() {
        if !source.references_value(parameter.value) {
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
        if environment.fixed_register_view(*register) != Some(fixed.fixed_view) {
            return Err(invalid());
        }
        let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
        builder.registers.push(VirtualRegister {
            id,
            scalar_type: ScalarType::Integer(parameter.scalar_type),
            class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: parameter.value,
                parameter_index: index,
            },
            definition_site: parameter.definition_site,
            entry_fixed_view: Some(fixed.fixed_view),
        });
        builder.definitions.push((
            parameter.value,
            id,
            parameter.definition_site,
            ScalarType::Integer(parameter.scalar_type),
        ));
    }
    for index in 0..builder.definitions.len() {
        let (value, input, site, scalar_type) = builder.definitions[index];
        let output = builder.copy(input, value, site, scalar_type)?;
        builder.definitions[index].1 = output;
    }
    // Forward successors name their materialized destination parameters explicitly.
    for (block_index, source_index) in order.iter().copied().enumerate() {
        let block = &source.blocks[source_index];
        let block_id = SelectedBlockId(u32::try_from(block_index).map_err(|_| invalid())?);
        for (parameter_index, parameter) in block.parameters.iter().enumerate() {
            if !source.references_value(parameter.value) {
                continue;
            }
            let id =
                VirtualRegisterId(u32::try_from(builder.registers.len()).map_err(|_| invalid())?);
            builder.registers.push(VirtualRegister {
                id,
                scalar_type: parameter.scalar_type,
                class,
                origin: VirtualRegisterOrigin::BlockParameter {
                    source_value: parameter.value,
                    block: block_id,
                    parameter_index,
                },
                definition_site: parameter.site,
                entry_fixed_view: None,
            });
            builder
                .definitions
                .push((parameter.value, id, parameter.site, parameter.scalar_type));
        }
    }
    let mut blocks = Vec::new();
    for (block_index, source_index) in order.iter().copied().enumerate() {
        let block = &source.blocks[source_index];
        let block_id = SelectedBlockId(u32::try_from(block_index).map_err(|_| invalid())?);
        let start = if block_index == 0 {
            0
        } else {
            builder.instructions.len()
        };
        for (operation_index, operation) in block.instructions.iter().enumerate() {
            if zero_compare::folded_zero(source, block, operation_index + 1).is_some() {
                continue;
            }
            let scalar_type = operation.scalar_type;
            let output = match &operation.kind {
                LegalizedScalarInstructionKind::Compare {
                    predicate,
                    operand_type,
                    left,
                    right,
                } => {
                    if let Some(zero) = zero_compare::folded_zero(source, block, operation_index) {
                        let input = if *left == zero.result { *right } else { *left };
                        let (_, register, _, actual_type) =
                            builder.resolve(input).ok_or_else(invalid)?;
                        if actual_type != ScalarType::Integer(*operand_type)
                            || scalar_type != ScalarType::Boolean
                        {
                            return Err(invalid());
                        }
                        builder.emit(
                            SelectedInstructionKind::CompareI64Zero,
                            constraints.keys.compare_i64_zero,
                            &[register],
                            SelectedInstructionProvenance {
                                operations: vec![zero.operation, operation.operation],
                                values: vec![input, zero.result, operation.result],
                                fuel: zero.fuel.iter().chain(&operation.fuel).copied().collect(),
                                ..Default::default()
                            },
                        )?;
                        continue;
                    }
                    let (_, left_register, _, left_type) =
                        builder.resolve(*left).ok_or_else(invalid)?;
                    let (_, right_register, _, right_type) =
                        builder.resolve(*right).ok_or_else(invalid)?;
                    if left_type != ScalarType::Integer(*operand_type)
                        || right_type != left_type
                        || scalar_type != ScalarType::Boolean
                    {
                        return Err(invalid());
                    }
                    let operands = if matches!(
                        predicate,
                        legalized_operations::LegalizedScalarComparison::LessOrEqual
                    ) {
                        [right_register, left_register]
                    } else {
                        [left_register, right_register]
                    };
                    builder.emit(
                        SelectedInstructionKind::CompareI64,
                        constraints.keys.compare_i64,
                        &operands,
                        SelectedInstructionProvenance {
                            operations: vec![operation.operation],
                            values: vec![*left, *right, operation.result],
                            fuel: operation.fuel.clone(),
                            ..Default::default()
                        },
                    )?;
                    continue;
                }
                LegalizedScalarInstructionKind::Constant(value) => {
                    let output = builder.register(
                        operation.result,
                        operation.definition_site,
                        scalar_type,
                    )?;
                    builder.emit(
                        SelectedInstructionKind::MaterializeI64 { value: *value },
                        constraints.keys.materialize_i64,
                        &[output],
                        SelectedInstructionProvenance {
                            operations: vec![operation.operation],
                            values: vec![operation.result],
                            fuel: operation.fuel.clone(),
                            ..Default::default()
                        },
                    )?;
                    output
                }
                LegalizedScalarInstructionKind::ExactBinary {
                    operator,
                    left,
                    right,
                    obligation,
                    accepted_fact,
                } => {
                    let (_, left_register, _, left_type) =
                        builder.resolve(*left).ok_or_else(invalid)?;
                    let (_, right_register, _, right_type) =
                        builder.resolve(*right).ok_or_else(invalid)?;
                    if left_type != scalar_type || right_type != scalar_type {
                        return Err(invalid());
                    }
                    let (kind, key) = match operator {
                        legalized_operations::LegalizedExactIntegerOperator::Add => (
                            SelectedInstructionKind::ExactAddI64 {
                                obligation: *obligation,
                                accepted_fact: *accepted_fact,
                            },
                            constraints.keys.add_i64,
                        ),
                        legalized_operations::LegalizedExactIntegerOperator::Subtract => (
                            SelectedInstructionKind::ExactSubtractI64 {
                                obligation: *obligation,
                                accepted_fact: *accepted_fact,
                            },
                            constraints.keys.subtract_i64,
                        ),
                    };
                    let output = builder.register(
                        operation.result,
                        operation.definition_site,
                        scalar_type,
                    )?;
                    builder.emit(
                        kind,
                        key,
                        &[left_register, right_register, output],
                        SelectedInstructionProvenance {
                            operations: vec![operation.operation],
                            values: vec![*left, *right, operation.result],
                            obligations: vec![*obligation],
                            fuel: operation.fuel.clone(),
                            ..Default::default()
                        },
                    )?;
                    output
                }
                LegalizedScalarInstructionKind::Call(call) => {
                    let key = constraints
                        .keys
                        .call_i64
                        .get(call.arguments.len())
                        .copied()
                        .ok_or_else(invalid)?;
                    crate::selection::scalar_call_abi::validate(
                        function,
                        call,
                        key,
                        row(catalog, key)?,
                        &environment,
                    )?;
                    let mut operands = Vec::new();
                    for argument in &call.arguments {
                        let (_, input, site, argument_type) =
                            builder.resolve(argument.source).ok_or_else(invalid)?;
                        operands.push(builder.copy(input, argument.source, site, argument_type)?);
                    }
                    let short_result = builder.register(
                        operation.result,
                        operation.definition_site,
                        scalar_type,
                    )?;
                    operands.push(short_result);
                    builder.emit(
                        SelectedInstructionKind::CallI64 {
                            callee: call.callee,
                        },
                        key,
                        &operands,
                        SelectedInstructionProvenance {
                            operations: vec![operation.operation],
                            values: call
                                .arguments
                                .iter()
                                .map(|argument| argument.source)
                                .chain(std::iter::once(operation.result))
                                .collect(),
                            obligations: call.requirement_obligations.clone(),
                            fuel: operation.fuel.clone(),
                            ..Default::default()
                        },
                    )?;
                    builder.copy(
                        short_result,
                        operation.result,
                        operation.definition_site,
                        scalar_type,
                    )?
                }
            };
            builder.definitions.push((
                operation.result,
                output,
                operation.definition_site,
                scalar_type,
            ));
        }
        let terminator =
            control::build(function, source, block, &order, &mut builder, &environment)?;
        let body_end = builder
            .instructions
            .len()
            .checked_sub(1)
            .ok_or_else(invalid)?;
        blocks.push(SelectedBlock {
            id: block_id,
            source_block: block.id,
            instructions: builder.instructions[start..body_end].to_vec(),
            terminator,
        });
    }
    Ok(SelectedFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
        entry_block: SelectedBlockId(0),
        virtual_registers: builder.registers,
        blocks,
    })
}

struct Builder<'a> {
    class: RegisterClassId,
    constraints: &'a SelectedSelectionConstraints,
    catalog: &'a ValidatedRegisterConstraintCatalog,
    registers: Vec<VirtualRegister>,
    instructions: Vec<SelectedInstruction>,
    definitions: Vec<(ValueId, VirtualRegisterId, ValueDefinitionSite, ScalarType)>,
}

impl Builder<'_> {
    fn resolve(
        &self,
        value: ValueId,
    ) -> Option<(ValueId, VirtualRegisterId, ValueDefinitionSite, ScalarType)> {
        self.definitions
            .iter()
            .find(|(source, ..)| *source == value)
            .copied()
    }

    fn register(
        &mut self,
        value: ValueId,
        site: ValueDefinitionSite,
        scalar_type: ScalarType,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let id = VirtualRegisterId(
            self.registers
                .len()
                .try_into()
                .map_err(|_| SelectedInstructionError::SourceCustodyMismatch)?,
        );
        self.registers.push(VirtualRegister {
            id,
            scalar_type,
            class: self.class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(
                    self.instructions
                        .len()
                        .try_into()
                        .map_err(|_| SelectedInstructionError::SourceCustodyMismatch)?,
                ),
                source_value: value,
            },
            definition_site: site,
            entry_fixed_view: None,
        });
        Ok(id)
    }

    fn emit(
        &mut self,
        kind: SelectedInstructionKind,
        key: RegisterConstraintKey,
        operands: &[VirtualRegisterId],
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        let id = SelectedInstructionId(
            self.instructions
                .len()
                .try_into()
                .map_err(|_| SelectedInstructionError::SourceCustodyMismatch)?,
        );
        self.instructions.push(instruction(
            id,
            kind,
            key,
            operands,
            provenance,
            self.catalog,
        )?);
        Ok(())
    }

    fn copy(
        &mut self,
        input: VirtualRegisterId,
        value: ValueId,
        site: ValueDefinitionSite,
        scalar_type: ScalarType,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let output = self.register(value, site, scalar_type)?;
        self.emit(
            SelectedInstructionKind::CopyI64,
            self.constraints.keys.copy_i64,
            &[input, output],
            SelectedInstructionProvenance {
                values: vec![value],
                ..Default::default()
            },
        )?;
        Ok(output)
    }
}
