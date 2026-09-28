//! Reconstruct fragment-published import, selected-call, and scalar custody
//! before the foreign-call derivation accepts the projected physical records.

use crate::OptimizedBoundaryOccurrence;
use crate::physical::derivation::foreign_calls::{
    fixed_scalar_shape, floating_control, fragment_instruction_span,
};
use crate::physical::fragment_publication::FragmentPublicationBinding;
use calling_conventions::ValueLocation;
use semantic_vocabulary::ScalarType;

/// The fragment-publication rejoin of one normalized-foreign D41 child: the
/// relocation-free object plan's unresolved import field, the declared import
/// symbol it names, and the selected-plan roster row the projected custody
/// was bound from. The three must agree on `{caller, operation, boundary}`
/// and the roster coordinate `{boundary, ordinal}` before physical evidence
/// may claim the projected custody row.
#[derive(Clone, Copy)]
pub(super) struct FragmentForeignCallRejoin<'a> {
    pub(super) field: &'a object_file::RelocationFreeObjectUnresolvedForeignCall,
    pub(super) import: &'a object_file::RelocationFreeObjectNormalizedImport,
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
    let mut fields = plan
        .unresolved_normalized_foreign_calls
        .iter()
        .filter(|candidate| {
            candidate.resolution.caller == occurrence.machine()
                && candidate.resolution.operation == occurrence.operation()
                && candidate.resolution.boundary == occurrence.boundary()
                && candidate.resolution.state
                    == machine_code::NormalizedForeignCallResolutionState::UnresolvedImportFieldV1
        })
        .collect::<Vec<_>>();
    if fields.len() != 1 {
        return Err("normalized foreign D41 child cannot rejoin one unresolved import field");
    }
    let field = fields.remove(0);
    if usize::try_from(field.resolution.field_section_offset) != Ok(foreign.text_offset) {
        return Err("normalized foreign D41 import field does not match its call offset");
    }
    let mut imports = plan
        .normalized_imports
        .iter()
        .filter(|import| {
            import.symbol == field.symbol
                && import.boundary == field.resolution.boundary
                && import.ordinal == field.resolution.ordinal
        })
        .collect::<Vec<_>>();
    if imports.len() != 1 {
        return Err("normalized foreign D41 child cannot rejoin one declared import symbol");
    }
    let import = imports.remove(0);
    if import.name
        != object_file::canonical_normalized_foreign_import_symbol_name(
            import.boundary,
            import.ordinal,
        )
    {
        return Err("normalized foreign D41 import symbol drifted from its canonical name");
    }
    let mut functions = selected
        .functions
        .iter()
        .filter(|function| function.machine == occurrence.machine())
        .collect::<Vec<_>>();
    if functions.len() != 1 {
        return Err("normalized foreign D41 child names an absent selected function");
    }
    let function = functions.remove(0);
    let index = usize::try_from(field.resolution.ordinal)
        .map_err(|_| "normalized foreign D41 roster ordinal does not fit this target")?;
    let record = function
        .normalized_foreign_calls
        .get(index)
        .ok_or("normalized foreign D41 roster row is absent for its ordinal")?;
    if record.instruction != field.resolution.instruction
        || record.operation != field.resolution.operation
        || record.call.boundary != field.resolution.boundary
    {
        return Err("normalized foreign D41 roster row lost its call target");
    }
    let Some(block) = function
        .blocks
        .iter()
        .find(|block| block.id == field.resolution.block)
    else {
        return Err("normalized foreign D41 roster row names an absent instruction");
    };
    // Instruction identities are function-global, not block-local indexes:
    // find the roster instruction by id inside its recorded block.
    let Some(instruction) = block
        .instructions
        .iter()
        .find(|instruction| instruction.id == field.resolution.instruction)
    else {
        return Err("normalized foreign D41 roster row names an absent instruction");
    };
    if !matches!(
        instruction.kind,
        selected_instructions::SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal }
            if boundary == field.resolution.boundary && ordinal == field.resolution.ordinal
    ) {
        return Err("normalized foreign D41 instruction kind does not match its roster row");
    }
    if record.call.binding.locator != foreign.locator
        || record.call.binding.boundary_entry_plan != foreign.boundary_entry_plan
        || record.call.binding.same_stack_contribution != foreign.same_stack_contribution
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
    Ok(FragmentForeignCallRejoin {
        field,
        import,
        record,
    })
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
    if !floating_control::rejoin(publication, function, block, fragment, record, foreign) {
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
                [calling_conventions::ValueLocation::Register { .. }]
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
                    [calling_conventions::ValueLocation::Register { .. }]
                )
            })
            .count();
    if instruction.operands.len()
        != register_arguments + usize::from(record.call.result_home.is_some())
    {
        return false;
    }
    // `Parameter`-destination callbacks claim a register placement without
    // occupying a call-row operand (the address bytes live inside the call
    // encoding), matching the projection's operand roster.
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
                [calling_conventions::ValueLocation::Register { .. }]
            )
        })
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    register_positions.sort_by_key(|position| {
        plan.parameters[*position].shape.class == calling_conventions::ValueClass::Float
    });
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
            .position(|position| *position == native_position)
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
                || outgoing.byte_size != u32::from(*byte_size)
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
    let (Some(home), Some(placement)) = (&record.call.result_home, &plan.result) else {
        return physical.is_none() && record.call.result_home.is_none() && plan.result.is_none();
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
    let Some(normalization) = block.instructions.get(position + 2) else {
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
                let Some(instruction) = function
                    .calls
                    .iter()
                    .map(|contract| (contract.operation, contract.instruction))
                    .chain(
                        function
                            .normalized_foreign_calls
                            .iter()
                            .map(|record| (record.operation, record.instruction)),
                    )
                    .find(|(operation, _)| *operation == requirement.defining_operation)
                    .map(|(_, instruction)| instruction)
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
                    == (selected_instructions::VirtualRegisterOrigin::InstructionResult {
                        instruction: producer.id,
                        source_value: home.source_value,
                    })
            })
    })
}
