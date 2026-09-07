//! Admission predicates and immutable inputs shared by construction and replay.

use super::Error;
use abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use machine_code::{FunctionFragmentEmissionPlan, FunctionTargetFrameLayout};
use object_file::{
    StagedOptimizedObjectTextSectionSource, StagedOptimizedRelocationFreeObjectContainer,
};
use semantic_vocabulary::MachineId;

pub(super) fn fragments(
    source: &StagedOptimizedRelocationFreeObjectContainer,
) -> &FunctionFragmentEmissionPlan {
    match source.source() {
        StagedOptimizedObjectTextSectionSource::Direct(text) => text.source().fragments(),
        StagedOptimizedObjectTextSectionSource::FixedFrame(text) => text.source().fragments(),
    }
}

pub(super) fn function(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
) -> Result<(&AbstractFunction, &target_operations::TargetFunction), Error> {
    let current = source.source().source().source().optimized_target();
    let mut abstracted = current
        .optimized()
        .plan()
        .functions
        .iter()
        .filter(|function| function.machine == machine);
    let mut targeted = current
        .target_operations()
        .functions
        .iter()
        .filter(|function| function.machine == machine);
    match (
        abstracted.next(),
        abstracted.next(),
        targeted.next(),
        targeted.next(),
    ) {
        (Some(abstracted), None, Some(targeted), None) => Ok((abstracted, targeted)),
        _ => Err(Error::Mismatch(
            "shared function has no unique current source",
        )),
    }
}

pub(super) fn frame(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
) -> Result<Option<&FunctionTargetFrameLayout>, Error> {
    let Some(layout) = source.source().source().source().frame_layout() else {
        return Ok(None);
    };
    let mut rows = layout
        .plan()
        .functions
        .iter()
        .filter(|row| row.machine == machine);
    match (rows.next(), rows.next()) {
        (Some(row), None) => Ok(Some(row)),
        _ => Err(Error::Mismatch(
            "shared frame has no unique function geometry",
        )),
    }
}

