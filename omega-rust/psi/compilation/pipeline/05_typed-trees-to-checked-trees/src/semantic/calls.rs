use checked_trees::expression::ExpressionHandle;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
pub(crate) mod dispatch_subjects;
mod parameter_domains;
pub(crate) use parameter_domains::call_parameter_domains;

/// Identify the value a shared formal lends, whether the source explicitly
/// lends shared or mutable access, or the shared loan was implicit. This only classifies the operand: callers still establish its
/// type, storage, loan validity and any temporary-owner cleanup. In particular,
/// a reference formal must never turn a value constructor into a reference
/// carrier, and this rule grants no implicit mutable or write-only lending.
/// A computed slice view lends a new descriptor, not an owned slice referent:
/// preserve its complete expression and type so preparation and consumption
/// name the same evaluated descriptor even when an explicit borrow encloses a
/// range. A view of existing storage peels like any shared borrow.
pub(crate) fn shared_argument_referent(
    program: &dyn ::validation::CheckingInput,
    argument: ExpressionHandle,
    formal: TypeReferenceHandle,
) -> Option<(ExpressionHandle, TypeReferenceHandle)> {
    let TypeReferenceNode::Reference {
        access: language_semantics::ReferenceAccess::Shared,
        referee,
        ..
    } = program.type_reference_table.type_reference(formal)
    else {
        return None;
    };
    if validation::slice_view_shell(program, formal, &[]).is_some()
        && !view_argument_names_existing_place(program, argument)
    {
        return Some((argument, formal));
    }
    let value = match program.expression_table.expression(argument) {
        typed_trees::expression::ExpressionNode::Borrow(borrow)
            if matches!(
                borrow.access,
                language_semantics::ReferenceAccess::Shared
                    | language_semantics::ReferenceAccess::Mutable
            ) =>
        {
            borrow.target
        }
        typed_trees::expression::ExpressionNode::Borrow(_) => return None,
        _ => argument,
    };
    Some((value, *referee))
}

/// Whether a view argument names existing storage, a stored view or a whole
/// collection lent as one, rather than computing a new descriptor such as a
/// range window or a call's view result. Existing storage lends through its
/// place's loan route; only a computed formation is an evaluated operand with
/// its own result. The place builder roots exactly the computed forms at their
/// expression.
pub(crate) fn view_argument_names_existing_place(
    program: &dyn ::validation::CheckingInput,
    argument: ExpressionHandle,
) -> bool {
    // A recast's canonical place names its backing for loan checking, not
    // an existing descriptor. Its unsized result must capture the region's
    // start and derived extent at this occurrence, just as a range does.
    if validation::slice_recast_source(program, argument).is_some() {
        return false;
    }
    validation::canonical_place_from_expression(program, argument)
        .is_some_and(|place| matches!(place.root, facts::PlaceRoot::Symbol(_)))
}
