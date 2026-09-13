//! Optimizer module role: executable entrance.
//! Replay selected register transport against the current scalar graph.
//! This checks the proposed stream in place; it does not call selection.

use super::integrity::validate_block_constraints;
use crate::selection::constraints::row;
use crate::selection::shared::*;
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarInstructionKind};
use semantic_vocabulary::IntegerValue;

mod aggregate_argument;
mod aggregate_memory;
mod aggregate_return;
mod boolean_value;
mod byte_input;
mod byte_output;
mod control;
mod ieee_comparison;
mod literal_compare;
mod process_exit;
mod provenance;
mod register_entry;
mod scalar_call;
mod scalar_stack;
mod structural;
mod structural_case;
mod unit_call;

#[cfg(test)]
mod raw_input_tests;
#[cfg(test)]
pub(in crate::selection) use raw_input_tests::validate;

pub(in crate::selection) fn validate_with_environment(
    function: usize,
    source: &LegalizedScalarFunction,
    selected: &SelectedFunction,
    constraints: &SelectedSelectionConstraints,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<(), SelectedInstructionError> {
    let catalog = environment.constraints();
    let invalid = || SelectedInstructionError::FunctionProjectionMismatch { function };
    let block = selected.blocks.first().ok_or_else(invalid)?;
    control::block_order(source, selected)?;
    if selected.machine != source.machine
        || selected.attachment != source.attachment
        || selected.provenance != source.provenance
        || selected.structural != source.structural
        || selected.entry_block != SelectedBlockId(0)
    {
        return Err(invalid());
    }
    let [operand] = row(catalog, constraints.keys.materialize_i64)?
        .operands
        .as_slice()
    else {
        return Err(invalid());
    };
    let mut replay = Replay {
        pending_provenance: SelectedInstructionProvenance::default(),
        required_values: super::value_transport::required_values(source),
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
    structural::indirect_results::entry(source, environment, &mut replay)?;
    structural::entry(source, environment, &mut replay)?;
    register_entry::validate(source, environment, catalog, &mut replay)?;
    scalar_stack::entry(source, &mut replay)?;
    // Check the predeclared destination roster before any edge refers to it.
    for block in selected.blocks.iter().filter(|block| {
        matches!(
            block.origin,
            selected_instructions::SelectedBlockOrigin::Source(_)
        )
    }) {
        let source_block = source
            .blocks
            .iter()
            .find(|source| source.id == block.source_block())
            .ok_or_else(invalid)?;
        for (parameter_index, parameter) in source_block.parameters.iter().enumerate() {
            if !replay.required_values.contains(&parameter.value) {
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
    for block in selected.blocks.iter().filter(|block| {
        matches!(
            block.origin,
            selected_instructions::SelectedBlockOrigin::Source(_)
        )
    }) {
        let source_block = source
            .blocks
            .iter()
            .find(|source| source.id == block.source_block())
            .ok_or_else(invalid)?;
        replay.block = block;
        if block.id != SelectedBlockId(0) {
            replay.block_cursor = 0;
        }
        if !crate::unobserved_owned_input::accepts(source) {
            structural::block_entry(source, source_block, &mut replay)?;
        }
        for (operation_index, operation) in source_block.instructions.iter().enumerate() {
            if matches!(
                operation.kind,
                LegalizedScalarInstructionKind::HostedExitProcessI32 { .. }
            ) {
                if operation_index + 1 != source_block.instructions.len() {
                    return Err(invalid());
                }
                continue;
            }
            if byte_input::validate(operation, &mut replay)?
                || byte_output::validate(operation, &mut replay)?
            {
                continue;
            }
            if (literal_compare::folded_zero(source, source_block, operation_index + 1).is_some()
                || literal_compare::folded_immediate(source, source_block, operation_index + 1)
                    .is_some())
                && control::branch_suffix(source, source_block, operation_index + 1)
            {
                continue;
            }
            if structural::operation(source, operation, environment, &mut replay)? {
                continue;
            }
            let result = operation.result.ok_or_else(invalid)?;
            let scalar_type = result.scalar_type;
            let output = if matches!(
                operation.kind,
                LegalizedScalarInstructionKind::Compare { .. }
                    | LegalizedScalarInstructionKind::BooleanNot { .. }
            ) && !control::branch_suffix(source, source_block, operation_index)
            {
                boolean_value::validate(operation, &mut replay)?
            } else {
                match &operation.kind {
                    LegalizedScalarInstructionKind::IeeeFloatCompare { .. } => {
                        ieee_comparison::validate(operation, &mut replay)?
                    }
                    LegalizedScalarInstructionKind::StructuralCaseMembership { .. } => {
                        structural::observe(source, &mut replay, operation)?
                    }
                    LegalizedScalarInstructionKind::StructuralScalarFieldRead { .. }
                    | LegalizedScalarInstructionKind::PrimitiveScalarRead { .. } => {
                        structural::read(source, &mut replay, operation)?
                    }
                    LegalizedScalarInstructionKind::ByteSequenceRead { .. }
                    | LegalizedScalarInstructionKind::ByteSequenceLength { .. } => {
                        structural::byte_observation(source, &mut replay, operation)?
                    }
                    LegalizedScalarInstructionKind::Compare {
                        predicate,
                        operand_type,
                        left,
                        right,
                    } => {
                        if !control::branch_suffix(source, source_block, operation_index) {
                            return Err(invalid());
                        }
                        if let Some(zero) =
                            literal_compare::folded_zero(source, source_block, operation_index)
                        {
                            let input = if *left == zero.result.ok_or_else(invalid)?.value {
                                *right
                            } else {
                                *left
                            };
                            let (_, register, _, actual_type) =
                                replay.resolve(input).ok_or_else(invalid)?;
                            if actual_type != *operand_type || scalar_type != ScalarType::Boolean {
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
                                    fuel: zero
                                        .fuel
                                        .iter()
                                        .chain(&operation.fuel)
                                        .copied()
                                        .collect(),
                                    ..Default::default()
                                },
                            )?;
                            continue;
                        }
                        if let Some(immediate) =
                            literal_compare::folded_immediate(source, source_block, operation_index)
                        {
                            let immediate_value = match immediate.kind {
                                LegalizedScalarInstructionKind::Constant(
                                    IntegerValue::Unsigned(value),
                                ) => value,
                                _ => return Err(invalid()),
                            };
                            let input = if *left == immediate.result.ok_or_else(invalid)?.value {
                                *right
                            } else {
                                *left
                            };
                            let (_, register, _, actual_type) =
                                replay.resolve(input).ok_or_else(invalid)?;
                            if actual_type != *operand_type || scalar_type != ScalarType::Boolean {
                                return Err(invalid());
                            }
                            replay.check_instruction(
                                SelectedInstructionKind::CompareI64Immediate {
                                    immediate: IntegerValue::Unsigned(immediate_value),
                                },
                                constraints.keys.compare_i64_immediate,
                                &[register],
                                &SelectedInstructionProvenance {
                                    operations: vec![immediate.operation, operation.operation],
                                    values: vec![
                                        input,
                                        immediate.result.ok_or_else(invalid)?.value,
                                        result.value,
                                    ],
                                    fuel: immediate
                                        .fuel
                                        .iter()
                                        .chain(&operation.fuel)
                                        .copied()
                                        .collect(),
                                    ..Default::default()
                                },
                            )?;
                            continue;
                        }
                        let (_, left_register, _, left_type) =
                            replay.resolve(*left).ok_or_else(invalid)?;
                        let (_, right_register, _, right_type) =
                            replay.resolve(*right).ok_or_else(invalid)?;
                        if left_type != *operand_type
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
                        if !control::branch_suffix(source, source_block, operation_index) {
                            return Err(invalid());
                        }
                        continue;
                    }
                    LegalizedScalarInstructionKind::IntegerWiden {
                        operand,
                        source_type,
                    }
                    | LegalizedScalarInstructionKind::IntegerExactCast {
                        operand,
                        source_type,
                        ..
                    } => {
                        let (_, input, _, actual_type) =
                            replay.resolve(*operand).ok_or_else(invalid)?;
                        if actual_type != ScalarType::Integer(*source_type)
                            || !matches!(scalar_type, ScalarType::Integer(integer)
                            if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                                && matches!(integer.bits(), 8 | 16 | 32 | 64)
                                && if matches!(operation.kind, LegalizedScalarInstructionKind::IntegerExactCast { .. }) {
                                    source_type.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                                        && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                                        && source_type.can_exact_cast_to(integer)
                                        && !(source_type.sign() == IntegerSign::Signed
                                            && integer.sign() == IntegerSign::Signed
                                            && (source_type.bits() != 64 || integer.bits() != 64))
                                        && !(source_type.bits() == 16 && integer.bits() > 16)
                                } else {
                                    source_type.sign() == IntegerSign::Unsigned && source_type.bits() == 8
                                        && matches!(integer.bits(), 16 | 32 | 64) && source_type.can_widen_to(integer)
                                })
                        {
                            return Err(invalid());
                        }
                        let output = replay.result_register(
                            result.value,
                            result.definition_site,
                            scalar_type,
                        )?;
                        replay.check_instruction(
                            if matches!(
                                operation.kind,
                                LegalizedScalarInstructionKind::IntegerExactCast { .. }
                            ) {
                                crate::selection::scalar_call_abi::integer_abi_normalization(
                                    scalar_type,
                                )
                            } else {
                                SelectedInstructionKind::CopyI64
                            },
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
                        if matches!(
                            scalar_type,
                            ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32)
                        ) && !matches!(value, IntegerValue::Unsigned(bits) if *bits <= u128::from(u32::MAX))
                            || matches!(
                                scalar_type,
                                ScalarType::IeeeFloat(
                                    semantic_vocabulary::IeeeFloatFormat::Binary64
                                )
                            ) && !matches!(value, IntegerValue::Unsigned(bits) if *bits <= u128::from(u64::MAX))
                        {
                            return Err(invalid());
                        }
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
                    LegalizedScalarInstructionKind::BitwiseAnd { left, right }
                    | LegalizedScalarInstructionKind::BitwiseXor { left, right } => {
                        let (_, left_register, _, left_type) =
                            replay.resolve(*left).ok_or_else(invalid)?;
                        let (_, right_register, _, right_type) =
                            replay.resolve(*right).ok_or_else(invalid)?;
                        if left_type != scalar_type
                            || right_type != scalar_type
                            || !matches!(scalar_type, ScalarType::Integer(integer)
                                if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                                && matches!(integer.bits(), 8 | 16 | 32 | 64))
                        {
                            return Err(invalid());
                        }
                        let output = replay.result_register(
                            result.value,
                            result.definition_site,
                            scalar_type,
                        )?;
                        replay.check_instruction(
                            if matches!(
                                operation.kind,
                                LegalizedScalarInstructionKind::BitwiseXor { .. }
                            ) {
                                SelectedInstructionKind::BitwiseXorI64
                            } else {
                                SelectedInstructionKind::BitwiseAndI64
                            },
                            constraints.keys.subtract_i64,
                            &[left_register, right_register, output],
                            &SelectedInstructionProvenance {
                                operations: vec![operation.operation],
                                values: vec![*left, *right, result.value],
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
                    LegalizedScalarInstructionKind::EstablishRecord { .. }
                    | LegalizedScalarInstructionKind::EstablishScalarCase { .. }
                    | LegalizedScalarInstructionKind::EstablishScalarArray { .. }
                    | LegalizedScalarInstructionKind::HostedExitProcessI32 { .. }
                    | LegalizedScalarInstructionKind::HostedWriteByteI32 { .. }
                    | LegalizedScalarInstructionKind::HostedReadByte { .. }
                    | LegalizedScalarInstructionKind::StructuralScalarFieldStore { .. }
                    | LegalizedScalarInstructionKind::EstablishPrimitiveLocal { .. }
                    | LegalizedScalarInstructionKind::PrimitiveLocalStore { .. }
                    | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { .. }
                    | LegalizedScalarInstructionKind::ByteSequenceWrite { .. }
                    | LegalizedScalarInstructionKind::BoundarySettlement(_)
                    | LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { .. }
                    | LegalizedScalarInstructionKind::ByteSequenceSubslice { .. } => {
                        return Err(invalid());
                    }
                    LegalizedScalarInstructionKind::Call(_) => {
                        scalar_call::validate(source, operation, &mut replay, environment, catalog)?
                    }
                }
            };
            replay
                .definitions
                .push((result.value, output, result.definition_site, scalar_type));
        }
        control::validate(source, source_block, &mut replay, environment, catalog)?;
        if !replay.pending_provenance.operations.is_empty()
            || replay.block_cursor != block.instructions.len()
        {
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
    super::def_use::validate_projected_def_use(function, selected, catalog)
}

struct Replay<'a> {
    // Zero-payload constructors retain their source position without storage.
    // The next instruction in this same block carries their ordered charges.
    pending_provenance: SelectedInstructionProvenance,
    required_values: std::collections::BTreeSet<ValueId>,
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
        self.check_register_class(self.class, site, scalar_type, origin, fixed)
    }

    fn check_register_class(
        &mut self,
        class: RegisterClassId,
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
            || register.class != class
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
        let settled = (!self.pending_provenance.operations.is_empty())
            .then(|| self.settle_provenance(provenance.clone()));
        let provenance = settled.as_ref().unwrap_or(provenance);
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
