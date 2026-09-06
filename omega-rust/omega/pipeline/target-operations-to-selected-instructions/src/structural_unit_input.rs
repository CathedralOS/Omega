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
