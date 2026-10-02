//! Account for authored statements independently of the operation roster.
//!
//! A place-loan declaration binds an existing place, rather than allocating a
//! runtime value. Its omission is justified by source geometry and exact loan
//! custody; absence of a planned operation is never evidence for erasure.

use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;
use checked_trees::{BorrowAccessKind, BorrowLoanLineage, CheckedTrees};
use language_core::ReferenceAccess;
use symbols::SymbolHandle;

pub(super) fn statement_owes_operation(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    index: usize,
    statement: &StatementNode,
) -> bool {
    !validation::is_fact_only_citation(&checked.typed, statement)
        && !place_loan_declaration(checked, machine, state, index, statement).unwrap_or(false)
}

fn place_loan_declaration(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    index: usize,
    statement: &StatementNode,
) -> Option<bool> {
    let StatementNode::LocalData(local) = statement else {
        return None;
    };
    if local.is_mutable
        || !checked
            .expression_table
            .expression_is_valid(local.initial_value)
    {
        return None;
    }
    let (authored_machine, authored_state) =
        crate::expression_preparation::source_custody::authored_state(checked, state).ok()?;
    if authored_machine.symbol != machine
        || !matches!(checked.statement_table.statements(authored_state.statement_nodes).get(index),
            Some(StatementNode::LocalData(candidate)) if candidate.symbol == local.symbol
                && candidate.initial_value == local.initial_value
                && candidate.type_reference == local.type_reference)
    {
        return None;
    }
    let reference = validation::reference_shell(&checked.typed, local.type_reference)?;
    let ExpressionNode::Borrow(initializer) =
        checked.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    if initializer.access != reference.access {
        return None;
    }
    // Shared scalar aliases can have observation roots without establishing a
    // structural value. Every other retained root is a real producer whose
    // operation must survive, even if a damaged roster no longer mentions it.
    let structural_root = checked
        .facts
        .values
        .structural_values
        .roots
        .iter()
        .any(|(_, root)| {
            root.state == state
                && root.statement_ordinal as usize == index
                && root.expression == local.initial_value
        });
    if structural_root
        && !(reference.access == ReferenceAccess::Shared
            && checked
                .typed
                .primitive_type_reference(reference.referee)
                .is_some())
    {
        return None;
    }
    let flows = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, row)| row.machine_symbol == machine && row.state_symbol == state)
        .collect::<Vec<_>>();
    let [(_, flow)] = flows.as_slice() else {
        return None;
    };
    // A selector or receiver call is still an effect, at any nesting ordinal.
    if checked
        .facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .iter()
        .any(|call| call.statement_index == index)
    {
        return None;
    }
    let access = match reference.access {
        ReferenceAccess::Shared => BorrowAccessKind::Read,
        ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
        ReferenceAccess::WriteOnly => BorrowAccessKind::WriteOnly,
    };
    let borrow = &checked.facts.borrow;
    let states = borrow
        .states
        .iter()
        .filter(|(_, row)| row.machine_symbol == machine && row.state_symbol == state)
        .collect::<Vec<_>>();
    let [(_, borrow_state)] = states.as_slice() else {
        return None;
    };
    let loans = borrow
        .loans
        .iter()
        .filter(|(handle, loan)| {
            borrow.state_owns_loan(borrow_state, *handle) && loan.owner_symbol == local.symbol
        })
        .collect::<Vec<_>>();
    let [(handle, loan)] = loans.as_slice() else {
        return None;
    };
    if loan.statement_index != index
        || loan.kind != access
        || !borrow.loan_owner_path(loan).is_empty()
    {
        return None;
    }
    // Derived carriers are replayed by the existing alias owner, including
    // captured parent identity, activation, last use and restoration. Asking
    // at the actual last use does not invent a use at the declaration.
    if loan.lineage != BorrowLoanLineage::DirectRoot {
        return crate::emission::call_source_custody::projected_receivers::aliases::parameter_source(
            checked, machine, state, loan.last_use_statement_index, local.symbol, true,
        ).ok().map(|source| source.is_some());
    }
    // A direct loan's spelled target must project to the place its ledger
    // entry records. Derived carriers carry that agreement inside their
    // lineage evidence instead, spelled against the parent's ultimate root
    // — asking for it here would refuse every local-rooted projection the
    // ledger already proved.
    let captured = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state,
        Some(index),
        initializer.target,
    )
    .ok()?;
    let source_type = match checked.expression_table.expression(initializer.target) {
        ExpressionNode::Cast(cast) if cast.form.is_recast() => cast.result_type,
        _ => validation::declared_place_type_raw(
            &checked.typed,
            authored_machine,
            Some(authored_state),
            initializer.target,
        )?,
    };
    let source_type = validation::reference_shell(&checked.typed, source_type)
        .map_or(source_type, |reference| reference.referee);
    // Qualification legality belongs to the checked recast/weakening judgment.
    // Here we replay storage custody: a declared domain or range does not turn
    // an alias into a runtime producer. Compare the declared carriers while
    // retaining access, extent and nominal identity; loan geometry and the
    // exact activation below still justify omitting the declaration.
    if !validation::declared_carriers_match(&checked.typed, reference.referee, source_type) {
        return None;
    }
    if loan.source_owner_symbol.is_valid()
        || loan.root_symbol != captured.captured_place().root_symbol
        || !crate::emission::call_source_custody::projected_receivers::aliases::loan_place_agrees(
            borrow.loan_segments(loan),
            &captured.captured_place().segments,
            matches!(checked.expression_table.expression(initializer.target),
                ExpressionNode::Cast(cast) if cast.form.is_recast()),
        )
    {
        return None;
    }
    let activations = checked
        .facts
        .flow
        .borrow_lifetimes
        .activations
        .span_or_empty(flow.borrow_activations)
        .iter()
        .filter(|activation| activation.loan == *handle)
        .collect::<Vec<_>>();
    Some(matches!(activations.as_slice(), [activation]
    if activation.source == checked_trees::FlowInvalidationSource::Statement {
        statement_index: index,
    }))
}
