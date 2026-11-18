//! Close foreign-call transport while physical planning owns instruction and
//! register origins. Later publication resolves the carried span obligations.
use ForeignCallPlanError as ObjectError;
use calling_conventions::ValueLocation;
use machine_code::foreign_call_plans::*;
use semantic_vocabulary::{MachineId, ScalarType};
use target_operations::CallSiteOwner;

mod descriptor_input {
    use selected_instructions::{
        SelectedBlock, SelectedFunction, SelectedInstructionKind as Kind,
        SelectedNormalizedForeignCall, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{IntegerValue, ScalarType};

    pub(super) fn validate(
        function: &SelectedFunction,
        block: &SelectedBlock,
        record: &SelectedNormalizedForeignCall,
    ) -> Option<()> {
        let Some(guard) = record.call.descriptor_input else {
            return Some(());
        };
        let operation = record.invocation_site.operation()?;
        let count = record
            .call
            .scalar_arguments
            .get(guard.count_scalar_argument)?
            .source
            .source_value();
        let call_position = block.instruction_index(record.instruction)?;
        let form = legalized_operations::TrappingForm {
            operation: legalized_operations::TrappingOperation::Subtract,
            carrier: legalized_operations::SaturatingCarrier::from_integer(guard.count_type)?,
        };
        let mut guards = block.instructions[..call_position]
            .iter()
            .enumerate()
            .filter(|(_, instruction)| {
                instruction.kind == (Kind::TrappingInteger { form })
                    && instruction.provenance.operations.as_slice() == [operation]
                    && instruction.provenance.values.as_slice() == [count]
            });
        let (guard_position, instruction) = guards.next()?;
        if guards.next().is_some() || !instruction.provenance.work.is_empty() {
            return None;
        }
        let [bound, input, output, scratch] = instruction.operands.as_slice() else {
            return None;
        };
        let input = function
            .virtual_registers
            .get(input.virtual_register.0 as usize)?;
        if input.scalar_type != ScalarType::Integer(guard.count_type)
            || !matches!(input.origin,
                VirtualRegisterOrigin::EntryParameter { source_value, .. }
                | VirtualRegisterOrigin::BlockParameter { source_value, .. }
                | VirtualRegisterOrigin::InstructionResult { source_value, .. } if source_value == count)
        {
            return None;
        }
        let bound_register = function
            .virtual_registers
            .get(bound.virtual_register.0 as usize)?;
        let VirtualRegisterOrigin::InstructionScratch {
            instruction: materialize,
            operand: 0,
        } = bound_register.origin
        else {
            return None;
        };
        let materialize_position = block.instruction_index(materialize)?;
        let materialize = block.instructions.get(materialize_position)?;
        if materialize_position >= guard_position
            || materialize.kind
                != (Kind::MaterializeI64 {
                    value: IntegerValue::Unsigned(u128::from(guard.max_bytes)),
                })
            || materialize.provenance.operations.as_slice() != [operation]
            || materialize.provenance.values.as_slice() != [count]
            || !materialize.provenance.work.is_empty()
            || materialize.operands.as_slice().first()?.virtual_register != bound.virtual_register
            || materialize.operands.len() != 1
            || bound_register.scalar_type != ScalarType::Integer(guard.count_type)
        {
            return None;
        }
        for (operand, expected) in [(output, 2), (scratch, 3)] {
            if function
                .virtual_registers
                .get(operand.virtual_register.0 as usize)?
                .origin
                != (VirtualRegisterOrigin::InstructionScratch {
                    instruction: instruction.id,
                    operand: expected,
                })
            {
                return None;
            }
        }
        Some(())
    }
}

pub fn unique_block_instruction(
    function: &selected_instructions::SelectedFunction,
    instruction: selected_instructions::SelectedInstructionId,
) -> Option<(
    &selected_instructions::SelectedBlock,
    &selected_instructions::SelectedInstruction,
)> {
    let mut instructions = function.blocks.iter().flat_map(|block| {
        block
            .instructions
            .iter()
            .filter(move |candidate| candidate.id == instruction)
            .map(move |candidate| (block, candidate))
    });
    let (Some(pair), None) = (instructions.next(), instructions.next()) else {
        return None;
    };
    Some(pair)
}

pub fn durable_home_offsets(
    function: &selected_instructions::SelectedFunction,
) -> Vec<(semantic_vocabulary::OperationId, u32)> {
    let mut producers = Vec::new();
    for contract in &function.calls {
        if let Some(placement) = &contract.call.result_placement
            && let target_operations::CallSiteOwner::Operation(operation) = contract.owner
        {
            producers.push((contract.instruction, operation, placement.shape.byte_size));
        }
    }
    for record in &function.normalized_foreign_calls {
        if let Some(home) = record.call.result.scalar_home() {
            producers.push((
                record.instruction,
                home.defining_operation,
                home.shape.byte_size,
            ));
        }
        for argument in &record.call.scalar_arguments {
            if let target_operations::TargetUnitScalarArgumentSource::Home(requirement) =
                argument.source
            {
                let Some(instruction) = producer_instruction(
                    function,
                    requirement.defining_operation,
                    requirement.source_value,
                ) else {
                    continue;
                };
                producers.push((
                    instruction,
                    requirement.defining_operation,
                    requirement.shape.byte_size,
                ));
            }
        }
    }
    producers.sort_by_key(|(instruction, _, _)| instruction.0);
    producers.dedup_by_key(|(_, operation, _)| *operation);
    let mut cursor = 0u32;
    let mut offsets = Vec::new();
    for (_, operation, byte_size) in producers {
        let Some(aligned) = cursor.checked_add(7).map(|cursor| cursor & !7) else {
            return Vec::new();
        };
        cursor = aligned;
        offsets.push((operation, cursor));
        let Some(next) = cursor.checked_add(byte_size) else {
            return Vec::new();
        };
        cursor = next;
    }
    offsets
}

pub fn producer_instruction(
    function: &selected_instructions::SelectedFunction,
    operation: semantic_vocabulary::OperationId,
    source_value: semantic_vocabulary::ValueId,
) -> Option<selected_instructions::SelectedInstructionId> {
    function
        .calls
        .iter()
        .filter_map(|contract| {
            contract
                .owner
                .operation()
                .map(|operation| (operation, contract.instruction))
        })
        .chain(
            function
                .normalized_foreign_calls
                .iter()
                .filter_map(|record| {
                    record
                        .invocation_site
                        .operation()
                        .map(|operation| (operation, record.instruction))
                }),
        )
        .find(|(candidate, _)| *candidate == operation)
        .map(|(_, instruction)| instruction)
        .or_else(|| scalar_observation_producer(function, operation, source_value))
}

pub(crate) fn scalar_observation_producer(
    function: &selected_instructions::SelectedFunction,
    operation: semantic_vocabulary::OperationId,
    source_value: semantic_vocabulary::ValueId,
) -> Option<selected_instructions::SelectedInstructionId> {
    let mut producers = function.instructions().filter(|instruction| {
        instruction.provenance.operations.contains(&operation)
            && instruction.operands.iter().any(|operand| {
                function
                    .virtual_registers
                    .get(operand.virtual_register.0 as usize)
                    .is_some_and(|register| {
                        register.origin
                            == selected_instructions::VirtualRegisterOrigin::InstructionResult {
                                instruction: instruction.id,
                                source_value,
                            }
                    })
            })
    });
    let producer = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    Some(producer.id)
}

pub fn foreign_result_operand_count(
    record: &selected_instructions::SelectedNormalizedForeignCall,
) -> Option<usize> {
    match (
        &record.call.result,
        &record.call.binding.boundary_entry_plan.call.result,
    ) {
        (target_operations::TargetBoundaryResult::Unit, None) => Some(0),
        (target_operations::TargetBoundaryResult::Scalar(_), Some(_)) => Some(1),
        (target_operations::TargetBoundaryResult::Structural(_), Some(placement)) => {
            match placement.locations.as_slice() {
                [
                    ValueLocation::Indirect {
                        pointer: calling_conventions::IndirectPointerLocation::Register(_),
                        ..
                    },
                ] => Some(1),
                locations
                    if !locations.is_empty()
                        && locations
                            .iter()
                            .all(|location| matches!(location, ValueLocation::Register { .. })) =>
                {
                    Some(locations.len())
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn scalar_result_normalization_kind(
    scalar_type: ScalarType,
    indirect: bool,
) -> Option<selected_instructions::SelectedInstructionKind> {
    use selected_instructions::SelectedInstructionKind as Kind;
    use semantic_vocabulary::IntegerSign;
    if indirect && matches!(scalar_type, ScalarType::IeeeFloat(_)) {
        return Some(Kind::CopyI64);
    }
    match scalar_type {
        ScalarType::Callable(_) => None,
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => {
            Some(Kind::Float32ToBits)
        }
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => {
            Some(Kind::Float64ToBits)
        }
        ScalarType::Boolean => Some(Kind::ZeroExtendU8),
        ScalarType::Integer(integer) => match (integer.sign(), integer.bits()) {
            (IntegerSign::Unsigned, Some(8)) => Some(Kind::ZeroExtendU8),
            (IntegerSign::Unsigned, Some(16)) => Some(Kind::ZeroExtendU16),
            (IntegerSign::Unsigned, Some(32)) => Some(Kind::ZeroExtendU32),
            (IntegerSign::Unsigned, Some(64)) | (IntegerSign::Signed, Some(64)) => {
                Some(Kind::CopyI64)
            }
            (IntegerSign::Signed, Some(8)) => Some(Kind::SignExtendI8),
            (IntegerSign::Signed, Some(16)) => Some(Kind::SignExtendI16),
            (IntegerSign::Signed, Some(32)) => Some(Kind::SignExtendI32),
            _ => None,
        },
    }
}

fn scalar_shape(scalar_type: ScalarType) -> Option<calling_conventions::ValueShape> {
    calling_conventions::ValueShape::for_scalar(scalar_type)
}

fn rematerializes_argument(
    instruction: &selected_instructions::SelectedInstruction,
    register: &selected_instructions::VirtualRegister,
    source: target_operations::TargetUnitScalarArgumentSource,
) -> bool {
    use register_model::RegisterOperandAccess;
    use selected_instructions::{SelectedInstructionKind, VirtualRegisterOrigin};
    let target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
        source_value,
        value,
        ..
    } = source
    else {
        return false;
    };
    let [output] = instruction.operands.as_slice() else {
        return false;
    };
    instruction.kind == SelectedInstructionKind::MaterializeI64 { value }
        && output.virtual_register == register.id
        && output.access == RegisterOperandAccess::Def
        && output.class == register.class
        && output.fixed_view.is_none()
        && output.tied_to.is_none()
        && !output.early_clobber
        && register.entry_fixed_view.is_none()
        && register.scalar_type == source.scalar_type()
        && register.origin
            == VirtualRegisterOrigin::InstructionResult {
                instruction: instruction.id,
                source_value,
            }
        && instruction.implicit_uses.is_empty()
        && instruction.implicit_defs.is_empty()
        && instruction.clobbers.is_empty()
        && instruction.provenance.operations.is_empty()
        && instruction.provenance.edges.is_empty()
        && instruction.provenance.obligations.is_empty()
        && instruction.provenance.work.is_empty()
        && instruction.provenance.values.as_slice() == [source_value]
}

pub fn scalar_call_result_producer(
    function: &selected_instructions::SelectedFunction,
    home: &target_operations::TargetUnitScalarHomeRequirement,
) -> bool {
    let mut internal = function.calls.iter().filter(|contract| {
        contract.owner == target_operations::CallSiteOwner::Operation(home.defining_operation)
    });
    let mut foreign = function
        .normalized_foreign_calls
        .iter()
        .filter(|record| record.invocation_site.operation() == Some(home.defining_operation));
    let (producer, result_home) = match (internal.next(), foreign.next()) {
        (Some(contract), None) if internal.next().is_none() => (contract.instruction, None),
        (None, Some(record)) if foreign.next().is_none() => {
            (record.instruction, Some(record.call.result.scalar_home()))
        }
        (None, None) => {
            let Some(producer) =
                scalar_observation_producer(function, home.defining_operation, home.source_value)
            else {
                return false;
            };
            (producer, None)
        }
        _ => return false,
    };
    if let Some(recorded) = result_home
        && recorded != Some(home)
    {
        return false;
    }
    let Some(producer) = function
        .instructions()
        .find(|instruction| instruction.id == producer)
    else {
        return false;
    };
    producer.operands.iter().any(|operand| {
        function
            .virtual_registers
            .get(operand.virtual_register.0 as usize)
            .is_some_and(|register| {
                register.origin
                    == selected_instructions::VirtualRegisterOrigin::InstructionResult {
                        instruction: producer.id,
                        source_value: home.source_value,
                    }
            })
    })
}

pub fn foreign_invocation_provenance_matches(
    provenance: &selected_instructions::SelectedInstructionProvenance,
    site: semantic_vocabulary::InvocationSite,
) -> bool {
    match site {
        semantic_vocabulary::InvocationSite::InvokeNoNormal(_)
        | semantic_vocabulary::InvocationSite::Cleanup { .. } => false,
        semantic_vocabulary::InvocationSite::Operation(operation) => {
            provenance.operations.last() == Some(&operation)
                && provenance.edges.is_empty()
                && provenance.work_settles_only_claimed_sites()
        }
        semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
            provenance.edges.as_slice() == [edge] && provenance.work_settles_only_claimed_sites()
        }
    }
}

fn scalar_argument_transport_context(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    argument: &target_operations::TargetUnitScalarCallArgument,
) -> String {
    use selected_instructions::{FrameStorageSlotId, SelectedInstructionKind};
    let outgoing = block.instructions.iter().position(|instruction| {
        matches!(
            instruction.kind,
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Outgoing(slot), byte_offset: 0,
            } if slot.invocation_site == record.invocation_site
                && slot.argument_index == argument.parameter_index
        )
    });
    let anchor = outgoing
        .or_else(|| {
            block.instructions.iter().position(|instruction| {
                instruction
                    .provenance
                    .values
                    .contains(&argument.source.source_value())
            })
        })
        .unwrap_or(0);
    let instructions =
        &block.instructions[anchor..anchor.saturating_add(6).min(block.instructions.len())];
    let mut registers = instructions
        .iter()
        .flat_map(|instruction| {
            instruction
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
        })
        .collect::<Vec<_>>();
    registers.sort_by_key(|register| register.0);
    registers.dedup();
    format!(
        "argument={argument:?}; slot={:?}; found_outgoing={}; steps={:?}; origins={:?}",
        function
            .outgoing_arguments
            .iter()
            .filter(|slot| slot.id.invocation_site == record.invocation_site
                && slot.id.argument_index == argument.parameter_index)
            .collect::<Vec<_>>(),
        outgoing.is_some(),
        instructions
            .iter()
            .map(|instruction| (
                instruction.id,
                &instruction.kind,
                instruction
                    .operands
                    .iter()
                    .map(|operand| (operand.virtual_register, operand.access))
                    .collect::<Vec<_>>(),
                &instruction.provenance.values,
                foreign_invocation_provenance_matches(
                    &instruction.provenance,
                    record.invocation_site
                ),
            ))
            .collect::<Vec<_>>(),
        registers
            .iter()
            .filter_map(|register| function
                .virtual_register(*register)
                .map(|value| (*register, value.origin)))
            .collect::<Vec<_>>()
    )
}

#[allow(clippy::too_many_arguments)]
fn scalar_argument_span<F>(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
    argument: &target_operations::TargetUnitScalarCallArgument,
    register_operand: usize,
    fixed_view: &F,
) -> Option<InstructionSpanSequence>
where
    F: Fn(&selected_instructions::SelectedOperand, target_operations::MachineRegister) -> bool,
{
    match argument.placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == argument.placement.shape.byte_size => {
            let operand = instruction.operands.get(register_operand)?;
            if usize::from(operand.operand) != register_operand || !fixed_view(operand, *register) {
                return None;
            }
            let register_value = function
                .virtual_registers
                .get(operand.virtual_register.0 as usize)?;
            let selected_instructions::VirtualRegisterOrigin::InstructionResult {
                instruction: materialization,
                source_value,
            } = register_value.origin
            else {
                return None;
            };
            if source_value != argument.source.source_value() {
                return None;
            }
            Some(InstructionSpanSequence(vec![materialization]))
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                alignment,
            },
        ] if *byte_size == argument.placement.shape.byte_size => {
            let slot = selected_instructions::OutgoingArgumentSlotId {
                invocation_site: record.invocation_site,
                argument_index: argument.parameter_index,
                role: selected_instructions::OutgoingArgumentSlotRole::Argument,
            };
            let mut outgoing = function
                .outgoing_arguments
                .iter()
                .filter(|outgoing| outgoing.id == slot);
            let (Some(outgoing), None) = (outgoing.next(), outgoing.next()) else {
                return None;
            };
            if outgoing.abi_stack_byte_offset != *stack_byte_offset
                || outgoing.byte_size != (*byte_size)
                || outgoing.alignment != *alignment
            {
                return None;
            }
            let source_value = argument.source.source_value();
            let stored = u8::try_from(*byte_size).ok()?;
            let address_index = block.instructions.iter().position(|candidate| {
                matches!(
                    candidate.kind,
                    selected_instructions::SelectedInstructionKind::FrameAddress {
                        slot: selected_instructions::FrameStorageSlotId::Outgoing(candidate),
                        byte_offset: 0,
                    } if candidate == slot
                ) && foreign_invocation_provenance_matches(
                    &candidate.provenance,
                    record.invocation_site,
                ) && candidate.provenance.values.as_slice() == [source_value]
            })?;
            let address = &block.instructions[address_index];
            let mut store_index = address_index.checked_add(1)?;
            let next = block.instructions.get(store_index)?;
            let rematerialized = next.operands.first().and_then(|operand| {
                function
                    .virtual_register(operand.virtual_register)
                    .filter(|value| rematerializes_argument(next, value, argument.source))
                    .map(|value| value.id)
            });
            if rematerialized.is_some() {
                store_index = store_index.checked_add(1)?;
            }
            let store = block.instructions.get(store_index)?;
            let selected_instructions::SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: stored_bytes,
            } = store.kind
            else {
                return None;
            };
            if stored_bytes != stored
                || !foreign_invocation_provenance_matches(&store.provenance, record.invocation_site)
                || store.provenance.values.as_slice() != [source_value]
            {
                return None;
            }
            let address_operand = store.operands.first()?;
            let address_value = function
                .virtual_registers
                .get(address_operand.virtual_register.0 as usize)?;
            if address_value.origin
                != (selected_instructions::VirtualRegisterOrigin::ScalarAbiAddress {
                    instruction: address.id,
                    source_value,
                })
            {
                return None;
            }
            if rematerialized.is_some_and(|value| {
                store
                    .operands
                    .get(1)
                    .map(|operand| operand.virtual_register)
                    != Some(value)
            }) {
                return None;
            }
            Some(InstructionSpanSequence(
                block.instructions[address_index..=store_index]
                    .iter()
                    .map(|instruction| instruction.id)
                    .collect(),
            ))
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn foreign_scalar_argument_custody<F>(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
    home_offsets: &[(semantic_vocabulary::OperationId, u32)],
    caller: MachineId,
    owner: CallSiteOwner,
    fixed_view: F,
) -> Result<Vec<ForeignScalarArgumentPlan>, ObjectError>
where
    F: Fn(&selected_instructions::SelectedOperand, target_operations::MachineRegister) -> bool,
{
    let invalid_argument =
        |reason, argument: Option<&target_operations::TargetUnitScalarCallArgument>| {
            ObjectError::InvalidForeignCallArgument {
                caller,
                owner,
                import: record
                    .call
                    .binding
                    .import_locator()
                    .map(|locator| format!("{locator:?}")),
                parameter: argument.map(|argument| argument.parameter_index),
                reason,
                transport: argument.map(|argument| {
                    scalar_argument_transport_context(function, block, record, argument)
                }),
            }
        };
    let invalid = |reason| invalid_argument(reason, None);
    let plan = &record.call.binding.boundary_entry_plan.call;
    let placements = record.call.binding.operand_placements();
    let parameter_slot_materializations = plan
        .callback_materializations
        .iter()
        .filter(|row| {
            matches!(
                row.destination,
                calling_conventions::NativePlace::Parameter(_)
            )
        })
        .count();
    if placements.len()
        != record.call.scalar_arguments.len()
            + record.call.structural_arguments.len()
            + parameter_slot_materializations
    {
        return Err(invalid(
            "argument roster does not match evaluated operand placements",
        ));
    }
    let register_positions =
        selected_instructions::calls::constraints::foreign_argument_registers(&record.call);
    let register_arguments = register_positions.len();
    if instruction.operands.len()
        != register_arguments
            + foreign_result_operand_count(record)
                .ok_or_else(|| invalid("argument roster has no valid result operand count"))?
    {
        return Err(invalid(
            "selected call operand count does not match argument and result roster",
        ));
    }
    if let target_operations::TargetBoundaryResult::Structural(_) = record.call.result {
        let placement = plan
            .result
            .as_ref()
            .ok_or_else(|| invalid("structural result placement is missing"))?;
        for (location, operand) in placement
            .locations
            .iter()
            .zip(&instruction.operands[register_arguments..])
        {
            let register = location
                .direct_register()
                .ok_or_else(|| invalid("structural result location has no direct register"))?;
            if !fixed_view(operand, register) {
                return Err(invalid(
                    "structural result operand lost its fixed ABI register",
                ));
            }
        }
    }
    let mut scalar_arguments = Vec::new();
    for (scalar_ordinal, argument) in record.call.scalar_arguments.iter().enumerate() {
        let invalid = |reason| invalid_argument(reason, Some(argument));
        let native_position = usize::try_from(argument.parameter_index)
            .map_err(|_| invalid("parameter index is not addressable"))?;
        let shape = scalar_shape(argument.source.scalar_type())
            .ok_or_else(|| invalid("argument scalar type has no ABI shape"))?;
        if scalar_ordinal.checked_sub(1).is_some_and(|previous| {
            record.call.scalar_arguments[previous].parameter_index >= argument.parameter_index
        }) || placements.get(native_position) != Some(&argument.placement)
            || argument.placement.shape != shape
        {
            return Err(invalid(
                "argument order, placement or scalar shape differs from evaluated plan",
            ));
        }
        let register_operand = register_positions
            .iter()
            .position(|(position, _)| *position == native_position)
            .unwrap_or(register_arguments);
        let (source, span) = match argument.source {
            source @ (target_operations::TargetUnitScalarArgumentSource::Parameter { .. }
            | target_operations::TargetUnitScalarArgumentSource::BlockParameter(_)
            | target_operations::TargetUnitScalarArgumentSource::BooleanImmediate {
                ..
            }
            | target_operations::TargetUnitScalarArgumentSource::IeeeFloatImmediate {
                ..
            }) => (
                machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
                    source_value: source.source_value(),
                    scalar_type: source.scalar_type(),
                    instruction: record.instruction,
                },
                InstructionSpanSequence(vec![record.instruction]),
            ),
            target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                defining_operation,
                source_value,
                scalar_type,
                value,
            } => {
                if semantic_vocabulary::ScalarTerm::integer(scalar_type, value).is_err() {
                    return Err(invalid(
                        "integer immediate is outside its declared scalar type",
                    ));
                }
                let span = scalar_argument_span(
                    function,
                    block,
                    record,
                    instruction,
                    argument,
                    register_operand,
                    &fixed_view,
                )
                .ok_or_else(|| {
                    invalid("integer immediate lost its exact emitted argument transport")
                })?;
                (
                    machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                        defining_operation,
                        source_value,
                        scalar_type,
                        value,
                    },
                    span,
                )
            }
            target_operations::TargetUnitScalarArgumentSource::Home(home) => {
                if home.shape != argument.placement.shape
                    || !scalar_call_result_producer(function, &home)
                {
                    return Err(invalid(
                        "scalar home shape or exact producer does not match",
                    ));
                }
                let byte_offset = home_offsets
                    .iter()
                    .find(|(operation, _)| *operation == home.defining_operation)
                    .map(|(_, offset)| *offset)
                    .ok_or_else(|| invalid("scalar home has no allocated durable offset"))?;
                let span = scalar_argument_span(
                    function,
                    block,
                    record,
                    instruction,
                    argument,
                    register_operand,
                    &fixed_view,
                )
                .ok_or_else(|| invalid("scalar home lost its exact emitted argument transport"))?;
                (
                    machine_code::InternalUnitScalarArgumentSourceRecord::Home(
                        machine_code::UnitScalarHomeRecord {
                            defining_operation: home.defining_operation,
                            source_value: home.source_value,
                            scalar_type: home.scalar_type,
                            shape: home.shape,
                            byte_offset,
                        },
                    ),
                    span,
                )
            }
        };
        scalar_arguments.push(ForeignScalarArgumentPlan {
            parameter_index: argument.parameter_index,
            source,
            placement: argument.placement.clone(),
            span,
        });
    }
    Ok(scalar_arguments)
}

