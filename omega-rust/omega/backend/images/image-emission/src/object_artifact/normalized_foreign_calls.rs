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
//! Custody that exists only as emitted-instruction evidence — scalar argument
//! records, durable scalar result homes, floating-control intervals, and
//! callback materialization — is not projected here. A call carrying scalar
//! argument rows does not derive a custody row yet and fails closed instead.

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
/// one on the stack. Scalar/result marshalling custody is not part of this
/// projection; rows under it remain empty and are widened by a later leg.
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
        let Some(instruction) = function
            .blocks
            .iter()
            .find(|block| block.id == call.block)
            .and_then(|block| block.instructions.get(call.instruction.0 as usize))
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
        if !record.call.scalar_arguments.is_empty() {
            return Err(ObjectError::InvalidForeignCallArgument {
                caller: call.caller,
                owner,
            });
        }
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
            scalar_arguments: Vec::new(),
            callback_address: None,
            scalar_result: None,
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
