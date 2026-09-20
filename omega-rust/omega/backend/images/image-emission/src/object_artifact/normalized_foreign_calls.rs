//! Foreign-call custody projected from the independently replayed fragment
//! publication source.
//!
//! The fragment route keeps `ObjectArtifact::foreign_calls` inside the sealed
//! replay surface, so an emitted image receives its normalized foreign-call
//! roster as separate custody: [`derive_normalized_foreign_call_custody`]
//! projects every section-level unresolved normalized foreign call back
//! through the retained selected-plan roster and frame layout into one exact
//! [`ObjectForeignCall`] row per call site.
//!
//! Scalar custody is projected for the admitted fixed-integer lane: an
//! integer immediate names the register copy or the contiguous
//! outgoing-slot address-plus-store pair that materializes it, a preceding
//! scalar-call result keeps its `Home` source and its allocated slot in the
//! caller's durable scalar-home area, and a scalar result records the exact
//! normalization instruction that produces the durable definition after the
//! call. Result and `Home` records share one per-caller home-area map so a
//! consumer names the producer's slot exactly. Floating-control intervals
//! and callback materialization stay out of the projection; selection
//! rejects callback-bearing calls before they reach this roster at all.

use calling_conventions::ValueLocation;
use semantic_vocabulary::{MachineId, ScalarType};
use target_operations::CallSiteOwner;

use super::carriers::{ObjectArtifact, ObjectForeignCall};
use super::errors::ObjectError;

