//! `&mut` structural-view results mint caller-side loans at the callee's own
//! tail source, not signature elision. Elision reads the declared signature —
//! rule 3 even collapses every `&mut`-returning `&mut self` method onto the
//! receiver — while the structural custody model asks which caller storage
//! the returned borrow actually denotes. The tail walk answers that: the
//! callee's return borrows, resolved through the callee's own locals back to
//! a `&mut` parameter, the machine's own authority, or a data-binding member.
use super::argument_borrow_loan_place;
use super::helper_call_borrow_loan_place;
use crate::borrow::accesses::BorrowAccessPlace;
use crate::borrow::accesses::borrow_access_place;
use crate::execution::terminal_unit::calls::{
    view_data_member_symbol, view_source_parameter_is_mutable,
};
use crate::semantic::calls::find_state_with_machine;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use language_core::ReferenceAccess;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use validation::reference_result_custody::mutable_borrowed_parts;

/// The caller-side place a `&mut` structural-view result loans: the call
/// receiver when the callee's returned borrows root inside `*self`, or the
/// one non-self argument its returns select. Answers `None` when the call
/// target is not a `mutable_borrowed_parts` callee or its tail does not
/// resolve to exactly one caller-side source — a minted-but-wrong loan is
/// worse than none, because downstream custody only admits exact matches.
pub(crate) fn mutable_view_call_loan_place(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    call: &checked_trees::expression::TableCallExpression,
) -> Option<BorrowAccessPlace> {
    let (callee_machine, callee_state) = find_state_with_machine(program, call.target_symbol)?;
    if mutable_borrowed_parts(program, callee_state.return_type).is_none() {
        return None;
    }
    match mutable_view_source(program, callee_machine, callee_state)? {
        MutableViewSource::SelfReceiver => {
            if !call.receiver.is_valid() {
                return None;
            }
            borrow_access_place(
                program,
                state_symbol,
                statement_index,
                call.receiver,
                machine_symbol,
            )
        }
        MutableViewSource::Parameter(non_self_index) => {
            let argument = program
                .expression_table
                .expression_handles(call.arguments)
                .get(non_self_index)
                .copied()?;
            argument_borrow_loan_place(
                program,
                state_symbol,
                statement_index,
                machine_symbol,
                argument,
            )
        }
    }
}

/// Which caller-side place the callee's `&mut` returns denote.
enum MutableViewSource {
    SelfReceiver,
    Parameter(usize),
}

/// Every `&mut` the callee returns must resolve to the same caller-side
/// source: one `&mut` parameter position, or the `*self` authority (directly
/// or through a `self` member). Returns resolving to callee-owned storage —
/// or to different sources — have no single caller loan to mint.
fn mutable_view_source(
    program: &TypedTrees,
    callee_machine: &typed_trees::machine::Machine,
    callee_state: &typed_trees::state::State,
) -> Option<MutableViewSource> {
    let statements = program
        .statement_table
        .statements(callee_state.statement_nodes);
    let parameters = program.state_parameters(callee_state);
    let mut source: Option<MutableViewSource> = None;
    for (ordinal, statement) in statements.iter().enumerate() {
        let returned = match statement {
            StatementNode::Transition(transition)
                if matches!(
                    transition.guard,
                    typed_trees::statement::TransitionGuardNode::Always
                ) && transition.exit == typed_trees::statement::TransitionExit::Ordinary
                    && !transition.continuation.is_valid() =>
            {
                match program.statement_table.transition_target(transition.target) {
                    TransitionTargetNode::Value(expression) => *expression,
                    _ => return None,
                }
            }
            StatementNode::Expression(expression) if ordinal + 1 == statements.len() => *expression,
            StatementNode::Transition(_) => return None,
            _ => continue,
        };
        let ExpressionNode::Borrow(borrow) = program.expression_table.expression(returned) else {
            return None;
        };
        if borrow.access != ReferenceAccess::Mutable {
            return None;
        }
        let root = mutable_view_caller_root(
            program,
            callee_machine.symbol,
            callee_state,
            statements,
            ordinal,
            borrow.target,
        )?;
        let resolved = if let Some(position) = parameters
            .iter()
            .position(|parameter| parameter.symbol == root)
        {
            let parameter = &parameters[position];
            if !view_source_parameter_is_mutable(program, parameter) {
                return None;
            }
            MutableViewSource::Parameter(parameters[..position].iter().count())
        } else if root == callee_machine.symbol
            || view_data_member_symbol(program, callee_state, callee_machine.symbol, root).is_some()
        {
            MutableViewSource::SelfReceiver
        } else {
            return None;
        };
        let agrees = match (&source, &resolved) {
            (None, _) => true,
            (Some(MutableViewSource::SelfReceiver), MutableViewSource::SelfReceiver) => true,
            (Some(MutableViewSource::Parameter(prior)), MutableViewSource::Parameter(next)) => {
                prior == next
            }
            _ => false,
        };
        if !agrees {
            return None;
        }
        source = Some(resolved);
    }
    source
}

