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

mod descriptor_input;
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
            let owner = CallSiteOwner::from(record.invocation_site);
            let invalid = |reason| ObjectError::InvalidForeignCallProjection {
                caller: function.machine,
                owner,
                reason,
            };
            if record.call.binding.native_target() != Some(section.target)
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
            let mut fields = section
                .unresolved_normalized_foreign_calls
                .iter()
                .filter(|field| {
                    field.caller == function.machine
                        && field.invocation_site == record.invocation_site
                });
            let field = match (record.call.binding.import(), fields.next(), fields.next()) {
                (Some(_), Some(field), None) => {
                    if field.instruction != record.instruction
                        || field.boundary != record.call.boundary
                        || usize::try_from(field.ordinal) != Ok(ordinal)
                        || field.block != block.id
                        || field.state != machine_code::NormalizedForeignCallResolutionState::UnresolvedImportFieldV1
                    {
                        return Err(invalid("placed import field does not match the selected call"));
                    }
                    Some(field)
                }
                (None, None, None) => None,
                _ => {
                    return Err(invalid(
                        "import relocation count does not match the native binding",
                    ));
                }
            };
            let text_offset = if let Some(field) = field {
                usize::try_from(field.field_section_offset)
                    .map_err(|_| invalid("import relocation offset is not addressable"))?
            } else {
                usize::try_from(
                    fragment_instruction_span(fragment, record.instruction)
                        .ok_or_else(|| invalid("native call has no emitted instruction span"))?
                        .0,
                )
                .map_err(|_| invalid("native instruction offset is not addressable"))?
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
                return Err(invalid(
                    "caller is missing or repeated in the abstract operation roster",
                ));
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
                        } if record.invocation_site
                            == semantic_vocabulary::BoundaryInvocationSite::Operation(
                                *psi_operation,
                            ) =>
                        {
                            Some(index)
                        }
                        abstract_operations::AbstractOperation::ExternalCompletion {
                            psi_edge,
                            ..
                        } if record.invocation_site
                            == semantic_vocabulary::BoundaryInvocationSite::ExternalCompletion(
                                *psi_edge,
                            ) =>
                        {
                            Some(index)
                        }
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
                        if record.call.binding.same_stack_contribution().is_some()
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
            let controls = if record.call.binding.same_stack_contribution().is_some()
                && record.invocation_site.operation().is_some()
            {
                Some(
                    floating_control::derive(
                        section.target,
                        function,
                        block,
                        fragment,
                        frame,
                        record,
                    )
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
pub fn fragment_instruction_span(
    fragment: &machine_code::PlacedFunctionFragment,
    instruction: selected_instructions::SelectedInstructionId,
) -> Option<(u64, u64)> {
    fragment
        .instruction_span(instruction)
        .map(|span| (span.section_offset, span.byte_count))
}

/// The byte offset one transported span's covered instructions end at, or
/// `None` when a covered step has no span or coverage is non-contiguous.
pub fn transported_span_end(
    fragment: &machine_code::PlacedFunctionFragment,
    block: &selected_instructions::SelectedBlock,
    call_position: usize,
    normalization_position: usize,
) -> Option<u64> {
    block.instructions[call_position..normalization_position]
        .iter()
        .try_fold(None, |previous, step| {
            let (start, count) = fragment_instruction_span(fragment, step.id)?;
            if previous.is_some_and(|end| end != start) {
                return None;
            }
            Some(Some(start.checked_add(count)?))
        })
        .flatten()
}

/// The durable scalar-home area one caller's producers allocate, in emitted
/// instruction order.
///
/// Homes on this route are register-resident — the durable definition is the
/// result's normalization output — so no frame slot exists for them. The
/// records still name each home by its offset in the caller's dense
/// durable-home area: every scalar-producing call row is one producer, its
/// The unique `(block, instruction)` site a roster row's instruction id
/// names, or `None` when it resolves to zero or several sites.
pub fn unique_block_instruction<'a>(
    function: &'a selected_instructions::SelectedFunction,
    instruction: selected_instructions::SelectedInstructionId,
) -> Option<(
    &'a selected_instructions::SelectedBlock,
    &'a selected_instructions::SelectedInstruction,
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

/// slot 8-aligned and sized by the home shape. Argument `Home` sources and
/// `scalar_result` records share this map so a consumer rejoins the exact
/// producer slot.
pub fn durable_home_offsets(
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

/// The non-call instruction producing a scalar-home value: a field or
/// primitive read, byte observation, or membership check whose provenance
/// claims the home's defining operation and whose result register is the
/// home's source value. Exactly one such instruction may exist — reads staged
/// over several instructions still define the value exactly once.
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
    let placements = record.call.binding.operand_placements();
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
    if placements.len()
        != record.call.scalar_arguments.len()
            + record.call.structural_arguments.len()
            + parameter_slot_materializations
    {
        return Err(invalid());
    }
    // Parameter identities are not operand offsets: an aggregate may
    // contribute multiple fragments or one owned-copy pointer.
    let register_positions =
        selected_instructions::calls::constraints::foreign_argument_registers(&record.call);
    let register_arguments = register_positions.len();
    if instruction.operands.len()
        != register_arguments + foreign_result_operand_count(record).ok_or_else(invalid)?
    {
        return Err(invalid());
    }
    if let target_operations::TargetBoundaryResult::Structural(_) = record.call.result {
        let placement = plan.result.as_ref().ok_or_else(invalid)?;
        for (location, operand) in placement
            .locations
            .iter()
            .zip(&instruction.operands[register_arguments..])
        {
            let register = location.direct_register().ok_or_else(invalid)?;
            if !fixed_view(operand, register) {
                return Err(invalid());
            }
        }
    }
    let mut scalar_arguments = Vec::new();
    for (scalar_ordinal, argument) in record.call.scalar_arguments.iter().enumerate() {
        let native_position = usize::try_from(argument.parameter_index).map_err(|_| invalid())?;
        let shape = scalar_shape(argument.source.scalar_type()).ok_or_else(invalid)?;
        if scalar_ordinal.checked_sub(1).is_some_and(|previous| {
            record.call.scalar_arguments[previous].parameter_index >= argument.parameter_index
        }) || placements.get(native_position) != Some(&argument.placement)
            || argument.placement.shape != shape
        {
            return Err(invalid());
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
                fragment_instruction_span(fragment, record.instruction).ok_or_else(invalid)?,
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

/// Reconstruct completed aggregate storage from the retained ABI and selected
/// memory effects. Plain aggregates use their real place/slot, never a scalar
/// home or a fabricated sum layout. Physical instruction replay has already
/// checked these selected instructions against their placed bytes.
pub fn structural_result_capture(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    fragment: &machine_code::PlacedFunctionFragment,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
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
    let (call_offset, call_bytes) = fragment_instruction_span(fragment, instruction.id)?;
    let (address_offset, address_bytes) = fragment_instruction_span(fragment, address.id)?;
    if address_bytes == 0 || address_offset < call_offset.checked_add(call_bytes)? {
        return None;
    }
    if indirect {
        let [argument] = operands else { return None };
        let origin = function
            .virtual_registers
            .get(argument.virtual_register.0 as usize)?
            .origin;
        let VirtualRegisterOrigin::AbiTransport {
            instruction: prepared,
            place,
            byte_offset: 0,
        } = origin
        else {
            return None;
        };
        let preparation = block
            .instructions
            .iter()
            .take(call_position)
            .find(|candidate| candidate.id == prepared)?;
        let [output] = preparation.operands.as_slice() else {
            return None;
        };
        let (prepared_offset, prepared_bytes) = fragment_instruction_span(fragment, prepared)?;
        return (place == result.place
            && output.virtual_register == argument.virtual_register
            && preparation.kind == address.kind
            && memory_matches(
                prepared,
                0,
                shape.byte_size,
                SelectedMemoryAccessRole::AddressLocal { slot },
            )
            && prepared_bytes != 0
            && prepared_offset.checked_add(prepared_bytes)? <= call_offset)
            .then_some(());
    }
    let mut previous_end = address_offset.checked_add(address_bytes)?;
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
            let (offset, width) = fragment_instruction_span(fragment, store.id)?;
            if width == 0 || offset < previous_end {
                return None;
            }
            previous_end = offset.checked_add(width)?;
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
        let (offset, width) = fragment_instruction_span(fragment, store.id)?;
        if width == 0 || offset < previous_end {
            return None;
        }
        previous_end = offset.checked_add(width)?;
        position = position.checked_add(1)?;
    }
    Some(())
}

/// ABI result fragments and hidden destination pointers are real operands;
/// Unit and aggregate results cannot be inferred from an optional scalar home.
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
    if matches!(
        record.call.result,
        target_operations::TargetBoundaryResult::Structural(_)
    ) {
        structural_result_capture(function, block, fragment, record, instruction)
            .ok_or_else(invalid)?;
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
        return Err(invalid());
    };
    let shape = scalar_shape(home.scalar_type).ok_or_else(invalid)?;
    if Some(home.defining_operation) != record.invocation_site.operation()
        || home.shape != shape
        || placement.shape != home.shape
        || !placement.is_complete_scalar_result()
    {
        return Err(invalid());
    }
    let transport = selected_instructions::calls::normalized_foreign::return_transport::resolve(
        function, block, record,
    )
    .ok_or_else(invalid)?;
    let normalization = block
        .instructions
        .get(transport.normalization_position)
        .ok_or_else(invalid)?;
    let Some(kind) = scalar_result_normalization_kind(
        home.scalar_type,
        matches!(
            placement.locations.as_slice(),
            [ValueLocation::Indirect { .. }]
        ),
    ) else {
        return Err(invalid());
    };
    let [input, output] = normalization.operands.as_slice() else {
        return Err(invalid());
    };
    let output_origin = function
        .virtual_registers
        .get(output.virtual_register.0 as usize)
        .map(|register| register.origin);
    if normalization.kind != kind
        || transport.result_registers.as_slice() != [input.virtual_register]
        || normalization.provenance.operations.as_slice()
            != [record.invocation_site.operation().ok_or_else(invalid)?]
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
    let Some((offset, byte_count)) = fragment_instruction_span(fragment, normalization.id) else {
        return Err(invalid());
    };
    let transported_end = transported_span_end(
        fragment,
        block,
        transport.call_position,
        transport.normalization_position,
    )
    .ok_or_else(invalid)?;
    if transported_end != offset {
        return Err(invalid());
    }
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
    indirect: bool,
) -> Option<selected_instructions::SelectedInstructionKind> {
    use selected_instructions::SelectedInstructionKind as Kind;
    use semantic_vocabulary::IntegerSign;
    if indirect && matches!(scalar_type, ScalarType::IeeeFloat(_)) {
        return Some(Kind::CopyI64);
    }
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
    calling_conventions::ValueShape::for_scalar(scalar_type)
}

/// The emitted interval one integer-immediate argument occupies: the register
/// copy whose result feeds the call operand, or the contiguous outgoing-slot
/// `FrameAddress`/`Store` pair this call's argument transport owns.
#[allow(clippy::too_many_arguments)]
pub fn scalar_immediate_span<F>(
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
            fragment_instruction_span(fragment, materialization)
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
            let store = block.instructions.get(address_index + 1)?;
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
            let (address_offset, address_bytes) = fragment_instruction_span(fragment, address.id)?;
            let (store_offset, store_bytes) = fragment_instruction_span(fragment, store.id)?;
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
pub fn scalar_call_result_producer(
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
        .filter(|record| record.invocation_site.operation() == Some(home.defining_operation));
    let (producer, result_home) = match (internal.next(), foreign.next()) {
        (Some(contract), None) if internal.next().is_none() => (contract.instruction, None),
        (None, Some(record)) if foreign.next().is_none() => {
            (record.instruction, Some(record.call.result.scalar_home()))
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

/// Exact operation or terminal-edge source custody, with disjoint namespaces.
/// Selection may prepend zero-storage establishment charges to the next
/// emitted instruction. Those operation identities remain real source/fuel
/// custody, not owners of its foreign invocation: a returning call is the last
/// operation, while a completion owns the single explicit edge. The retained
/// call record and source graph establish the owner; this query does not infer
/// one from a constructor's numeric identity or discard its charges.
pub fn foreign_invocation_provenance_matches(
    provenance: &selected_instructions::SelectedInstructionProvenance,
    site: semantic_vocabulary::BoundaryInvocationSite,
) -> bool {
    match site {
        semantic_vocabulary::BoundaryInvocationSite::Operation(operation) => {
            provenance.operations.last() == Some(&operation)
                && provenance.edges.is_empty()
                && provenance.fuel_settles_only_claimed_sites()
        }
        semantic_vocabulary::BoundaryInvocationSite::ExternalCompletion(edge) => {
            provenance.edges.as_slice() == [edge] && provenance.fuel_settles_only_claimed_sites()
        }
    }
}