/// One derived custody row per unresolved normalized foreign call the placed
/// text section retains.
///
/// Every row rejoins the `{boundary, ordinal}` site the resolver recorded
/// against the selected roster's instruction custody before its evaluated
/// binding (locator, entry plan, provider execution, same-stack contribution)
/// is projected. `caller_live_bytes` is the caller's committed frame extent
/// plus the architecture's pushed-return-address width when the call leaves
/// one on the stack. The admitted scalar lane projects each argument's
/// exact emitted custody and the result's exact normalization span; both
/// carry the durable-home area slot the caller's producer roster allocates.
pub fn derive_normalized_foreign_call_custody(
    source: &object_file::StagedOptimizedRelocationFreeObjectContainer,
) -> Result<Vec<ObjectForeignCall>, ObjectError> {
    let section = source.source().text_section();
    let emissions = source.source().source().source().source();
    let optimized = emissions.optimized_target();
    let selected = emissions.selected_plan();
    let layout = emissions.frame_layout();
    let architecture = source.source().source().fragments().target.architecture;
    let mut custody = Vec::new();
    for call in &section.unresolved_normalized_foreign_calls {
        let owner = CallSiteOwner::Operation(call.operation);
        let text_offset = usize::try_from(call.field_section_offset)
            .map_err(|_| ObjectError::TextSizeOverflow)?;
        let invalid = || ObjectError::InvalidForeignCallSite {
            caller: call.caller,
            owner,
            offset: text_offset,
        };
        if call.state != machine_code::NormalizedForeignCallResolutionState::UnresolvedImportFieldV1
        {
            return Err(invalid());
        }
        let mut functions = selected
            .functions
            .iter()
            .filter(|function| function.machine == call.caller);
        let (Some(function), None) = (functions.next(), functions.next()) else {
            return Err(invalid());
        };
        let index = usize::try_from(call.ordinal).map_err(|_| invalid())?;
        let Some(record) = function.normalized_foreign_calls.get(index) else {
            return Err(invalid());
        };
        if record.instruction != call.instruction
            || record.operation != call.operation
            || record.call.boundary != call.boundary
        {
            return Err(ObjectError::ForeignCallTargetMismatch {
                caller: call.caller,
                owner,
            });
        }
        let Some(block) = function.blocks.iter().find(|block| block.id == call.block) else {
            return Err(invalid());
        };
        // Instruction identities are function-global, not block-local indexes:
        // find the roster instruction by id inside its recorded block.
        let Some(instruction) = block
            .instructions
            .iter()
            .find(|instruction| instruction.id == call.instruction)
        else {
            return Err(invalid());
        };
        if !matches!(
            instruction.kind,
            selected_instructions::SelectedInstructionKind::NormalizedForeignCall {
                boundary,
                ordinal,
            } if boundary == call.boundary && ordinal == call.ordinal
        ) {
            return Err(invalid());
        }
        let Some(fragment) = caller_fragment(section, call.caller) else {
            return Err(invalid());
        };
        let home_offsets = durable_home_offsets(function);
        let scalar_arguments = foreign_scalar_argument_custody(
            function,
            block,
            fragment,
            record,
            instruction,
            &home_offsets,
            call.caller,
            owner,
            |operand, register| {
                operand.fixed_view
                    == emissions
                        .register_environment()
                        .fixed_register_view(register)
            },
        )?;
        let scalar_result = foreign_scalar_result_custody(
            function,
            block,
            fragment,
            record,
            instruction,
            &home_offsets,
            call.caller,
            owner,
        )?;
        let mut abstracted = optimized
            .optimized()
            .plan()
            .functions
            .iter()
            .filter(|function| function.machine == call.caller);
        let (Some(abstracted), None) = (abstracted.next(), abstracted.next()) else {
            return Err(invalid());
        };
        let mut ordinals =
            abstracted.operations.iter().enumerate().filter_map(
                |(index, operation)| match operation {
                    abstract_operations::AbstractOperation::BoundaryCall {
                        psi_operation, ..
                    } if *psi_operation == call.operation => Some(index),
                    _ => None,
                },
            );
        let operation_ordinal = match (ordinals.next(), ordinals.next()) {
            (Some(ordinal), None) => ordinal,
            (None, _) => {
                return Err(ObjectError::ForeignCallOwnerNotInProvenance {
                    caller: call.caller,
                    owner,
                });
            }
            _ => {
                return Err(ObjectError::DuplicateForeignCallOwner {
                    caller: call.caller,
                    owner,
                });
            }
        };
        let mut frames = layout
            .functions
            .iter()
            .filter(|frame| frame.machine == call.caller);
        let (Some(frame), None) = (frames.next(), frames.next()) else {
            return Err(invalid());
        };
        let committed = frame
            .frame_size_bytes
            .checked_sub(frame.red_zone_resident_bytes)
            .and_then(|extent| {
                extent.checked_add(match architecture {
                    target::Architecture::X86_64 => 8,
                    target::Architecture::Aarch64 => 0,
                })
            })
            .ok_or(ObjectError::TextSizeOverflow)?;
        let caller_live_bytes =
            u32::try_from(committed).map_err(|_| ObjectError::TextSizeOverflow)?;
        custody.push(ObjectForeignCall {
            machine: call.caller,
            owner,
            operation_ordinal,
            locator: record.call.binding.locator.clone(),
            provider_execution: record.call.provider_execution.into(),
            boundary_entry_plan: record.call.binding.boundary_entry_plan.clone(),
            caller_live_bytes,
            same_stack_contribution: record.call.binding.same_stack_contribution.clone(),
            scalar_arguments,
            callback_address: None,
            scalar_result,
            x86_floating_control: None,
            aarch64_floating_control: None,
            text_offset,
        });
    }
    for (index, row) in custody.iter().enumerate() {
        if custody[..index]
            .iter()
            .any(|prior| prior.machine == row.machine && prior.owner == row.owner)
        {
            return Err(ObjectError::DuplicateForeignCallOwner {
                caller: row.machine,
                owner: row.owner,
            });
        }
    }
    Ok(custody)
}

/// The one placed function fragment the caller's machine emitted, or none.
fn caller_fragment(
    section: &machine_code::RelocationFreeTextSectionPlacement,
    caller: MachineId,
) -> Option<&machine_code::PlacedFunctionFragment> {
    let mut functions = section
        .functions
        .iter()
        .filter(|function| function.machine == caller);
    match (functions.next(), functions.next()) {
        (Some(function), None) => Some(function),
        _ => None,
    }
}