/// The returned borrow's place ends at caller-frame storage: a `&mut`
/// parameter, the machine's `*self` authority, or a `self` member. Roots
/// that land on callee view locals substitute that local's own borrow
/// source — the same loan replay the tracker performs, kept syntactic so
/// this helper stays usable where no `CheckFacts` exist.
fn mutable_view_caller_root(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    ordinal: usize,
    target: ExpressionHandle,
) -> Option<SymbolHandle> {
    let mut place = borrow_access_place(program, state.symbol, ordinal, target, machine)?;
    for _ in 0..8 {
        if view_source_root_is_caller_storage(program, state, machine, place.root_symbol) {
            return Some(place.root_symbol);
        }
        let (index, local) =
            statements
                .iter()
                .take(ordinal)
                .enumerate()
                .find_map(|(index, statement)| match statement {
                    StatementNode::LocalData(local) if local.symbol == place.root_symbol => {
                        Some((index, local))
                    }
                    _ => None,
                })?;
        let next =
            initializer_borrow_place(program, state.symbol, index, machine, local.initial_value)?;
        let mut segments = next.segments;
        segments.extend(place.segments);
        place = BorrowAccessPlace {
            root_symbol: next.root_symbol,
            segments,
        };
    }
    None
}

/// The borrow place a view local's initializer denotes, without the tracker's
/// live loans: calls delegate to the shared call-place analysis (which the
/// `as_mut_slice`-shape collection views answer themselves), explicit
/// `&mut` reborrows and named places resolve through `borrow_access_place`.
fn initializer_borrow_place(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    initial: ExpressionHandle,
) -> Option<BorrowAccessPlace> {
    match program.expression_table.expression(initial) {
        ExpressionNode::Call(call) => helper_call_borrow_loan_place(
            program,
            state_symbol,
            statement_index,
            machine_symbol,
            call,
        ),
        ExpressionNode::Borrow(borrow) if borrow.access == ReferenceAccess::Mutable => {
            borrow_access_place(
                program,
                state_symbol,
                statement_index,
                borrow.target,
                machine_symbol,
            )
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            borrow_access_place(
                program,
                state_symbol,
                statement_index,
                initial,
                machine_symbol,
            )
        }
        _ => None,
    }
}

/// Caller-frame roots for the tail walk: the machine itself, one of its
/// data-binding member fields, or a `&mut` parameter of the callee state.
fn view_source_root_is_caller_storage(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    machine: SymbolHandle,
    root: SymbolHandle,
) -> bool {
    root == machine
        || program.state_parameters(state).iter().any(|parameter| {
            parameter.symbol == root && view_source_parameter_is_mutable(program, parameter)
        })
        || view_data_member_symbol(program, state, machine, root).is_some()
}
