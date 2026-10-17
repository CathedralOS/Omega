//! Preservation of a binding's observed value and its descriptor home.
//!
//! A captured descriptor can survive writes through it, while replacing or
//! exposing its home retires that relationship. These queries never replay an
//! initializer and never supply current referent contents.

use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

/// An immutable local's initializer describes this occurrence only while no
/// exclusive borrow roots at its binding: `expose(&mut view)` hands the
/// descriptor to a callee that can install a different slice, so the saved
/// initializer no longer names the referent the call sees. A borrow that
/// selects into the binding (`&mut view[a..b]`, `&mut view[i]`) lends backing
/// contents through a descriptor of its own and cannot replace the binding's
/// descriptor, so it leaves the saved length valid; under
/// `bare-range-reference-access` that spelling is the only mutable window.
/// The scan is deliberately state-wide -- the caller's obligation order is
/// not the statement order -- and covers every carrier that can hold an
/// expression.
pub(crate) fn binding_is_exclusively_exposed(
    program: &dyn crate::CheckingInput,
    state: &State,
    symbol: SymbolHandle,
) -> bool {
    let mut nodes = Vec::new();
    let mut seen = std::collections::HashSet::with_hasher(symbols::BuildSymbolHasher);
    for root in statement_expression_roots(program, state) {
        crate::expression_traversal::collect_expression_nodes(program, root, &mut nodes, &mut seen);
    }
    nodes
        .iter()
        .any(|node| match program.expression_table.expression(*node) {
            ExpressionNode::Borrow(borrow) => {
                borrow.access.is_exclusive()
                    && borrow_root_symbol(program, borrow.target) == Some(symbol)
                    && !selects_collection_contents(program, borrow.target)
            }
            _ => false,
        })
}

/// Whether a place selects elements of its root binding rather than
/// the binding (or one of its fields) itself. An indexed step anywhere on the
/// path lends contents under a new descriptor; the root's own descriptor stays
/// where the declaration left it.
fn selects_collection_contents(
    program: &dyn crate::CheckingInput,
    mut expression: ExpressionHandle,
) -> bool {
    let mut visited = 0;
    while program.expression_table.expression_is_valid(expression) && visited < 128 {
        visited += 1;
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(_) => return true,
            _ => return false,
        }
    }
    false
}

/// Metadata and value observations have different storage dependencies. A
/// builtin element write can replace a nested descriptor, but never the outer
/// slice's pointer or extent. Whole-value observations still depend on contents.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BindingObservation {
    WholeValue,
    Descriptor,
}

/// An entry observation names the value the call edge still sees only while
/// nothing in `state` writes the carrier. `binding_is_exclusively_exposed`
/// covers `&mut`/`&out` borrow expressions; the remaining write channels are
/// an assignment reaching the observed storage, a receiver call
/// whose callee claims a mutable `self`, and a sealed atomic operation on
/// the place. Reads and moves leave the observed value alone -- a moved
/// name cannot be an actual afterwards. The scan is state-wide for the same
/// reason as the borrow scan: obligation order is not statement order.
pub(crate) fn binding_is_written(
    program: &dyn crate::CheckingInput,
    state: &State,
    symbol: SymbolHandle,
    observation: BindingObservation,
) -> bool {
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
    {
        match statement {
            StatementNode::Assignment(assignment)
                if borrow_root_symbol(program, assignment.target) == Some(symbol) =>
            {
                // Indexed syntax alone does not prove an element write:
                // authored indexing can execute arbitrary code. Only builtin
                // coordinates separate descriptor storage from its elements.
                let selects_contents = observation == BindingObservation::Descriptor
                    && selects_collection_contents(program, assignment.target)
                    && program
                        .machine_holding_state(state.symbol)
                        .is_some_and(|machine| {
                            crate::place_has_builtin_coordinates(
                                program,
                                machine,
                                Some(state),
                                assignment.target,
                            )
                        });
                if !selects_contents {
                    return true;
                }
            }
            StatementNode::Call(call)
                if call.receiver_root_symbol == symbol
                    && call_receiver_is_mutable(program, call.target_symbol) =>
            {
                return true;
            }
            _ => {}
        }
    }
    let mut nodes = Vec::new();
    let mut seen = std::collections::HashSet::with_hasher(symbols::BuildSymbolHasher);
    for root in statement_expression_roots(program, state) {
        crate::expression_traversal::collect_expression_nodes(program, root, &mut nodes, &mut seen);
    }
    nodes
        .iter()
        .any(|node| match program.expression_table.expression(*node) {
            ExpressionNode::Call(call) => {
                call.receiver.is_valid()
                    && borrow_root_symbol(program, call.receiver) == Some(symbol)
                    && call_receiver_is_mutable(program, call.target_symbol)
            }
            ExpressionNode::Atomic(atomic) => {
                borrow_root_symbol(program, atomic.value) == Some(symbol)
                    || borrow_root_symbol(program, atomic.result) == Some(symbol)
            }
            _ => false,
        })
}

/// Unknown callees conservatively invalidate a receiver observation. Resolved
/// static calls carry every writable place through their explicit arguments.
fn call_receiver_is_mutable(
    program: &dyn crate::CheckingInput,
    target_symbol: SymbolHandle,
) -> bool {
    crate::machine_calls::calls::machine_state_by_symbol(program, target_symbol).is_none()
}

/// Every expression root a statement of `state` can carry: the channels a
/// nested borrow or receiver call could reach an observed carrier through.
fn statement_expression_roots(
    program: &dyn crate::CheckingInput,
    state: &State,
) -> Vec<ExpressionHandle> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .flat_map(|statement| program.tables.expression_roots(statement))
        .map(|(_, root)| root)
        .collect()
}

/// The root symbol a borrowable place path descends from: `view`, `view.field`,
/// or `view[i]` all root at `view`'s local symbol.
fn borrow_root_symbol(
    program: &dyn crate::CheckingInput,
    mut expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let mut visited = 0;
    while program.expression_table.expression_is_valid(expression) && visited < 128 {
        visited += 1;
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => expression = borrow.target,
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
            ExpressionNode::Name(path) => return Some(path.head_symbol),
            _ => return None,
        }
    }
    None
}
