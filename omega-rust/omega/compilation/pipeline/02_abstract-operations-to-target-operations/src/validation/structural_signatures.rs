//! Reconcile retained structural ABI headers against the source declarations.
//! Executable bodies remain the responsibility of common graph replay.

use super::structural_shapes;
use abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use calling_conventions::{CallPlan, CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use semantic_vocabulary::{IntegerCarrier, IntegerType, ScalarType};
use target::NativeTarget;
use target_operations::{
    MixedStructuralScalarFunctionAbi, ScalarAbiValue, ScalarFunctionAbi,
    TargetDynamicDescriptorParameterAbi, TargetFunction, TargetStructuralParameter,
};
use terminal_psi::{
    StructuralAccess, StructuralTypeDeclaration, TerminalDynamicDescriptorParameter,
};

pub(super) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
    native_target: NativeTarget,
    declarations: &[StructuralTypeDeclaration],
) -> Option<()> {
    let signature = signature(source, declarations, native_target)?;
    let expected_plan =
        evaluate_call_plan(CallingPolicy::native_for_target(native_target), &signature).ok()?;
    entrance(
        source,
        &signature,
        &expected_plan,
        &target.graph.call_plan,
        &target.graph.scalar_parameters,
        &target.graph.parameters,
        &target.graph.dynamic_parameters,
    )?;
    // Standalone receiving entrances: a caller outside this plan — native
    // entry, emitted fragments, installation records — observes only the
    // published ABI rows, never a caller's embedded plan. The two forms are
    // mutually exclusive by construction, so a present row must independently
    // re-derive the same signature and bind its scalar and result identities;
    // a physically plausible plan on an ineligible function is a substituted
    // entrance, not a copy equivalence.
    if let Some(abi) = &target.scalar_abi {
        scalar_entrance(source, &expected_plan, abi)?;
    }
    if let Some(abi) = &target.mixed_structural_scalar_abi {
        mixed_entrance(source, &signature, &expected_plan, abi)?;
    }
    Some(())
}

/// A function's declared borrowed descriptor parameters: the leading run of
/// `DynamicDescriptorParameter` interface declarations, each bound to the
/// dense trailing lane position and authored source position the producer's
/// signature preparation admits. A stray declaration after the leading run,
/// or a row whose ordinal, owner, source position, or borrowed access does
/// not match the lane contract, has no honest signature.
pub(super) fn declared_dynamic_parameters(
    source: &AbstractFunction,
) -> Option<Vec<&TerminalDynamicDescriptorParameter>> {
    let declared = source
        .operations
        .iter()
        .take_while(|operation| {
            matches!(
                operation,
                AbstractOperation::DynamicDescriptorParameter { .. }
            )
        })
        .filter_map(|operation| match operation {
            AbstractOperation::DynamicDescriptorParameter { parameter } => Some(parameter),
            _ => None,
        })
        .collect::<Vec<_>>();
    let nonself_structural_count = source
        .structural_parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    if source
        .operations
        .iter()
        .skip(declared.len())
        .any(|operation| {
            matches!(
                operation,
                AbstractOperation::DynamicDescriptorParameter { .. }
            )
        })
        || declared.iter().enumerate().any(|(index, parameter)| {
            parameter.owner != source.machine
                || parameter.ordinal != u32::try_from(index).unwrap_or(u32::MAX)
                || parameter.source_position
                    != u32::try_from(source.parameters.len() + nonself_structural_count + index)
                        .unwrap_or(u32::MAX)
                || !matches!(
                    parameter.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                )
        })
    {
        return None;
    }
    Some(declared)
}

