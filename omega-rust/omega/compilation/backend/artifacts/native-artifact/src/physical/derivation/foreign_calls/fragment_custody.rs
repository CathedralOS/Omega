//! Reconstruct fragment-published external-entry, selected-call, and scalar custody
//! before the foreign-call derivation accepts the projected physical records.
//! Imports rejoin exact relocation fields; kernel entries instead decode the
//! complete placed instruction against the retained number and ABI operands.
//! Neither path can replace the shared source, argument, or result checks.

use crate::OptimizedBoundaryOccurrence;
use crate::physical::derivation::foreign_calls::{
    fixed_scalar_shape, floating_control, fragment_instruction_span,
};
use crate::physical::fragment_publication::FragmentPublicationBinding;
use calling_conventions::ValueLocation;
use semantic_vocabulary::ScalarType;

/// The selected-plan roster row for one normalized-foreign D41 child, plus
/// its unresolved field and symbol for imports only. Kernel entries have no
/// import: their independently decoded instruction is the physical entry.
#[derive(Clone, Copy)]
pub(super) struct FragmentForeignCallRejoin<'a> {
    pub(super) import: Option<(
        &'a object_file::RelocationFreeObjectUnresolvedForeignCall,
        &'a object_file::RelocationFreeObjectNormalizedImport,
    )>,
    pub(super) record: &'a selected_instructions::SelectedNormalizedForeignCall,
}

