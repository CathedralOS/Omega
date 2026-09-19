//! Which boundary scalar targets, result values, argument presentations and
//! mutable byte-array views a structural call may admit.

use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::calls::call_operations::ExpectedCallValueResult;
use crate::execution::terminal_unit::types::{
    base_type_identity_with_substitutions, byte_sequence_carrier, substituted_formal_type,
};
use crate::execution::terminal_unit::{
    DataMember, Multiplicity, PrimitiveType, SymbolHandle, TypeReferenceNode, TypedTrees,
    base_type_identity, is_reference, is_unit, type_graph_requires_nominal_drop,
};

pub(crate) fn is_registered_boundary_scalar_target(
    scalar_callees: Option<ScalarCalleePlans<'_>>,
    machine: SymbolHandle,
    state: SymbolHandle,
    result: PrimitiveType,
) -> bool {
    let Some(scalar_callees) = scalar_callees else {
        return false;
    };
    let mut targets = scalar_callees
        .boundary_returns
        .machines
        .iter()
        .filter(|plan| plan.machine == machine);
    targets
        .next()
        .is_some_and(|plan| plan.state == state && plan.result_type == result)
        && targets.next().is_none()
}

/// `substitutions` carries the call edge's admitted specialization: a
/// generic requirement's `Type` result formal resolves to its derived actual
/// before shape evidence is replayed, and a compound result such as
/// `Task<T>` substitutes inside the identity comparison itself.
pub(crate) fn boundary_value_result_matches(
    program: &TypedTrees,
    return_type: typed_trees::types::TypeReferenceHandle,
    expected: &ExpectedCallValueResult<'_>,
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
) -> bool {
    let return_type = substituted_formal_type(program, return_type, substitutions);
    match expected {
        ExpectedCallValueResult::Scalar(expected) => {
            program.primitive_type_reference(return_type) == Some(*expected)
        }
        ExpectedCallValueResult::Structural(expected) => {
            program.primitive_type_reference(return_type).is_none()
                && !is_unit(program, return_type)
                && !is_reference(program, return_type)
                && !type_graph_requires_nominal_drop(program, return_type)
                && crate::checks::type_multiplicity(program, return_type) == expected.multiplicity
                && base_type_identity_with_substitutions(program, return_type, &[], substitutions)
                    .is_some_and(|identity| identity == expected.type_identity)
        }
    }
}

/// Whether a projected caller place may be presented at this boundary
/// requirement parameter. The ordinary rule is the transitive lane's: the
/// projected type carries the parameter's exact normalized identity.
///
/// One carrier presentation additionally crosses. A concrete owned
/// `[u8; N] in D` destination is admitted at a requirement that declares the
/// borrowed `[u8]` view, which is the settled `Console::read_line` surface: the
/// requirement keeps its checked mutable-slice type while the lowering derives
/// the existing writable extent from this exact call-site place. That is why the
/// argument retains the projected path instead of a materialized view. Equal
/// element types establish nothing on their own -- both sides must classify as
/// byte-sequence carriers, and no other pair of carriers agrees.
pub(crate) fn boundary_argument_presentation_is_admitted(
    program: &TypedTrees,
    projected_type: typed_trees::types::TypeReferenceHandle,
    parameter_type: typed_trees::types::TypeReferenceHandle,
    target_identity: &str,
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
) -> bool {
    if base_type_identity(program, projected_type, &[])
        .is_some_and(|identity| identity == target_identity)
    {
        return true;
    }
    matches!(
        (
            byte_sequence_carrier(program, projected_type, &[]),
            byte_sequence_carrier(
                program,
                substituted_formal_type(program, parameter_type, substitutions),
                substitutions,
            ),
        ),
        (
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }),
            Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView),
        )
    )
}

/// A fixed byte array lends initialized elements without becoming a bounded owner.
pub(crate) fn fixed_byte_array_mutable_view_is_admitted(
    program: &TypedTrees,
    mut source: typed_trees::types::TypeReferenceHandle,
    target: typed_trees::types::TypeReferenceHandle,
) -> bool {
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(source)
    {
        source = *referee;
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(_),
    } = program.type_reference_table.type_reference(source)
    else {
        return false;
    };
    let TypeReferenceNode::Reference {
        access: language_core::ReferenceAccess::Mutable,
        referee,
        ..
    } = program.type_reference_table.type_reference(target)
    else {
        return false;
    };
    let TypeReferenceNode::Slice {
        element_type: target_element,
    } = program.type_reference_table.type_reference(*referee)
    else {
        return false;
    };
    [*element_type, *target_element].into_iter().all(|element| {
        matches!(
            program.type_reference_table.type_reference(element),
            TypeReferenceNode::Named { .. }
        ) && program.primitive_type_reference(element) == Some(PrimitiveType::U8)
    }) && crate::checks::type_multiplicity(program, source) == Multiplicity::Unrestricted
}

pub(crate) fn provider_attachment_receiver_matches(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    call_site: &crate::semantic_calls::CallSite<'_>,
    provider_symbol: SymbolHandle,
) -> bool {
    let (field_name, selected_field) = match call_site {
        crate::semantic_calls::CallSite::Statement(call) => {
            let [self_name, field_name] = program.statement_table.name_path_members(call.receiver)
            else {
                return false;
            };
            if self_name.as_str() != "self" {
                return false;
            }
            (field_name.clone(), None)
        }
        crate::semantic_calls::CallSite::Expression { call, .. } => {
            let (_, Some(receiver)) = crate::lookup::call_receiver_parts(program, call.receiver)
            else {
                return false;
            };
            let [self_name, field_name] = receiver.members() else {
                return false;
            };
            if self_name.as_str() != "self" {
                return false;
            }
            (field_name.clone(), Some(receiver.member_symbol(1)))
        }
        crate::semantic_calls::CallSite::TransitionNamed { .. } => return false,
    };
    let Some(attached_name) = machine.attached_data.as_ref() else {
        return false;
    };
    let Some(attached) = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *attached_name)
    else {
        return false;
    };
    program.data_members(attached).iter().any(|member| {
        let DataMember::Field(field) = member else {
            return false;
        };
        if field.name != field_name
            || field.relevance.is_erased()
            || selected_field.is_some_and(|symbol| symbol != field.symbol)
        {
            return false;
        }
        typed_trees::service::exact_bound_service_requirement(program, field.type_reference)
            == Some(provider_symbol)
            || matches!(
                program
                    .type_reference_table
                    .type_reference(field.type_reference),
                TypeReferenceNode::Named { symbol, .. }
                    | TypeReferenceNode::DynamicTrait { symbol, .. }
                    if *symbol == provider_symbol
            )
    })
}
