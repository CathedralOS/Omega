//! Foreign-call custody projected from the independently replayed fragment
//! publication source.
//!
//! The fragment route publishes `ObjectArtifact::foreign_calls` before stack
//! sizing and image emission. [`derive_normalized_foreign_call_custody`]
//! projects every normalized foreign call from the retained selected-plan
//! roster and frame layout into one exact
//! [`ObjectForeignCall`] row per call site.
//! Import fields alone are not a call inventory: kernel entries are fully
//! encoded and have no relocation. Both mechanisms retain the same binding
//! and scalar custody; only imports carry floating controls and opaque
//! user-stack contributions.
//!
//! Scalar custody preserves fixed integer, Boolean and IEEE types: an
//! integer immediate names the register copy or the contiguous
//! outgoing-slot address-plus-store pair that materializes it, a preceding
//! scalar-call result keeps its `Home` source and its allocated slot in the
//! caller's durable scalar-home area, and a scalar result records the exact
//! normalization instruction that produces the durable definition after the
//! call. Result and `Home` records share one per-caller home-area map so a
//! consumer names the producer's slot exactly. Floating controls name the
//! call-owned frame slot and the selected save/restore intervals.
//! Incoming parameters, block parameters and Boolean/IEEE constants use
//! ordinary `SelectedCall` custody: the argument retains its exact SSA value
//! and type, and its span names the call instruction. The validated selected
//! graph retains the exact constant payload; physical replay owns its transport,
//! entry transport and edge copies, including values on ranked backedges.
//! No parameter is relabeled as an immediate or an operation-defined home.
//! Callback-bearing rows carry exact custody: the placed materialization
//! record and the selected callback argument agree on destination, span, and
//! relocation shape, and the plan's parameter roster accounts for the slot.

mod floating_control;

use calling_conventions::ValueLocation;
use semantic_vocabulary::{MachineId, ScalarType};
use target_operations::CallSiteOwner;

use super::carriers::{ObjectArtifact, ObjectForeignCall};
use super::errors::ObjectError;

