use omega_abstract_operations::{
    AbstractFunctionResult, AbstractOperationPlanWithPlacedViewInputs,
};
use omega_calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, ValueShape, evaluate_call_plan,
};
use omega_target::NativeTarget;
use omega_target_operations::{TargetOperationPlanWithPlacedViewInputs, TargetPlacedViewInput};

use crate::{
    LoweringError, PlacedViewInputTranslationError, SelectedPlacedViewInputPlan,
    lower_to_target_operations,
};

/// Lower the first bounded plan-laid input family: one direct program-entry
/// pointer on an otherwise parameterless, one-block Unit function.
///
/// This produces ABI custody only. It does not select backing, establish a
/// lifetime, or emit a placed access event.
pub fn lower_to_target_operations_with_placed_view_inputs(
    source: &AbstractOperationPlanWithPlacedViewInputs,
    target: NativeTarget,
    selections: &[SelectedPlacedViewInputPlan<'_>],
) -> Result<TargetOperationPlanWithPlacedViewInputs, LoweringError> {
    let plan = lower_to_target_operations(&source.plan, target)?;
    let (entry_call_plan, placed_view_inputs) =
        derive_placed_entry_abi(source, target, selections)?;
    let lowered = TargetOperationPlanWithPlacedViewInputs {
        plan,
        entry_call_plan,
        placed_view_inputs,
    };
    validate_placed_view_input_translation(source, selections, target, &lowered)?;
    Ok(lowered)
}

/// Independently reconstruct the complete bounded placed-input target carrier
/// and reject any missing, extra, reordered, or target-substituted data.
pub fn validate_placed_view_input_translation(
    source: &AbstractOperationPlanWithPlacedViewInputs,
    selections: &[SelectedPlacedViewInputPlan<'_>],
    expected_target: NativeTarget,
    candidate: &TargetOperationPlanWithPlacedViewInputs,
) -> Result<(), PlacedViewInputTranslationError> {
    let expected_plan = lower_to_target_operations(&source.plan, expected_target)
        .map_err(|_| PlacedViewInputTranslationError::CandidatePlanMismatch)?;
    if candidate.plan != expected_plan {
        return Err(PlacedViewInputTranslationError::CandidatePlanMismatch);
    }
    let (expected_call_plan, expected_inputs) =
        derive_placed_entry_abi(source, expected_target, selections)
            .map_err(placed_error_from_lowering)?;
    if candidate.entry_call_plan != expected_call_plan {
        return Err(PlacedViewInputTranslationError::CandidateEntryCallPlanMismatch);
    }
    if candidate.placed_view_inputs != expected_inputs {
        return Err(PlacedViewInputTranslationError::CandidateInputRosterMismatch);
    }
    Ok(())
}

fn derive_placed_entry_abi(
    source: &AbstractOperationPlanWithPlacedViewInputs,
    target: NativeTarget,
    selections: &[SelectedPlacedViewInputPlan<'_>],
) -> Result<(CallPlan, Vec<TargetPlacedViewInput>), LoweringError> {
    let [input] = source.placed_view_inputs.as_slice() else {
        return Err(PlacedViewInputTranslationError::UnsupportedInputCount(
            source.placed_view_inputs.len(),
        )
        .into());
    };
    if selections.len() != 1 {
        return Err(PlacedViewInputTranslationError::SelectionCountMismatch {
            expected: 1,
            actual: selections.len(),
        }
        .into());
    }
    if input.machine != source.plan.entry {
        return Err(PlacedViewInputTranslationError::InputIsNotDirectEntry.into());
    }
    let Some(entry) = source
        .plan
        .functions
        .iter()
        .find(|function| function.machine == source.plan.entry)
    else {
        return Err(
            PlacedViewInputTranslationError::UnsupportedEntryFunctionShape(source.plan.entry)
                .into(),
        );
    };
    if entry.result != AbstractFunctionResult::Unit
        || !entry.parameters.is_empty()
        || !entry.structural_parameters.is_empty()
        || entry.block_entries.len() != 1
    {
        return Err(
            PlacedViewInputTranslationError::UnsupportedEntryFunctionShape(source.plan.entry)
                .into(),
        );
    }
    let selection = &selections[0];
    if selection.terminal_input != input {
        return Err(PlacedViewInputTranslationError::SelectionRowMismatch.into());
    }
    if selection
        .placement_plan
        .identity()
        .compatibility_fingerprint()
        != input.placement_report_fingerprint
        || selection
            .placement_plan
            .content_interpretation()
            .commitment()
            != input.placement_commitment
    {
        return Err(PlacedViewInputTranslationError::PlacementPlanIdentityMismatch.into());
    }
    let layout = selection.placement_plan.layout();
    let Some(referent_byte_size) = layout.size else {
        return Err(PlacedViewInputTranslationError::PlacementPlanHasNoConcreteSize.into());
    };
    let pointer_size = u16::try_from(target.pointer_size)
        .map_err(|_| PlacedViewInputTranslationError::TargetPointerShapeUnsupported)?;
    let pointer_alignment = u16::try_from(target.pointer_alignment)
        .map_err(|_| PlacedViewInputTranslationError::TargetPointerShapeUnsupported)?;
    let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
    let entry_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![pointer_shape],
            result: None,
        },
    )
    .map_err(PlacedViewInputTranslationError::AbiPlan)?;
    let [placement] = entry_call_plan.parameters.as_slice() else {
        return Err(PlacedViewInputTranslationError::TargetPointerShapeUnsupported.into());
    };
    let placement = placement.clone();
    Ok((
        entry_call_plan,
        vec![TargetPlacedViewInput {
            terminal: input.clone(),
            abi_parameter_ordinal: 0,
            referent_byte_size,
            referent_alignment: layout.align,
            placement,
        }],
    ))
}

fn placed_error_from_lowering(error: LoweringError) -> PlacedViewInputTranslationError {
    match error {
        LoweringError::PlacedViewInput(error) => error,
        _ => PlacedViewInputTranslationError::CandidatePlanMismatch,
    }
}
