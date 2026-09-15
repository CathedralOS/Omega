//! Statements, including the operand-hoisting rewrites.
//!
//! Most statements lower one-to-one. Runtime-indexed operand reads and
//! guarded arm-local value calls are hoisted into synthetic `let` bindings
//! and continuation states ahead of the rewritten statement, so later passes
//! see only root-level reads and plain locals. `guarded_call_arguments`
//! captures the values an arm-local return call needs.
//!
//! This file owns the statement entry point. `statement_nodes.rs` lowers
//! each statement node, transition guard and target, `indexed_read_hoisting.rs`
//! hoists runtime-indexed operand reads, `value_call_hoisting.rs` hoists
//! scalar value calls, `guarded_arm_rewrites.rs` rewrites guarded arms and
//! their argument calls, `match_subject_hoisting.rs` hoists membership and
//! comparison match subjects and `guarded_call_arguments.rs` captures the
//! values an arm-local return call needs.

mod guarded_arm_rewrites;
mod guarded_call_arguments;
mod indexed_read_hoisting;
mod match_subject_hoisting;
mod statement_nodes;
mod value_call_hoisting;

use crate::lowering::statement::statement_nodes::lower_statement_node;
use crate::resolution::lowerer::Lowerer;
use diagnostics::Diagnostic;
use symbol_resolved_trees::statement::Statement;
use syntax_trees::{self as syntax, SyntaxTrees};

/// Lowers a syntax statement to one or more symbol-resolved statements.
///
/// Most statements lower one-to-one. Assignments and local-data declarations
/// can yield EXTRA statements first: every runtime-indexed read `arr[i]` used
/// as a SUB-EXPRESSION OPERAND (a child of a binary/cast/etc., not the root of
/// the value) is hoisted into a synthetic `let __hoist_N = arr[i];` placed
/// before the rewritten statement, and the operand is replaced with a name
/// referencing that temp. The hoisted `let` is itself a root-level whole-value
/// indexed read, which already lowers natively; the rewritten parent then only
/// reads a plain local. The hoisted statements come first so later passes seed
/// and resolve the temps' symbols (locals are bound by ORDER + NAME).
pub(crate) fn lower_statement_handle(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    statement: syntax::statement::StatementHandle,
    statement_index: usize,
    has_preceding_transition: bool,
) -> Result<Vec<Statement>, Diagnostic> {
    lower_statement_node(
        lowerer,
        syntax_trees,
        syntax_trees.statements.statement(statement),
        statement_index,
        has_preceding_transition,
    )
}