fn structural_result_capture(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
    spans: &mut Vec<InstructionSpanObligation>,
) -> Option<()> {
    use selected_instructions::{
        FrameStorageSlotId, LocalStorageSlotId, SelectedInstructionKind as Kind,
        SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, VirtualRegisterOrigin,
    };
    let target_operations::TargetBoundaryResult::Structural(home) = &record.call.result else {
        return None;
    };
    let (operation, result) = home.operation_result()?;
    let target_operations::TargetStructuralHomeLayout::Aggregate(shape) = home.layout else {
        return None;
    };
    let placement = record
        .call
        .binding
        .boundary_entry_plan
        .call
        .result
        .as_ref()?;
    let contract = function.structural.as_ref()?;
    if Some(operation) != record.invocation_site.operation()
        || placement.shape.byte_size != shape.byte_size
        || placement.shape.alignment != shape.alignment
        || !contract.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind
                    == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        producer: operation,
                        structural_type: result.structural_type,
                    }
        })
        || !contract
            .structural_types
            .iter()
            .any(|declared| declared.id == result.structural_type)
    {
        return None;
    }
    let slot = LocalStorageSlotId::Structural {
        operation,
        place: result.place,
    };
    let mut slots = function
        .local_storage_slots
        .iter()
        .filter(|candidate| candidate.id == slot);
    let storage = slots.next()?;
    if slots.next().is_some()
        || storage.byte_size != shape.byte_size
        || storage.alignment != shape.alignment
    {
        return None;
    }
    let result_operands = foreign_result_operand_count(record)?;
    let operands = instruction
        .operands
        .get(instruction.operands.len().checked_sub(result_operands)?..)?;
    let transport = selected_instructions::calls::normalized_foreign::return_transport::resolve(
        function, block, record,
    )?;
    let call_position = transport.call_position;
    let mut position = transport.normalization_position;
    let address = block.instructions.get(position)?;
    if address.kind
        != (Kind::FrameAddress {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: 0,
        })
    {
        return None;
    }
    let [pointer] = address.operands.as_slice() else {
        return None;
    };
    let indirect = matches!(
        placement.locations.as_slice(),
        [ValueLocation::Indirect { .. }]
    );
    let origin = if indirect {
        SelectedMemoryAccessOrigin::Operation(operation)
    } else {
        SelectedMemoryAccessOrigin::Block(block.source_block())
    };
    let memory_matches =
        |selected: selected_instructions::SelectedInstructionId, offset, width, role| {
            let mut accesses = function
                .memory_accesses
                .iter()
                .filter(|access| access.instruction == selected);
            let Some(access) = accesses.next() else {
                return false;
            };
            accesses.next().is_none()
                && access.origin == origin
                && access.place == result.place
                && access.byte_offset == offset
                && access.byte_count == width
                && access.role == role
        };
    let transport_matches = |register: selected_instructions::VirtualRegisterId,
                             selected: selected_instructions::SelectedInstructionId,
                             offset| {
        function
            .virtual_registers
            .get(register.0 as usize)
            .is_some_and(|value| {
                value.origin
                    == VirtualRegisterOrigin::AbiTransport {
                        instruction: selected,
                        place: result.place,
                        byte_offset: offset,
                    }
            })
    };
    if !transport_matches(pointer.virtual_register, address.id, 0)
        || !memory_matches(
            address.id,
            0,
            shape.byte_size,
            SelectedMemoryAccessRole::AddressLocal { slot },
        )
    {
        return None;
    }
    spans.push(InstructionSpanObligation::NonEmpty(address.id));
    spans.push(InstructionSpanObligation::Before {
        first: instruction.id,
        second: address.id,
    });
    if indirect {
        let [argument] = operands else { return None };
        // Register homes may split the hidden pointer before its fixed argument
        // register: each split is a `CopyI64` whose output keeps this result
        // place's transport identity. Follow those copies back to the one
        // `FrameAddress` that names the result slot.
        let mut register = argument.virtual_register;
        let mut preparation = None;
        // Each copy reads a strictly earlier instruction, so a valid chain
        // visits at most the instructions that precede the call.
        for _ in 0..=call_position {
            let VirtualRegisterOrigin::AbiTransport {
                instruction: prepared,
                place,
                byte_offset: 0,
            } = function.virtual_registers.get(register.0 as usize)?.origin
            else {
                return None;
            };
            if place != result.place {
                return None;
            }
            let candidate = block
                .instructions
                .iter()
                .take(call_position)
                .find(|candidate| candidate.id == prepared)?;
            match (candidate.kind, candidate.operands.as_slice()) {
                (Kind::CopyI64, [input, output])
                    if output.virtual_register == register
                        && function
                            .memory_accesses
                            .iter()
                            .all(|access| access.instruction != prepared) =>
                {
                    register = input.virtual_register;
                }
                (_, [output]) if output.virtual_register == register => {
                    preparation = Some(candidate);
                    break;
                }
                _ => return None,
            }
        }
        let preparation = preparation?;
        let prepared = preparation.id;
        (preparation.kind == address.kind
            && memory_matches(
                prepared,
                0,
                shape.byte_size,
                SelectedMemoryAccessRole::AddressLocal { slot },
            ))
        .then_some(())?;
        spans.push(InstructionSpanObligation::NonEmpty(prepared));
        spans.push(InstructionSpanObligation::Before {
            first: prepared,
            second: instruction.id,
        });
        return Some(());
    }
    let mut previous = address.id;
    position = position.checked_add(1)?;
    if transport.result_registers.len() != placement.locations.len() {
        return None;
    }
    for (location, register) in placement.locations.iter().zip(transport.result_registers) {
        let ValueLocation::Register {
            value_byte_offset,
            byte_size,
            ..
        } = *location
        else {
            return None;
        };
        if value_byte_offset.checked_add(byte_size)? > shape.byte_size {
            return None;
        }
        let mut value = register;
        let mut store = block.instructions.get(position)?;
        if matches!(store.kind, Kind::Float32ToBits | Kind::Float64ToBits) {
            let expected = match byte_size {
                4 => Kind::Float32ToBits,
                8 => Kind::Float64ToBits,
                _ => return None,
            };
            let [input, output] = store.operands.as_slice() else {
                return None;
            };
            if store.kind != expected
                || input.virtual_register != value
                || !transport_matches(output.virtual_register, store.id, value_byte_offset)
            {
                return None;
            }
            spans.push(InstructionSpanObligation::NonEmpty(store.id));
            spans.push(InstructionSpanObligation::Before {
                first: previous,
                second: store.id,
            });
            previous = store.id;
            value = output.virtual_register;
            position = position.checked_add(1)?;
            store = block.instructions.get(position)?;
        }
        let actual = match store.kind {
            Kind::Store {
                byte_offset,
                byte_size,
            } => (byte_offset, u32::from(byte_size)),
            Kind::StorePacked { byte_offset, width } => (byte_offset, u32::from(width.byte_size())),
            _ => return None,
        };
        if actual != (value_byte_offset, byte_size)
            || store.operands.first()?.virtual_register != pointer.virtual_register
            || store.operands.get(1)?.virtual_register != value
            || !memory_matches(
                store.id,
                value_byte_offset,
                byte_size,
                SelectedMemoryAccessRole::WritePlace,
            )
        {
            return None;
        }
        spans.push(InstructionSpanObligation::NonEmpty(store.id));
        spans.push(InstructionSpanObligation::Before {
            first: previous,
            second: store.id,
        });
        previous = store.id;
        position = position.checked_add(1)?;
    }
    Some(())
}