/// Independently replays the custody projection's join: the unique unresolved
/// import field owned by this call site, its declared import symbol, and the
/// selected roster row retaining the call's evaluated binding. A substituted
/// plan row, placement, or provider binding fails here rather than realizing
/// silently.
pub(super) fn fragment_normalized_foreign_rejoin<'a>(
    publication: &'a FragmentPublicationBinding,
    occurrence: &OptimizedBoundaryOccurrence,
    foreign: &image_emission::ObjectForeignCall,
    structural_arguments: &[terminal_psi::StructuralArgument],
) -> Result<FragmentForeignCallRejoin<'a>, &'static str> {
    let plan = publication.relocation_free_object();
    let selected = publication.selected_plan();
    let mut functions = selected
        .functions
        .iter()
        .filter(|function| function.machine == occurrence.machine());
    let (Some(function), None) = (functions.next(), functions.next()) else {
        return Err("normalized foreign D41 child names an absent selected function");
    };
    let mut records = function
        .normalized_foreign_calls
        .iter()
        .enumerate()
        .filter(|(_, record)| {
            record.operation == occurrence.operation()
                && record.call.boundary == occurrence.boundary()
        });
    let (Some((ordinal, record)), None) = (records.next(), records.next()) else {
        return Err("normalized foreign D41 child cannot rejoin one selected call");
    };
    let mut instructions = function.blocks.iter().flat_map(|block| {
        block
            .instructions
            .iter()
            .filter(move |instruction| instruction.id == record.instruction)
            .map(move |instruction| (block, instruction))
    });
    let (Some((block, instruction)), None) = (instructions.next(), instructions.next()) else {
        return Err("normalized foreign D41 roster row names an absent instruction");
    };
    if !matches!(instruction.kind,
        selected_instructions::SelectedInstructionKind::NormalizedForeignCall {
            boundary, ordinal: actual,
        } if boundary == occurrence.boundary() && usize::try_from(actual) == Ok(ordinal))
    {
        return Err("normalized foreign D41 instruction kind does not match its roster row");
    }
    let mut fields = plan
        .unresolved_normalized_foreign_calls
        .iter()
        .filter(|candidate| {
            candidate.resolution.caller == occurrence.machine()
                && candidate.resolution.operation == occurrence.operation()
        });
    let import = match (record.call.binding.import(), fields.next(), fields.next()) {
        (Some(_), Some(field), None) => {
            let resolution = &field.resolution;
            if resolution.boundary != occurrence.boundary()
                || resolution.instruction != instruction.id
                || resolution.block != block.id
                || usize::try_from(resolution.ordinal) != Ok(ordinal)
                || usize::try_from(resolution.field_section_offset) != Ok(foreign.text_offset)
                || resolution.state
                    != machine_code::NormalizedForeignCallResolutionState::UnresolvedImportFieldV1
            {
                return Err("normalized foreign D41 import field does not match its selected call");
            }
            let mut imports = plan.normalized_imports.iter().filter(|import| {
                import.symbol == field.symbol
                    && import.boundary == resolution.boundary
                    && import.ordinal == resolution.ordinal
            });
            let (Some(import), None) = (imports.next(), imports.next()) else {
                return Err(
                    "normalized foreign D41 child cannot rejoin one declared import symbol",
                );
            };
            if import.name
                != object_file::canonical_normalized_foreign_import_symbol_name(
                    import.boundary,
                    import.ordinal,
                )
            {
                return Err("normalized foreign D41 import symbol drifted from its canonical name");
            }
            Some((field, import))
        }
        (None, None, None) => {
            let kernel = record
                .call
                .binding
                .system_call()
                .ok_or("normalized external entry mechanism is absent")?;
            let section = publication.text_section();
            let fragment = section
                .functions
                .iter()
                .find(|fragment| fragment.machine == occurrence.machine())
                .ok_or("kernel entry has no placed caller")?;
            let span = fragment
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .find(|span| span.instruction == instruction.id)
                .ok_or("kernel entry has no placed instruction")?;
            let start =
                usize::try_from(span.section_offset).map_err(|_| "kernel entry offset overflow")?;
            let size =
                usize::try_from(span.byte_count).map_err(|_| "kernel entry size overflow")?;
            let end = start.checked_add(size).ok_or("kernel entry end overflow")?;
            let bytes = section
                .bytes
                .get(start..end)
                .ok_or("kernel entry is outside placed text")?;
            if start != foreign.text_offset {
                return Err("kernel entry moved away from its selected instruction");
            }
            let views = instruction
                .operands
                .iter()
                .map(|operand| operand.fixed_view)
                .collect::<Option<Vec<_>>>()
                .ok_or("kernel entry lacks fixed operand homes")?;
            let accesses = instruction
                .operands
                .iter()
                .map(|operand| operand.access)
                .collect::<Vec<_>>();
            match section.target.architecture {
                target::Architecture::X86_64 => {
                    isa_x86_64::validate_x86_64_selected_system_call_form(
                        section.target,
                        isa_x86_64::validated_x86_64_physical_register_model(),
                        instruction.kind,
                        span.alternative,
                        &views,
                        &accesses,
                        kernel.number,
                        bytes,
                    )
                    .map_err(|_| "kernel entry bytes disagree with its selected binding")?;
                }
                target::Architecture::Aarch64 => {
                    isa_aarch64::validate_aarch64_selected_system_call_form(
                        section.target,
                        isa_aarch64::validated_aarch64_physical_register_model(),
                        instruction.kind,
                        span.alternative,
                        &views,
                        &accesses,
                        kernel.number,
                        bytes,
                    )
                    .map_err(|_| "kernel entry bytes disagree with its selected binding")?;
                }
            }
            None
        }
        _ => return Err("external call import fields disagree with its entry mechanism"),
    };
    if record.call.binding != foreign.binding
        || machine_code::ProviderExecutionRecord::from(record.call.provider_execution)
            != foreign.provider_execution
        || record.call.structural_arguments.len() != structural_arguments.len()
        || !fragment_scalar_argument_custody_rejoin(
            publication,
            function,
            block,
            record,
            instruction,
            foreign,
        )
    {
        return Err("normalized foreign D41 roster row drifted from its projected custody");
    }
    Ok(FragmentForeignCallRejoin { import, record })
}

