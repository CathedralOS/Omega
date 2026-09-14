//! Reconcile retained structural ABI headers against the source declarations.
//! Executable bodies remain the responsibility of common graph replay.

use super::structural_shapes;
use abstract_operations::{AbstractFunction, AbstractFunctionResult};
use calling_conventions::{CallPlan, CallSignature, CallingPolicy, evaluate_call_plan};
use target::NativeTarget;
use target_operations::{TargetFunction, TargetStructuralParameter};
use terminal_psi::StructuralTypeDeclaration;

pub(super) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
    native_target: NativeTarget,
    declarations: &[StructuralTypeDeclaration],
) -> Option<()> {
    header(
        source,
        &target.graph.call_plan,
        &target.graph.parameters,
        native_target,
        declarations,
    )?;
    if let Some(abi) = &target.mixed_structural_scalar_abi {
        header(
            source,
            &abi.call_plan,
            &abi.structural_parameters,
            native_target,
            declarations,
        )?;
    }
    Some(())
}

/// The complete call signature one source function's declarations determine:
/// scalar parameters first in declared order, then each structural parameter's
/// access-selected shape, then the result. Every producer of an ABI plan for
/// this function — its own entrance or any caller's embedded call plan — must
/// reconstruct this same signature; the physical shape alone is not authority.
pub(super) fn signature(
    source: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
) -> Option<CallSignature> {
    let structural_shapes = source
        .structural_parameters
        .iter()
        .map(|parameter| {
            let referent =
                structural_shapes::reconstruct(parameter.structural_type, declarations).ok()?;
            Some(structural_shapes::parameter_shape(
                referent,
                parameter.access,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(CallSignature {
        parameters: source
            .parameters
            .iter()
            .map(|parameter| structural_shapes::scalar_shape(parameter.scalar_type))
            .chain(structural_shapes.iter().copied())
            .collect(),
        result: match &source.result {
            AbstractFunctionResult::Unit => None,
            AbstractFunctionResult::Scalar(result) => {
                Some(structural_shapes::scalar_shape(result.scalar_type))
            }
            AbstractFunctionResult::Structural(result) => {
                Some(structural_shapes::reconstruct(result.structural_type, declarations).ok()?)
            }
        },
    })
}

fn header(
    source: &AbstractFunction,
    actual_plan: &CallPlan,
    actual_parameters: &[TargetStructuralParameter],
    native_target: NativeTarget,
    declarations: &[StructuralTypeDeclaration],
) -> Option<()> {
    if actual_parameters.len() != source.structural_parameters.len()
        || actual_plan.parameters.len() != source.parameters.len() + actual_parameters.len()
    {
        return None;
    }
    let signature = signature(source, declarations)?;
    let expected_plan =
        evaluate_call_plan(CallingPolicy::native_for_target(native_target), &signature).ok()?;
    if &expected_plan != actual_plan {
        return None;
    }
    for (((declared, actual), shape), placement) in source
        .structural_parameters
        .iter()
        .zip(actual_parameters)
        .zip(&signature.parameters[source.parameters.len()..])
        .zip(
            expected_plan
                .parameters
                .iter()
                .skip(source.parameters.len()),
        )
    {
        if actual.place != declared.place
            || actual.structural_type != declared.structural_type
            || actual.multiplicity != declared.multiplicity
            || actual.access != declared.access
            || actual.projected_qualifications != declared.projected_qualifications
            || actual.shape != *shape
            || &actual.placement != placement
        {
            return None;
        }
    }
    Some(())
}
