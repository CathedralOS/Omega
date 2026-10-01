//! Which boundary scalar targets, result values, argument presentations and
//! mutable byte-array views a structural call may admit.

use crate::execution::terminal_unit::calls::call_operations::ExpectedCallValueResult;
use crate::execution::terminal_unit::types::{
    base_type_identity, is_unit, type_graph_requires_nominal_drop,
};
use crate::execution::terminal_unit::types::{
    base_type_identity_with_substitutions, byte_sequence_carrier, substituted_formal_type,
};
use crate::execution::terminal_unit::{
    DataMember, Multiplicity, PrimitiveType, StateParameter, SymbolHandle, TypeReferenceNode,
    TypedTrees, is_reference,
};

#[cfg(test)]
mod tests;

/// A Fused binding projected from an ordinary parameter retains its exact
/// requirement and compiler-issued erasure authorization. This proves the call
/// receiver; attachment-field requirements remain a separate roster obligation.
pub(super) fn parameter_binding_receiver_matches(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    call_site: &crate::semantic::calls::CallSite<'_>,
    requirement: SymbolHandle,
) -> bool {
    let Some((source, _)) =
        parameter_binding_receiver_source(program, state, statement_index, call_site, requirement)
    else {
        return false;
    };
    parameters.iter().any(|parameter| {
        !parameter.is_self
            && program
                .state_parameters(state)
                .get(parameter.position as usize)
                .is_some_and(|candidate| candidate.symbol == source.symbol)
    })
}

/// Resolve the ordinary source parameter whose projected Binding has an exact
/// Fused authorization. This identifies a receiver, not a call admission: the
/// call planner separately requires its structural parameter, and attachment
/// collection uses this only for an already-planned matching boundary call.
pub(crate) fn parameter_binding_receiver_source<'program>(
    program: &'program TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    call_site: &crate::semantic::calls::CallSite<'_>,
    requirement: SymbolHandle,
) -> Option<(&'program StateParameter, crate::flow::CanonicalPlace)> {
    let place = match call_site {
        crate::semantic::calls::CallSite::Expression { call, .. } => {
            crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                call.receiver,
            )
        }
        crate::semantic::calls::CallSite::Statement(call) => {
            crate::lookup::projected_statement_receiver_place(program, state, statement_index, call)
                .map(|(root, segments)| crate::flow::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(root),
                    segments,
                })
        }
        crate::semantic::calls::CallSite::TransitionNamed { .. } => None,
    };
    let place = place?;
    let facts::PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    if place.segments.is_empty() {
        return None;
    }
    let source = program.state_parameters(state).iter().find(|parameter| {
        parameter.symbol == root && !parameter.is_self && !parameter.relevance.is_erased()
    })?;
    let (reference, _) = super::argument_paths::projected_argument_path(
        program,
        state.symbol,
        statement_index,
        &place,
    )?;
    // A `Binding<R>` member projected out of a parameter's record storage is
    // the migrated service spelling (`filesystem.host.open_create`): the
    // binding's requirement is exact at the member, and the record's own
    // attachment carries the provider provenance the Fused receipt carried
    // for a direct `Binding` parameter. Direct parameter bindings still need
    // the erasure authorization -- `segments.is_empty()` already excludes
    // them above.
    ((typed_trees::service::exact_bound_service_requirement(program, reference)
        == Some(requirement)
        && program.fused_service_erasure(requirement).is_some())
        || field_type_carries_requirement(program, reference, requirement)
        || service_record_member_carries_requirement(program, reference, requirement))
    .then_some((source, place))
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
    let formal = substituted_formal_type(program, parameter_type, substitutions);
    match (
        byte_sequence_carrier(program, projected_type, &[]),
        byte_sequence_carrier(program, formal, substitutions),
    ) {
        (
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }),
            Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }),
        ) => true,
        // A stored shared view field presents its own descriptor at a shared
        // view formal: the callee reborrows the stored view's backing for the
        // call, not the record. An exclusive view on either side would
        // duplicate or widen a loan, so both must be shared.
        (
            Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }),
            Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }),
        ) => {
            shared_view_reference(program, projected_type)
                && crate::execution::terminal_unit::types::structural_access_for_type_reference(
                    program, formal,
                ) == Some(checked_trees::CheckedStructuralAccess::SharedBorrow)
        }
        _ => false,
    }
}

/// Whether a declared field type is a shared `&[u8]` view, through any
/// domain constraint shells.
fn shared_view_reference(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { access, .. } => {
                return *access == language_core::ReferenceAccess::Shared;
            }
            _ => return false,
        }
    }
}

/// A fixed array lends its initialized extent as the borrowed element view a
/// checked-body callee declares. This is the element-typed sibling of the byte
/// presentation above: the source keeps its literal backing, the target keeps
/// its declared `&[T]`/`&mut [T]` view, and the projected path names the
/// storage rather than materializing a view local.
pub(crate) fn fixed_array_slice_view_is_admitted(
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
        access: language_core::ReferenceAccess::Mutable | language_core::ReferenceAccess::Shared,
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
    program.normalized_type_identity(*element_type).as_str()
        == program.normalized_type_identity(*target_element).as_str()
        && crate::checks::type_multiplicity(program, source) == Multiplicity::Unrestricted
}

