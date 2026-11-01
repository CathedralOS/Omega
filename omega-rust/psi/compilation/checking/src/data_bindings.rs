//! Exact lexical completion of a data binding's captured initializer.
//! The parser records the capture and its completion as semantic nodes. Names
//! are diagnostic only; resolution and the same-state declaration authenticate
//! the owner. Existing storage is observed through a reference capture and does
//! not acquire an anonymous owned disposal boundary.
use crate::CheckingInput;
use language_core::LocalBindingRole;
use typed_trees::{expression::ExpressionNode, statement::StatementNode};

pub fn completed_owner(
    program: &dyn CheckingInput,
    state: &typed_trees::state::State,
    statement: usize,
) -> Option<symbols::SymbolHandle> {
    let StatementNode::LocalData(end) = program.statement_table.statement_at(state, statement)?
    else {
        return None;
    };
    if end.role != LocalBindingRole::RecordPattern || end.pattern.is_some() {
        return None;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(end.initial_value) else {
        return None;
    };
    let symbol = path.resolved_symbol()?;
    let (capture_at, capture) = program.state_local_declaration(state, symbol)?;
    if capture_at >= statement
        || capture.pattern.is_none()
        || crate::reference_shell(program, capture.type_reference).is_some()
    {
        return None;
    }
    Some(symbol)
}