#[allow(clippy::too_many_arguments)]
fn foreign_scalar_result_custody(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
    home_offsets: &[(semantic_vocabulary::OperationId, u32)],
    caller: MachineId,
    owner: CallSiteOwner,
    spans: &mut Vec<InstructionSpanObligation>,
) -> Result<Option<ForeignScalarResultPlan>, ObjectError> {
    let invalid = |reason| ObjectError::InvalidForeignCallArgument {
        caller,
        owner,
        import: record
            .call
            .binding
            .import_locator()
            .map(|locator| format!("{locator:?}")),
        parameter: None,
        reason,
        transport: None,
    };
    let plan = &record.call.binding.boundary_entry_plan.call;
    if matches!(
        record.call.result,
        target_operations::TargetBoundaryResult::Structural(_)
    ) {
        structural_result_capture(function, block, record, instruction, spans)
            .ok_or_else(|| invalid("structural result lost its exact storage capture"))?;
        return Ok(None);
    }
    let (Some(home), Some(placement)) = (record.call.result.scalar_home(), &plan.result) else {
        if matches!(
            record.call.result,
            target_operations::TargetBoundaryResult::Unit
        ) && plan.result.is_none()
        {
            return Ok(None);
        }
        return Err(invalid(
            "scalar result home and placement disagree about Unit",
        ));
    };
    let shape = scalar_shape(home.scalar_type)
        .ok_or_else(|| invalid("result scalar type has no ABI shape"))?;
    if Some(home.defining_operation) != record.invocation_site.operation()
        || home.shape != shape
        || placement.shape != home.shape
        || !placement.is_complete_scalar_result()
    {
        return Err(invalid(
            "scalar result owner, shape or complete placement differs from evaluated plan",
        ));
    }
    let transport = selected_instructions::calls::normalized_foreign::return_transport::resolve(
        function, block, record,
    )
    .ok_or_else(|| invalid("scalar result has no exact selected return transport"))?;
    let normalization = block
        .instructions
        .get(transport.normalization_position)
        .ok_or_else(|| invalid("scalar result normalization instruction is missing"))?;
    let Some(kind) = scalar_result_normalization_kind(
        home.scalar_type,
        matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect { .. }]
        ),
    ) else {
        return Err(invalid("scalar result has no supported normalization kind"));
    };
    let [input, output] = normalization.operands.as_slice() else {
        return Err(invalid(
            "scalar result normalization does not have input and output operands",
        ));
    };
    let output_origin = function
        .virtual_registers
        .get(output.virtual_register.0 as usize)
        .map(|register| register.origin);
    if normalization.kind != kind
        || transport.result_registers.as_slice() != [input.virtual_register]
        || normalization.provenance.operations.as_slice()
            != [record
                .invocation_site
                .operation()
                .ok_or_else(|| invalid("scalar result has no operation owner"))?]
        || normalization.provenance.values.as_slice() != [home.source_value]
        || output_origin
            != Some(
                selected_instructions::VirtualRegisterOrigin::InstructionResult {
                    instruction: normalization.id,
                    source_value: home.source_value,
                },
            )
    {
        return Err(invalid(
            "scalar result normalization lost its kind, value or source provenance",
        ));
    }
    spans.push(InstructionSpanObligation::Contiguous(
        InstructionSpanSequence(
            block.instructions[transport.call_position..=transport.normalization_position]
                .iter()
                .map(|instruction| instruction.id)
                .collect(),
        ),
    ));
    let Some(byte_offset) = home_offsets
        .iter()
        .find(|(operation, _)| *operation == home.defining_operation)
        .map(|(_, offset)| *offset)
    else {
        return Err(invalid("scalar result has no allocated durable offset"));
    };
    Ok(Some(ForeignScalarResultPlan {
        home: machine_code::UnitScalarHomeRecord {
            defining_operation: home.defining_operation,
            source_value: home.source_value,
            scalar_type: home.scalar_type,
            shape: home.shape,
            byte_offset,
        },
        source: placement.clone(),
        instruction: normalization.id,
    }))
}

