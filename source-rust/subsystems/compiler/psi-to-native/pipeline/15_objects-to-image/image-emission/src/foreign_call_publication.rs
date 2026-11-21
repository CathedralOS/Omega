//! Resolve the foreign-call physical plan against current placed instruction
//! spans. No selected function or earlier executable program is read here.
use super::carriers::{ObjectArtifact, ObjectForeignCall};
use super::errors::ObjectError;
use machine_code::foreign_call_plans::{
    ForeignCallPlanError, InstructionSpanObligation, InstructionSpanSequence,
    PhysicalForeignCallPlan,
};
use semantic_vocabulary::MachineId;
use target_operations::CallSiteOwner;

impl From<ForeignCallPlanError> for ObjectError {
    fn from(error: ForeignCallPlanError) -> Self {
        match error {
            ForeignCallPlanError::TextSizeOverflow => Self::TextSizeOverflow,
            ForeignCallPlanError::InvalidForeignCallProjection {
                caller,
                owner,
                reason,
            } => Self::InvalidForeignCallProjection {
                caller,
                owner,
                reason,
            },
            ForeignCallPlanError::InvalidForeignCallArgument {
                caller,
                owner,
                import,
                parameter,
                reason,
                transport,
            } => Self::InvalidForeignCallArgument {
                caller,
                owner,
                import,
                parameter,
                reason,
                transport,
            },
            ForeignCallPlanError::ForeignCallOwnerNotInProvenance { caller, owner } => {
                Self::ForeignCallOwnerNotInProvenance { caller, owner }
            }
            ForeignCallPlanError::DuplicateForeignCallOwner { caller, owner } => {
                Self::DuplicateForeignCallOwner { caller, owner }
            }
        }
    }
}

pub fn derive_normalized_foreign_call_custody(
    plans: &machine_code::foreign_call_plans::PhysicalForeignCallPlans,
    section: &machine_code::RelocationFreeTextSectionPlacement,
) -> Result<Vec<ObjectForeignCall>, ObjectError> {
    if plans.psi != section.psi || plans.target != section.target {
        return Err(ObjectError::ForeignCallProgramMismatch);
    }
    plans
        .calls
        .iter()
        .map(|plan| place_foreign_call(plan, section))
        .collect()
}

pub fn fragment_instruction_span(
    fragment: &machine_code::PlacedFunctionFragment,
    instruction: selected_instructions::SelectedInstructionId,
) -> Option<(u64, u64)> {
    let mut spans = fragment
        .instructions()
        .filter(|span| span.instruction == instruction);
    let span = spans.next()?;
    span.section_offset.checked_add(span.byte_count)?;
    spans
        .next()
        .is_none()
        .then_some((span.section_offset, span.byte_count))
}

/// Check each exact instruction, including empty encodings, before joining the
/// first start and final end. A gap, overlap, missing span or overflow refuses.
pub fn resolve_instruction_sequence(
    fragment: &machine_code::PlacedFunctionFragment,
    sequence: &InstructionSpanSequence,
) -> Option<(u64, u64)> {
    let (&first, rest) = sequence.0.split_first()?;
    let (start, count) = fragment_instruction_span(fragment, first)?;
    let mut end = start.checked_add(count)?;
    for &instruction in rest {
        let (next, count) = fragment_instruction_span(fragment, instruction)?;
        if next != end {
            return None;
        }
        end = next.checked_add(count)?;
    }
    Some((start, end.checked_sub(start)?))
}