/// The byte span one selected instruction occupies in the placed fragment,
/// rebased to absolute object `.text` like the call site's `text_offset`, or
/// none.
fn instruction_span(
    fragment: &machine_code::PlacedFunctionFragment,
    instruction: selected_instructions::SelectedInstructionId,
) -> Option<(u64, u64)> {
    fragment
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|span| span.instruction == instruction)
        .map(|span| (span.section_offset, span.byte_count))
}

/// The durable scalar-home area one caller's producers allocate, in emitted
/// instruction order.
///
/// Homes on this route are register-resident — the durable definition is the
/// result's normalization output — so no frame slot exists for them. The
/// records still name each home by its offset in the caller's dense
/// durable-home area: every scalar-producing call row is one producer, its
/// slot 8-aligned and sized by the home shape. Argument `Home` sources and
/// `scalar_result` records share this map so a consumer rejoins the exact
/// producer slot.
fn durable_home_offsets(
    function: &selected_instructions::SelectedFunction,
) -> Vec<(semantic_vocabulary::OperationId, u32)> {
    let mut producers = Vec::new();
    for contract in &function.calls {
        if let Some(placement) = &contract.call.result_placement {
            producers.push((
                contract.instruction,
                contract.operation,
                placement.shape.byte_size,
            ));
        }
    }
    for record in &function.normalized_foreign_calls {
        if let Some(home) = &record.call.result_home {
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
                // The consuming record names only the defining operation; the
                // producer row is resolved over the caller's call rosters.
                let Some(instruction) =
                    producer_instruction(function, requirement.defining_operation)
                else {
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
        let Some(next) = cursor.checked_add(u32::from(byte_size)) else {
            return Vec::new();
        };
        cursor = next;
    }
    offsets
}

/// The selected instruction a scalar-home producer roster row emits, found by
/// the home's defining operation over the caller's scalar call rosters.
fn producer_instruction(
    function: &selected_instructions::SelectedFunction,
    operation: semantic_vocabulary::OperationId,
) -> Option<selected_instructions::SelectedInstructionId> {
    function
        .calls
        .iter()
        .map(|contract| (contract.operation, contract.instruction))
        .chain(
            function
                .normalized_foreign_calls
                .iter()
                .map(|record| (record.operation, record.instruction)),
        )
        .find(|(candidate, _)| *candidate == operation)
        .map(|(_, instruction)| instruction)
}

/// Scalar-argument custody for one roster row's admitted fixed-integer lane.
///
/// Every argument keeps its authored plan position and the exact placement
/// the evaluated plan assigned. An `IntegerImmediate` names the instruction
/// that materializes the value: for a register destination, the copy whose
/// result feeds the call's operand at its register-argument position; for a
/// stack destination, the contiguous outgoing-slot `FrameAddress`/`Store`
/// pair owned by this call. A `Home` argument — a preceding scalar call's
/// result read back through its durable definition — keeps its `Home` record
/// with the producer's allocated home-area slot and the same materialization
/// span mechanics: the producing call must be the exact roster row the
/// requirement names, and the consuming operand still resolves through the
/// emitted copy or outgoing-slot store. Other sources, callback
/// materialization, and mixed scalar/structural lanes fail closed rather
/// than claiming custody they cannot prove.
#[allow(clippy::too_many_arguments)]
fn foreign_scalar_argument_custody<F>(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    fragment: &machine_code::PlacedFunctionFragment,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
    home_offsets: &[(semantic_vocabulary::OperationId, u32)],
    caller: MachineId,
    owner: CallSiteOwner,
    fixed_view: F,
) -> Result<Vec<machine_code::ForeignCallScalarArgumentRecord>, ObjectError>
where
    F: Fn(&selected_instructions::SelectedOperand, target_operations::MachineRegister) -> bool,
{
    let invalid = || ObjectError::InvalidForeignCallArgument { caller, owner };
    let plan = &record.call.binding.boundary_entry_plan.call;
    if !plan.callback_materializations.is_empty()
        || (!record.call.scalar_arguments.is_empty()
            && !record.call.structural_arguments.is_empty())
        || plan.parameters.len()
            != record.call.scalar_arguments.len() + record.call.structural_arguments.len()
    {
        return Err(invalid());
    }
    // The call's operand roster is every register-resident argument word in
    // authored plan order — scalar arguments and structural referent pointers
    // alike — then the optional scalar result definition.
    let register_arguments = record
        .call
        .scalar_arguments
        .iter()
        .filter(|argument| {
            matches!(
                argument.placement.locations.as_slice(),
                [ValueLocation::Register { .. }]
            )
        })
        .count()
        + record
            .call
            .structural_arguments
            .iter()
            .filter(|argument| {
                matches!(
                    argument.destination.locations.as_slice(),
                    [ValueLocation::Register { .. }]
                )
            })
            .count();
    if instruction.operands.len()
        != register_arguments + usize::from(record.call.result_home.is_some())
    {
        return Err(invalid());
    }
    let mut register_operand = 0usize;
    let mut scalar_arguments = Vec::new();
    for (index, argument) in record.call.scalar_arguments.iter().enumerate() {
        let ScalarType::Integer(integer) = argument.source.scalar_type() else {
            return Err(invalid());
        };
        if integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
            || !matches!(integer.bits(), 8 | 16 | 32 | 64)
            || argument.parameter_index != index as u32
            || plan.parameters.get(index) != Some(&argument.placement)
            || argument.placement.shape
                != calling_conventions::ValueShape::integer(integer.bits() / 8, integer.bits() / 8)
        {
            return Err(invalid());
        }
        let register_placed = matches!(
            argument.placement.locations.as_slice(),
            [ValueLocation::Register { .. }]
        );
        let (source, span) = match argument.source {
            target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                defining_operation,
                source_value,
                scalar_type,
                value,
            } => {
                if semantic_vocabulary::ScalarTerm::integer(scalar_type, value).is_err() {
                    return Err(invalid());
                }
                let span = scalar_immediate_span(
                    function,
                    block,
                    fragment,
                    record,
                    instruction,
                    argument,
                    register_operand,
                    &fixed_view,
                )
                .ok_or_else(invalid)?;
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
                    return Err(invalid());
                }
                let byte_offset = home_offsets
                    .iter()
                    .find(|(operation, _)| *operation == home.defining_operation)
                    .map(|(_, offset)| *offset)
                    .ok_or_else(invalid)?;
                let span = scalar_immediate_span(
                    function,
                    block,
                    fragment,
                    record,
                    instruction,
                    argument,
                    register_operand,
                    &fixed_view,
                )
                .ok_or_else(invalid)?;
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
            _ => return Err(invalid()),
        };
        if register_placed {
            register_operand += 1;
        }
        scalar_arguments.push(machine_code::ForeignCallScalarArgumentRecord {
            parameter_index: argument.parameter_index,
            source,
            placement: argument.placement.clone(),
            code_offset: usize::try_from(span.0).map_err(|_| ObjectError::TextSizeOverflow)?,
            byte_count: usize::try_from(span.1).map_err(|_| ObjectError::TextSizeOverflow)?,
        });
    }
    Ok(scalar_arguments)
}

/// Scalar-result custody for one roster row: the caller's durable-home slot
/// the result occupies, its plan placement, and the exact normalization
/// instruction that produces the durable definition.
///
/// The normalization is emitted immediately after the call inside the same
/// block, reads the call's raw result operand, and its output vreg is the
/// durable definition every downstream use resolves. A result home exists
/// exactly when the selected instruction defines a result and the evaluated
/// plan assigns one register of that shape; `scalar_result` is `None`
/// otherwise.
fn foreign_scalar_result_custody(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    fragment: &machine_code::PlacedFunctionFragment,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
    home_offsets: &[(semantic_vocabulary::OperationId, u32)],
    caller: MachineId,
    owner: CallSiteOwner,
) -> Result<Option<machine_code::ForeignCallScalarResultRecord>, ObjectError> {
    let invalid = || ObjectError::InvalidForeignCallArgument { caller, owner };
    let plan = &record.call.binding.boundary_entry_plan.call;
    let (Some(home), Some(placement)) = (&record.call.result_home, &plan.result) else {
        if record.call.result_home.is_none() && plan.result.is_none() {
            return Ok(None);
        }
        return Err(invalid());
    };
    let ScalarType::Integer(integer) = home.scalar_type else {
        return Err(invalid());
    };
    if integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
        || !matches!(integer.bits(), 8 | 16 | 32 | 64)
        || home.defining_operation != record.operation
        || home.shape
            != calling_conventions::ValueShape::integer(integer.bits() / 8, integer.bits() / 8)
        || placement.shape != home.shape
        || !matches!(
            placement.locations.as_slice(),
            [
                ValueLocation::Register {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] if *byte_size == home.shape.byte_size
        )
    {
        return Err(invalid());
    }
    let Some(position) = block
        .instructions
        .iter()
        .position(|candidate| candidate.id == record.instruction)
    else {
        return Err(invalid());
    };
    let Some(normalization) = block.instructions.get(position + 1) else {
        return Err(invalid());
    };
    let Some(kind) = scalar_result_normalization_kind(home.scalar_type) else {
        return Err(invalid());
    };
    let [input, output] = normalization.operands.as_slice() else {
        return Err(invalid());
    };
    let Some(call_result) = instruction.operands.last() else {
        return Err(invalid());
    };
    let output_origin = function
        .virtual_registers
        .get(output.virtual_register.0 as usize)
        .map(|register| register.origin);
    if normalization.kind != kind
        || input.virtual_register != call_result.virtual_register
        || normalization.provenance.values.as_slice() != [home.source_value]
        || output_origin
            != Some(
                selected_instructions::VirtualRegisterOrigin::InstructionResult {
                    instruction: normalization.id,
                    source_value: home.source_value,
                },
            )
    {
        return Err(invalid());
    }
    let Some((offset, byte_count)) = instruction_span(fragment, normalization.id) else {
        return Err(invalid());
    };
    let Some(byte_offset) = home_offsets
        .iter()
        .find(|(operation, _)| *operation == home.defining_operation)
        .map(|(_, offset)| *offset)
    else {
        return Err(invalid());
    };
    Ok(Some(machine_code::ForeignCallScalarResultRecord {
        home: machine_code::UnitScalarHomeRecord {
            defining_operation: home.defining_operation,
            source_value: home.source_value,
            scalar_type: home.scalar_type,
            shape: home.shape,
            byte_offset,
        },
        source: placement.clone(),
        code_offset: usize::try_from(offset).map_err(|_| ObjectError::TextSizeOverflow)?,
        byte_count: usize::try_from(byte_count).map_err(|_| ObjectError::TextSizeOverflow)?,
    }))
}

/// The normalization kind one scalar result's durable definition emits.
fn scalar_result_normalization_kind(
    scalar_type: ScalarType,
) -> Option<selected_instructions::SelectedInstructionKind> {
    use selected_instructions::SelectedInstructionKind as Kind;
    use semantic_vocabulary::IntegerSign;
    match scalar_type {
        ScalarType::Boolean => Some(Kind::ZeroExtendU8),
        ScalarType::Integer(integer) => match (integer.sign(), integer.bits()) {
            (IntegerSign::Unsigned, 8) => Some(Kind::ZeroExtendU8),
            (IntegerSign::Unsigned, 16) => Some(Kind::ZeroExtendU16),
            (IntegerSign::Unsigned, 32) => Some(Kind::ZeroExtendU32),
            (IntegerSign::Unsigned, 64) | (IntegerSign::Signed, 64) => Some(Kind::CopyI64),
            (IntegerSign::Signed, 8) => Some(Kind::SignExtendI8),
            (IntegerSign::Signed, 16) => Some(Kind::SignExtendI16),
            (IntegerSign::Signed, 32) => Some(Kind::SignExtendI32),
            _ => None,
        },
        _ => None,
    }
}

/// The emitted interval one integer-immediate argument occupies: the register
/// copy whose result feeds the call operand, or the contiguous outgoing-slot
/// `FrameAddress`/`Store` pair this call's argument transport owns.
#[allow(clippy::too_many_arguments)]
fn scalar_immediate_span<F>(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    fragment: &machine_code::PlacedFunctionFragment,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
    argument: &target_operations::TargetUnitScalarCallArgument,
    register_operand: usize,
    fixed_view: &F,
) -> Option<(u64, u64)>
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
            instruction_span(fragment, materialization)
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
                operation: record.operation,
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
                || outgoing.byte_size != u32::from(*byte_size)
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
                ) && candidate.provenance.operations.as_slice() == [record.operation]
                    && candidate.provenance.values.as_slice() == [source_value]
            })?;
            let address = &block.instructions[address_index];
            let store = block.instructions.get(address_index + 1)?;
            let selected_instructions::SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: stored_bytes,
            } = store.kind
            else {
                return None;
            };
            if stored_bytes != stored
                || store.provenance.operations.as_slice() != [record.operation]
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
            let (address_offset, address_bytes) = instruction_span(fragment, address.id)?;
            let (store_offset, store_bytes) = instruction_span(fragment, store.id)?;
            if store_offset != address_offset + address_bytes {
                return None;
            }
            Some((address_offset, address_bytes + store_bytes))
        }
        _ => None,
    }
}

