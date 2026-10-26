//! Read the normal form of a compiler-created initializer value probe.
//!
//! Ordinary terminal-call normalization may introduce immutable local captures
//! before the original value exit. Both source-custody validation and checked
//! invocation replay must visit those same operands: requiring one statement
//! discards a valid direct call before its selected carrier can be checked.
//! The private probe retains
//! one unconditional value exit at its authored source point, preceded only by
//! initialized immutable captures. Source/declaration rosters, carrier proof and
//! execution admission remain the consuming owners' independent obligations.
//!
//! Private constant preparation changes selections and destinations; its
//! reference-result normalization can add initialized immutable local captures,
//! copying the original statements. Full helper checking removes these exact
//! probe roots before ordinary body preparation and restores their declarations
//! afterwards. Named exits, guards and assignments are not additional forms of
//! this private value-probe protocol.

use source::SourceSpan;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

pub(super) struct ValueProbeBody {
    pub(super) reference: SourceSpan,
    pub(super) roots: Vec<ExpressionHandle>,
}

pub(super) fn value_probe_body(
    program: &TypedTrees,
    state: &State,
) -> Result<ValueProbeBody, String> {
    let Some((last, captures)) = program
        .statement_table
        .statements(state.statement_nodes)
        .split_last()
    else {
        return Err("initializer probe lost its value exit".into());
    };
    let StatementNode::Transition(transition) = last else {
        return Err("initializer probe lost its value exit".into());
    };
    if !matches!(transition.guard, TransitionGuardNode::Always)
        || !matches!(transition.exit, TransitionExit::Ordinary)
        || transition.continuation.is_valid()
        || !transition.proof_selectors.is_empty()
    {
        return Err("initializer probe changed its unconditional value exit".into());
    }
    let TransitionTargetNode::Value(value) =
        program.statement_table.transition_target(transition.target)
    else {
        return Err("initializer probe does not return its value".into());
    };
    if !value.is_valid() {
        return Err("initializer probe lost its value expression".into());
    }
    let mut roots = Vec::new();
    for capture in captures {
        let StatementNode::LocalData(local) = capture else {
            return Err("initializer probe has an operation outside its value captures".into());
        };
        if local.is_mutable || local.ordinary_value_initializer().is_none() {
            return Err("initializer probe has an incomplete or mutable value capture".into());
        }
        roots.push(local.initial_value);
    }
    roots.push(*value);
    Ok(ValueProbeBody {
        reference: transition.source_span,
        roots,
    })
}
