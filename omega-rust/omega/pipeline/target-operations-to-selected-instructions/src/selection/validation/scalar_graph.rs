//! Optimizer module role: executable entrance.
//! Replay selected register transport against the current scalar graph.
//! This checks the proposed stream in place; it does not call selection.

use super::integrity::{validate_block_constraints, validate_def_use};
use crate::selection::constraints::{fixed_input_constraint, row};
use crate::selection::shared::*;
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarInstructionKind};
use semantic_vocabulary::IntegerValue;

mod byte_output;
mod control;
mod scalar_call;
mod structural;
mod unit_call;
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
        || selected.structural != source.structural
        || selected.ranked != source.ranked
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
        transport: structural::Transport::default(),
        constraints,
    };
    structural::entry(source, &environment, &mut replay)?;
    for (index, parameter) in source.parameters.iter().enumerate() {
        if !source.references_value(parameter.value) {
            continue;
        }
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] = parameter.placement.locations.as_slice()
        else {
            return Err(invalid());
        };
        if *byte_size
            != match parameter.scalar_type {
                ScalarType::Boolean => 1,
                ScalarType::Integer(integer) if matches!(integer.bits(), 8 | 16 | 32 | 64) => {
                    integer.bits() / 8
                }
                _ => return Err(invalid()),
            }
        {
            return Err(invalid());
        }
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
            parameter.scalar_type,
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
            parameter.scalar_type,
        ));
    }
    for index in 0..replay.definitions.len() {
        let (value, input, site, scalar_type) = replay.definitions[index];
        let output = if scalar_type == ScalarType::Boolean
            || matches!(scalar_type, ScalarType::Integer(integer) if matches!(integer.bits(), 8 | 32))
        {
            let output = replay.result_register(value, site, scalar_type)?;
            replay.check_instruction(
                if scalar_type == ScalarType::Boolean
                    || matches!(scalar_type, ScalarType::Integer(integer) if integer.bits() == 8)
                {
                    SelectedInstructionKind::ZeroExtendU8
                } else {
                    SelectedInstructionKind::ZeroExtendU32
                },
                constraints.keys.copy_i64,
                &[input, output],
                &SelectedInstructionProvenance {
                    values: vec![value],
                    ..Default::default()
                },
            )?;
            output
        } else {
            replay.check_copy(input, value, site, scalar_type)?
        };
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
            if byte_output::validate(operation, &mut replay)? {
                continue;
            }
            if zero_compare::folded_zero(source, source_block, operation_index + 1).is_some() {
                continue;
            }
            if structural::operation(source, operation, &environment, &mut replay)? {
                continue;
            }
            let result = operation.result.ok_or_else(invalid)?;
            let scalar_type = result.scalar_type;
            let output = match &operation.kind {
                LegalizedScalarInstructionKind::ByteSequenceRead { .. }
                | LegalizedScalarInstructionKind::ByteSequenceLength { .. } => {
                    structural::byte_observation(&mut replay, operation)?
                }
                LegalizedScalarInstructionKind::Compare {
                    predicate,
                    operand_type,
                    left,
                    right,
                } => {
                    if !control::branch_suffix(source_block, operation_index) {
                        return Err(invalid());
                    }
                    if let Some(zero) =
                        zero_compare::folded_zero(source, source_block, operation_index)
                    {
                        let input = if *left == zero.result.ok_or_else(invalid)?.value {
                            *right
                        } else {
                            *left
                        };
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
                                values: vec![
                                    input,
                                    zero.result.ok_or_else(invalid)?.value,
                                    result.value,
                                ],
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
                            values: vec![*left, *right, result.value],
                            fuel: operation.fuel.clone(),
                            ..Default::default()
                        },
                    )?;
                    continue;
                }
                LegalizedScalarInstructionKind::BooleanNot { .. } => {
                    if !control::branch_suffix(source_block, operation_index) {
                        return Err(invalid());
                    }
                    continue;
                }
                LegalizedScalarInstructionKind::IntegerWiden {
                    operand,
                    source_type,
                } => {
                    let (_, input, _, actual_type) =
                        replay.resolve(*operand).ok_or_else(invalid)?;
                    if actual_type != ScalarType::Integer(*source_type)
                        || source_type.sign() != IntegerSign::Unsigned
                        || source_type.bits() != 8
                        || !matches!(scalar_type, ScalarType::Integer(integer)
                            if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                                && matches!(integer.bits(), 16 | 32 | 64)
                                && source_type.can_widen_to(integer))
                    {
                        return Err(invalid());
                    }
                    let output = replay.result_register(
                        result.value,
                        result.definition_site,
                        scalar_type,
                    )?;
                    replay.check_instruction(
                        SelectedInstructionKind::CopyI64,
                        constraints.keys.copy_i64,
                        &[input, output],
                        &SelectedInstructionProvenance {
                            operations: vec![operation.operation],
                            values: vec![*operand, result.value],
                            fuel: operation.fuel.clone(),
                            ..Default::default()
                        },
                    )?;
                    output
                }
                LegalizedScalarInstructionKind::Constant(value) => {
                    if scalar_type == ScalarType::Boolean
                        && !matches!(value, IntegerValue::Unsigned(0 | 1))
                    {
                        return Err(invalid());
                    }
                    let register = replay.result_register(
                        result.value,
                        result.definition_site,
                        scalar_type,
                    )?;
                    replay.check_instruction(
                        SelectedInstructionKind::MaterializeI64 { value: *value },
                        constraints.keys.materialize_i64,
                        &[register],
                        &SelectedInstructionProvenance {
                            operations: vec![operation.operation],
                            values: vec![result.value],
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
                        result.value,
                        result.definition_site,
                        scalar_type,
                    )?;
                    replay.check_instruction(
                        kind,
                        key,
                        &[left_register, right_register, output],
                        &SelectedInstructionProvenance {
                            operations: vec![operation.operation],
                            values: vec![*left, *right, result.value],
                            obligations: vec![*obligation],
                            fuel: operation.fuel.clone(),
                            ..Default::default()
                        },
                    )?;
                    output
                }
                LegalizedScalarInstructionKind::LinuxWriteByteI32 { .. }
                | LegalizedScalarInstructionKind::StructuralScalarFieldStore { .. }
                | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { .. }
                | LegalizedScalarInstructionKind::BoundarySettlement(_)
                | LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { .. }
                | LegalizedScalarInstructionKind::ByteSequenceSubslice { .. } => {
                    return Err(invalid());
                }
                LegalizedScalarInstructionKind::Call(_) => {
                    scalar_call::validate(source, operation, &mut replay, &environment, catalog)?
                }
            };
            replay
                .definitions
                .push((result.value, output, result.definition_site, scalar_type));
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
    if replay.transport.calls != selected.calls
        || replay.transport.slots != selected.outgoing_arguments
        || replay.transport.local_slots != selected.local_storage_slots
        || replay.transport.memory != selected.memory_accesses
        || replay.transport.settlements != selected.boundary_settlements
    {
        return Err(invalid());
    }
    validate_def_use(function, selected, catalog)
}

struct Replay<'a> {
    transport: structural::Transport,
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
            || register.definition_site != Some(site)
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
