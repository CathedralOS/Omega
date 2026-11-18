/*
We reconstruct fragment-published external-entry, selected-call, callback, and
scalar argument/result custody from the current physical transport plan before
foreign-call physical evidence derivation accepts the projected records (`fragment_normalized_foreign_rejoin`).

Symbolic imports rejoin their exact unresolved relocation fields and canonical
import symbol names, whereas kernel syscall and table-dispatch entries decode
the complete placed instruction bytes against the retained syscall number or
table binding and fixed register operand homes.
*/

use crate::OptimizedBoundaryOccurrence;
use crate::physical::derivation::foreign_calls::floating_control;
use crate::physical::fragment_publication::FragmentPublicationBinding;
use calling_conventions::NativePlace;
use calling_conventions::ValueLocation;
use object_file::canonical_normalized_foreign_import_symbol_name;

#[derive(Clone, Copy)]
pub(super) struct FragmentForeignCallRejoin<'a> {
    pub(super) import: Option<(
        &'a object_file::RelocationFreeObjectUnresolvedForeignCall,
        &'a object_file::RelocationFreeObjectNormalizedImport,
    )>,
    pub(super) record: &'a selected_instructions::SelectedNormalizedForeignCall,
    pub(super) call_span: &'a machine_code::PlacedInstructionSpan,
}

pub(super) fn fragment_normalized_foreign_rejoin<'a>(
    pass: crate::physical::PhysicalEvidencePass,
    publication: &'a FragmentPublicationBinding,
    occurrence: &OptimizedBoundaryOccurrence,
    foreign: &image_emission::ObjectForeignCall,
    structural_arguments: &[terminal_psi::StructuralArgument],
) -> Result<FragmentForeignCallRejoin<'a>, &'static str> {
    let invocation_site = occurrence.invocation_site();
    if matches!(
        invocation_site,
        semantic_vocabulary::InvocationSite::Cleanup { .. }
    ) {
        return Err("owned module cleanup cannot carry normalized foreign fragment custody");
    }
    let plan = publication.relocation_free_object();
    let mut plans = publication
        .foreign_call_plans()
        .calls
        .iter()
        .filter(|plan| {
            plan.machine == occurrence.machine()
                && plan.record.invocation_site == invocation_site
                && plan.record.call.boundary == occurrence.boundary()
        });
    let (Some(transport), None) = (plans.next(), plans.next()) else {
        return Err("normalized foreign D41 child cannot rejoin one physical call");
    };
    let record = &transport.record;
    let ordinal = transport.ordinal;
    let instruction_kind = selected_instructions::SelectedInstructionKind::NormalizedForeignCall {
        boundary: record.call.boundary,
        ordinal: u32::try_from(ordinal).map_err(|_| "foreign call roster ordinal overflow")?,
    };
    let placed = image_emission::place_foreign_call(transport, publication.text_section())
        .map_err(|_| "foreign call transport does not match its physical span obligations")?;
    if &placed != foreign {
        return Err("normalized foreign D41 child changed its physical transport custody");
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
        .filter(|placed| placed.block == transport.block)
        .flat_map(|placed| &placed.instructions)
        .filter(|span| span.instruction == record.instruction);
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
    let import = match (
        record.call.binding.import_locator(),
        fields.next(),
        fields.next(),
    ) {
        (Some(_), Some(field), None) => {
            let resolution = &field.resolution;
            if resolution.call_function_offset != call_span.function_offset
                || resolution.call_section_offset != call_span.section_offset
                || resolution.call_byte_count != call_span.byte_count
                || resolution.boundary != occurrence.boundary()
                || resolution.instruction != record.instruction
                || resolution.block != transport.block
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
                != canonical_normalized_foreign_import_symbol_name(import.boundary, import.ordinal)
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
                .instruction_span(record.instruction)
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
            pass.verify(|| {
                let views = transport
                    .operand_views
                    .iter()
                    .copied()
                    .collect::<Option<Vec<_>>>()
                    .ok_or("kernel entry lacks fixed operand homes")?;
                let accesses = &transport.operand_accesses;
                if record.call.binding.table_dispatch().is_some() {
                    match section.target.architecture {
                        target::Architecture::X86_64 => {
                            target_x86_64::inspection::selected_table_call_effects(
                                record,
                                span.alternative,
                                &views,
                                accesses,
                                bytes,
                            )
                            .ok_or("table entry bytes disagree with its selected binding")?;
                        }
                        target::Architecture::Aarch64 => {
                            target_aarch64::inspection::selected_table_call_effects(
                                record,
                                span.alternative,
                                &views,
                                accesses,
                                bytes,
                            )
                            .ok_or("table entry bytes disagree with its selected binding")?;
                        }
                    }
                } else {
                    let kernel = record
                        .call
                        .binding
                        .system_call()
                        .ok_or("normalized external entry mechanism is absent")?;
                    match section.target.architecture {
                        target::Architecture::X86_64 => {
                            target_x86_64::inspection::selected_system_call_effects(
                                section.target,
                                target_x86_64::validated_x86_64_physical_register_model(),
                                instruction_kind,
                                span.alternative,
                                &views,
                                accesses,
                                kernel.number,
                                bytes,
                            )
                            .ok_or("kernel entry bytes disagree with its selected binding")?;
                        }
                        target::Architecture::Aarch64 => {
                            target_aarch64::inspection::selected_system_call_effects(
                                section.target,
                                target_aarch64::validated_aarch64_physical_register_model(),
                                instruction_kind,
                                span.alternative,
                                &views,
                                accesses,
                                kernel.number,
                                bytes,
                            )
                            .ok_or("kernel entry bytes disagree with its selected binding")?;
                        }
                    }
                }
                Ok(())
            })?;
            None
        }
        _ => return Err("external call import fields disagree with its entry mechanism"),
    };
    if record.call.binding != foreign.binding
        || machine_code::ExternalExecutionRecord::from(record.call.provider_execution)
            != foreign.provider_execution
        || record.call.structural_arguments.len() != structural_arguments.len()
        || !fragment_callback_materialization_rejoin(record, foreign)
        || record
            .call
            .binding
            .boundary_entry_plan
            .call
            .parameters
            .len()
            != record.call.scalar_arguments.len()
                + record.call.structural_arguments.len()
                + record
                    .call
                    .binding
                    .boundary_entry_plan
                    .call
                    .callback_materializations
                    .iter()
                    .filter(|row| matches!(row.destination, NativePlace::Parameter(_)))
                    .count()
        || !floating_control::rejoin(pass, publication, record, foreign)
    {
        return Err("normalized foreign D41 roster row drifted from its projected custody");
    }
    Ok(FragmentForeignCallRejoin {
        import,
        record,
        call_span,
    })
}

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
                && match materialization.destination.clone() {
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
                        placements,
                    } => {
                        matches!(callback.destination, NativePlace::Field { .. })
                            && callback.field_placements.as_ref() == Some(&placements)
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
