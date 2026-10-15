//! A whole bounded byte owner local lends its own storage to a call.
//!
//! An explicit variable-fill carrier retains its storage and live length.
//! Lending it leaves the owner in place, so no custody moves and no claim
//! transfers. Exact fixed arrays use ordinary fixed-array storage and loans;
//! qualification never chooses a capacity or hidden live-length field.
//!
//! A shared byte-view formal may also observe the owner's live extent, using
//! the ordinary whole-local argument or an explicit shared borrow. Other
//! accesses require the same bounded carrier and explicit borrow spelling.
//! Every formal domain must already be declared on the local, and each route
//! retains the exact captured access. Terminal checks the current predicates
//! and presentation independently; lending never changes the owner's invariants.
use super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralResultBindingPlan, ExpressionNode, Multiplicity, StateParameter,
    SymbolHandle, exact_structural_argument_access, names_whole_local,
};
use crate::execution::terminal_unit::types::{
    byte_sequence_carrier, byte_sequence_type_identity, structural_access_for_type_reference,
};
use checked_trees::CheckedByteSequenceCarrier;

#[allow(clippy::too_many_arguments)]
pub(super) fn loan(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    source_state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    place: &validation::CanonicalPlace,
    result: &CheckedUnitStructuralResultBindingPlan,
    parameter: &StateParameter,
    target_identity: typed_trees::type_identity::TypeIdentityId,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let symbol = place.root.as_symbol()?;
    if !place.segments.is_empty() {
        return None;
    }
    let local = program
        .statement_table
        .statements(source_state.statement_nodes)
        .get(usize::try_from(result.statement_index).ok()?)?
        .as_local_data()?;
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
        && super::super::argument_presentation::classify(
            program,
            local.type_reference,
            parameter.type_reference,
            target_identity,
            &[],
        )
        .is_some();
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
        validation::field_domains::domain_constraint_symbols(program, local.type_reference);
    if !validation::field_domains::domain_constraint_symbols(program, parameter.type_reference)
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
    Some(CheckedUnitStructuralArgumentPlan::structural_result(
        result.binding_ordinal,
        target_identity.to_owned(),
        access,
    ))
}
