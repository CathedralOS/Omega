//! Which boundary scalar targets, argument presentations and
//! mutable byte-array views a structural call may admit.

use crate::checks::type_multiplicity;
use crate::execution::terminal_unit::{
    Multiplicity, PrimitiveType, SymbolHandle, TypeReferenceNode,
};

/// A boundary requirement call answers to its static namespace: the exact
/// declaring trait. Selecting a provider and establishing an entry context
/// belong to realization; no parameter, field or value supplies a provider,
/// and this query invents no implicit `self` authority.
pub(crate) fn boundary_trait_receiver_is_namespace(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    call_site: &validation::CallSite<'_>,
    requirement: SymbolHandle,
) -> bool {
    call.has_receiver && namespace_receiver_matches(program, state, call, call_site, requirement)
}

/// A static namespace is the exact declaring trait. Runtime selectors resolve
/// their applied ancestor environment through ordinary receiver custody.
fn namespace_receiver_matches(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    call_site: &validation::CallSite<'_>,
    requirement: SymbolHandle,
) -> bool {
    let Some(definition) = program.trait_definition_by_symbol(call.receiver_symbol) else {
        return false;
    };
    let scope = match call_site {
        validation::CallSite::Expression { call, .. }
            if call.requirement_application.is_valid() =>
        {
            let Some(machine) = program.machine_holding_state(state.symbol) else {
                return false;
            };
            let Some(scope) =
                program.applied_requirement_scope(machine.symbol, call.requirement_application)
            else {
                return false;
            };
            scope
        }
        _ => typed_trees::trait_definition::AppliedTraitScope::abstract_scope(definition),
    };
    definition.is_boundary
        && definition.symbol == requirement
        && scope.declaration.symbol == requirement
        && program
            .closed_requirement_application(call.target_symbol)
            .is_none_or(|application| {
                program.requirement_application_matches_scope(application.signature.symbol, &scope)
            })
}

/// Failure-only explanation for an unclaimed boundary occurrence. This uses
/// the same admission relation as planning; it never supplies missing custody.
pub(crate) fn boundary_authority_refusal(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
) -> Option<&'static str> {
    let owner = program.traits().iter().find(|owner| {
        owner.is_boundary
            && program
                .executable_trait_machine_instances(owner)
                .any(|signature| signature.symbol == call.target_symbol)
    })?;
    let site = validation::find_call_site(
        program,
        machine,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    (!namespace_receiver_matches(program, state, call, &site, owner.symbol))
        .then_some("boundary authority: receiver is not the requirement's declaring namespace")
}

/// A fixed array lends its initialized extent as the borrowed element view a
/// checked-body callee declares. This is the element-typed sibling of the byte
/// presentation above: the source keeps its literal backing, the target keeps
/// its declared `&[T]`/`&mut [T]` view, and the projected path names the
/// storage rather than materializing a view local.
/// Lending does not copy the array or its elements. Their multiplicity stays
/// with the backing owner; callers independently establish initialized storage,
/// qualifications and exact loan access before presenting this carrier.
pub(crate) fn fixed_array_slice_view_is_admitted(
    program: &dyn ::validation::CheckingInput,
    source: typed_trees::types::TypeReferenceHandle,
    target: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let Some((_, element_type, target_element)) =
        fixed_array_slice_view_parts(program, source, target)
    else {
        return false;
    };
    program.normalized_type_identity(*element_type)
        == program.normalized_type_identity(*target_element)
}

/// A fixed byte array lends initialized elements without becoming a bounded owner.
pub(crate) fn fixed_byte_array_view_is_admitted(
    program: &dyn ::validation::CheckingInput,
    source: typed_trees::types::TypeReferenceHandle,
    target: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let Some((source, element_type, target_element)) =
        fixed_array_slice_view_parts(program, source, target)
    else {
        return false;
    };
    [*element_type, *target_element].into_iter().all(|element| {
        matches!(
            program.type_reference_table.type_reference(element),
            TypeReferenceNode::Named { .. }
        ) && program.primitive_type_reference(element) == Some(PrimitiveType::U8)
    }) && type_multiplicity(program, source) == Multiplicity::Unrestricted
}

/// Peels one reference layer off `source` and destructures it as a literal
/// fixed array while `target` destructures as a shared-or-mutable reference to
/// a slice. Returns the peeled source and both element types when the source/
/// target shape is the fixed-array-to-slice view both admissions test.
fn fixed_array_slice_view_parts(
    program: &dyn ::validation::CheckingInput,
    mut source: typed_trees::types::TypeReferenceHandle,
    target: typed_trees::types::TypeReferenceHandle,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    &typed_trees::types::TypeReferenceHandle,
    &typed_trees::types::TypeReferenceHandle,
)> {
    source = validation::peel_declared_constraints(program, source);
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(source)
    {
        source = validation::peel_declared_constraints(program, *referee);
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(_),
    } = program.type_reference_table.type_reference(source)
    else {
        return None;
    };
    let TypeReferenceNode::Reference {
        access: language_core::ReferenceAccess::Mutable | language_core::ReferenceAccess::Shared,
        referee,
        ..
    } = program
        .type_reference_table
        .type_reference(validation::peel_declared_constraints(program, target))
    else {
        return None;
    };
    let TypeReferenceNode::Slice {
        element_type: target_element,
    } = program
        .type_reference_table
        .type_reference(validation::peel_declared_constraints(program, *referee))
    else {
        return None;
    };
    Some((source, element_type, target_element))
}