/// Rejoins one roster row's scalar-argument lane against its projected
/// custody records. Each projected record must keep the roster argument's
/// authored plan position and placement verbatim and name the exact emitted
/// custody the projection derived — the register copy or contiguous
/// outgoing-slot address-plus-store pair for an integer immediate or a
/// scalar-call `Home`, and the exact durable-home slot and normalization span
/// for a scalar result — re-derived here from the retained text section
/// rather than trusted.
/// Every projected callback materialization must rejoin the roster row's
/// selected callback argument and the plan's matching nominal destination —
/// a substituted, dropped, or reordered callback cannot self-certify.
fn fragment_callback_materialization_rejoin(
    record: &selected_instructions::SelectedNormalizedForeignCall,
    foreign: &image_emission::ObjectForeignCall,
) -> bool {
    let plan = &record.call.binding.boundary_entry_plan.call;
    if record.call.callbacks.len() != foreign.callback_addresses.len()
        || foreign.callback_addresses.len() != plan.callback_materializations.len()
    {
        return false;
    }
    record
        .call
        .callbacks
        .iter()
        .zip(foreign.callback_addresses.iter())
        .all(|(callback, materialization)| {
            *callback == materialization.target
                && plan
                    .callback_materializations
                    .iter()
                    .filter(|row| row.destination == callback.destination)
                    .count()
                    == 1
                && match materialization.destination {
                    machine_code::CallbackAddressDestination::Register(register) => {
                        callback.application.placement.locations.as_slice()
                            == [ValueLocation::Register {
                                register,
                                value_byte_offset: 0,
                                byte_size: callback.application.shape.byte_size,
                            }]
                    }
                    machine_code::CallbackAddressDestination::OutgoingStack { byte_offset } => {
                        callback.application.placement.locations.as_slice()
                            == [ValueLocation::Stack {
                                stack_byte_offset: byte_offset,
                                value_byte_offset: 0,
                                byte_size: callback.application.shape.byte_size,
                                alignment: callback.application.shape.alignment,
                            }]
                    }
                    machine_code::CallbackAddressDestination::RecordField {
                        root_register,
                        byte_offset,
                    } => {
                        matches!(
                            callback.destination,
                            calling_conventions::NativePlace::Field { .. }
                        ) && callback.field_byte_offset
                            == Some(usize::try_from(byte_offset).unwrap_or(usize::MAX))
                            && callback.application.placement.locations.as_slice()
                                == [ValueLocation::Register {
                                    register: root_register,
                                    value_byte_offset: 0,
                                    byte_size: callback.application.shape.byte_size,
                                }]
                    }
                }
        })
}