mod floating_control {
    use machine_code::FunctionTargetFrameLayout;
    use machine_code::foreign_call_plans::{
        ForeignFloatingControlPlan, InstructionSpanObligation, InstructionSpanSequence,
    };
    use selected_instructions::{
        LocalStorageSlotId, SelectedBlock, SelectedFunction, SelectedNormalizedForeignCall,
    };

    pub(super) fn derive(
        function: &SelectedFunction,
        block: &SelectedBlock,
        frame: &FunctionTargetFrameLayout,
        record: &SelectedNormalizedForeignCall,
        spans: &mut Vec<InstructionSpanObligation>,
    ) -> Result<ForeignFloatingControlPlan, &'static str> {
        let operation = record
            .invocation_site
            .operation()
            .ok_or("floating controls have no source operation")?;
        let slot = LocalStorageSlotId::Boundary { operation };
        let transport =
            selected_instructions::calls::normalized_foreign::return_transport::resolve(
                function, block, record,
            )
            .ok_or("foreign result transport has no exact control restore")?;
        let call_position = transport.call_position;
        let restore_position = transport
            .restore_position
            .ok_or("foreign control restore is absent")?;
        let save_position =
            selected_instructions::calls::normalized_foreign::return_transport::save_position(
                function,
                block,
                record,
                call_position,
            )
            .ok_or(
                "foreign control save envelope contains unrelated effects or invalid private transport",
            )?;
        let save = &block.instructions[save_position];
        let restore = block
            .instructions
            .get(restore_position)
            .ok_or("foreign control restore position is outside the block")?;
        let mut homes = function
            .local_storage_slots
            .iter()
            .filter(|home| home.id == slot);
        let home = homes.next().ok_or("foreign control slot is absent")?;
        let mut placements = frame
            .local_storage_slots
            .iter()
            .filter(|home| home.id == slot);
        let placement = placements
            .next()
            .ok_or("foreign control slot has no frame placement")?;
        if homes.next().is_some()
            || placements.next().is_some()
            || home.byte_size != 8
            || home.alignment != 8
            || placement.size_bytes != home.byte_size
            || placement.alignment_bytes != home.alignment
            || placement.frame_offset_bytes < frame.outgoing_abi_area.byte_size
            || !placement.frame_offset_bytes.is_multiple_of(8)
            || placement
                .frame_offset_bytes
                .checked_add(8)
                .ok_or("foreign control frame extent overflows")?
                > frame.frame_size_bytes
        {
            return Err("foreign control slot extent or alignment does not match the frame");
        }
        let saved_slot_byte_offset = u32::try_from(
            placement
                .frame_offset_bytes
                .checked_sub(frame.red_zone_resident_bytes)
                .ok_or("foreign control slot precedes committed frame")?,
        )
        .map_err(|_| "foreign control slot offset is not addressable")?;
        spans.push(InstructionSpanObligation::Before {
            first: save.id,
            second: record.instruction,
        });
        spans.push(InstructionSpanObligation::Contiguous(
            InstructionSpanSequence(
                block.instructions[call_position..=restore_position]
                    .iter()
                    .map(|instruction| instruction.id)
                    .collect(),
            ),
        ));
        Ok(ForeignFloatingControlPlan {
            saved_slot_byte_offset,
            save: save.id,
            restore: restore.id,
        })
    }
}