/// A fixed byte array lends initialized elements without becoming a bounded owner.
pub(crate) fn fixed_byte_array_view_is_admitted(
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
        access: language_core::ReferenceAccess::Mutable | language_core::ReferenceAccess::Shared,
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

/// The `self.<field>` receiver a boundary call site routes through, when the
/// call is carried by an attached provider field at all: the field's authored
/// name plus the resolved member symbol an expression-site receiver carries.
/// A direct `Boundary::method` call and a non-`self` receiver have neither.
fn provider_attachment_receiver(
    program: &TypedTrees,
    call_site: &crate::semantic::calls::CallSite<'_>,
) -> Option<(typed_trees::name::Identifier, Option<SymbolHandle>)> {
    match call_site {
        crate::semantic::calls::CallSite::Statement(call) => {
            let [self_name, field_name] = program.statement_table.name_path_members(call.receiver)
            else {
                return None;
            };
            if !self_name.is_self_receiver() {
                return None;
            }
            Some((field_name.clone(), None))
        }
        crate::semantic::calls::CallSite::Expression { call, .. } => {
            let (_, Some(receiver)) = crate::lookup::call_receiver_parts(program, call.receiver)
            else {
                return None;
            };
            let [self_name, field_name] = receiver.members() else {
                return None;
            };
            if !self_name.is_self_receiver() {
                return None;
            }
            Some((field_name.clone(), Some(receiver.member_symbol(1))))
        }
        crate::semantic::calls::CallSite::TransitionNamed { .. } => None,
    }
}

/// The receiver field's authored name alone, so a caller can tell whether a
/// call is routed through `self.<field>` at all.
pub(crate) fn provider_attachment_receiver_field(
    program: &TypedTrees,
    call_site: &crate::semantic::calls::CallSite<'_>,
) -> Option<typed_trees::name::Identifier> {
    provider_attachment_receiver(program, call_site).map(|(name, _)| name)
}

pub(crate) fn provider_attachment_receiver_matches(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    call_site: &crate::semantic::calls::CallSite<'_>,
    provider_symbol: SymbolHandle,
) -> bool {
    let Some((field_name, selected_field)) = provider_attachment_receiver(program, call_site)
    else {
        return false;
    };
    if !machine.attached_data_symbol.is_valid() {
        return false;
    }
    // The symbol, not the bare name: leaf-keyed attachments collide when two
    // modules declare same-named records (`left::Runner`/`right::Runner`), and
    // a name match would route `self.reader` to the sibling leaf's carrier.
    let Some(attached) = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == machine.attached_data_symbol)
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
        field_type_carries_requirement(program, field.type_reference, provider_symbol)
            || service_record_member_carries_requirement(
                program,
                field.type_reference,
                provider_symbol,
            )
    })
}

/// Whether `type_reference` presents `provider_symbol` as a direct boundary
/// carrier: an exact `Binding<requirement>` slot, or the provider field spelled
/// `&'a mut <boundary trait>` (unwrapping qualifications and the mutable
/// borrow to the trait symbol; a shared `&` borrow is not a provider handle).
fn field_type_carries_requirement(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    provider_symbol: SymbolHandle,
) -> bool {
    typed_trees::service::exact_bound_service_requirement(program, type_reference)
        == Some(provider_symbol)
        || {
            let mut unwrapped = type_reference;
            loop {
                match program.type_reference_table.type_reference(unwrapped) {
                    TypeReferenceNode::Constrained {
                        base_type: inner, ..
                    } => unwrapped = *inner,
                    TypeReferenceNode::Reference {
                        referee: inner,
                        access: language_core::ReferenceAccess::Mutable,
                        ..
                    } => unwrapped = *inner,
                    TypeReferenceNode::Named { symbol, .. }
                    | TypeReferenceNode::DynamicTrait { symbol, .. } => {
                        break *symbol == provider_symbol;
                    }
                    _ => break false,
                }
            }
        }
}

/// The ergonomic service-record spelling: `self.fs` is a `data` record that
/// holds the raw boundary binding as a member (`host: Binding<FilesystemHost>`)
/// rather than being the binding itself. The `self.<field>` receiver reaches
/// the requirement when the field's record type carries it exactly one level
/// down; deeper nesting stays refused so a named member cannot launder an
/// indirect boundary path past the attachment rule.
fn service_record_member_carries_requirement(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    provider_symbol: SymbolHandle,
) -> bool {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *symbol)
    else {
        return false;
    };
    program.data_members(definition).iter().any(|member| {
        let DataMember::Field(field) = member else {
            return false;
        };
        !field.relevance.is_erased()
            && field_type_carries_requirement(program, field.type_reference, provider_symbol)
    })
}