fn fragment_scalar_argument_custody_rejoin(
    publication: &FragmentPublicationBinding,
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
    foreign: &image_emission::ObjectForeignCall,
) -> bool {
    let plan = &record.call.binding.boundary_entry_plan.call;
    // Only a parameter-destination materialization mints a plan slot; field
    // destinations write through their root record parameter's slot, which
    // the structural argument roster already carries.
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
    if foreign.scalar_arguments.len() != record.call.scalar_arguments.len()
        || plan.parameters.len()
            != record.call.scalar_arguments.len()
                + record.call.structural_arguments.len()
                + parameter_slot_materializations
    {
        return false;
    }
    if !fragment_callback_materialization_rejoin(record, foreign) {
        return false;
    }
    let mut fragments = publication
        .text_section()
        .functions
        .iter()
        .filter(|fragment| fragment.machine == foreign.machine);
    let (Some(fragment), None) = (fragments.next(), fragments.next()) else {
        return false;
    };
    if record.call.binding.system_call().is_some() {
        if foreign.x86_floating_control.is_some()
            || foreign.aarch64_floating_control.is_some()
            || !foreign.callback_addresses.is_empty()
        {
            return false;
        }
    } else if !floating_control::rejoin(publication, function, block, fragment, record, foreign) {
        return false;
    }
    let home_offsets = fragment_durable_home_offsets(function);
    if !fragment_scalar_result_custody_rejoin(
        function,
        block,
        fragment,
        record,
        instruction,
        &home_offsets,
        foreign.scalar_result.as_ref(),
    ) {
        return false;
    }
    // Parameter identities are not operand offsets: an aggregate may
    // contribute multiple fragments or one owned-copy pointer.
    let register_positions =
        selected_instructions::calls::constraints::foreign_argument_registers(&record.call);
    let register_arguments = register_positions.len();
    if !foreign_result_operand_count(record).is_some_and(|results| {
        register_arguments.checked_add(results) == Some(instruction.operands.len())
    }) {
        return false;
    }
    for (scalar_ordinal, (argument, physical)) in record
        .call
        .scalar_arguments
        .iter()
        .zip(&foreign.scalar_arguments)
        .enumerate()
    {
        let Ok(native_position) = usize::try_from(argument.parameter_index) else {
            return false;
        };
        let Some(shape) = fixed_scalar_shape(argument.source.scalar_type()) else {
            return false;
        };
        if scalar_ordinal.checked_sub(1).is_some_and(|previous| {
            record.call.scalar_arguments[previous].parameter_index >= argument.parameter_index
        }) || physical.parameter_index != argument.parameter_index
            || physical.placement != argument.placement
            || plan.parameters.get(native_position) != Some(&argument.placement)
            || argument.placement.shape != shape
        {
            return false;
        }
        let register_operand = register_positions
            .iter()
            .position(|(position, _)| *position == native_position)
            .unwrap_or(register_arguments);
        let span_rejoins = match (argument.source, physical.source) {
            (
                source @ (target_operations::TargetUnitScalarArgumentSource::Parameter { .. }
                | target_operations::TargetUnitScalarArgumentSource::BlockParameter(_)
                | target_operations::TargetUnitScalarArgumentSource::BooleanImmediate {
                    ..
                }
                | target_operations::TargetUnitScalarArgumentSource::IeeeFloatImmediate {
                    ..
                }),
                machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
                    source_value,
                    scalar_type,
                    instruction: selected_call,
                },
            ) => {
                // Retained selected/physical replay validates the SSA source's
                // entry, edge or constant transport. Rejoin its exact call coordinate,
                // value and type here; equal-width values are not substitutes.
                source_value == source.source_value()
                    && scalar_type == source.scalar_type()
                    && selected_call == record.instruction
                    && fragment_instruction_span(fragment, selected_call)
                        == Some((physical.code_offset as u64, physical.byte_count as u64))
            }
            (
                target_operations::TargetUnitScalarArgumentSource::IntegerImmediate {
                    defining_operation,
                    source_value,
                    scalar_type,
                    value,
                },
                machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    defining_operation: projected_operation,
                    source_value: projected_value,
                    scalar_type: projected_type,
                    value: projected_integer,
                },
            ) => {
                (defining_operation, source_value, scalar_type, value)
                    == (
                        projected_operation,
                        projected_value,
                        projected_type,
                        projected_integer,
                    )
                    && semantic_vocabulary::ScalarTerm::integer(scalar_type, value).is_ok()
                    && fragment_immediate_argument_span_rejoin(
                        function,
                        block,
                        fragment,
                        record,
                        instruction,
                        argument,
                        register_operand,
                        physical,
                    )
            }
            (
                target_operations::TargetUnitScalarArgumentSource::Home(home),
                machine_code::InternalUnitScalarArgumentSourceRecord::Home(projected),
            ) => {
                let allocated = home_offsets
                    .iter()
                    .find(|(operation, _)| *operation == home.defining_operation)
                    .map(|(_, offset)| *offset);
                home.shape == argument.placement.shape
                    && projected
                        == machine_code::UnitScalarHomeRecord {
                            defining_operation: home.defining_operation,
                            source_value: home.source_value,
                            scalar_type: home.scalar_type,
                            shape: home.shape,
                            byte_offset: allocated.unwrap_or(u32::MAX),
                        }
                    && allocated.is_some()
                    && fragment_immediate_argument_span_rejoin(
                        function,
                        block,
                        fragment,
                        record,
                        instruction,
                        argument,
                        register_operand,
                        physical,
                    )
                    && fragment_scalar_call_result_producer(function, &home)
            }
            _ => false,
        };
        if !span_rejoins {
            return false;
        }
    }
    true
}

