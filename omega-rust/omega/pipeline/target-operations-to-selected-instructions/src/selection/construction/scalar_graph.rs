//! Optimizer module role: executable entrance.
//! Scalar instructions share one selection path regardless of the caller's result.

use crate::selection::constraints::{fixed_input_constraint, instruction, row};
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
mod integer_conversion;
mod process_exit;
mod scalar_call;
mod scalar_stack;
mod structural;
mod structural_case;
mod unit_call;
mod zero_compare;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) fn build(
    function: usize,
    source: &LegalizedScalarFunction,
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
    build_with_environment(function, source, constraints, &environment)
}

pub(super) fn build_with_environment(
    function: usize,
    source: &LegalizedScalarFunction,
    constraints: &SelectedSelectionConstraints,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
    let order = control::block_order(source)?;
    let catalog = environment.constraints();
    let materialize = row(catalog, constraints.keys.materialize_i64)?;
    let [operand] = materialize.operands.as_slice() else {
        return Err(invalid());
    };
    let class = operand.class;
    let mut builder = Builder {
        pending_provenance: SelectedInstructionProvenance::default(),
        required_values: crate::selection::value_transport::required_values(source),
        class,
        constraints,
        catalog,
        registers: Vec::new(),
        instructions: Vec::new(),
        case_blocks: Vec::new(),
        case_body_end: None,
        definitions: Vec::new(),
        transport: structural::Transport::default(),
    };
    structural::entry(function, source, environment, &mut builder)?;
    // Entry ABI precoloring ends at a copy. The semantic parameter may remain
    // live across calls without being pinned to a caller-clobbered register.
    for (index, parameter) in source.parameters.iter().enumerate() {
        if crate::selection::scalar_call_abi::scalar_stack_placement(&parameter.placement).is_some()
        {
            continue;
        }
        if !builder.required_values.contains(&parameter.value) {
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
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => 4,
                ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => 8,
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
        let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
        let parameter_class = if let Some((_, key)) =
            crate::selection::scalar_call_abi::incoming_float_transfer(
                parameter.scalar_type,
                &constraints.keys,
            ) {
            row(catalog, key)?
                .operands
                .first()
                .ok_or_else(invalid)?
                .class
        } else {
            class
        };
        builder.registers.push(VirtualRegister {
            id,
            scalar_type: parameter.scalar_type,
            class: parameter_class,
            origin: VirtualRegisterOrigin::EntryParameter {
                source_value: parameter.value,
                parameter_index: index,
            },
            definition_site: Some(parameter.definition_site),
            entry_fixed_view: Some(fixed.fixed_view),
        });
        builder.definitions.push((
            parameter.value,
            id,
            parameter.definition_site,
            parameter.scalar_type,
        ));
    }
    for index in 0..builder.definitions.len() {
        let (value, input, site, scalar_type) = builder.definitions[index];
        let output = if let Some((kind, key)) =
            crate::selection::scalar_call_abi::incoming_float_transfer(
                scalar_type,
                &constraints.keys,
            ) {
            let output = builder.register(value, site, scalar_type)?;
            builder.emit(
                kind,
                key,
                &[input, output],
                SelectedInstructionProvenance {
                    values: vec![value],
                    ..Default::default()
                },
            )?;
            output
        } else if crate::selection::scalar_call_abi::integer_abi_normalization(scalar_type)
            != SelectedInstructionKind::CopyI64
        {
            let output = builder.register(value, site, scalar_type)?;
            builder.emit(
                crate::selection::scalar_call_abi::integer_abi_normalization(scalar_type),
                constraints.keys.copy_i64,
                &[input, output],
                SelectedInstructionProvenance {
                    values: vec![value],
                    ..Default::default()
                },
            )?;
            output
        } else {
            builder.copy(input, value, site, scalar_type)?
        };
        builder.definitions[index].1 = output;
    }
    scalar_stack::entry(source, &mut builder)?;
    // Forward successors name their materialized destination parameters explicitly.
    for (block_index, source_index) in order.iter().copied().enumerate() {
        let block = &source.blocks[source_index];
        let block_id = SelectedBlockId(u32::try_from(block_index).map_err(|_| invalid())?);
        for (parameter_index, parameter) in block.parameters.iter().enumerate() {
            if !builder.required_values.contains(&parameter.value) {
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
                definition_site: Some(parameter.site),
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
        if !crate::unobserved_owned_input::accepts(source) {
            structural::block_entry(source, block, &mut builder)?;
        }
        for (operation_index, operation) in block.instructions.iter().enumerate() {
            if matches!(
                operation.kind,
                LegalizedScalarInstructionKind::HostedExitProcessI32 { .. }
            ) {
                if operation_index + 1 != block.instructions.len() {
                    return Err(invalid());
                }
                continue;
            }
            if byte_input::emit(operation, block_id, start, &mut builder)?
                || byte_output::emit(operation, block_id, start, &mut builder)?
            {
                continue;
            }
            if zero_compare::folded_zero(source, block, operation_index + 1).is_some()
                && control::branch_suffix(source, block, operation_index + 1)
            {
                continue;
            }
            if structural::operation(
                function,
                source,
                block_id,
                start,
                operation,
                environment,
                &mut builder,
            )? {
                continue;
            }
            let result = operation.result.ok_or_else(invalid)?;
            let scalar_type = result.scalar_type;
            let output = if matches!(
                operation.kind,
                LegalizedScalarInstructionKind::Compare { .. }
                    | LegalizedScalarInstructionKind::BooleanNot { .. }
            ) && !control::branch_suffix(source, block, operation_index)
            {
                boolean_value::emit(operation, &mut builder)?
            } else {
                match &operation.kind {
                    LegalizedScalarInstructionKind::IeeeFloatCompare { .. } => {
                        ieee_comparison::emit(operation, &mut builder)?
                    }
                    LegalizedScalarInstructionKind::StructuralCaseMembership { .. } => {
                        structural::observe(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::StructuralScalarFieldRead { .. }
                    | LegalizedScalarInstructionKind::PrimitiveScalarRead { .. } => {
                        structural::read(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::ByteSequenceRead { .. }
                    | LegalizedScalarInstructionKind::ByteSequenceLength { .. } => {
                        structural::byte_observation(&mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::Compare { .. } => {
                        boolean_value::emit_branch_comparison(
                            function,
                            source,
                            block,
                            operation_index,
                            &mut builder,
                        )?;
                        continue;
                    }
                    LegalizedScalarInstructionKind::BooleanNot { .. } => {
                        if !control::branch_suffix(source, block, operation_index) {
                            return Err(invalid());
                        }
                        continue;
                    }
                    LegalizedScalarInstructionKind::IntegerWiden { .. }
                    | LegalizedScalarInstructionKind::IntegerExactCast { .. } => {
                        integer_conversion::emit(operation, &mut builder, function)?
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
                        let output =
                            builder.register(result.value, result.definition_site, scalar_type)?;
                        builder.emit(
                            SelectedInstructionKind::MaterializeI64 { value: *value },
                            constraints.keys.materialize_i64,
                            &[output],
                            SelectedInstructionProvenance {
                                operations: vec![operation.operation],
                                values: vec![result.value],
                                fuel: operation.fuel.clone(),
                                ..Default::default()
                            },
                        )?;
                        output
                    }
                    LegalizedScalarInstructionKind::BitwiseAnd { left, right }
                    | LegalizedScalarInstructionKind::BitwiseXor { left, right } => {
                        let (_, left_register, _, left_type) =
                            builder.resolve(*left).ok_or_else(invalid)?;
                        let (_, right_register, _, right_type) =
                            builder.resolve(*right).ok_or_else(invalid)?;
                        if left_type != scalar_type
                            || right_type != scalar_type
                            || !matches!(scalar_type, ScalarType::Integer(integer)
                                if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                                && matches!(integer.bits(), 8 | 16 | 32 | 64))
                        {
                            return Err(invalid());
                        }
                        let output =
                            builder.register(result.value, result.definition_site, scalar_type)?;
                        // Subtraction shares operand constraints and conservatively models
                        // x64 flag clobbers; the distinct bitwise form carries the semantics.
                        builder.emit(
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
                            SelectedInstructionProvenance {
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
                        let output =
                            builder.register(result.value, result.definition_site, scalar_type)?;
                        builder.emit(
                            kind,
                            key,
                            &[left_register, right_register, output],
                            SelectedInstructionProvenance {
                                operations: vec![operation.operation],
                                values: vec![*left, *right, result.value],
                                obligations: vec![*obligation],
                                fuel: operation.fuel.clone(),
                                ..Default::default()
                            },
                        )?;
                        output
                    }
                    LegalizedScalarInstructionKind::EstablishScalarCase { .. }
                    | LegalizedScalarInstructionKind::EstablishScalarRecord { .. }
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
                        scalar_call::emit(function, source, operation, environment, &mut builder)?
                    }
                }
            };
            builder
                .definitions
                .push((result.value, output, result.definition_site, scalar_type));
        }
        let terminator =
            if let Some(exited) = process_exit::build(block, block_id, start, &mut builder)? {
                exited
            } else {
                control::build(function, source, block, &order, &mut builder, environment)?
            };
        if !builder.pending_provenance.operations.is_empty() {
            return Err(invalid());
        }
        let body_end = builder.case_body_end.take().unwrap_or(
            builder
                .instructions
                .len()
                .checked_sub(1)
                .ok_or_else(invalid)?,
        );
        blocks.push(SelectedBlock {
            id: block_id,
            origin: selected_instructions::SelectedBlockOrigin::Source(block.id),
            instructions: builder.instructions[start..body_end].to_vec(),
            terminator,
        });
    }
    blocks.append(&mut builder.case_blocks);
    Ok(SelectedFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
        structural: source.structural.clone(),
        outgoing_arguments: builder.transport.slots,
        local_storage_slots: builder.transport.local_slots,
        calls: builder.transport.calls,
        memory_accesses: builder.transport.memory,
        boundary_settlements: builder.transport.settlements,
        entry_block: SelectedBlockId(0),
        virtual_registers: builder.registers,
        blocks,
    })
}

struct Builder<'a> {
    // Zero-payload constructors retain their source position without storage.
    // The next instruction in this same block carries their ordered charges.
    pending_provenance: SelectedInstructionProvenance,
    required_values: std::collections::BTreeSet<ValueId>,
    transport: structural::Transport,
    class: RegisterClassId,
    constraints: &'a SelectedSelectionConstraints,
    catalog: &'a ValidatedRegisterConstraintCatalog,
    registers: Vec<VirtualRegister>,
    instructions: Vec<SelectedInstruction>,
    case_blocks: Vec<SelectedBlock>,
    case_body_end: Option<usize>,
    definitions: Vec<(ValueId, VirtualRegisterId, ValueDefinitionSite, ScalarType)>,
}

impl Builder<'_> {
    fn settle_provenance(
        &mut self,
        provenance: SelectedInstructionProvenance,
    ) -> SelectedInstructionProvenance {
        if self.pending_provenance.operations.is_empty() {
            return provenance;
        }
        let mut pending = std::mem::take(&mut self.pending_provenance);
        pending.operations.extend(provenance.operations);
        pending.values.extend(provenance.values);
        pending.edges.extend(provenance.edges);
        pending.obligations.extend(provenance.obligations);
        pending.fuel.extend(provenance.fuel);
        pending
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
            definition_site: Some(site),
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
        let provenance = self.settle_provenance(provenance);
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