/// The producing call one scalar-call-result `Home` names: exactly one
/// internal or foreign call roster row claims the home's defining operation,
/// a foreign producer must record exactly this home as its result, and the
/// producer instruction defines the home's source value as its own
/// instruction result.
fn scalar_call_result_producer(
    function: &selected_instructions::SelectedFunction,
    home: &target_operations::TargetUnitScalarHomeRequirement,
) -> bool {
    let mut internal = function
        .calls
        .iter()
        .filter(|contract| contract.operation == home.defining_operation);
    let mut foreign = function
        .normalized_foreign_calls
        .iter()
        .filter(|record| record.operation == home.defining_operation);
    let (producer, result_home) = match (internal.next(), foreign.next()) {
        (Some(contract), None) if internal.next().is_none() => (contract.instruction, None),
        (None, Some(record)) if foreign.next().is_none() => {
            (record.instruction, Some(record.call.result_home.as_ref()))
        }
        _ => return false,
    };
    if let Some(recorded) = result_home
        && recorded != Some(home)
    {
        return false;
    }
    let Some(producer) = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
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

/// Image-side custody join for [`super::image_output::validate_executable_image`].
///
/// An image's foreign-call roster is the object's own emitted roster followed
/// by fragment-publication custody rows. Every appended row must rejoin object
/// custody: its caller is an object function, its owner is a semantic
/// operation attributed inside that function, and its `text_offset` — the
/// mutable import field — lies inside one attributed interval for that
/// operation. Owners stay unique across the whole roster.
pub(crate) fn image_foreign_calls_match_object(
    artifact: &ObjectArtifact,
    image_rows: &[ObjectForeignCall],
) -> bool {
    let Some(custody) = image_rows.strip_prefix(artifact.foreign_calls()) else {
        return false;
    };
    let mut seen = Vec::new();
    for row in image_rows {
        if seen.contains(&(row.machine, row.owner)) {
            return false;
        }
        seen.push((row.machine, row.owner));
    }
    custody.iter().all(|row| {
        let Some(operation) = row.owner.operation() else {
            return false;
        };
        artifact
            .functions()
            .iter()
            .any(|function| function.machine == row.machine)
            && artifact
                .semantic_code_attribution()
                .iter()
                .any(|row_attribution| {
                    row_attribution.machine == row.machine
                        && row_attribution.attribution.site
                            == machine_code::SemanticCodeSite::Operation(operation)
                        && row.text_offset >= row_attribution.text_offset
                        && row.text_offset
                            < row_attribution
                                .text_offset
                                .saturating_add(row_attribution.attribution.byte_count)
                })
    })
}
