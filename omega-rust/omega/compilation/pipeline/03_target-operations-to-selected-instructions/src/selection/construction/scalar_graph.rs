//! Optimizer module role: executable entrance.
//! Scalar instructions share one selection path regardless of the caller's result.

use crate::selection::constraints::{fixed_input_constraint, row};
use crate::selection::model::SelectedInstructionError;
use calling_conventions::ValueLocation;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstructionKind, SaturatingCarrier, TrappingForm,
    TrappingOperation,
};
use optimization_unit::ValueDefinitionSite;
use register_model::{
    RegisterClassId, RegisterConstraintKey, RegisterOperandAccess,
    ValidatedRegisterConstraintCatalog,
};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, MachineSemanticKind, SelectedBlock, SelectedBlockId,
    SelectedConstraintKeys, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedSelectionConstraints,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType, ValueId};

mod aggregate_argument;
mod aggregate_memory;
mod aggregate_return;
mod atomic_access;
mod boolean_value;
mod byte_input;
mod byte_output;
mod call_arguments;
mod cleanup_call;
mod control;
mod float_result_policy;
mod ieee_comparison;
mod ieee_float_arithmetic;
mod integer_arithmetic;
pub(in crate::selection) mod integer_conversion;
mod normalized_foreign;
mod placed_scalar_access;
mod process_exit;
mod register_access;
mod scalar_call;
mod scalar_stack;
mod structural;
mod structural_case;
mod unit_call;

/// The block whose instruction rows name `operation` — each argument
/// consumer used to re-walk `source.blocks` for the same find.
pub(super) fn block_containing_operation(
    source: &LegalizedScalarFunction,
    operation: semantic_vocabulary::OperationId,
) -> Result<semantic_vocabulary::BlockId, SelectedInstructionError> {
    source
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|row| row.operation == operation)
        })
        .map(|block| block.id)
        .ok_or_else(SelectedInstructionError::custody)
}

/// Select one legalized function. A shape refusal raised anywhere beneath is
/// attributed here to the function's machine and to the instruction being
/// selected at the time, so a diagnostic names the operation without a
/// reader re-walking the function by index.
pub(super) fn build_with_environment(
    function: usize,
    source: &LegalizedScalarFunction,
    constraints: &SelectedSelectionConstraints,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let mut current_operation = None;
    select_function(
        function,
        source,
        constraints,
        environment,
        &mut current_operation,
    )
    .map_err(|error| error.attributed(source.machine, current_operation))
}