/// Re-derives the emitted interval one integer-immediate argument occupies
/// and compares it to the projected record: the register copy whose result
/// feeds the call operand, or the contiguous outgoing-slot
/// `FrameAddress`/`Store` pair this call's argument transport owns.
#[allow(clippy::too_many_arguments)]
fn fragment_immediate_argument_span_rejoin(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    fragment: &machine_code::PlacedFunctionFragment,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
    argument: &target_operations::TargetUnitScalarCallArgument,
    register_operand: usize,
    physical: &machine_code::ForeignCallScalarArgumentRecord,
) -> bool {
    let span = match argument.placement.locations.as_slice() {
        [
            calling_conventions::ValueLocation::Register {
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == argument.placement.shape.byte_size => {
            let Some(operand) = instruction.operands.get(register_operand) else {
                return false;
            };
            if usize::from(operand.operand) != register_operand {
                return false;
            }
            let Some(register_value) = function
                .virtual_registers
                .get(operand.virtual_register.0 as usize)
            else {
                return false;
            };
            let selected_instructions::VirtualRegisterOrigin::InstructionResult {
                instruction: materialization,
                source_value,
            } = register_value.origin
            else {
                return false;
            };
            if source_value != argument.source.source_value() {
                return false;
            }
            fragment_instruction_span(fragment, materialization)
        }
        [
            calling_conventions::ValueLocation::Stack {
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
                return false;
            };
            if outgoing.abi_stack_byte_offset != *stack_byte_offset
                || outgoing.byte_size != (*byte_size)
                || outgoing.alignment != *alignment
            {
                return false;
            }
            let source_value = argument.source.source_value();
            let Ok(stored) = u8::try_from(*byte_size) else {
                return false;
            };
            let Some(address_index) = block.instructions.iter().position(|candidate| {
                matches!(
                    candidate.kind,
                    selected_instructions::SelectedInstructionKind::FrameAddress {
                        slot: selected_instructions::FrameStorageSlotId::Outgoing(candidate),
                        byte_offset: 0,
                    } if candidate == slot
                ) && candidate.provenance.operations.as_slice() == [record.operation]
                    && candidate.provenance.values.as_slice() == [source_value]
            }) else {
                return false;
            };
            let address = &block.instructions[address_index];
            let Some(store) = block.instructions.get(address_index + 1) else {
                return false;
            };
            let selected_instructions::SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: stored_bytes,
            } = store.kind
            else {
                return false;
            };
            if stored_bytes != stored
                || store.provenance.operations.as_slice() != [record.operation]
                || store.provenance.values.as_slice() != [source_value]
            {
                return false;
            }
            let Some(address_operand) = store.operands.first() else {
                return false;
            };
            let Some(address_value) = function
                .virtual_registers
                .get(address_operand.virtual_register.0 as usize)
            else {
                return false;
            };
            if address_value.origin
                != (selected_instructions::VirtualRegisterOrigin::ScalarAbiAddress {
                    instruction: address.id,
                    source_value,
                })
            {
                return false;
            }
            let Some((address_offset, address_bytes)) =
                fragment_instruction_span(fragment, address.id)
            else {
                return false;
            };
            let Some((store_offset, store_bytes)) = fragment_instruction_span(fragment, store.id)
            else {
                return false;
            };
            if store_offset != address_offset + address_bytes {
                return false;
            }
            Some((address_offset, address_bytes + store_bytes))
        }
        _ => None,
    };
    span.is_some_and(|(offset, bytes)| {
        usize::try_from(offset) == Ok(physical.code_offset)
            && usize::try_from(bytes) == Ok(physical.byte_count)
    })
}

/// Reconstruct completed aggregate storage from the retained ABI and selected
/// memory effects. Plain aggregates use their real place/slot, never a scalar
/// home or a fabricated sum layout. Physical instruction replay has already
/// checked these selected instructions against their placed bytes.
fn structural_result_capture(
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
    if operation != record.operation
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
    let call_position = block
        .instructions
        .iter()
        .position(|candidate| candidate.id == instruction.id)?;
    let mut position = call_position.checked_add(1)?;
    if record.call.binding.import().is_some() {
        let restore = block.instructions.get(position)?;
        if restore.kind
            != (Kind::RestoreFloatingControl {
                slot: LocalStorageSlotId::Boundary { operation },
            })
        {
            return None;
        }
        position = position.checked_add(1)?;
    }
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
    for (location, operand) in placement.locations.iter().zip(operands) {
        let ValueLocation::Register {
            value_byte_offset,
            byte_size,
            ..
        } = *location
        else {
            return None;
        };
        if value_byte_offset.checked_add(byte_size)? > shape.byte_size
            || !transport_matches(operand.virtual_register, instruction.id, value_byte_offset)
        {
            return None;
        }
        let mut value = operand.virtual_register;
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
fn foreign_result_operand_count(
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

/// Re-derives a roster row's scalar-result custody — the durable-home record,
/// its plan placement, and the normalization instruction span — and requires
/// the projected record to equal the re-derivation exactly.
fn fragment_scalar_result_custody_rejoin(
    function: &selected_instructions::SelectedFunction,
    block: &selected_instructions::SelectedBlock,
    fragment: &machine_code::PlacedFunctionFragment,
    record: &selected_instructions::SelectedNormalizedForeignCall,
    instruction: &selected_instructions::SelectedInstruction,
    home_offsets: &[(semantic_vocabulary::OperationId, u32)],
    physical: Option<&machine_code::ForeignCallScalarResultRecord>,
) -> bool {
    let plan = &record.call.binding.boundary_entry_plan.call;
    if matches!(
        record.call.result,
        target_operations::TargetBoundaryResult::Structural(_)
    ) {
        return physical.is_none()
            && structural_result_capture(function, block, fragment, record, instruction).is_some();
    }
    let (Some(home), Some(placement)) = (record.call.result.scalar_home(), &plan.result) else {
        return physical.is_none()
            && matches!(
                record.call.result,
                target_operations::TargetBoundaryResult::Unit
            )
            && plan.result.is_none();
    };
    let Some(physical) = physical else {
        return false;
    };
    let Some(shape) = fixed_scalar_shape(home.scalar_type) else {
        return false;
    };
    if home.defining_operation != record.operation
        || home.shape != shape
        || placement.shape != home.shape
        || !matches!(
            placement.locations.as_slice(),
            [
                calling_conventions::ValueLocation::Register {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] if *byte_size == home.shape.byte_size
        )
    {
        return false;
    }
    let Some(position) = block
        .instructions
        .iter()
        .position(|candidate| candidate.id == record.instruction)
    else {
        return false;
    };
    // Only imports restore user-space floating controls before normalizing
    // their result. Kernel entries normalize immediately after supervisor return.
    let normalization_position = if record.call.binding.import().is_some() {
        let Some(restore) = block.instructions.get(position + 1) else {
            return false;
        };
        if restore.kind
            != (selected_instructions::SelectedInstructionKind::RestoreFloatingControl {
                slot: selected_instructions::LocalStorageSlotId::Boundary {
                    operation: record.operation,
                },
            })
        {
            return false;
        }
        position + 2
    } else {
        position + 1
    };
    let Some(normalization) = block.instructions.get(normalization_position) else {
        return false;
    };
    let [input, output] = normalization.operands.as_slice() else {
        return false;
    };
    let Some(call_result) = instruction.operands.last() else {
        return false;
    };
    let output_origin = function
        .virtual_registers
        .get(output.virtual_register.0 as usize)
        .map(|register| register.origin);
    let Some(expected_kind) = fragment_scalar_result_normalization_kind(home.scalar_type) else {
        return false;
    };
    if normalization.kind != expected_kind
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
        return false;
    }
    let Some((offset, byte_count)) = fragment_instruction_span(fragment, normalization.id) else {
        return false;
    };
    let Some(byte_offset) = home_offsets
        .iter()
        .find(|(operation, _)| *operation == home.defining_operation)
        .map(|(_, offset)| *offset)
    else {
        return false;
    };
    *physical
        == machine_code::ForeignCallScalarResultRecord {
            home: machine_code::UnitScalarHomeRecord {
                defining_operation: home.defining_operation,
                source_value: home.source_value,
                scalar_type: home.scalar_type,
                shape: home.shape,
                byte_offset,
            },
            source: placement.clone(),
            code_offset: usize::try_from(offset).unwrap_or(usize::MAX),
            byte_count: usize::try_from(byte_count).unwrap_or(usize::MAX),
        }
}

/// The normalization kind one scalar result's durable definition emits —
/// independently reconstructed from the retained semantic type.
fn fragment_scalar_result_normalization_kind(
    scalar_type: ScalarType,
) -> Option<selected_instructions::SelectedInstructionKind> {
    use selected_instructions::SelectedInstructionKind as Kind;
    match scalar_type {
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => {
            Some(Kind::Float32ToBits)
        }
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => {
            Some(Kind::Float64ToBits)
        }
        ScalarType::Boolean => Some(Kind::ZeroExtendU8),
        ScalarType::Integer(integer) => match (integer.sign(), integer.bits()) {
            (semantic_vocabulary::IntegerSign::Unsigned, 8) => Some(Kind::ZeroExtendU8),
            (semantic_vocabulary::IntegerSign::Unsigned, 16) => Some(Kind::ZeroExtendU16),
            (semantic_vocabulary::IntegerSign::Unsigned, 32) => Some(Kind::ZeroExtendU32),
            (semantic_vocabulary::IntegerSign::Unsigned, 64)
            | (semantic_vocabulary::IntegerSign::Signed, 64) => Some(Kind::CopyI64),
            (semantic_vocabulary::IntegerSign::Signed, 8) => Some(Kind::SignExtendI8),
            (semantic_vocabulary::IntegerSign::Signed, 16) => Some(Kind::SignExtendI16),
            (semantic_vocabulary::IntegerSign::Signed, 32) => Some(Kind::SignExtendI32),
            _ => None,
        },
    }
}

/// The caller's durable scalar-home area, re-derived in emitted instruction
/// order: every scalar-producing selected call row is one producer whose slot
/// is 8-aligned and sized by its home shape. Projection and this rejoin share
/// the rule so `Home` sources and `scalar_result` records name the same slot.
fn fragment_durable_home_offsets(
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
                let Some(instruction) = function
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
                    .find(|(operation, _)| *operation == requirement.defining_operation)
                    .map(|(_, instruction)| instruction)
                    .or_else(|| {
                        scalar_observation_producer(
                            function,
                            requirement.defining_operation,
                            requirement.source_value,
                        )
                    })
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
        let Some(next) = cursor.checked_add(byte_size) else {
            return Vec::new();
        };
        cursor = next;
    }
    offsets
}

/// The producing call one scalar-call-result `Home` names: exactly one
/// internal or foreign call roster row claims the home's defining operation,
/// a foreign producer must record exactly this home as its result, and the
/// producer instruction defines the home's source value as its own
/// instruction result.
fn fragment_scalar_call_result_producer(
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
                    == (selected_instructions::VirtualRegisterOrigin::InstructionResult {
                        instruction: producer.id,
                        source_value: home.source_value,
                    })
            })
    })
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
