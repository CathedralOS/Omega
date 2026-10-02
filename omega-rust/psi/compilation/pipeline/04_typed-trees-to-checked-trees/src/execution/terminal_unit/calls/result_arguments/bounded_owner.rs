//! A whole bounded byte owner local lends its own storage to a call.
//!
//! `let text: [u8; 8] in Utf8 = "150";` binds a bounded carrier: an owned
//! capacity with a live length (wiki/spec/language/domains.md, "Byte
//! containers and encoding domains"). `parse(&text)` against a formal
//! `text: &[u8; 8] in Utf8` loans that owner exactly as a caller parameter of
//! the same formal would: the callee sees the declared capacity, reads the
//! live length the construction set, and cannot resize or replace it. The
//! loan retains the local, so no custody moves and no claim transfers.
//!
//! A shared byte-view formal may also observe the owner's live extent, using
//! the ordinary whole-local argument or an explicit shared borrow. Other
//! accesses require the same bounded carrier and explicit borrow spelling.
//! Every formal domain must already be declared on the local, and each route
//! retains the exact captured access. Terminal checks the current predicates
//! and presentation independently; lending never changes the owner's invariants.
use super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralResultBindingPlan,
    ExpressionNode, Multiplicity, StateParameter, StatementNode, SymbolHandle, TypedTrees,
    exact_structural_argument_access, names_whole_local,
};
use crate::execution::terminal_unit::types::{
    byte_sequence_carrier, byte_sequence_type_identity, structural_access_for_type_reference,
};
use checked_trees::CheckedByteSequenceCarrier;

#[allow(clippy::too_many_arguments)]
pub(super) fn loan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    source_state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    place: &crate::flow::CanonicalPlace,
    result: &CheckedUnitStructuralResultBindingPlan,
    parameter: &StateParameter,
    target_identity: &str,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    if !place.segments.is_empty() || parameter.is_self {
        return None;
    }
    let StatementNode::LocalData(local) = program
        .statement_table
        .statements(source_state.statement_nodes)
        .get(usize::try_from(result.statement_index).ok()?)?
    else {
        return None;
    };
    let CheckedByteSequenceCarrier::BoundedOwned { capacity } =
        byte_sequence_carrier(program, local.type_reference, &[])?
    else {
        return None;
    };
    let access = structural_access_for_type_reference(program, parameter.type_reference)?;
    let formal_carrier = byte_sequence_carrier(program, parameter.type_reference, &[])?;
    let shared_view = access == CheckedStructuralAccess::SharedBorrow
        && matches!(
            formal_carrier,
            CheckedByteSequenceCarrier::BorrowedView { .. }
        )
        && structural_access_for_type_reference(program, local.type_reference)
            == Some(CheckedStructuralAccess::Owned)
        && super::super::argument_presentation::is_admitted(
            program,
            local.type_reference,
            parameter.type_reference,
            target_identity,
            &[],
        );
    // Exclusive access cannot erase a qualified owner's invariants by turning
    // it into a bare mutable byte view. Its existing same-carrier loan remains.
    if !shared_view && formal_carrier != (CheckedByteSequenceCarrier::BoundedOwned { capacity }) {
        return None;
    }
    let expected_borrow = match access {
        CheckedStructuralAccess::SharedBorrow => language_semantics::ReferenceAccess::Shared,
        CheckedStructuralAccess::MutableBorrow => language_semantics::ReferenceAccess::Mutable,
        CheckedStructuralAccess::WriteOnlyBorrow => language_semantics::ReferenceAccess::WriteOnly,
        CheckedStructuralAccess::Owned => return None,
    };
    let value_expression = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) if borrow.access == expected_borrow => borrow.target,
        ExpressionNode::Name(_) if shared_view => expression,
        _ => return None,
    };
    if !program
        .expression_table
        .expression_is_valid(value_expression)
        || local.symbol != symbol
        || !names_whole_local(program, call, value_expression, symbol)
        || result.multiplicity != program.type_multiplicity(local.type_reference)
        || !matches!(
            result.multiplicity,
            Multiplicity::Unrestricted | Multiplicity::Affine
        )
        || usize::try_from(result.statement_index).ok()? >= call.statement_index
        || (!shared_view && result.type_identity != target_identity)
        || byte_sequence_type_identity(program, parameter.type_reference, &[], &[])?
            != target_identity
        || byte_sequence_type_identity(program, local.type_reference, &[], &[])?
            != result.type_identity
    {
        return None;
    }
    let declared =
        crate::facts::field_domain::domain_constraint_symbols(program, local.type_reference);
    if !crate::facts::field_domain::domain_constraint_symbols(program, parameter.type_reference)
        .iter()
        .all(|domain| declared.contains(domain))
    {
        return None;
    }
    if exact_structural_argument_access(
        program,
        facts,
        machine,
        source_state.symbol,
        call,
        place,
        access,
    )? != access
    {
        return None;
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path: Vec::new(),
        type_identity: target_identity.to_owned(),
        access,
    })
}

#[cfg(test)]
mod tests;
