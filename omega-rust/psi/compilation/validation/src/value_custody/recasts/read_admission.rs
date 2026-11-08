//! Recheck one retained view at its original formation, before its later read.
//! A descriptor and an invalidation footprint cannot establish that view.

use super::recast_judgments::{judge_scalar_recast, strip_mutable};
use crate::frozen_program::CheckingQuery;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(crate) fn established_reference_formation(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Result<(), &'static str> {
    if program.defers_pending_layout_geometry() {
        return Err("retained view formation has no strict layout judgment");
    }
    // Reentry supplies no permission. Query-local proof memos and counted cuts
    // keep any dependent answers out of the shared completed-positive memos.
    // There is no new cached establishment verdict at a later consuming cut.
    let query = CheckingQuery::recast_formation(program, expression)
        .ok_or("retained view formation is recursive or exceeds its query depth")?;
    let program: &dyn crate::CheckingInput = &query;
    let (machine, state) = super::pending_views::formation_owner(program, expression)
        .ok_or("retained view formation has no unique original body")?;
    let mut statements = program
        .statement_table
        .indexed_statements(state.statement_nodes)
        .filter_map(|(index, statement)| {
            program
                .tables
                .expression_roots(statement)
                .into_iter()
                .any(|(_, root)| {
                    root.is_valid()
                        && crate::expression_owners::expression_contains(program, root, expression)
                })
                .then_some(index)
        });
    let index = statements
        .next()
        .ok_or("retained view formation has no original statement")?;
    if statements.next().is_some() {
        return Err("retained view formation has ambiguous statement cuts");
    }
    let formation = super::reference_formation_source(program, expression)
        .ok_or("retained view has no original reference formation")?;
    let cast_handle = strip_mutable(program, expression);
    let cast = program
        .expression_table
        .cast_node(cast_handle)
        .filter(|cast| cast.form.is_recast())
        .ok_or("retained view formation has no representation cast")?;
    let source_access = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => Some(borrow.access),
        _ => None,
    };
    let frames = crate::CallFrameResolver::new(program)
        .ok_or("retained view formation has no original lender resolver")?;
    let mut diagnostics = Vec::new();
    // This is the normal source judgment, with its original owner, actual
    // source occurrence and pre-operation proof cut. The consuming read and
    // its unknown value are never premises of formation or footprint proofs.
    judge_scalar_recast(
        program,
        Some(&frames),
        machine,
        state,
        index,
        cast,
        cast_handle,
        formation.view_type,
        formation.access,
        source_access,
        &mut diagnostics,
    );
    if !diagnostics.is_empty() {
        if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!(
                "STRUCT retained-view-formation-refused expression={expression:?} machine={:?} state={:?} statement={index} diagnostic={:?}",
                machine.symbol,
                state.symbol,
                diagnostics.first(),
            );
        }
        return Err("retained view failed its independent original formation judgment");
    }
    Ok(())
}
