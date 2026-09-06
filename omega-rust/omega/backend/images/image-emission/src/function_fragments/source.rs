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
    if (fragments.functions.is_empty() && fragments.structural_unit_functions.is_empty())
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
    if !fragments.structural_unit_functions.is_empty() {
        return super::structural::admit(source);
    }
    for fragment in &fragments.functions {
        let (abstracted, targeted) = function(source, fragment.machine)?;
        let unit = matches!(abstracted.result, AbstractFunctionResult::Unit);
        if abstracted.attachment != fragment.attachment
            || targeted.provenance != fragment.provenance
            || targeted.mixed_structural_scalar_abi.is_some()
            || !abstracted.structural_parameters.is_empty()
            || !abstracted.entry_claims.is_empty()
            || !abstracted.published_service_ceiling.is_empty()
            || (unit
                && (!abstracted.parameters.is_empty()
                    || targeted.fixed_integer_scalar_abi.is_some()))
            || (!unit && targeted.fixed_integer_scalar_abi.is_none())
        {
            return Err(Error::Unsupported(
                "shared function has unsupported ABI or boundary effects",
            ));
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
                } if unit => {
                    let (body, target) = function(source, *callee)?;
                    target
                        .fixed_integer_scalar_abi
                        .as_ref()
                        .is_some_and(|abi| abi.parameters.len() == arguments.len())
                        && !matches!(body.result, AbstractFunctionResult::Unit)
                        && !body
                            .operations
                            .iter()
                            .any(|operation| matches!(operation, AbstractOperation::Call { .. }))
                        && arguments.len() == body.parameters.len()
                        && requirement_obligations.is_empty()
                        && crash_continuations.is_empty()
                }
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
                | AbstractOperation::ExactIntegerSubtract { .. }
                    if !unit =>
                {
                    true
                }
                AbstractOperation::Jump {
                    trivial_affine_discards,
                    ..
                } if !unit => trivial_affine_discards.is_empty(),
                AbstractOperation::Conditional {
                    when_true,
                    when_false,
                    ..
                } if !unit => {
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
    if let Some(function) = plan
        .structural_unit_functions
        .iter()
        .find(|row| row.machine == machine)
    {
        return Ok((function.attachment, &function.provenance));
    }
    Err(Error::Mismatch("missing placed fragment"))
}
