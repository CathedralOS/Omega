//! Non-observing projected captures; later uses retain ordinary borrow checks.
use super::{
    ExpressionNode, Machine, ReferenceAccess, State, TypeReferenceHandle, TypeReferenceNode,
    WriteOnlyRoot, receiver,
};
use typed_trees::statement::TableLocalData;

/// Resolve a formation once, including selector evaluation and exact referee
/// admission. An invalid observation retains its diagnostic even if the place
/// otherwise resolves, so it cannot fall through as an unchecked local borrow.
pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    local: &TableLocalData,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> bool {
    let Some((actual, referee)) = capture(program, machine, state, local, roots, diagnostics)
    else {
        return false;
    };
    if crate::value_custody::type_references::type_references_match(program, actual, referee) {
        return true;
    }
    diagnostics.push(diagnostics::Diagnostic::error(format!(
        "machine `{}` state `{}` captures `{}` of type `{}` as `&write {}`; a write-only formation must preserve the declared constraint atoms exactly, so a weaker, stronger, or different referee remains rejected",
        machine.name,
        state.name,
        program.expression_table.display_name(
            match program.expression_table.expression(local.initial_value) {
                ExpressionNode::Borrow(borrow) => borrow.target,
                _ => local.initial_value,
            }
        ),
        program.display_type_reference_with_constraints(actual),
        program.display_type_reference_with_constraints(referee),
    )));
    true
}

/// The shared formation shape: an immutable `&write` local initialized from a
/// `&write` borrow of a place with builtin coordinates. Returns the captured
/// place's exact type and the local's declared referee once that shape holds.
fn capture(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    local: &TableLocalData,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) -> Option<(TypeReferenceHandle, TypeReferenceHandle)> {
    if local.is_mutable
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
    {
        return None;
    }
    let TypeReferenceNode::Reference {
        referee,
        access: ReferenceAccess::WriteOnly,
        ..
    } = program
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return None;
    };
    let Some(borrow) = program
        .expression_table
        .borrow_expression(local.initial_value)
    else {
        return None;
    };
    if borrow.access != ReferenceAccess::WriteOnly
        || !crate::place_has_builtin_coordinates(program, machine, Some(state), borrow.target)
    {
        return None;
    }
    // Mutable parameters and earlier `let mut` bindings may attenuate at
    // formation, and earlier immutable `&mut` carriers preserve mutable
    // authority until attenuation. None is a write-only root for ordinary
    // expression validation, where reading remains legal — the same sources
    // the checked-call subloan gate uses.
    let mut sources = roots.to_vec();
    sources.extend(super::mutable_formation_sources(
        program,
        state,
        Some(local.symbol),
    ));
    let (_, actual, attached_root) = receiver::nonobserving_projection(
        program,
        machine,
        state,
        borrow.target,
        &sources,
        roots,
        diagnostics,
    )?;
    (!attached_root && super::is_admitted_checked_referee(program, actual))
        .then_some((actual, *referee))
}
