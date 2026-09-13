//! Reconstruct structural parameter shape and incoming ABI storage.

use calling_conventions::{
    CallPlan, IndirectPointerLocation, ValueClass, ValueLocation, ValuePlacement,
};
use semantic_vocabulary::{IntegerCarrier, ScalarType};
use terminal_psi::{StructuralAccess, StructuralTypeShape};

#[derive(Clone, Copy)]
pub(crate) struct Parameter<'a> {
    pub semantic: &'a terminal_psi::StructuralParameterDeclaration,
    pub target: &'a target_operations::TargetStructuralParameter,
}

/// Each graph parameter has its own storage relation; result shape and statement
/// sequencing cannot turn an owned fragment into a borrowed pointer.
pub(crate) fn accepts_graph(
    call_plan: &CallPlan,
    parameters: &[Parameter<'_>],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    let Some(scalar_count) = call_plan.parameters.len().checked_sub(parameters.len()) else {
        return false;
    };
    let mut shapes = call_plan.parameters[..scalar_count]
        .iter()
        .map(|placement| placement.shape)
        .collect::<Vec<_>>();
    for (position, parameter) in parameters.iter().enumerate() {
        let semantic = parameter.semantic;
        // Multiplicity and qualification obligations govern operations, not
        // the physical shape of an owned value. Exact source/ownership replay
        // retains those obligations; layout must not require one body family
        // merely to carry a qualified or linear value through its ABI.
        let Some(shape) = (if semantic.access == StructuralAccess::Owned {
            crate::structural_reference_input::primitive_array_shape(
                semantic.structural_type,
                structural_types,
            )
            .or_else(|| {
                crate::structural_reference_input::shape(semantic.structural_type, structural_types)
            })
        } else {
            crate::structural_reference_input::parameter_shape(semantic, structural_types)
        }) else {
            return false;
        };
        if semantic.position as usize != position
            || parameters[..position]
                .iter()
                .any(|previous| previous.semantic.place == semantic.place)
            || parameter.target.place != semantic.place
            || parameter.target.structural_type != semantic.structural_type
            || parameter.target.access != semantic.access
            || parameter.target.multiplicity != semantic.multiplicity
            || parameter.target.projected_qualifications != semantic.projected_qualifications
            || parameter.target.shape != shape
            || parameter.target.placement != call_plan.parameters[scalar_count + position]
        {
            return false;
        }
        shapes.push(shape);
    }
    calling_conventions::evaluate_call_plan(
        call_plan.policy,
        &calling_conventions::CallSignature {
            parameters: shapes,
            result: call_plan.result.as_ref().map(|placement| placement.shape),
        },
    )
    .is_ok_and(|expected| expected == *call_plan)
}

pub(crate) fn accepts_borrowed_parameters(
    call_plan: &CallPlan,
    parameters: &[Parameter<'_>],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    if parameters.is_empty() {
        return false;
    }
    let Some(scalar_count) = call_plan.parameters.len().checked_sub(parameters.len()) else {
        return false;
    };
    let result_shape = call_plan.result.as_ref().map(|placement| placement.shape);
    if result_shape.is_some_and(|shape| {
        ![1, 2, 4, 8].contains(&shape.byte_size)
            || shape != calling_conventions::ValueShape::integer(shape.byte_size, shape.byte_size)
    }) {
        return false;
    }
    let mut shapes = call_plan.parameters[..scalar_count]
        .iter()
        .map(|placement| placement.shape)
        .collect::<Vec<_>>();
    if shapes.iter().any(|shape| {
        !([1, 2, 4, 8].contains(&shape.byte_size)
            && *shape == calling_conventions::ValueShape::integer(shape.byte_size, shape.byte_size)
            || [
                calling_conventions::ValueShape::float(4),
                calling_conventions::ValueShape::float(8),
            ]
            .contains(shape))
    }) {
        return false;
    }
    for (position, parameter) in parameters.iter().enumerate() {
        let semantic = parameter.semantic;
        let Some(declaration) = structural_types
            .iter()
            .find(|declaration| declaration.id == semantic.structural_type)
        else {
            return false;
        };
        let primitive = matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_));
        // Shared record observations use the same exact incoming pointer ABI.
        // The operation reader separately rejoins each readable field and result.
        let shared_record = semantic.access == StructuralAccess::SharedBorrow
            && matches!(declaration.shape, StructuralTypeShape::Record { .. });
        if (parameters.len() > 1 && !primitive && !shared_record)
            || result_shape.is_some()
                && !shared_record
                && declaration.shape != StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)
                && !matches!(declaration.shape,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                    if integer.carrier() == IntegerCarrier::Fixed
                        && [8, 16, 32, 64].contains(&integer.bits()))
        {
            return false;
        }
        let Some(referent) =
            crate::structural_reference_input::shape(semantic.structural_type, structural_types)
        else {
            return false;
        };
        let shape = calling_conventions::ValueShape::borrowed_reference(
            referent.byte_size,
            referent.alignment,
        );
        if semantic.position as usize != position
            || parameters[..position]
                .iter()
                .any(|previous| previous.semantic.place == semantic.place)
            || !(matches!(
                semantic.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            ) || semantic.access == StructuralAccess::SharedBorrow
                && (primitive || shared_record))
            || semantic.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !semantic.qualifications.is_empty()
            || !semantic.projected_qualifications.is_empty()
            || parameter.target.place != semantic.place
            || parameter.target.structural_type != semantic.structural_type
            || parameter.target.access != semantic.access
            || parameter.target.multiplicity != semantic.multiplicity
            || !parameter.target.projected_qualifications.is_empty()
            || parameter.target.shape != shape
            || parameter.target.placement != call_plan.parameters[scalar_count + position]
        {
            return false;
        }
        shapes.push(shape);
    }
    calling_conventions::evaluate_call_plan(
        call_plan.policy,
        &calling_conventions::CallSignature {
            parameters: shapes,
            result: result_shape,
        },
    )
    .is_ok_and(|expected| expected == *call_plan)
}

