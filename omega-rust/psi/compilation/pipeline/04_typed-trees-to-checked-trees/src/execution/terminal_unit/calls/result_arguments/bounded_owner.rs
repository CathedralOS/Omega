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
//! The lane requires the same carrier on both sides (bounded owner, equal
//! capacity), every domain the formal names already declared on the local,
//! the authored `&`/`&mut` spelling of exactly the whole local, and the
//! exact captured borrow access. Terminal re-derives the predicates from the
//! local's establishment and checks the presentation independently.
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
    // The formal is a loan of the same owner: `&[u8; N] in D` names the
    // owner's carrier, with the reference supplying only the access.
    if byte_sequence_carrier(program, parameter.type_reference, &[])
        != Some(CheckedByteSequenceCarrier::BoundedOwned { capacity })
    {
        return None;
    }
    let access = structural_access_for_type_reference(program, parameter.type_reference)?;
    let expected_borrow = match access {
        CheckedStructuralAccess::SharedBorrow => language_semantics::ReferenceAccess::Shared,
        CheckedStructuralAccess::MutableBorrow => language_semantics::ReferenceAccess::Mutable,
        CheckedStructuralAccess::WriteOnlyBorrow => language_semantics::ReferenceAccess::WriteOnly,
        CheckedStructuralAccess::Owned => return None,
    };
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
        return None;
    };
    if borrow.access != expected_borrow
        || !program.expression_table.expression_is_valid(borrow.target)
        || local.symbol != symbol
        || !names_whole_local(program, call, borrow.target, symbol)
        || result.multiplicity != program.type_multiplicity(local.type_reference)
        || !matches!(
            result.multiplicity,
            Multiplicity::Unrestricted | Multiplicity::Affine
        )
        || usize::try_from(result.statement_index).ok()? >= call.statement_index
        || result.type_identity != target_identity
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