/// One derived custody row per normalized foreign call in the selected roster.
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
    // The selected roster, not the import relocation list, inventories calls.
    // A kernel entry is already encoded and deliberately has no import field.
    for function in &selected.functions {
        let fragment =
            caller_fragment(section, function.machine).ok_or(ObjectError::TextSizeOverflow)?;
        for (ordinal, record) in function.normalized_foreign_calls.iter().enumerate() {
            let owner = CallSiteOwner::Operation(record.operation);
            let invalid = || ObjectError::InvalidForeignCallSite {
                caller: function.machine,
                owner,
                offset: 0,
            };
            if record.call.binding.native_target() != Some(section.target)
                || !record.call.binding.has_valid_entry_mechanism()
            {
                return Err(invalid());
            }
            let mut instructions = function.blocks.iter().flat_map(|block| {
                block
                    .instructions
                    .iter()
                    .filter(move |instruction| instruction.id == record.instruction)
                    .map(move |instruction| (block, instruction))
            });
            let (Some((block, instruction)), None) = (instructions.next(), instructions.next())
            else {
                return Err(invalid());
            };
            if !matches!(
                instruction.kind,
                selected_instructions::SelectedInstructionKind::NormalizedForeignCall {
                    boundary, ordinal: actual,
                } if boundary == record.call.boundary && usize::try_from(actual) == Ok(ordinal)
            ) {
                return Err(invalid());
            }
            let mut fields = section
                .unresolved_normalized_foreign_calls
                .iter()
                .filter(|field| {
                    field.caller == function.machine && field.operation == record.operation
                });
            let field = match (record.call.binding.import(), fields.next(), fields.next()) {
                (Some(_), Some(field), None) => {
                    if field.instruction != record.instruction
                        || field.boundary != record.call.boundary
                        || usize::try_from(field.ordinal) != Ok(ordinal)
                        || field.block != block.id
                        || field.state != machine_code::NormalizedForeignCallResolutionState::UnresolvedImportFieldV1
                    {
                        return Err(invalid());
                    }
                    Some(field)
                }
                (None, None, None) => None,
                _ => return Err(invalid()),
            };
            let text_offset = if let Some(field) = field {
                usize::try_from(field.field_section_offset).map_err(|_| invalid())?
            } else {
                usize::try_from(
                    instruction_span(fragment, record.instruction)
                        .ok_or_else(invalid)?
                        .0,
                )
                .map_err(|_| invalid())?
            };
            let home_offsets = durable_home_offsets(function);
            let scalar_arguments = foreign_scalar_argument_custody(
                function,
                block,
                fragment,
                record,
                instruction,
                &home_offsets,
                function.machine,
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
                function.machine,
                owner,
            )?;
            let callback_addresses = callback_address_custody(
                record,
                field.map_or(&[], |field| field.callback_materializations.as_slice()),
                function.machine,
                owner,
            )?;
            let mut abstracted = optimized
                .optimized()
                .plan()
                .functions
                .iter()
                .filter(|candidate| candidate.machine == function.machine);
            let (Some(abstracted), None) = (abstracted.next(), abstracted.next()) else {
                return Err(invalid());
            };
            let mut ordinals =
                abstracted
                    .operations
                    .iter()
                    .enumerate()
                    .filter_map(|(index, operation)| match operation {
                        abstract_operations::AbstractOperation::BoundaryCall {
                            psi_operation,
                            ..
                        } if *psi_operation == record.operation => Some(index),
                        _ => None,
                    });
            let operation_ordinal = match (ordinals.next(), ordinals.next()) {
                (Some(ordinal), None) => ordinal,
                (None, _) => {
                    return Err(ObjectError::ForeignCallOwnerNotInProvenance {
                        caller: function.machine,
                        owner,
                    });
                }
                _ => {
                    return Err(ObjectError::DuplicateForeignCallOwner {
                        caller: function.machine,
                        owner,
                    });
                }
            };
            let mut frames = layout
                .functions
                .iter()
                .filter(|frame| frame.machine == function.machine);
            let (Some(frame), None) = (frames.next(), frames.next()) else {
                return Err(invalid());
            };
            let committed = frame
                .frame_size_bytes
                .checked_sub(frame.red_zone_resident_bytes)
                .and_then(|extent| {
                    extent.checked_add(
                        if record.call.binding.import().is_some()
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
            let controls = if record.call.binding.import().is_some() {
                Some(
                    floating_control::derive(
                        section.target,
                        function,
                        block,
                        fragment,
                        frame,
                        record,
                    )
                    .ok_or_else(invalid)?,
                )
            } else {
                if !record.call.callbacks.is_empty() {
                    return Err(invalid());
                }
                None
            };
            custody.push(ObjectForeignCall {
                machine: function.machine,
                owner,
                operation_ordinal,
                binding: record.call.binding.clone(),
                provider_execution: record.call.provider_execution.into(),
                caller_live_bytes,
                scalar_arguments,
                callback_addresses,
                scalar_result,
                x86_floating_control: controls.as_ref().and_then(|controls| controls.x86),
                aarch64_floating_control: controls.as_ref().and_then(|controls| controls.aarch64),
                text_offset,
            });
        }
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
        if let Some(placement) = &contract.call.result_placement
            && let Some(operation) = contract.owner.operation()
        {
            producers.push((contract.instruction, operation, placement.shape.byte_size));
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
                // producer row is resolved over the caller's call rosters and,
                // for observation-produced homes, the instruction roster.
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

/// The selected instruction a scalar-home producer roster row emits, found by
/// the home's defining operation over the caller's scalar call rosters.
fn producer_instruction(
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
                .map(|record| (record.operation, record.instruction)),
        )
        .find(|(candidate, _)| *candidate == operation)
        .map(|(_, instruction)| instruction)
        .or_else(|| scalar_observation_producer(function, operation, source_value))
}

/// The non-call instruction producing a scalar-home value: a field or
/// primitive read, byte observation, or membership check whose provenance
/// claims the home's defining operation and whose result register is the
/// home's source value. Exactly one such instruction may exist — reads staged
/// over several instructions still define the value exactly once.
fn scalar_observation_producer(
    function: &selected_instructions::SelectedFunction,
    operation: semantic_vocabulary::OperationId,
    source_value: semantic_vocabulary::ValueId,
) -> Option<selected_instructions::SelectedInstructionId> {
    let mut producers = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| {
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

/// Callback materialization custody for one roster row: the selected call's
/// `callback` argument, the evaluated plan's materialization roster, and the
/// placed section record must all agree — the placed bytes carry the exact
/// destination, span, and relocation shape the object writer rejoins.
fn callback_address_custody(
    record: &selected_instructions::SelectedNormalizedForeignCall,
    placed: &[machine_code::PlacedCallbackAddressMaterialization],
    caller: MachineId,
    owner: CallSiteOwner,
) -> Result<Vec<machine_code::CallbackAddressMaterialization>, ObjectError> {
    let invalid = || ObjectError::InvalidForeignCallSite {
        caller,
        owner,
        offset: 0,
    };
    let plan = &record.call.binding.boundary_entry_plan.call;
    // The fixup roster preserves the selected call's callback roster order;
    // each placed group joins its roster entry positionally, and the plan
    // roster must hold exactly one matching nominal destination per entry.
    if record.call.callbacks.len() != placed.len()
        || placed.len() != plan.callback_materializations.len()
    {
        return Err(invalid());
    }
    let mut custody = Vec::with_capacity(placed.len());
    for (callback, placed) in record.call.callbacks.iter().zip(placed.iter()) {
        if plan
            .callback_materializations
            .iter()
            .filter(|row| row.destination == callback.destination)
            .count()
            != 1
        {
            return Err(invalid());
        }
        let destination = match placed.destination {
            machine_code::CallbackAddressDestination::Register(register) => {
                machine_code::CallbackAddressDestination::Register(register)
            }
            machine_code::CallbackAddressDestination::OutgoingStack { byte_offset } => {
                machine_code::CallbackAddressDestination::OutgoingStack { byte_offset }
            }
            machine_code::CallbackAddressDestination::RecordField {
                root_register,
                byte_offset,
            } => {
                if callback.field_byte_offset
                    != Some(usize::try_from(byte_offset).map_err(|_| invalid())?)
                {
                    return Err(invalid());
                }
                machine_code::CallbackAddressDestination::RecordField {
                    root_register,
                    byte_offset,
                }
            }
        };
        let encoding = match placed.encoding {
            machine_code::PlacedCallbackAddressEncoding::X86_64Relative32 {
                relocation_function_offset,
                ..
            } => machine_code::CallbackAddressEncoding::X86_64Relative32 {
                relocation_offset: usize::try_from(relocation_function_offset)
                    .map_err(|_| invalid())?,
            },
            machine_code::PlacedCallbackAddressEncoding::Aarch64PageAddress {
                page_relocation_function_offset,
                page_offset_relocation_function_offset,
                ..
            } => machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                page_relocation_offset: usize::try_from(page_relocation_function_offset)
                    .map_err(|_| invalid())?,
                page_offset_relocation_offset: usize::try_from(
                    page_offset_relocation_function_offset,
                )
                .map_err(|_| invalid())?,
            },
        };
        custody.push(machine_code::CallbackAddressMaterialization {
            target: callback.clone(),
            destination,
            code_offset: usize::try_from(placed.code_function_offset).map_err(|_| invalid())?,
            byte_count: usize::try_from(placed.code_byte_count).map_err(|_| invalid())?,
            encoding,
        });
    }
    Ok(custody)
}

/// Scalar-argument custody for one roster row's admitted fixed scalar lane.
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
/// emitted copy or outgoing-slot store. Other sources and callback
/// materialization fail closed rather than claiming custody they cannot prove.
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
    // Only a parameter-destination materialization mints a plan slot; a
    // field destination writes through its root record parameter's own
    // slot, which the structural argument roster already carries.
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
    if plan.parameters.len()
        != record.call.scalar_arguments.len()
            + record.call.structural_arguments.len()
            + parameter_slot_materializations
    {
        return Err(invalid());
    }
    // The call's operand roster is every register-resident argument word in
    // canonical bank order — GPR scalars and structural referent pointers,
    // then IEEE inputs — followed by the optional scalar result definition.
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
    // A callback materialization with a `Parameter` destination claims the
    // parameter's register placement without occupying a call-row operand —
    // the address bytes travel inside the call encoding — so its position is
    // excluded from the operand roster here just as the arity check above
    // already does.
    let mut register_positions = plan
        .parameters
        .iter()
        .enumerate()
        .filter(|(position, placement)| {
            !record.call.callbacks.iter().any(|callback| {
                usize::try_from(callback.application.native_ordinal) == Ok(*position)
                    && matches!(
                        callback.destination,
                        calling_conventions::NativePlace::Parameter(_)
                    )
            }) && matches!(
                placement.locations.as_slice(),
                [ValueLocation::Register { .. }]
            )
        })
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    register_positions.sort_by_key(|position| {
        plan.parameters[*position].shape.class == calling_conventions::ValueClass::Float
    });
    let mut scalar_arguments = Vec::new();
    for (scalar_ordinal, argument) in record.call.scalar_arguments.iter().enumerate() {
        let native_position = usize::try_from(argument.parameter_index).map_err(|_| invalid())?;
        let shape = scalar_shape(argument.source.scalar_type()).ok_or_else(invalid)?;
        if scalar_ordinal.checked_sub(1).is_some_and(|previous| {
            record.call.scalar_arguments[previous].parameter_index >= argument.parameter_index
        }) || plan.parameters.get(native_position) != Some(&argument.placement)
            || argument.placement.shape != shape
        {
            return Err(invalid());
        }
        let register_operand = register_positions
            .iter()
            .position(|position| *position == native_position)
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
                instruction_span(fragment, record.instruction).ok_or_else(invalid)?,
            ),
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
        };
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
/// The normalization follows the call (and, for imports, its floating-control
/// restore) in the same block, reads the raw result operand, and its output vreg is the
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
    let shape = scalar_shape(home.scalar_type).ok_or_else(invalid)?;
    if home.defining_operation != record.operation
        || home.shape != shape
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
    // Only imports restore user-space floating controls before normalizing
    // their result. Kernel entries normalize immediately after supervisor return.
    let normalization_position = if record.call.binding.import().is_some() {
        let Some(restore) = block.instructions.get(position + 1) else {
            return Err(invalid());
        };
        if restore.kind
            != (selected_instructions::SelectedInstructionKind::RestoreFloatingControl {
                slot: selected_instructions::LocalStorageSlotId::Boundary {
                    operation: record.operation,
                },
            })
        {
            return Err(invalid());
        }
        position + 2
    } else {
        position + 1
    };
    let Some(normalization) = block.instructions.get(normalization_position) else {
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
        || normalization.provenance.operations.as_slice() != [record.operation]
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
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => {
            Some(Kind::Float32ToBits)
        }
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => {
            Some(Kind::Float64ToBits)
        }
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
    }
}

/// Object custody derives the same fixed ABI shape from semantic type, never
/// from the width of a scratch register or a producer-supplied placement.
fn scalar_shape(scalar_type: ScalarType) -> Option<calling_conventions::ValueShape> {
    use calling_conventions::ValueShape;
    match scalar_type {
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => {
            Some(ValueShape::float(4))
        }
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => {
            Some(ValueShape::float(8))
        }
        ScalarType::Integer(integer)
            if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            Some(ValueShape::integer(
                u32::from(integer.bits() / 8),
                integer.bits() / 8,
            ))
        }
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
        .filter(|contract| contract.owner.operation() == Some(home.defining_operation));
    let mut foreign = function
        .normalized_foreign_calls
        .iter()
        .filter(|record| record.operation == home.defining_operation);
    let (producer, result_home) = match (internal.next(), foreign.next()) {
        (Some(contract), None) if internal.next().is_none() => (contract.instruction, None),
        (None, Some(record)) if foreign.next().is_none() => {
            (record.instruction, Some(record.call.result_home.as_ref()))
        }
        (None, None) => {
            // Observation-produced homes live outside the call rosters: a
            // field, byte, or membership read defines the value in its own
            // instruction. The roster tail still re-proves the result origin.
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

/// Image-side custody join for [`super::image_output::validate_direct_executable_image`].
///
/// Both publication routes retain their complete roster in the object before
/// emission. Images cannot append late calls or weaken their retained evidence.
pub(crate) fn image_foreign_calls_match_object(
    artifact: &ObjectArtifact,
    image_rows: &[ObjectForeignCall],
) -> bool {
    image_rows == artifact.foreign_calls()
}