pub fn capture(
    selected: &post_allocation_machine::PostAllocationMachineProgram,
    layout: &machine_code::TargetFrameLayoutPlan,
) -> Result<PhysicalForeignCallPlans, ForeignCallPlanError> {
    let target = selected.target;
    let architecture = target.architecture;
    let mut custody = Vec::new();
    for function in selected.functions.iter() {
        if function.normalized_foreign_calls.is_empty() {
            continue;
        }
        let home_offsets = durable_home_offsets(function);
        let publication = selected.contracts.publication().function(function.machine);
        let mut frames = layout
            .functions
            .iter()
            .filter(|frame| frame.machine == function.machine);
        let frame = match (frames.next(), frames.next()) {
            (Some(frame), None) => Some(frame),
            _ => None,
        };
        for (ordinal, record) in function.normalized_foreign_calls.iter().enumerate() {
            let owner = CallSiteOwner::from(record.invocation_site);
            let invalid = |reason| ObjectError::InvalidForeignCallProjection {
                caller: function.machine,
                owner,
                reason,
            };
            if record.call.binding.native_target() != Some(target)
                || !record.call.binding.has_valid_entry_mechanism()
            {
                return Err(invalid(
                    "binding target or native entry mechanism does not match",
                ));
            }
            let (block, instruction) = unique_block_instruction(function, record.instruction)
                .ok_or_else(|| invalid("selected call instruction is missing or repeated"))?;
            descriptor_input::validate(function, block, record).ok_or_else(|| {
                invalid("descriptor input guard does not match the selected call")
            })?;
            if !foreign_invocation_provenance_matches(
                &instruction.provenance,
                record.invocation_site,
            ) {
                return Err(invalid(
                    "selected call instruction lost its invocation provenance",
                ));
            }
            if !matches!(
                instruction.kind,
                selected_instructions::SelectedInstructionKind::NormalizedForeignCall {
                    boundary, ordinal: actual,
                } if boundary == record.call.boundary && usize::try_from(actual) == Ok(ordinal)
            ) {
                return Err(invalid(
                    "selected call boundary or roster ordinal does not match",
                ));
            }
            if record.invocation_site.operation().is_none()
                && selected_instructions::calls::normalized_foreign::return_transport::resolve(
                    function, block, record,
                )
                .is_none()
            {
                return Err(invalid("external completion has no exact return transport"));
            }
            let mut spans = Vec::new();
            let scalar_arguments = foreign_scalar_argument_custody(
                function,
                block,
                record,
                instruction,
                &home_offsets,
                function.machine,
                owner,
                |operand, register| {
                    operand.fixed_view == selected.environment.fixed_register_view(register)
                },
            )?;
            let scalar_result = foreign_scalar_result_custody(
                function,
                block,
                record,
                instruction,
                &home_offsets,
                function.machine,
                owner,
                &mut spans,
            )?;
            let site = match record.invocation_site {
                semantic_vocabulary::InvocationSite::Operation(operation) => {
                    target_vocabulary::publication::PsiProvenance::Operation(operation)
                }
                semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
                    target_vocabulary::publication::PsiProvenance::Edge(edge)
                }
                semantic_vocabulary::InvocationSite::InvokeNoNormal(_)
                | semantic_vocabulary::InvocationSite::Cleanup { .. } => {
                    return Err(ObjectError::ForeignCallOwnerNotInProvenance {
                        caller: function.machine,
                        owner,
                    });
                }
            };
            let operation_ordinal = publication
                .and_then(|publication| publication.ordinal(site))
                .ok_or(ObjectError::ForeignCallOwnerNotInProvenance {
                    caller: function.machine,
                    owner,
                })? as usize;
            let Some(frame) = frame else {
                return Err(invalid("caller frame is missing or repeated"));
            };
            if let target_operations::TargetBoundaryResult::Structural(home) = &record.call.result {
                let slot = selected_instructions::LocalStorageSlotId::Structural {
                    operation: record
                        .invocation_site
                        .operation()
                        .ok_or_else(|| invalid("structural result has no source operation"))?,
                    place: home.place(),
                };
                let mut homes = frame
                    .local_storage_slots
                    .iter()
                    .filter(|home| home.id == slot);
                let Some(placed) = homes.next() else {
                    return Err(invalid(
                        "structural result storage is missing from the frame",
                    ));
                };
                let shape = home.layout.shape();
                if homes.next().is_some()
                    || placed.size_bytes != shape.byte_size
                    || placed.alignment_bytes != shape.alignment
                    || placed.frame_offset_bytes < frame.outgoing_abi_area.byte_size
                    || !placed
                        .frame_offset_bytes
                        .is_multiple_of(u64::from(shape.alignment))
                    || placed
                        .frame_offset_bytes
                        .checked_add(u64::from(shape.byte_size))
                        .is_none_or(|end| end > frame.frame_size_bytes)
                {
                    return Err(invalid(
                        "structural result storage has the wrong frame extent or alignment",
                    ));
                }
            }
            let committed = frame
                .frame_size_bytes
                .checked_sub(frame.red_zone_resident_bytes)
                .and_then(|extent| {
                    extent.checked_add(
                        if record.call.binding.system_call().is_none()
                            && architecture == target::Architecture::X86_64
                        {
                            8
                        } else {
                            0
                        },
                    )
                })
                .ok_or(ObjectError::TextSizeOverflow)?;
            let caller_live_bytes =
                u32::try_from(committed).map_err(|_| ObjectError::TextSizeOverflow)?;
            let controls = if record.call.binding.requires_floating_control_preservation()
                && record.invocation_site.operation().is_some()
            {
                Some(
                    floating_control::derive(function, block, frame, record, &mut spans)
                        .map_err(invalid)?,
                )
            } else {
                if !record.call.callbacks.is_empty() {
                    return Err(invalid(
                        "callback addresses require an operation-owned native stack call",
                    ));
                }
                None
            };
            custody.push(PhysicalForeignCallPlan {
                machine: function.machine,
                block: block.id,
                ordinal,
                record: record.clone(),
                operand_views: instruction
                    .operands
                    .iter()
                    .map(|operand| operand.fixed_view)
                    .collect(),
                operand_accesses: instruction
                    .operands
                    .iter()
                    .map(|operand| operand.access)
                    .collect(),
                operation_ordinal,
                caller_live_bytes,
                arguments: scalar_arguments,
                result: scalar_result,
                floating_control: controls,
                spans,
            });
        }
    }
    for (index, row) in custody.iter().enumerate() {
        if custody[..index].iter().any(|prior| {
            prior.machine == row.machine
                && prior.record.invocation_site == row.record.invocation_site
        }) {
            return Err(ObjectError::DuplicateForeignCallOwner {
                caller: row.machine,
                owner: row.record.invocation_site.into(),
            });
        }
    }
    Ok(PhysicalForeignCallPlans {
        psi: selected.psi,
        target,
        physical_identity: selected.identity,
        calls: custody,
    })
}

