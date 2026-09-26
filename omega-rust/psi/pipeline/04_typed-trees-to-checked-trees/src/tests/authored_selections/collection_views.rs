//! A collection-view call is the compiler-owned operation, not the spelling.
//!
//! The same program spells `as_slice` twice: once as the compiler-owned view
//! of a fixed array, and once as a declared machine on a data receiver. Only
//! the first is a view, and `semantic::calls::collection_view_call` is the one
//! place which decides that for every consumer lane.

use super::{AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionTarget};
use crate::tests::front_end::{checked_program, typed_program};
use language_semantics::declaration_selection::CollectionViewOperation;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

const BOTH_SPELLINGS: &str = r#"
    data Tally { total: u8; }

    machine Tally::as_slice(&self) -> u8 {
        self.total
    }

    machine main(tally: &Tally) -> u8 {
        let fixed: [u8; 2] = [1, 2];
        let view: &[u8] = fixed.as_slice();
        tally.as_slice()
    }
"#;

/// Both `as_slice` calls, as (expression, resolved-target) pairs.
fn as_slice_calls(program: &typed_trees::TypedTrees) -> Vec<(ExpressionHandle, bool)> {
    program
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            let ExpressionNode::Call(call) = node else {
                return None;
            };
            (call.target.as_str() == CollectionViewOperation::SharedSlice.authored_spelling())
                .then_some((expression, call.target_symbol.is_valid()))
        })
        .collect()
}

#[test]
fn a_resolved_nominal_machine_spelled_as_slice_is_not_a_collection_view() {
    let program = typed_program(BOTH_SPELLINGS);
    let calls = as_slice_calls(&program);
    assert_eq!(calls.len(), 2, "both `as_slice` calls are retained");

    for (expression, target_is_resolved) in calls {
        let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
            panic!("retained call expression");
        };
        let selected = crate::semantic::calls::collection_view_call(&program, call);
        if target_is_resolved {
            assert_eq!(
                selected, None,
                "`Tally::as_slice` is a declared machine, not a compiler-owned view"
            );
        } else {
            assert_eq!(
                selected,
                Some(CollectionViewOperation::SharedSlice),
                "`fixed.as_slice()` is the compiler-owned shared-slice view"
            );
        }
    }
}

#[test]
fn checking_retains_the_view_operation_and_the_declared_machine_separately() {
    let checked = checked_program(BOTH_SPELLINGS);
    let mut view_selections = 0usize;
    let mut resolved_selections = 0usize;

    for (expression, node) in checked.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        if call.target.as_str() != CollectionViewOperation::SharedSlice.authored_spelling() {
            continue;
        }
        for occurrence in checked
            .expression_table
            .authored_selection_occurrences(expression)
        {
            let Some(selection) = checked.authored_declaration_selections().get(occurrence) else {
                continue;
            };
            match selection.target() {
                AuthoredDeclarationSelectionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::CollectionView(operation),
                ) => {
                    assert_eq!(operation, CollectionViewOperation::SharedSlice);
                    view_selections += 1;
                }
                AuthoredDeclarationSelectionTarget::Resolved(_) => resolved_selections += 1,
                other => panic!("unexpected `as_slice` selection target: {other:?}"),
            }
        }
    }

    assert_eq!(view_selections, 1, "one compiler-owned view is retained");
    assert_eq!(
        resolved_selections, 1,
        "the declared machine keeps its resolved declaration"
    );
}