/// The complete call signature one source function's declarations determine:
/// scalar parameters first in declared order, then each structural parameter's
/// access-selected shape, then each descriptor parameter's `{instance, table}`
/// pointer pair, then the result. Every producer of an ABI plan for this
/// function — its own entrance, any caller's embedded call plan, or a
/// published standalone receiving ABI — must reconstruct this same signature;
/// the physical shape alone is not authority.
pub(super) fn signature(
    source: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
    native_target: NativeTarget,
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
    let dynamic_parameters = declared_dynamic_parameters(source)?;
    let pointer_size = u16::try_from(native_target.pointer_size).ok()?;
    let pointer_alignment = u16::try_from(native_target.pointer_alignment).ok()?;
    let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
    Some(CallSignature {
        parameters: source
            .parameters
            .iter()
            .map(|parameter| structural_shapes::scalar_shape(parameter.scalar_type))
            .chain(structural_shapes.iter().copied())
            .chain(std::iter::repeat_n(
                pointer_shape,
                dynamic_parameters.len() * 2,
            ))
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

/// One receiving entrance's retained rows against the derived signature plan:
/// the anonymous placements bind back to the declared scalar and structural
/// parameter identities, so a substituted value, type, or placement row
/// cannot share the physical shape of the honest entrance.
#[allow(clippy::too_many_arguments)]
fn entrance(
    source: &AbstractFunction,
    signature: &CallSignature,
    expected_plan: &CallPlan,
    actual_plan: &CallPlan,
    actual_scalar_parameters: &[ScalarAbiValue],
    actual_parameters: &[TargetStructuralParameter],
    actual_dynamic_parameters: &[TargetDynamicDescriptorParameterAbi],
) -> Option<()> {
    let declared_dynamic_parameters = declared_dynamic_parameters(source)?;
    if actual_parameters.len() != source.structural_parameters.len()
        || actual_dynamic_parameters.len() != declared_dynamic_parameters.len()
        || actual_plan.parameters.len()
            != source.parameters.len()
                + actual_parameters.len()
                + declared_dynamic_parameters.len() * 2
        || actual_plan != expected_plan
    {
        return None;
    }
    scalar_rows(source, actual_scalar_parameters, expected_plan)?;
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
    // Each retained descriptor row must carry its declared parameter verbatim
    // and bind the exact `{instance, table}` placements the independently
    // evaluated plan assigns the dense trailing lane.
    let descriptor_base = source.parameters.len() + source.structural_parameters.len();
    for (index, (declared, actual)) in declared_dynamic_parameters
        .iter()
        .zip(actual_dynamic_parameters)
        .enumerate()
    {
        let (Some(instance), Some(table)) = (
            expected_plan.parameters.get(descriptor_base + index * 2),
            expected_plan
                .parameters
                .get(descriptor_base + index * 2 + 1),
        ) else {
            return None;
        };
        if actual.parameter != **declared || actual.instance != *instance || actual.table != *table
        {
            return None;
        }
    }
    Some(())
}

/// The published scalar-only ABI a function receives standalone calls through.
/// `derive_fixed_scalar_function_abi` admits only the service-free family with
/// no structural parameters, entry claims, or dynamic descriptors, and only
/// fixed native shapes; a present ABI on any other function is forged no
/// matter how consistent its plan looks.
fn scalar_entrance(
    source: &AbstractFunction,
    expected_plan: &CallPlan,
    abi: &ScalarFunctionAbi,
) -> Option<()> {
    if !source.structural_parameters.is_empty()
        || !source.entry_claims.is_empty()
        || !source.published_service_ceiling.is_empty()
        || source.operations.iter().any(|operation| {
            matches!(
                operation,
                AbstractOperation::DynamicDescriptorParameter { .. }
            )
        })
        || source
            .parameters
            .iter()
            .any(|parameter| fixed_native_scalar_shape(parameter.scalar_type).is_none())
    {
        return None;
    }
    let result = source.result.scalar()?;
    fixed_native_scalar_shape(result.scalar_type)?;
    if abi.call_plan != *expected_plan
        || abi.result.value != result.value
        || abi.result.scalar_type != result.scalar_type
        || expected_plan.result.as_ref() != Some(&abi.result.placement)
    {
        return None;
    }
    scalar_rows(source, &abi.parameters, expected_plan)
}

/// The published mixed ABI for a scalar-result function whose parameter
/// roster ends in structural parameters. The scalar prefix and result admit
/// only Booleans and fixed native integers; the structural suffix replays
/// through the shared entrance check.
fn mixed_entrance(
    source: &AbstractFunction,
    signature: &CallSignature,
    expected_plan: &CallPlan,
    abi: &MixedStructuralScalarFunctionAbi,
) -> Option<()> {
    if source.structural_parameters.is_empty()
        || !source.entry_claims.is_empty()
        || !source.published_service_ceiling.is_empty()
        || source.operations.iter().any(|operation| {
            matches!(
                operation,
                AbstractOperation::DynamicDescriptorParameter { .. }
            )
        })
        || source
            .parameters
            .iter()
            .any(|parameter| fixed_integer_or_boolean_shape(parameter.scalar_type).is_none())
    {
        return None;
    }
    let result = source.result.scalar()?;
    fixed_integer_or_boolean_shape(result.scalar_type)?;
    entrance(
        source,
        signature,
        expected_plan,
        &abi.call_plan,
        &abi.scalar_parameters,
        &abi.structural_parameters,
        &[],
    )?;
    if abi.result.value != result.value
        || abi.result.scalar_type != result.scalar_type
        || expected_plan.result.as_ref() != Some(&abi.result.placement)
    {
        return None;
    }
    Some(())
}

/// Ordered scalar rows bind the plan's anonymous parameter placements back to
/// the declared value and type identities.
fn scalar_rows(
    source: &AbstractFunction,
    actual: &[ScalarAbiValue],
    expected_plan: &CallPlan,
) -> Option<()> {
    if actual.len() != source.parameters.len() {
        return None;
    }
    for ((actual, declared), placement) in actual
        .iter()
        .zip(&source.parameters)
        .zip(&expected_plan.parameters)
    {
        if actual.value != declared.value
            || actual.scalar_type != declared.scalar_type
            || actual.placement != *placement
        {
            return None;
        }
    }
    Some(())
}

/// Producer-independent fixed native shape eligibility for the published
/// scalar ABI: Booleans, fixed 8/16/32/64 integers, and IEEE floats.
pub(super) fn fixed_native_scalar_shape(scalar_type: ScalarType) -> Option<ValueShape> {
    match scalar_type {
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::Integer(integer) => fixed_native_integer_shape(integer),
        ScalarType::IeeeFloat(format) => Some(ValueShape::float(match format {
            semantic_vocabulary::IeeeFloatFormat::Binary32 => 4,
            semantic_vocabulary::IeeeFloatFormat::Binary64 => 8,
        })),
    }
}

/// The narrower mixed-ABI scalar family: Booleans and fixed native integers.
fn fixed_integer_or_boolean_shape(scalar_type: ScalarType) -> Option<ValueShape> {
    match scalar_type {
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::Integer(integer) => fixed_native_integer_shape(integer),
        ScalarType::IeeeFloat(_) => None,
    }
}

pub(super) fn fixed_native_integer_shape(integer: IntegerType) -> Option<ValueShape> {
    if integer.carrier() != IntegerCarrier::Fixed || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let bytes = integer.bits().div_ceil(8);
    Some(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
}
