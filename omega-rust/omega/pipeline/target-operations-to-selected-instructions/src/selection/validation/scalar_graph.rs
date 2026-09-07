//! Replay selected register transport against the current scalar graph.
//! This checks the proposed stream in place; it does not call selection.

use super::integrity::{validate_block_constraints, validate_def_use};
use crate::selection::constraints::{fixed_input_constraint, row};
use crate::selection::shared::*;
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarInstructionKind};

mod control;
mod zero_compare;

pub(in crate::selection) fn validate(
    function: usize,
    source: &LegalizedScalarFunction,
    selected: &SelectedFunction,
    native_target: target::NativeTarget,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let invalid = || SelectedInstructionError::FunctionProjectionMismatch { function };
    let block = selected.blocks.first().ok_or_else(invalid)?;
    control::block_order(source, selected)?;
    if selected.machine != source.machine
        || selected.attachment != source.attachment
        || selected.provenance != source.provenance
        || selected.entry_block != SelectedBlockId(0)
    {
        return Err(invalid());
    }
    let environment = register_environment::validate_target_register_environment(
        native_target,
        physical.model().clone(),
        catalog.catalog().clone(),
    )
    .map_err(|_| invalid())?;
    let [operand] = row(catalog, constraints.keys.materialize_i64)?
        .operands
        .as_slice()
    else {
        return Err(invalid());
    };
    let mut replay = Replay {
        function,
        selected,
        block,
        class: operand.class,
        instruction_cursor: 0,
        block_cursor: 0,
        register_cursor: 0,
        definitions: Vec::new(),
        constraints,
    };
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
        let id = replay.check_register(
            parameter.definition_site,
            ScalarType::Integer(parameter.scalar_type),
            VirtualRegisterOrigin::EntryParameter {
                source_value: parameter.value,
                parameter_index: index,
            },
            Some(fixed.fixed_view),
        )?;
        replay.definitions.push((
            parameter.value,
            id,
            parameter.definition_site,
            ScalarType::Integer(parameter.scalar_type),
        ));
    }
    for index in 0..replay.definitions.len() {
        let (value, input, site, scalar_type) = replay.definitions[index];
        let output = replay.check_copy(input, value, site, scalar_type)?;
        replay.definitions[index].1 = output;
    }
    // Check the predeclared destination roster before any edge refers to it.
    for block in &selected.blocks {
        let source_block = source
            .blocks
            .iter()
            .find(|source| source.id == block.source_block)
            .ok_or_else(invalid)?;
        for (parameter_index, parameter) in source_block.parameters.iter().enumerate() {
            if !source.references_value(parameter.value) {
                continue;
            }
            let id = replay.check_register(
                parameter.site,
                parameter.scalar_type,
                VirtualRegisterOrigin::BlockParameter {
                    source_value: parameter.value,
                    block: block.id,
                    parameter_index,
                },
                None,
            )?;
            replay
                .definitions
                .push((parameter.value, id, parameter.site, parameter.scalar_type));
        }
    }
    for block in &selected.blocks {
        let source_block = source
            .blocks
            .iter()
            .find(|source| source.id == block.source_block)
            .ok_or_else(invalid)?;
        replay.block = block;
        if block.id != SelectedBlockId(0) {
            replay.block_cursor = 0;
        }
        for (operation_index, operation) in source_block.instructions.iter().enumerate() {
            if zero_compare::folded_zero(source, source_block, operation_index + 1).is_some() {
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
                    if let Some(zero) =
                        zero_compare::folded_zero(source, source_block, operation_index)
                    {
                        let input = if *left == zero.result { *right } else { *left };
                        let (_, register, _, actual_type) =
                            replay.resolve(input).ok_or_else(invalid)?;
                        if actual_type != ScalarType::Integer(*operand_type)
                            || scalar_type != ScalarType::Boolean
                        {
                            return Err(invalid());
                        }
                        replay.check_instruction(
                            SelectedInstructionKind::CompareI64Zero,
                            constraints.keys.compare_i64_zero,
                            &[register],
                            &SelectedInstructionProvenance {
                                operations: vec![zero.operation, operation.operation],
                                values: vec![input, zero.result, operation.result],
                                fuel: zero.fuel.iter().chain(&operation.fuel).copied().collect(),
                                ..Default::default()
                            },
                        )?;
                        continue;
                    }
                    let (_, left_register, _, left_type) =
                        replay.resolve(*left).ok_or_else(invalid)?;
                    let (_, right_register, _, right_type) =
                        replay.resolve(*right).ok_or_else(invalid)?;
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
                    replay.check_instruction(
                        SelectedInstructionKind::CompareI64,
                        constraints.keys.compare_i64,
                        &operands,
                        &SelectedInstructionProvenance {
                            operations: vec![operation.operation],
                            values: vec![*left, *right, operation.result],
                            fuel: operation.fuel.clone(),
                            ..Default::default()
                        },
                    )?;
                    continue;
                }
                LegalizedScalarInstructionKind::Constant(value) => {
                    let register = replay.result_register(
                        operation.result,
                        operation.definition_site,
                        scalar_type,
                    )?;
                    replay.check_instruction(
                        SelectedInstructionKind::MaterializeI64 { value: *value },
                        constraints.keys.materialize_i64,
                        &[register],
                        &SelectedInstructionProvenance {
                            operations: vec![operation.operation],
                            values: vec![operation.result],
                            fuel: operation.fuel.clone(),
                            ..Default::default()
                        },
                    )?;
                    register
                }
                LegalizedScalarInstructionKind::ExactBinary {
                    operator,
                    left,
                    right,
                    obligation,
                    accepted_fact,
                } => {
                    let (_, left_register, _, left_type) =
                        replay.resolve(*left).ok_or_else(invalid)?;
                    let (_, right_register, _, right_type) =
                        replay.resolve(*right).ok_or_else(invalid)?;
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
                    let output = replay.result_register(
                        operation.result,
                        operation.definition_site,
                        scalar_type,
                    )?;
                    replay.check_instruction(
                        kind,
                        key,
                        &[left_register, right_register, output],
                        &SelectedInstructionProvenance {
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
                            replay.resolve(argument.source).ok_or_else(invalid)?;
                        operands.push(replay.check_copy(
                            input,
                            argument.source,
                            site,
                            argument_type,
                        )?);
                    }
                    let short_result = replay.result_register(
                        operation.result,
                        operation.definition_site,
                        scalar_type,
                    )?;
                    operands.push(short_result);
                    replay.check_instruction(
                        SelectedInstructionKind::CallI64 {
                            callee: call.callee,
                        },
                        key,
                        &operands,
                        &SelectedInstructionProvenance {
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
                    replay.check_copy(
                        short_result,
                        operation.result,
                        operation.definition_site,
                        scalar_type,
                    )?
                }
            };
            replay.definitions.push((
                operation.result,
                output,
                operation.definition_site,
                scalar_type,
            ));
        }
        control::validate(source, source_block, &mut replay, &environment, catalog)?;
        if replay.block_cursor != block.instructions.len() {
            return Err(invalid());
        }
        replay.instruction_cursor = replay
            .instruction_cursor
            .checked_add(1)
            .ok_or_else(invalid)?;
        validate_block_constraints(function, block, selected, catalog)?;
    }
    if replay.register_cursor != selected.virtual_registers.len() {
        return Err(invalid());
    }
    validate_def_use(function, selected, catalog)
}

struct Replay<'a> {
    function: usize,
    selected: &'a SelectedFunction,
    block: &'a SelectedBlock,
    class: RegisterClassId,
    instruction_cursor: usize,
    block_cursor: usize,
    register_cursor: usize,
    definitions: Vec<(ValueId, VirtualRegisterId, ValueDefinitionSite, ScalarType)>,
    constraints: &'a SelectedSelectionConstraints,
}