fn select_function(
    function: usize,
    source: &LegalizedScalarFunction,
    constraints: &SelectedSelectionConstraints,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    current_operation: &mut Option<semantic_vocabulary::OperationId>,
) -> Result<SelectedFunction, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::unsupported_shape(function);
    let order = control::block_order(source)?;
    let catalog = environment.constraints();
    let materialize = row(catalog, constraints.keys.materialize_i64)?;
    let [operand] = materialize.operands.as_slice() else {
        return Err(invalid());
    };
    let class = operand.class;
    let mut builder = Builder {
        environment,
        pending_provenance: SelectedInstructionProvenance::default(),
        required_values: crate::selection::value_transport::required_values(source),
        class,
        constraints,
        catalog,
        registers: Vec::new(),
        pending_result_origins: Vec::new(),
        instructions: Vec::new(),
        case_blocks: Vec::new(),
        block_body_end: None,
        edge_work: Vec::new(),
        definitions: Vec::new(),
        transport: structural::Transport::default(),
    };
    structural::indirect_results::entry(source, environment, &mut builder)?;
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
                    u32::from(integer.bits() / 8)
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
        } else if crate::selection::scalar_call_abi::integer_carrier_normalization(scalar_type)
            != SelectedInstructionKind::CopyI64
        {
            let output = builder.register(value, site, scalar_type)?;
            builder.emit(
                crate::selection::scalar_call_abi::integer_carrier_normalization(scalar_type),
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
        structural::block_entry(source, block, &mut builder)?;
        for (operation_index, operation) in block.instructions.iter().enumerate() {
            *current_operation = Some(operation.operation);
            if byte_input::emit(operation, block_id, start, &mut builder)?
                || byte_output::emit(operation, block_id, start, &mut builder)?
            {
                continue;
            }
            if (crate::selection::literal_compare_input::folded_zero(
                source,
                block,
                operation_index + 1,
            )
            .is_some()
                || crate::selection::literal_compare_input::folded_immediate(
                    source,
                    block,
                    operation_index + 1,
                )
                .is_some())
                && control::branch_suffix(source, block, operation_index + 1)
            {
                continue;
            }
            if let LegalizedScalarInstructionKind::PortWrite { port, value, .. } = operation.kind {
                if operation.result.is_some() {
                    return Err(invalid());
                }
                let key = builder.constraints.keys.port_write.ok_or_else(invalid)?;
                builder.emit(
                    SelectedInstructionKind::PortWrite { port, value },
                    key,
                    &[],
                    SelectedInstructionProvenance {
                        operations: vec![operation.operation],
                        fuel: operation.fuel.clone(),
                        ..Default::default()
                    },
                )?;
                continue;
            }
            if let LegalizedScalarInstructionKind::AtomicFence { .. } = operation.kind {
                if operation.result.is_some() {
                    return Err(invalid());
                }
                let key = builder.constraints.keys.atomic_fence.ok_or_else(invalid)?;
                builder.emit(
                    SelectedInstructionKind::AtomicFence,
                    key,
                    &[],
                    SelectedInstructionProvenance {
                        operations: vec![operation.operation],
                        fuel: operation.fuel.clone(),
                        ..Default::default()
                    },
                )?;
                continue;
            }
            if register_access::emit(function, operation, &mut builder)? {
                continue;
            }
            if placed_scalar_access::emit(source, operation, &mut builder)? {
                continue;
            }
            if atomic_access::emit(function, source, operation, &mut builder)? {
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
                    LegalizedScalarInstructionKind::SelectImplementation { table } => {
                        if scalar_type != target_operations::selector_carrier_type() {
                            return Err(invalid());
                        }
                        let output =
                            builder.register(result.value, result.definition_site, scalar_type)?;
                        let key = builder
                            .constraints
                            .keys
                            .byte_literal_image_address
                            .ok_or_else(invalid)?;
                        builder.emit(
                            SelectedInstructionKind::ImmutableImageAddress {
                                reference: selected_instructions::SelectedImageAddressReference::ImplementationTable {
                                    table: *table,
                                },
                            },
                            key,
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
                    LegalizedScalarInstructionKind::PortWrite { .. }
                    | LegalizedScalarInstructionKind::RegisterAccess { .. }
                    | LegalizedScalarInstructionKind::AtomicFence { .. }
                    | LegalizedScalarInstructionKind::AtomicAccess { .. }
                    | LegalizedScalarInstructionKind::PlacedScalarAccess { .. } => {
                        return Err(invalid());
                    }
                    LegalizedScalarInstructionKind::IeeeFloatCompare { .. } => {
                        ieee_comparison::emit(operation, &mut builder)?
                    }
                    LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic { .. } => {
                        ieee_float_arithmetic::emit(operation, &mut builder)?
                    }
                    LegalizedScalarInstructionKind::FloatResultPolicy { .. } => {
                        float_result_policy::emit(operation, &mut builder)?
                    }
                    LegalizedScalarInstructionKind::StructuralCaseMembership { .. } => {
                        structural::observe(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::StructuralScalarFieldRead { .. }
                    | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                        ..
                    }
                    | LegalizedScalarInstructionKind::PrimitiveScalarRead { .. } => {
                        structural::read(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::ByteSequenceRead { .. }
                    | LegalizedScalarInstructionKind::ByteSequenceLength { .. } => {
                        structural::byte_observation(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::ByteSequencePredicate { .. } => {
                        structural::byte_predicate(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::ByteSequenceEqual { .. } => {
                        structural::byte_equal(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead { .. } => {
                        structural::byte_field_read(source, &mut builder, operation)?
                    }
                    LegalizedScalarInstructionKind::ElementViewRead { .. }
                    | LegalizedScalarInstructionKind::ElementViewLength { .. } => {
                        structural::element_observation(source, &mut builder, operation)?
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
                    | LegalizedScalarInstructionKind::IntegerExactCast { .. }
                    | LegalizedScalarInstructionKind::ScalarRepresentationView { .. } => {
                        integer_conversion::emit(operation, &mut builder, function)?
                    }
                    LegalizedScalarInstructionKind::BitwiseAnd { .. }
                    | LegalizedScalarInstructionKind::BitwiseNot { .. }
                    | LegalizedScalarInstructionKind::BitwiseOr { .. }
                    | LegalizedScalarInstructionKind::BitwiseXor { .. }
                    | LegalizedScalarInstructionKind::Constant(_)
                    | LegalizedScalarInstructionKind::ExactBinary { .. }
                    | LegalizedScalarInstructionKind::ExactShiftLeft { .. }
                    | LegalizedScalarInstructionKind::ExactShiftRight { .. }
                    | LegalizedScalarInstructionKind::SaturatingAdd { .. }
                    | LegalizedScalarInstructionKind::SaturatingDivide { .. }
                    | LegalizedScalarInstructionKind::SaturatingMultiply { .. }
                    | LegalizedScalarInstructionKind::SaturatingRemainder { .. }
                    | LegalizedScalarInstructionKind::SaturatingSubtract { .. }
                    | LegalizedScalarInstructionKind::TrappingBinary { .. }
                    | LegalizedScalarInstructionKind::TrappingConvert { .. }
                    | LegalizedScalarInstructionKind::WrappingAdd { .. }
                    | LegalizedScalarInstructionKind::WrappingDivide { .. }
                    | LegalizedScalarInstructionKind::WrappingMultiply { .. }
                    | LegalizedScalarInstructionKind::WrappingRemainder { .. }
                    | LegalizedScalarInstructionKind::WrappingShiftLeft { .. }
                    | LegalizedScalarInstructionKind::WrappingShiftRight { .. }
                    | LegalizedScalarInstructionKind::WrappingSubtract { .. } => {
                        integer_arithmetic::emit(function, environment, operation, &mut builder)?
                    }
                    LegalizedScalarInstructionKind::EstablishRecord { .. }
                    | LegalizedScalarInstructionKind::EstablishCase { .. }
                    | LegalizedScalarInstructionKind::EstablishStructuralArray { .. }
                    | LegalizedScalarInstructionKind::EstablishScalarArray { .. }
                    | LegalizedScalarInstructionKind::EstablishZeroedStructural { .. }
                    | LegalizedScalarInstructionKind::EstablishStructuralLocal { .. }
                    | LegalizedScalarInstructionKind::StructuralLeafCopy { .. }
                    | LegalizedScalarInstructionKind::StoreStructuralField { .. }
                    | LegalizedScalarInstructionKind::EstablishTrivialAffineLocal { .. }
                    | LegalizedScalarInstructionKind::EstablishReference { .. }
                    | LegalizedScalarInstructionKind::EstablishPlacedReference { .. }
                    | LegalizedScalarInstructionKind::ReleaseReference { .. }
                    | LegalizedScalarInstructionKind::HostedWriteByteI32 { .. }
                    | LegalizedScalarInstructionKind::HostedReadByte { .. }
                    | LegalizedScalarInstructionKind::IeeeFloatClassify { .. }
                    | LegalizedScalarInstructionKind::StructuralScalarFieldStore { .. }
                    | LegalizedScalarInstructionKind::EstablishPrimitiveLocal { .. }
                    | LegalizedScalarInstructionKind::PrimitiveLocalStore { .. }
                    | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { .. }
                    | LegalizedScalarInstructionKind::ByteSequenceWrite { .. }
                    | LegalizedScalarInstructionKind::ElementViewWrite { .. }
                    | LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore {
                        ..
                    }
                    | LegalizedScalarInstructionKind::ByteSequenceStore { .. }
                    | LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore { .. }
                    | LegalizedScalarInstructionKind::StructuralByteViewFieldStore { .. }
                    | LegalizedScalarInstructionKind::BoundarySettlement(_)
                    | LegalizedScalarInstructionKind::NormalizedForeignCall(_)
                    | LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
                        ..
                    }
                    | LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { .. }
                    | LegalizedScalarInstructionKind::ByteSequenceSubslice { .. }
                    | LegalizedScalarInstructionKind::EstablishElementView { .. }
                    | LegalizedScalarInstructionKind::ElementViewSubslice { .. } => {
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
        *current_operation = None;
        let terminator = if let Some(exited) = process_exit::build(
            function,
            source,
            block,
            block_id,
            start,
            &mut builder,
            environment,
        )? {
            exited
        } else {
            control::build(function, source, block, &order, &mut builder, environment)?
        };
        if !builder.pending_provenance.operations.is_empty()
            || !builder.pending_result_origins.is_empty()
        {
            return Err(invalid());
        }
        let body_end = builder.block_body_end.take().unwrap_or(
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
        // Calls retain the ordinary receiver-home and ABI contracts, but are
        // not predecessor instructions: only the selected successor runs them.
        let effect = match &block.terminator {
            legalized_operations::LegalizedScalarTerminator::Jump { effect, .. }
            | legalized_operations::LegalizedScalarTerminator::Conditional { effect, .. }
            | legalized_operations::LegalizedScalarTerminator::StructuralCase { effect, .. } => {
                Some(*effect)
            }
            _ => None,
        };
        if let Some(effect) = effect {
            for (owner, call) in block.terminator.cleanup_calls() {
                let edge = owner.edge().ok_or_else(invalid)?;
                let start = builder.instructions.len();
                cleanup_call::emit(*owner, call, effect, &mut builder, environment)?;
                let instructions = builder.instructions[start..].to_vec();
                builder.edge_work_at(edge)?.cleanup.extend(instructions);
            }
        }
    }
    blocks.append(&mut builder.case_blocks);
    crate::selection::returned_view_input::replay_memory_origins(
        source,
        &mut builder.transport.memory,
    )?;
    let mut selected = SelectedFunction {
        machine: source.machine,
        attachment: source.attachment,
        nominal_cleanup_subject: source.nominal_cleanup_subject,
        provenance: source.provenance.clone(),
        structural: source.structural.clone(),
        outgoing_arguments: builder.transport.slots,
        local_storage_slots: builder.transport.local_slots,
        calls: builder.transport.calls,
        normalized_foreign_calls: builder.transport.normalized_foreign_calls,
        memory_accesses: builder.transport.memory,
        boundary_settlements: builder.transport.settlements,
        entry_block: SelectedBlockId(0),
        virtual_registers: builder.registers,
        blocks,
    };
    crate::selection::edge_transfers::prepare(
        function,
        &mut selected,
        constraints,
        catalog,
        environment,
        builder.edge_work,
    )?;
    Ok(selected)
}

struct Builder<'a> {
    environment: &'a register_environment::ValidatedTargetRegisterEnvironment,
    // Zero-payload constructors retain their source position without storage.
    // The next instruction in this same block carries their ordered charges.
    pending_provenance: SelectedInstructionProvenance,
    required_values: std::collections::BTreeSet<ValueId>,
    transport: structural::Transport,
    class: RegisterClassId,
    constraints: &'a SelectedSelectionConstraints,
    catalog: &'a ValidatedRegisterConstraintCatalog,
    registers: Vec<VirtualRegister>,
    // Reserving a result does not select its defining instruction: operand
    // copies and scratch materialization can precede that definition.
    pending_result_origins: Vec<VirtualRegisterId>,
    instructions: Vec<SelectedInstruction>,
    case_blocks: Vec<SelectedBlock>,
    block_body_end: Option<usize>,
    edge_work: Vec<crate::selection::edge_transfers::EdgeWork>,
    definitions: Vec<(ValueId, VirtualRegisterId, ValueDefinitionSite, ScalarType)>,
}

impl Builder<'_> {
    fn edge_work_at(
        &mut self,
        edge: semantic_vocabulary::EdgeId,
    ) -> Result<&mut crate::selection::edge_transfers::EdgeWork, SelectedInstructionError> {
        let position = self
            .edge_work
            .iter()
            .position(|work| work.edge == edge)
            .unwrap_or_else(|| {
                let position = self.edge_work.len();
                self.edge_work
                    .push(crate::selection::edge_transfers::EdgeWork {
                        edge,
                        preparation: Vec::new(),
                        cleanup: Vec::new(),
                    });
                position
            });
        self.edge_work
            .get_mut(position)
            .ok_or_else(SelectedInstructionError::custody)
    }

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
                .map_err(|_| SelectedInstructionError::custody())?,
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
                        .map_err(|_| SelectedInstructionError::custody())?,
                ),
                source_value: value,
            },
            definition_site: Some(site),
            entry_fixed_view: None,
        });
        self.pending_result_origins.push(id);
        Ok(id)
    }

    /// Every zero-offset frame-address emission shares the kind and its
    /// constraint-key lookup; only the slot identity, result register, and
    /// provenance differ.
    fn emit_frame_address(
        &mut self,
        slot: FrameStorageSlotId,
        pointer: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::FrameAddress {
                slot,
                byte_offset: 0,
            },
            self.constraints
                .keys
                .frame_address
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[pointer],
            provenance,
        )
    }

    /// Every zero-offset `Local(slot)` frame-address emission shares the kind
    /// and its constraint-key lookup.
    fn emit_local_frame_address(
        &mut self,
        slot: LocalStorageSlotId,
        pointer: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit_frame_address(FrameStorageSlotId::Local(slot), pointer, provenance)
    }

    /// Every `AddressOffset` projection shares the kind and its
    /// constraint-key lookup; only the offset, operand pair, and provenance
    /// differ.
    fn emit_address_offset(
        &mut self,
        byte_offset: u32,
        source_pointer: VirtualRegisterId,
        projected_pointer: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::AddressOffset { byte_offset },
            self.constraints
                .keys
                .address_offset
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[source_pointer, projected_pointer],
            provenance,
        )
    }

    /// Every `Load64` emission shares the kind and its constraint-key lookup;
    /// only the offset, operand pair, and provenance differ.
    fn emit_load64(
        &mut self,
        byte_offset: u32,
        source_pointer: VirtualRegisterId,
        loaded: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::Load64 { byte_offset },
            self.constraints
                .keys
                .load64
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[source_pointer, loaded],
            provenance,
        )
    }

    /// Every `MaterializeI64` emission shares the kind and its constraint-key
    /// lookup; only the literal, destination register, and provenance differ.
    fn emit_materialize_i64(
        &mut self,
        value: semantic_vocabulary::IntegerValue,
        destination: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::MaterializeI64 { value },
            self.constraints.keys.materialize_i64,
            &[destination],
            provenance,
        )
    }

    /// Every `CopyI64` emission shares the kind and its constraint-key
    /// lookup; only the operand pair and provenance differ.
    fn emit_copy_i64(
        &mut self,
        source: VirtualRegisterId,
        destination: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::CopyI64,
            self.constraints.keys.copy_i64,
            &[source, destination],
            provenance,
        )
    }

    /// Every `ByteViewAddress` emission joins a base and an index through the
    /// add constraint; only the operands and provenance differ.
    fn emit_byte_view_address(
        &mut self,
        base: VirtualRegisterId,
        index: VirtualRegisterId,
        destination: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::ByteViewAddress,
            self.constraints.keys.add_i64,
            &[base, index, destination],
            provenance,
        )
    }

    /// Every `Store64` emission shares the kind and its constraint-key
    /// lookup; only the slot, offset, stored register, and provenance differ.
    fn emit_store64(
        &mut self,
        slot: FrameStorageSlotId,
        byte_offset: u32,
        value: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::Store64 { slot, byte_offset },
            self.constraints
                .keys
                .store64
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[value],
            provenance,
        )
    }

    /// Every `Store` emission shares the kind and its constraint-key lookup;
    /// call sites differ only in offset, width, the operand pair, and the
    /// provenance they attach.
    fn emit_store(
        &mut self,
        byte_offset: u32,
        byte_size: u8,
        pointer: VirtualRegisterId,
        value: VirtualRegisterId,
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        self.emit(
            SelectedInstructionKind::Store {
                byte_offset,
                byte_size,
            },
            self.constraints
                .keys
                .store
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[pointer, value],
            provenance,
        )
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
                .map_err(|_| SelectedInstructionError::custody())?,
        );
        let provenance = self.settle_provenance(provenance);
        let constraint =
            if let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = kind {
                let record = self
                    .transport
                    .normalized_foreign_calls
                    .get(ordinal as usize)
                    .filter(|record| record.instruction == id && record.call.boundary == boundary)
                    .ok_or(SelectedInstructionError::custody())?;
                std::borrow::Cow::Owned(
                    super::super::scalar_call_abi::normalized_foreign::call_constraint(
                        &record.call,
                        self.environment,
                    )
                    .ok_or(SelectedInstructionError::custody())?,
                )
            } else if matches!(
                kind.call_target(),
                Some(selected_instructions::SelectedCallTarget::Dynamic { .. })
            ) {
                std::borrow::Cow::Owned(
                    selected_instructions::calls::dynamic::instantiate_constraint(
                        row(self.catalog, key)?,
                        self.environment.physical(),
                    )
                    .ok_or(SelectedInstructionError::custody())?,
                )
            } else {
                std::borrow::Cow::Borrowed(row(self.catalog, key)?)
            };
        if constraint.key != key {
            return Err(SelectedInstructionError::custody());
        }
        let store_operands;
        let operands = if matches!(kind, SelectedInstructionKind::Store64 { .. }) {
            store_operands = super::super::constraints::store_registers(
                id,
                operands,
                &constraint,
                &mut self.registers,
            )?;
            store_operands.as_slice()
        } else {
            operands
        };
        let instruction = super::super::constraints::instruction_from_row(
            id,
            kind,
            operands,
            provenance,
            &constraint,
        )?;
        for operand in &instruction.operands {
            if !matches!(
                operand.access,
                RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
            ) {
                continue;
            }
            // Encoding-owned definitions belong to this instruction, even
            // when input preparation inserted copies after scratch allocation.
            // A separately materialized division scratch is a Use here and
            // therefore keeps its materialization producer.
            if let VirtualRegisterOrigin::InstructionScratch {
                instruction: definition,
                ..
            } = &mut self.registers[operand.virtual_register.0 as usize].origin
            {
                *definition = id;
            }
            let Some(position) = self
                .pending_result_origins
                .iter()
                .position(|register| *register == operand.virtual_register)
            else {
                continue;
            };
            let register = &mut self.registers[operand.virtual_register.0 as usize];
            let VirtualRegisterOrigin::InstructionResult {
                instruction: definition,
                ..
            } = &mut register.origin
            else {
                return Err(SelectedInstructionError::custody());
            };
            *definition = id;
            self.pending_result_origins.swap_remove(position);
        }
        self.instructions.push(instruction);
        Ok(())
    }

    /// Fixed integer inputs constrain this instruction's observation, not the
    /// lifetime of its source value. Give each pinned use its own ordinary
    /// carrier copy, so a source can survive destructive fixed outputs or be
    /// forwarded to another block. The original result is still established
    /// by the actual operation after the copies; `emit` rejoins its first Def.
    fn emit_integer(
        &mut self,
        kind: SelectedInstructionKind,
        key: register_model::RegisterConstraintKey,
        operands: &[VirtualRegisterId],
        provenance: SelectedInstructionProvenance,
    ) -> Result<(), SelectedInstructionError> {
        let pinned = row(self.catalog, key)?
            .operands
            .iter()
            .filter(|operand| {
                operand.access == RegisterOperandAccess::Use && operand.fixed_view.is_some()
            })
            .map(|operand| usize::from(operand.operand))
            .collect::<Vec<_>>();
        let mut observed = operands.to_vec();
        for position in pinned {
            let input = *observed
                .get(position)
                .ok_or_else(SelectedInstructionError::custody)?;
            let register = self
                .registers
                .get(input.0 as usize)
                .ok_or_else(SelectedInstructionError::custody)?;
            let value = match register.origin {
                // An encoding-owned scratch already has a bounded producer
                // and use; it is not a source value with a surviving lifetime.
                VirtualRegisterOrigin::InstructionScratch { .. } => continue,
                VirtualRegisterOrigin::EntryParameter { source_value, .. }
                | VirtualRegisterOrigin::BlockParameter { source_value, .. }
                | VirtualRegisterOrigin::InstructionResult { source_value, .. } => source_value,
                _ => return Err(SelectedInstructionError::custody()),
            };
            let site = register
                .definition_site
                .ok_or_else(SelectedInstructionError::custody)?;
            let scalar_type = register.scalar_type;
            observed[position] = self.copy(input, value, site, scalar_type)?;
        }
        self.emit(kind, key, &observed, provenance)
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