/// Shared scalar-record receivers use the ordinary pointer ABI and scalar result.
pub(crate) fn accepts_shared_record(
    call_plan: &CallPlan,
    parameters: &[Parameter<'_>],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    !parameters.is_empty()
        && parameters.iter().all(|parameter| {
            parameter.semantic.access == StructuralAccess::SharedBorrow
                && parameter.semantic.multiplicity
                    == terminal_psi::StructuralMultiplicity::Unrestricted
                && crate::structural_reference_input::plain_record_shape(
                    parameter.semantic.structural_type,
                    structural_types,
                )
                .is_some()
        })
        && accepts_graph(call_plan, parameters, structural_types)
}

/// A byte descriptor is borrowed through one native pointer, independently of
/// the function's result carrier. Legalization and return selection validate
/// that carrier separately; borrowing does not imply a Unit result.
pub(crate) fn accepts_borrowed_view(
    call_plan: &CallPlan,
    parameters: &[Parameter<'_>],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    if parameters.is_empty() {
        return false;
    }
    let Some(scalar_count) = call_plan.parameters.len().checked_sub(parameters.len()) else {
        return false;
    };
    let mut shapes = call_plan.parameters[..scalar_count]
        .iter()
        .map(|placement| placement.shape)
        .collect::<Vec<_>>();
    if shapes.iter().any(|shape| {
        ![
            calling_conventions::ValueShape::integer(8, 8),
            calling_conventions::ValueShape::integer(1, 1),
        ]
        .contains(shape)
    }) {
        return false;
    }
    shapes.extend(
        parameters
            .iter()
            .map(|_| calling_conventions::ValueShape::borrowed_reference(16, 8)),
    );
    let expected = calling_conventions::evaluate_call_plan(
        call_plan.policy,
        &calling_conventions::CallSignature {
            parameters: shapes,
            result: call_plan.result.as_ref().map(|result| result.shape),
        },
    );
    expected
        .as_ref()
        .is_ok_and(|expected| expected == call_plan)
        && parameters.iter().enumerate().all(|(position, parameter)| {
            parameter.semantic.position as usize == position
                && !parameter.semantic.is_self
                && matches!(
                    parameter.semantic.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                )
                && parameter.semantic.multiplicity
                    == terminal_psi::StructuralMultiplicity::Unrestricted
                && parameter.semantic.qualifications.is_empty()
                && parameter.semantic.projected_qualifications.is_empty()
                && parameter.target.place == parameter.semantic.place
                && parameter.target.structural_type == parameter.semantic.structural_type
                && parameter.target.access == parameter.semantic.access
                && parameter.target.projected_qualifications.is_empty()
                && parameter.target.multiplicity == parameter.semantic.multiplicity
                && parameter.target.shape
                    == calling_conventions::ValueShape::borrowed_reference(16, 8)
                && parameter.target.placement == call_plan.parameters[scalar_count + position]
                && structural_types.iter().any(|declaration| {
                    declaration.id == parameter.semantic.structural_type
                        && declaration.shape
                            == StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView,
                            )
                })
        })
}

/// An indirect owned input uses the ABI's caller-prepared value copy. Keeping
/// that address does not borrow the caller's original value or create another
/// source owner. Callers must first reconstruct the complete plan with
/// `accepts_graph`; the copy offset is an outbound obligation, not an incoming
/// frame address. Only the pointer's register/stack location is read at entry.
pub(crate) fn owned_indirect_pointer(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    placement: &ValuePlacement,
) -> Option<IndirectPointerLocation> {
    if parameter.access != StructuralAccess::Owned
        || placement.shape.class != ValueClass::Integer
        || placement.shape.byte_size == 0
    {
        return None;
    }
    let [
        ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset: Some(_),
            byte_size,
            alignment,
        },
    ] = placement.locations.as_slice()
    else {
        return None;
    };
    (*byte_size == placement.shape.byte_size && *alignment == placement.shape.alignment)
        .then_some(*pointer)
}
