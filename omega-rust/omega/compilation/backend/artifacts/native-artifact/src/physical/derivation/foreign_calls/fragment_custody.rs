//! Reconstruct fragment-published external-entry, selected-call, and scalar custody
//! before the foreign-call derivation accepts the projected physical records.
//! Imports rejoin exact relocation fields; kernel entries instead decode the
//! complete placed instruction against the retained number and ABI operands.
//! Neither path can replace the shared source, argument, or result checks.

use crate::OptimizedBoundaryOccurrence;
use crate::physical::derivation::foreign_calls::{floating_control, foreign_scalar_shape};
use crate::physical::fragment_publication::FragmentPublicationBinding;
use calling_conventions::ValueLocation;
use image_emission::{
    foreign_result_operand_count, fragment_instruction_span, structural_result_capture,
};
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
    pub(super) call_span: &'a machine_code::PlacedInstructionSpan,
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
    let invocation_site = occurrence.invocation_site();
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
            record.invocation_site == invocation_site
                && record.call.boundary == occurrence.boundary()
        });
    let (Some((ordinal, record)), None) = (records.next(), records.next()) else {
        return Err("normalized foreign D41 child cannot rejoin one selected call");
    };
    let (block, instruction) =
        image_emission::unique_block_instruction(function, record.instruction)
            .ok_or("normalized foreign D41 roster row names an absent instruction")?;
    if !image_emission::foreign_invocation_provenance_matches(
        &instruction.provenance,
        record.invocation_site,
    ) {
        return Err("normalized foreign call changed its invocation source custody");
    }
    if !matches!(instruction.kind,
        selected_instructions::SelectedInstructionKind::NormalizedForeignCall {
            boundary, ordinal: actual,
        } if boundary == occurrence.boundary() && usize::try_from(actual) == Ok(ordinal))
    {
        return Err("normalized foreign D41 instruction kind does not match its roster row");
    }
    if record.invocation_site.operation().is_none()
        && selected_instructions::calls::normalized_foreign::return_transport::resolve(
            function, block, record,
        )
        .is_none()
    {
        return Err("normalized foreign completion lost its containment trap");
    }
    let mut fragments = publication
        .text_section()
        .functions
        .iter()
        .filter(|fragment| fragment.machine == occurrence.machine());
    let (Some(fragment), None) = (fragments.next(), fragments.next()) else {
        return Err("normalized foreign call lacks one placed caller");
    };
    let mut spans = fragment
        .blocks
        .iter()
        .filter(|placed| placed.block == block.id)
        .flat_map(|placed| &placed.instructions)
        .filter(|span| span.instruction == instruction.id);
    let (Some(call_span), None) = (spans.next(), spans.next()) else {
        return Err("normalized foreign call lacks one placed instruction span");
    };
    let mut fields = plan
        .unresolved_normalized_foreign_calls
        .iter()
        .filter(|candidate| {
            candidate.resolution.caller == occurrence.machine()
                && candidate.resolution.invocation_site == invocation_site
        });
    let import = match (record.call.binding.import(), fields.next(), fields.next()) {
        (Some(_), Some(field), None) => {
            let resolution = &field.resolution;
            if resolution.call_function_offset != call_span.function_offset
                || resolution.call_section_offset != call_span.section_offset
                || resolution.call_byte_count != call_span.byte_count
                || resolution.boundary != occurrence.boundary()
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
            let section = publication.text_section();
            let fragment = section
                .function(occurrence.machine())
                .ok_or("kernel entry has no placed caller")?;
            let span = fragment
                .instruction_span(instruction.id)
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
            if record.call.binding.table_dispatch().is_some() {
                if section.target.architecture != target::Architecture::X86_64 {
                    return Err("table entry has no decoder for its architecture");
                }
                isa_x86_64::validate_x86_64_selected_table_call_form(
                    record,
                    span.alternative,
                    &views,
                    &accesses,
                    bytes,
                )
                .map_err(|_| "table entry bytes disagree with its selected binding")?;
            } else {
                let kernel = record
                    .call
                    .binding
                    .system_call()
                    .ok_or("normalized external entry mechanism is absent")?;
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
    Ok(FragmentForeignCallRejoin {
        import,
        record,
        call_span,
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
    let home_offsets = image_emission::durable_home_offsets(function);
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
        let Some(shape) = foreign_scalar_shape(argument.source.scalar_type()) else {
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
                    && image_emission::scalar_call_result_producer(function, &home)
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
    image_emission::scalar_immediate_span(
        function,
        block,
        fragment,
        record,
        instruction,
        argument,
        register_operand,
        &|_: &selected_instructions::SelectedOperand, _: target_operations::MachineRegister| true,
    )
    .is_some_and(|(offset, bytes)| {
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
    let Some(shape) = foreign_scalar_shape(home.scalar_type) else {
        return false;
    };
    if Some(home.defining_operation) != record.invocation_site.operation()
        || home.shape != shape
        || placement.shape != home.shape
        || !placement.is_complete_scalar_result()
    {
        return false;
    }
    let Some(transport) =
        selected_instructions::calls::normalized_foreign::return_transport::resolve(
            function, block, record,
        )
    else {
        return false;
    };
    let Some(normalization) = block.instructions.get(transport.normalization_position) else {
        return false;
    };
    let [input, output] = normalization.operands.as_slice() else {
        return false;
    };
    let output_origin = function
        .virtual_registers
        .get(output.virtual_register.0 as usize)
        .map(|register| register.origin);
    let Some(expected_kind) = fragment_scalar_result_normalization_kind(
        home.scalar_type,
        matches!(
            placement.locations.as_slice(),
            [calling_conventions::ValueLocation::Indirect { .. }]
        ),
    ) else {
        return false;
    };
    if normalization.kind != expected_kind
        || transport.result_registers.as_slice() != [input.virtual_register]
        || normalization.provenance.operations.as_slice()
            != record.invocation_site.operation().as_slice()
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
    let transported_end = image_emission::transported_span_end(
        fragment,
        block,
        transport.call_position,
        transport.normalization_position,
    );
    if transported_end != Some(offset) {
        return false;
    }
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
    indirect: bool,
) -> Option<selected_instructions::SelectedInstructionKind> {
    use selected_instructions::SelectedInstructionKind as Kind;
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