pub(super) fn admit(source: &StagedOptimizedRelocationFreeObjectContainer) -> Result<(), Error> {
    object_file::validate_optimized_relocation_free_object_container(source)
        .map_err(Error::Source)?;
    let fragments = fragments(source);
    if fragments.functions.is_empty()
        || fragments.target.pointer_size != 8
        || fragments.target.pointer_alignment != 8
    {
        return Err(Error::Unsupported(
            "shared image publication requires a nonempty 64-bit function roster",
        ));
    }
    let current = source.source().source().source();
    let has_frame = current.frame_layout().is_some();
    if has_frame != current.frame_protocol().is_some()
        || has_frame
            != matches!(
                source.source(),
                StagedOptimizedObjectTextSectionSource::FixedFrame(_)
            )
    {
        return Err(Error::Mismatch(
            "shared text does not apply its exact frame",
        ));
    }
    for fragment in &fragments.functions {
        let (abstracted, targeted) = function(source, fragment.machine)?;
        let unit = matches!(abstracted.result, AbstractFunctionResult::Unit);
        let selected = current
            .selected_plan()
            .functions
            .iter()
            .find(|row| row.machine == fragment.machine)
            .ok_or(Error::Mismatch("missing selected function"))?;
        let structural = selected.structural.as_ref();
        if abstracted.attachment != fragment.attachment
            || targeted.provenance != fragment.provenance
            || targeted.mixed_structural_scalar_abi.is_some()
            || (!abstracted.structural_parameters.is_empty() && structural.is_none())
            || (structural.is_some() && !unit)
            || (structural.is_none()
                && (!abstracted.entry_claims.is_empty()
                    || !abstracted.published_service_ceiling.is_empty()))
            || (unit && targeted.scalar_abi.is_some())
            || (!unit && targeted.scalar_abi.is_none())
        {
            return Err(Error::Unsupported(
                "shared function has unsupported ABI or boundary effects",
            ));
        }
        if unit && !abstracted.parameters.is_empty() {
            let body = unit_scalar_body(targeted).ok_or(Error::Mismatch(
                "parameterized Unit function has no retained scalar ABI",
            ))?;
            if body.scalar_parameters.len() != abstracted.parameters.len()
                || body.call_plan.parameters.len() != abstracted.parameters.len()
                || body.call_plan.result.is_some()
                || !body.parameters.is_empty()
                || body
                    .scalar_parameters
                    .iter()
                    .zip(&abstracted.parameters)
                    .zip(&body.call_plan.parameters)
                    .any(|((row, declaration), placement)| {
                        row.value != declaration.value
                            || row.scalar_type != declaration.scalar_type
                            || row.placement != *placement
                    })
            {
                return Err(Error::Mismatch(
                    "Unit scalar ABI differs from current source",
                ));
            }
        } else if unit_scalar_body(targeted).is_some() {
            return Err(Error::Mismatch("unexpected Unit scalar ABI"));
        }
        for operation in &abstracted.operations {
            let admitted = match operation {
                AbstractOperation::IntegerConstant { .. } => true,
                AbstractOperation::Call {
                    callee,
                    arguments,
                    requirement_obligations,
                    crash_continuations,
                    ..
                } => {
                    let (body, target) = function(source, *callee)?;
                    target
                        .scalar_abi
                        .as_ref()
                        .is_some_and(|abi| abi.parameters.len() == arguments.len())
                        && !matches!(body.result, AbstractFunctionResult::Unit)
                        && arguments.len() == body.parameters.len()
                        && requirement_obligations.is_empty()
                        && crash_continuations.is_empty()
                }
                AbstractOperation::CallUnit { psi_operation, .. } => selected.calls.iter().any(|row| row.operation == *psi_operation && row.call.result_placement.is_none()),
                AbstractOperation::BoundaryCall { psi_operation, .. } => selected.boundary_settlements.iter().any(|row| row.settlement.operation == *psi_operation)
                    || selected.calls.iter().any(|row| row.operation == *psi_operation && matches!(row.call.source, legalized_operations::LegalizedCallUnitSource::InstalledProvider { .. })),
                AbstractOperation::Return {
                    cleanup_actions, ..
                }
                | AbstractOperation::ReturnUnit {
                    cleanup_actions, ..
                } => cleanup_actions.is_empty(),
                AbstractOperation::IntegerEqual { .. }
                | AbstractOperation::IntegerLessThan { .. }
                | AbstractOperation::IntegerLessOrEqual { .. }
                | AbstractOperation::BooleanNot { .. }
                | AbstractOperation::IntegerWiden { .. }
                | AbstractOperation::ExactIntegerAdd { .. }
                | AbstractOperation::ExactIntegerSubtract { .. } => true,
                AbstractOperation::Jump {
                    trivial_affine_discards,
                    ..
                } => trivial_affine_discards.is_empty(),
                AbstractOperation::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    when_true.trivial_affine_discards.is_empty()
                        && when_false.trivial_affine_discards.is_empty()
                }
                _ => false,
            };
            if !admitted {
                return Err(Error::Unsupported(
                    "shared function contains an unaccounted operation",
                ));
            }
        }
    }
    Ok(())
}

/// Borrow already validated target ABI facts; this does not construct an ABI plan.
pub(super) fn unit_scalar_body(
    function: &target_operations::TargetFunction,
) -> Option<&target_operations::TargetUnitBody> {
    match &function.operation {
        target_operations::TargetOperation::UnitBody(body)
            if !body.scalar_parameters.is_empty() =>
        {
            Some(body)
        }
        _ => None,
    }
}

pub(super) fn fragment_metadata(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    machine: MachineId,
) -> Result<
    (
        Option<semantic_vocabulary::StructuralTypeId>,
        &target_operations::TerminalPsiProvenance,
    ),
    Error,
> {
    let plan = fragments(source);
    if let Some(function) = plan.functions.iter().find(|row| row.machine == machine) {
        return Ok((function.attachment, &function.provenance));
    }
    Err(Error::Mismatch("missing placed fragment"))
}