pub fn place_foreign_call(
    plan: &PhysicalForeignCallPlan,
    section: &machine_code::RelocationFreeTextSectionPlacement,
) -> Result<ObjectForeignCall, ObjectError> {
    let record = &plan.record;
    let owner = CallSiteOwner::from(record.invocation_site);
    let invalid = |reason| ObjectError::InvalidForeignCallProjection {
        caller: plan.machine,
        owner,
        reason,
    };
    let mut functions = section
        .functions
        .iter()
        .filter(|function| function.machine == plan.machine);
    let (Some(fragment), None) = (functions.next(), functions.next()) else {
        return Err(invalid("foreign call has no unique placed caller"));
    };
    if record.call.binding.native_target() != Some(section.target)
        || !record.call.binding.has_valid_entry_mechanism()
    {
        return Err(invalid(
            "binding target or native entry mechanism does not match",
        ));
    }
    let mut spans = fragment
        .blocks
        .iter()
        .filter(|block| block.block == plan.block)
        .flat_map(|block| &block.instructions)
        .filter(|span| span.instruction == record.instruction);
    let (Some(call_span), None) = (spans.next(), spans.next()) else {
        return Err(invalid(
            "native call has no unique emitted instruction span",
        ));
    };
    for obligation in &plan.spans {
        let valid = match obligation {
            InstructionSpanObligation::NonEmpty(instruction) => {
                fragment_instruction_span(fragment, *instruction)
                    .is_some_and(|(_, count)| count != 0)
            }
            InstructionSpanObligation::Before { first, second } => {
                fragment_instruction_span(fragment, *first)
                    .zip(fragment_instruction_span(fragment, *second))
                    .is_some_and(|((start, count), (next, _))| {
                        start.checked_add(count).is_some_and(|end| end <= next)
                    })
            }
            InstructionSpanObligation::Contiguous(sequence) => {
                resolve_instruction_sequence(fragment, sequence).is_some()
            }
        };
        if !valid {
            return Err(invalid(
                "foreign transport spans violate their physical plan",
            ));
        }
    }
    let mut fields = section
        .unresolved_normalized_foreign_calls
        .iter()
        .filter(|field| {
            field.caller == plan.machine && field.invocation_site == record.invocation_site
        });
    let field = match (
        record.call.binding.import_locator(),
        fields.next(),
        fields.next(),
    ) {
        (Some(_), Some(field), None) => {
            if field.instruction != record.instruction
                || field.boundary != record.call.boundary
                || usize::try_from(field.ordinal) != Ok(plan.ordinal)
                || field.block != plan.block
                || field.call_section_offset != call_span.section_offset
                || field.call_function_offset != call_span.function_offset
                || field.call_byte_count != call_span.byte_count
                || field.state
                    != machine_code::NormalizedForeignCallResolutionState::UnresolvedImportFieldV1
            {
                return Err(invalid(
                    "placed import field does not match the physical call",
                ));
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
    let text_offset =
        usize::try_from(field.map_or(call_span.section_offset, |field| field.field_section_offset))
            .map_err(|_| invalid("native instruction offset is not addressable"))?;
    let scalar_arguments = plan
        .arguments
        .iter()
        .map(|argument| {
            let (offset, count) = resolve_instruction_sequence(fragment, &argument.span)
                .ok_or_else(|| {
                    invalid("scalar argument transport has no exact contiguous emitted span")
                })?;
            Ok(machine_code::ForeignCallScalarArgumentRecord {
                parameter_index: argument.parameter_index,
                source: argument.source,
                placement: argument.placement.clone(),
                code_offset: usize::try_from(offset).map_err(|_| ObjectError::TextSizeOverflow)?,
                byte_count: usize::try_from(count).map_err(|_| ObjectError::TextSizeOverflow)?,
            })
        })
        .collect::<Result<_, ObjectError>>()?;
    let scalar_result = plan
        .result
        .as_ref()
        .map(|result| {
            let (offset, count) = fragment_instruction_span(fragment, result.instruction)
                .ok_or_else(|| invalid("scalar result normalization has no emitted span"))?;
            Ok::<_, ObjectError>(machine_code::ForeignCallScalarResultRecord {
                home: result.home,
                source: result.source.clone(),
                code_offset: usize::try_from(offset).map_err(|_| ObjectError::TextSizeOverflow)?,
                byte_count: usize::try_from(count).map_err(|_| ObjectError::TextSizeOverflow)?,
            })
        })
        .transpose()?;
    let control = plan
        .floating_control
        .map(|control| {
            let (save, save_count) = fragment_instruction_span(fragment, control.save)
                .ok_or_else(|| invalid("foreign control save has no emitted span"))?;
            let (restore, restore_count) = fragment_instruction_span(fragment, control.restore)
                .ok_or_else(|| invalid("foreign control restore has no emitted span"))?;
            Ok::<_, ObjectError>(machine_code::ForeignCallFloatingControlRecord {
                target: section.target,
                saved_slot_byte_offset: control.saved_slot_byte_offset,
                save_offset: usize::try_from(save).map_err(|_| ObjectError::TextSizeOverflow)?,
                save_byte_count: usize::try_from(save_count)
                    .map_err(|_| ObjectError::TextSizeOverflow)?,
                restore_offset: usize::try_from(restore)
                    .map_err(|_| ObjectError::TextSizeOverflow)?,
                restore_byte_count: usize::try_from(restore_count)
                    .map_err(|_| ObjectError::TextSizeOverflow)?,
            })
        })
        .transpose()?;
    let (x86_floating_control, aarch64_floating_control) = match section.target.architecture {
        target::Architecture::X86_64 => (control, None),
        target::Architecture::Aarch64 => (None, control),
    };
    let callback_addresses = callback_address_custody(
        record,
        field.map_or(&[], |field| field.callback_materializations.as_slice()),
        plan.machine,
        owner,
    )?;
    Ok(ObjectForeignCall {
        machine: plan.machine,
        owner,
        operation_ordinal: plan.operation_ordinal,
        binding: record.call.binding.clone(),
        provider_execution: record.call.provider_execution.into(),
        caller_live_bytes: plan.caller_live_bytes,
        scalar_arguments,
        callback_addresses,
        scalar_result,
        x86_floating_control,
        aarch64_floating_control,
        text_offset,
    })
}

pub(crate) fn image_foreign_calls_match_object(
    artifact: &ObjectArtifact,
    image_rows: &[ObjectForeignCall],
) -> bool {
    image_rows == artifact.foreign_calls()
}
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
        let destination = match placed.destination.clone() {
            machine_code::CallbackAddressDestination::Register(register) => {
                machine_code::CallbackAddressDestination::Register(register)
            }
            machine_code::CallbackAddressDestination::OutgoingStack { byte_offset } => {
                machine_code::CallbackAddressDestination::OutgoingStack { byte_offset }
            }
            machine_code::CallbackAddressDestination::RecordField {
                root_register,
                placements,
            } => {
                if callback.field_placements.as_ref() != Some(&placements) {
                    return Err(invalid());
                }
                machine_code::CallbackAddressDestination::RecordField {
                    root_register,
                    placements,
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

#[cfg(test)]
mod tests {
    use super::*;
    use machine_code::{PlacedBlockSpan, PlacedFunctionFragment, PlacedInstructionSpan};
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
    };

    fn fragment(start: u64, sizes: &[u64]) -> PlacedFunctionFragment {
        let mut cursor = start;
        let instructions = sizes
            .iter()
            .enumerate()
            .map(|(index, &size)| {
                let row = PlacedInstructionSpan {
                    instruction: SelectedInstructionId(index as u32),
                    alternative: MachineAlternativeKey {
                        family: MachineAlternativeFamily::MaterializeI64,
                        variant: 0,
                    },
                    function_offset: cursor - start,
                    section_offset: cursor,
                    byte_count: size,
                };
                cursor += size;
                row
            })
            .collect();
        PlacedFunctionFragment {
            source_function_index: 0,
            machine: MachineId::new(1).unwrap(),
            section_offset: start,
            byte_count: cursor - start,
            blocks: vec![PlacedBlockSpan {
                block: SelectedBlockId(0),
                function_offset: 0,
                section_offset: start,
                byte_count: cursor - start,
                instructions,
            }],
        }
    }

    #[test]
    fn physical_transport_uses_current_spans_after_relayout() {
        let sequence = InstructionSpanSequence(vec![
            SelectedInstructionId(0),
            SelectedInstructionId(1),
            SelectedInstructionId(2),
        ]);
        assert_eq!(
            resolve_instruction_sequence(&fragment(32, &[7, 0, 3]), &sequence),
            Some((32, 10))
        );
        assert_eq!(
            resolve_instruction_sequence(&fragment(1024, &[2, 0, 3]), &sequence),
            Some((1024, 5))
        );
    }

    #[test]
    fn malformed_transport_cannot_hide_gaps_overlaps_or_duplicate_instruction_spans() {
        let sequence =
            InstructionSpanSequence(vec![SelectedInstructionId(0), SelectedInstructionId(1)]);
        for changed in [35, 37] {
            let mut rows = fragment(32, &[4, 4]);
            rows.blocks[0].instructions[1].section_offset = changed;
            assert_eq!(resolve_instruction_sequence(&rows, &sequence), None);
        }
        let mut rows = fragment(32, &[4, 4]);
        let duplicate = rows.blocks[0].instructions[0].clone();
        rows.blocks[0].instructions.push(duplicate);
        assert_eq!(resolve_instruction_sequence(&rows, &sequence), None);
        let mut rows = fragment(32, &[4, 4]);
        rows.blocks[0].instructions[0].section_offset = u64::MAX;
        assert_eq!(resolve_instruction_sequence(&rows, &sequence), None);
        assert_eq!(
            resolve_instruction_sequence(&fragment(32, &[4]), &sequence),
            None
        );
        assert_eq!(
            resolve_instruction_sequence(&fragment(32, &[4]), &InstructionSpanSequence(Vec::new())),
            None
        );
    }
}
