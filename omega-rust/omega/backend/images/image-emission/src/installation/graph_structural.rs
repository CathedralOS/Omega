//! Graph parameters retain exact value or reference ABI and published pointer homes.
//! This is record shape only: the mandatory exact image join retains graph replay.
#[cfg(test)]
mod tests;
use super::InstalledFunction;
use calling_conventions::{
    CallSignature, CallingPolicy, ValueClass, ValueLocation, evaluate_call_plan,
};
use machine_code::StructuralSourceLocation;
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

pub(super) fn function_is_exact(
    function: &InstalledFunction,
    target: target::NativeTarget,
) -> bool {
    if function.unit_body
        || function.unit_affine_cleanup.is_some()
        || function.scalar_affine_cleanup.is_some()
        || !function.unit_continuations.is_empty()
        || !function.scalar_control_affine_cleanups.is_empty()
    {
        return false;
    }
    let (parameters, homes, scalars, plan) =
        if let Some(abi) = &function.mixed_structural_scalar_abi {
            if !function.unit_parameters.is_empty()
                || !function.unit_parameter_homes.is_empty()
                || abi.structural_parameters.len() != function.scalar_structural_parameters.len()
                || abi
                    .structural_parameters
                    .iter()
                    .zip(&function.scalar_structural_parameters)
                    .any(|(expected, actual)| {
                        expected.place != actual.place
                            || expected.structural_type != actual.structural_type
                            || expected.multiplicity != actual.multiplicity
                            || expected.access != actual.access
                            || expected.shape != actual.shape
                    })
            {
                return false;
            }
            (
                &function.scalar_structural_parameters,
                &function.scalar_structural_parameter_homes,
                &abi.scalar_parameters,
                &abi.call_plan,
            )
        } else {
            let Some(abi) = &function.parameter_abi else {
                return false;
            };
            if !abi.entry_register_spills.is_empty()
                || function.scalar_abi.is_some()
                || !function.scalar_structural_parameters.is_empty()
                || !function.scalar_structural_parameter_homes.is_empty()
            {
                return false;
            }
            (
                &function.unit_parameters,
                &function.unit_parameter_homes,
                &abi.parameters,
                &abi.call_plan,
            )
        };
    let mut shapes = Vec::new();
    for (position, scalar) in scalars.iter().enumerate() {
        // Scalar banks do not depend on whether the accompanying structural
        // argument is an owned value or a borrowed pointer.
        let Some(shape) = super::borrowed_structural::scalar_shape(scalar.scalar_type) else {
            return false;
        };
        if scalar.placement.shape != shape
            || scalars[..position]
                .iter()
                .any(|prior| prior.value == scalar.value)
        {
            return false;
        }
        shapes.push(shape);
    }
    shapes.extend(parameters.iter().map(|parameter| parameter.shape));
    let Ok(expected) = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: shapes,
            result: plan.result.as_ref().map(|result| result.shape),
        },
    ) else {
        return false;
    };
    if expected != *plan
        || scalars
            .iter()
            .zip(&plan.parameters)
            .any(|(scalar, placement)| scalar.placement != *placement)
    {
        return false;
    }
    let mut remaining_homes = homes.iter();
    let mut owned = false;
    for (position, (parameter, placement)) in parameters
        .iter()
        .zip(&plan.parameters[scalars.len()..])
        .enumerate()
    {
        if parameter.shape != placement.shape
            || parameters[..position]
                .iter()
                .any(|prior| prior.place == parameter.place)
        {
            return false;
        }
        if parameter.access == StructuralAccess::Owned {
            if placement.shape.class != ValueClass::Integer
                || (placement.locations.is_empty()
                    && placement.shape != calling_conventions::ValueShape::integer(0, 1))
                || !(placement.locations.iter().all(|location| {
                    matches!(
                        location,
                        ValueLocation::Register { .. } | ValueLocation::Stack { .. }
                    )
                }) || matches!(
                    placement.locations.as_slice(),
                    [ValueLocation::Indirect {
                        copy_stack_byte_offset: Some(_),
                        ..
                    }]
                ))
            {
                return false;
            }
            // The canonical plan fixes pointer placement, payload extent and
            // outbound copy storage. Keep published indirect home identity;
            // graph/source replay separately validates qualification and claims.
            if let [ValueLocation::Indirect { pointer, .. }] = placement.locations.as_slice() {
                let Some(home) = remaining_homes.next() else {
                    return false;
                };
                let location = match pointer {
                    calling_conventions::IndirectPointerLocation::Register(register) => {
                        StructuralSourceLocation::IncomingIndirectPointer {
                            register: *register,
                        }
                    }
                    calling_conventions::IndirectPointerLocation::Stack {
                        stack_byte_offset,
                        alignment,
                    } => StructuralSourceLocation::IncomingIndirectStackPointer {
                        stack_byte_offset: *stack_byte_offset,
                        alignment: *alignment,
                    },
                };
                if parameter.place != home.place
                    || parameter.structural_type != home.structural_type
                    || parameter.access != home.access
                    || parameter.multiplicity != home.multiplicity
                    || parameter.shape != home.shape
                    || home.source != *placement
                    || !home.indirect
                    || home.location != location
                {
                    return false;
                }
            }
            owned = true;
            continue;
        }
        let Some(home) = remaining_homes.next() else {
            return false;
        };
        if parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || parameter.place != home.place
            || parameter.structural_type != home.structural_type
            || parameter.access != home.access
            || parameter.multiplicity != home.multiplicity
            || parameter.shape != home.shape
            || home.source != *placement
            || !home.indirect
            || super::borrowed_structural::pointer_location(placement)
                .map(|location| StructuralSourceLocation::IncomingBorrowedPointer { location })
                != Some(home.location)
        {
            return false;
        }
    }
    owned && remaining_homes.next().is_none()
}