#[cfg(test)]
mod scalar_normalization_tests {
    use super::scalar_result_normalization_kind;
    use selected_instructions::SelectedInstructionKind as Kind;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, MachineId, ScalarType, ValueId, WidthParameter,
    };

    #[test]
    fn scalar_result_normalization_requires_a_known_supported_width() {
        for (sign, bits, expected) in [
            (IntegerSign::Unsigned, 8, Kind::ZeroExtendU8),
            (IntegerSign::Unsigned, 16, Kind::ZeroExtendU16),
            (IntegerSign::Unsigned, 32, Kind::ZeroExtendU32),
            (IntegerSign::Unsigned, 64, Kind::CopyI64),
            (IntegerSign::Signed, 8, Kind::SignExtendI8),
            (IntegerSign::Signed, 16, Kind::SignExtendI16),
            (IntegerSign::Signed, 32, Kind::SignExtendI32),
            (IntegerSign::Signed, 64, Kind::CopyI64),
        ] {
            let scalar = ScalarType::Integer(IntegerType::new(sign, bits).unwrap());
            assert_eq!(
                scalar_result_normalization_kind(scalar, false),
                Some(expected)
            );
        }
        assert_eq!(
            scalar_result_normalization_kind(
                ScalarType::Integer(IntegerType::address(64).unwrap()),
                false
            ),
            Some(Kind::CopyI64)
        );
        for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
            let unknown = ScalarType::Integer(IntegerType::parameterized(
                sign,
                WidthParameter {
                    owner: MachineId::new(1).unwrap(),
                    value: ValueId::new(1).unwrap(),
                },
            ));
            for indirect in [false, true] {
                assert_eq!(scalar_result_normalization_kind(unknown, indirect), None);
            }
        }
    }
}

#[cfg(test)]
mod width_tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ValueId, WidthParameter};

    #[test]
    fn result_normalization_requires_a_known_supported_width() {
        let scalar = |sign, bits| ScalarType::Integer(IntegerType::new(sign, bits).unwrap());
        assert_eq!(
            scalar_result_normalization_kind(scalar(IntegerSign::Signed, 8), false),
            Some(selected_instructions::SelectedInstructionKind::SignExtendI8)
        );
        assert_eq!(
            scalar_result_normalization_kind(scalar(IntegerSign::Unsigned, 64), false),
            Some(selected_instructions::SelectedInstructionKind::CopyI64)
        );
        let parameter = ScalarType::Integer(IntegerType::parameterized(
            IntegerSign::Signed,
            WidthParameter {
                owner: MachineId::new(1).unwrap(),
                value: ValueId::new(1).unwrap(),
            },
        ));
        assert_eq!(scalar_result_normalization_kind(parameter, false), None);
        assert_eq!(scalar_result_normalization_kind(parameter, true), None);
    }
}
