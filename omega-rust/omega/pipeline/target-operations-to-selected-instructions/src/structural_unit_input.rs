//! Input-only recognition of the existing owned indirect-pair ABI.

use calling_conventions::{
    CallPlan, CallingPolicy, EntryControl, IndirectPointerLocation, MachineRegister, ValueClass,
    ValueLocation,
};
use semantic_vocabulary::{IntegerCarrier, IntegerSign, ScalarType};
use terminal_psi::{BindingRelevance, StructuralAccess, StructuralFieldType, StructuralTypeShape};

#[derive(Clone, Copy)]
pub(crate) struct Parameter<'a> {
    pub semantic: &'a terminal_psi::StructuralParameterDeclaration,
    pub target: &'a target_operations::TargetStructuralParameter,
}

pub(crate) fn accepts_write_borrow(
    call_plan: &CallPlan,
    parameters: &[Parameter<'_>],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    let [parameter] = parameters else {
        return false;
    };
    let Some(scalar_count) = call_plan.parameters.len().checked_sub(1) else {
        return false;
    };
    let semantic = parameter.semantic;
    let Some(referent) =
        crate::structural_reference_input::shape(semantic.structural_type, structural_types)
    else {
        return false;
    };
    let shape =
        calling_conventions::ValueShape::borrowed_reference(referent.byte_size, referent.alignment);
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
    shapes.push(shape);
    calling_conventions::evaluate_call_plan(
        call_plan.policy,
        &calling_conventions::CallSignature {
            parameters: shapes,
            result: None,
        },
    )
    .is_ok_and(|expected| expected == *call_plan)
        && semantic.position == 0
        && matches!(
            semantic.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        && semantic.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && semantic.qualifications.is_empty()
        && semantic.projected_qualifications.is_empty()
        && parameter.target.place == semantic.place
        && parameter.target.structural_type == semantic.structural_type
        && parameter.target.access == semantic.access
        && parameter.target.multiplicity == semantic.multiplicity
        && parameter.target.projected_qualifications.is_empty()
        && parameter.target.shape == shape
        && parameter.target.placement == call_plan.parameters[scalar_count]
}

/// An immutable byte descriptor is borrowed through one native pointer.
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
    }) || call_plan
        .result
        .as_ref()
        .is_some_and(|result| result.shape != calling_conventions::ValueShape::integer(8, 8))
    {
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
                && parameter.semantic.access == StructuralAccess::SharedBorrow
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

pub(crate) fn accepts(
    call_plan: &CallPlan,
    parameters: &[Parameter<'_>],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    if call_plan.policy != CallingPolicy::MicrosoftX64
        || call_plan.result.is_some()
        || !call_plan.callback_materializations.is_empty()
        || call_plan.stack_alignment != 16
        || call_plan.shadow_bytes != 32
        || call_plan.entry_control != EntryControl::CallReturn
        || parameters.len() != 2
        || call_plan.parameters.len() != 2
    {
        return false;
    }
    for (index, parameter) in parameters.iter().enumerate() {
        if parameter.semantic.position != index as u32
            || parameter.semantic.is_self
            || parameter.semantic.access != StructuralAccess::Owned
            || parameter.target.place != parameter.semantic.place
            || parameter.target.structural_type != parameter.semantic.structural_type
            || parameter.target.multiplicity != parameter.semantic.multiplicity
            || parameter.target.access != StructuralAccess::Owned
            || parameter.target.shape.class != ValueClass::Integer
            || parameter.target.shape.byte_size != 16
            || parameter.target.shape.alignment != 8
            || parameter.target.placement != call_plan.parameters[index]
            || parameter.target.placement.locations.len() != 1
        {
            return false;
        }
        let ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(pointer),
            copy_stack_byte_offset: Some(copy_stack_byte_offset),
            byte_size,
            alignment,
        } = parameter.target.placement.locations[0]
        else {
            return false;
        };
        if pointer != [MachineRegister::X86Rcx, MachineRegister::X86Rdx][index]
            || copy_stack_byte_offset != [32, 48][index]
            || byte_size != 16
            || alignment != 8
        {
            return false;
        }
    }
    if parameters[0].semantic.structural_type != parameters[1].semantic.structural_type
        || parameters[0].semantic.multiplicity != parameters[1].semantic.multiplicity
        || parameters[0].semantic.qualifications != parameters[1].semantic.qualifications
        || parameters[0].semantic.place == parameters[1].semantic.place
    {
        return false;
    }
    let Some(declaration) = structural_types
        .iter()
        .find(|declaration| declaration.id == parameters[0].semantic.structural_type)
    else {
        return false;
    };
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return false;
    };
    if fields.len() != 2
        || fields
            .iter()
            .any(|field| field.relevance != BindingRelevance::Relevant)
    {
        return false;
    }
    matches!(fields[0].field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer))
        if integer.carrier() == IntegerCarrier::Address && integer.sign() == IntegerSign::Unsigned
            && integer.bits() == 64)
        && matches!(fields[1].field_type, StructuralFieldType::Scalar(ScalarType::Integer(integer))
            if integer.carrier() == IntegerCarrier::Fixed && integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 64)
}
