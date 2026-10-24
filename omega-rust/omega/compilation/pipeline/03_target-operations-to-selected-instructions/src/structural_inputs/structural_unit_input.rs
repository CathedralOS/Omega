//! Reconstruct structural parameter shape and incoming ABI storage.

use crate::structural_inputs::structural_reference_input::{parameter_shape, stored_value_shape};
use calling_conventions::{
    CallPlan, IndirectPointerLocation, ValueClass, ValueLocation, ValuePlacement,
};
use terminal_psi::StructuralAccess;

#[derive(Clone, Copy)]
pub(crate) struct Parameter<'a> {
    pub semantic: &'a terminal_psi::StructuralParameterDeclaration,
    pub target: &'a target_operations::TargetStructuralParameter,
}

/// The parameter views one structural contract offers its ABI storage check.
pub(crate) fn parameter_views(
    contract: &legalized_operations::LegalizedStructuralContract,
) -> Vec<Parameter<'_>> {
    contract
        .parameters
        .iter()
        .map(|parameter| Parameter {
            semantic: &parameter.semantic,
            target: &parameter.target,
        })
        .collect()
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
        // Owned fragments take the same stored-carrier geometry as every
        // other owned home; only the borrowed forms select a descriptor.
        let Some(shape) = (if semantic.access == StructuralAccess::Owned {
            stored_value_shape(semantic.structural_type, structural_types)
        } else {
            parameter_shape(semantic, structural_types)
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