impl Replay<'_> {
    fn invalid(&self) -> SelectedInstructionError {
        SelectedInstructionError::FunctionProjectionMismatch {
            function: self.function,
        }
    }

    fn resolve(
        &self,
        value: ValueId,
    ) -> Option<(ValueId, VirtualRegisterId, ValueDefinitionSite, ScalarType)> {
        self.definitions
            .iter()
            .find(|(source, ..)| *source == value)
            .copied()
    }

    fn check_register(
        &mut self,
        site: ValueDefinitionSite,
        scalar_type: ScalarType,
        origin: VirtualRegisterOrigin,
        fixed: Option<RegisterViewId>,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let register = self
            .selected
            .virtual_registers
            .get(self.register_cursor)
            .ok_or_else(|| self.invalid())?;
        if register.id.0 as usize != self.register_cursor
            || register.scalar_type != scalar_type
            || register.class != self.class
            || register.origin != origin
            || register.definition_site != site
            || register.entry_fixed_view != fixed
        {
            return Err(self.invalid());
        }
        self.register_cursor += 1;
        Ok(register.id)
    }

    fn result_register(
        &mut self,
        value: ValueId,
        site: ValueDefinitionSite,
        scalar_type: ScalarType,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let instruction = SelectedInstructionId(
            self.instruction_cursor
                .try_into()
                .map_err(|_| self.invalid())?,
        );
        self.check_register(
            site,
            scalar_type,
            VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value: value,
            },
            None,
        )
    }

    fn check_instruction(
        &mut self,
        kind: SelectedInstructionKind,
        key: RegisterConstraintKey,
        operands: &[VirtualRegisterId],
        provenance: &SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        let instruction = self
            .block
            .instructions
            .get(self.block_cursor)
            .ok_or_else(|| self.invalid())?;
        if instruction.id.0 as usize != self.instruction_cursor
            || instruction.kind != kind
            || instruction.constraint != key
            || instruction.provenance != *provenance
            || instruction
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
                .ne(operands.iter().copied())
        {
            return Err(self.invalid());
        }
        self.instruction_cursor += 1;
        self.block_cursor += 1;
        Ok(())
    }

    fn check_copy(
        &mut self,
        input: VirtualRegisterId,
        value: ValueId,
        site: ValueDefinitionSite,
        scalar_type: ScalarType,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let output = self.result_register(value, site, scalar_type)?;
        self.check_instruction(
            SelectedInstructionKind::CopyI64,
            self.constraints.keys.copy_i64,
            &[input, output],
            &SelectedInstructionProvenance {
                values: vec![value],
                ..Default::default()
            },
        )?;
        Ok(output)
    }
}
