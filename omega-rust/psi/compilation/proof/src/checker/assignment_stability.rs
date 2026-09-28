//! Proof that an assignment guard still holds when the assignment runs.

use crate::checker::AssignmentRangeContext;
use crate::checker::arrival_stability;
use crate::checker::guards::unwrap_true_guard_condition;
use crate::obligations::{BoundedAssignmentObligation, ProofPlan};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

pub(crate) fn collect_stable_assignment_conditions(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    condition: ExpressionHandle,
    context: &AssignmentRangeContext<'_>,
    conditions: &mut Vec<ExpressionHandle>,
) {
    let Some(prefix) = prepare_assignment_prefix(proof_plan, obligation, context) else {
        return;
    };
    collect_stable_conditions(proof_plan, condition, &prefix, conditions);
}

fn collect_stable_conditions(
    proof_plan: &ProofPlan<'_>,
    condition: ExpressionHandle,
    prefix: &arrival_stability::ArrivalPrefix<'_>,
    conditions: &mut Vec<ExpressionHandle>,
) {
    let unwrapped = unwrap_true_guard_condition(proof_plan, condition);
    if unwrapped != condition {
        collect_stable_conditions(proof_plan, unwrapped, prefix, conditions);
        return;
    }
    if let ExpressionNode::Binary(binary) =
        proof_plan.program.expression_table.expression(condition)
        && binary.operator == BinaryOperator::And
    {
        collect_stable_conditions(proof_plan, binary.left, prefix, conditions);
        collect_stable_conditions(proof_plan, binary.right, prefix, conditions);
    } else {
        let mut reads = Vec::new();
        collect_read_place_paths(proof_plan, condition, &mut reads);
        if prefix.preserves_reads(&reads, &reads) {
            conditions.push(condition);
        }
    }
}

/// Prepare the effects crossed by incoming-edge facts before THIS assignment.
/// Every earlier statement in the state must have a complete write
/// frame provably DISJOINT from every place the guard condition or assignment
/// value reads. Prefix member paths alias (`self.state` vs
/// `self.state.count`); distinct roots do not (`self.pixels[i]` vs `self.i` --
/// the render-loop shape stays provable). Resolved pure calls therefore
/// preserve the guard; opaque frames and unsupported statement shapes still
/// drop it. Value dependencies are common to all conjuncts and checked once;
/// guard dependencies are checked separately so one invalidated conjunct does
/// not discard unrelated surviving facts. No summary outlives this query.
fn prepare_assignment_prefix<'query>(
    proof_plan: &'query ProofPlan<'_>,
    obligation: &BoundedAssignmentObligation,
    context: &'query AssignmentRangeContext<'_>,
) -> Option<arrival_stability::ArrivalPrefix<'query>> {
    let program = proof_plan.program;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == obligation.machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == obligation.state_symbol)?;

    // Every place the guard fact (and the value it refines) depends on. The
    // DE-HOISTED binary operands are included too: the obligation value may
    // spell `__hoist_N + 1` while the guard fact is applied to the hoisted
    // read's PLACE (`tallies[self.k]`), so an aliasing write into the
    // collection (or to the index) must drop the fact even though the value's
    // own read path is only the local name.
    // GUARD reads and VALUE reads are tracked separately: a body `let` that
    // DEFINES a name the value reads (the operand hoist's own
    // `let __hoist_N = tallies[self.k]`) is the fact's SOURCE, not an
    // invalidation -- but a `let` shadowing a name the GUARD read (the guard
    // evaluates in the SOURCE state's scope, so a same-named body local is a
    // different binding) must still kill the fact, and any ASSIGNMENT
    // aliasing either list still kills it.
    let mut read_paths: Vec<Vec<String>> = Vec::new();
    collect_read_place_paths(proof_plan, obligation.value, &mut read_paths);
    if let Some(operands) = &obligation.binary_operands {
        collect_read_place_paths(proof_plan, operands.left, &mut read_paths);
        collect_read_place_paths(proof_plan, operands.right, &mut read_paths);
    }
    let call_frames = context.call_frames()?;

    let statements = program.statement_table.statements(state.statement_nodes);
    if !matches!(statements.get(obligation.statement_index),
        Some(StatementNode::Assignment(assignment))
            if assignment.target == obligation.target && assignment.value == obligation.value)
    {
        return None;
    }
    let prefix = arrival_stability::prepare_prefix(
        proof_plan,
        machine,
        state,
        obligation.statement_index,
        call_frames,
    )?;
    prefix.preserves_reads(&[], &read_paths).then_some(prefix)
}

pub(crate) fn resolved_writes_overlap_reads(written: &[String], reads: &[Vec<String>]) -> bool {
    reads.iter().any(|read| {
        let read = read.join(".");
        written
            .iter()
            .any(|write| validation::frame_paths_overlap(&read, write))
    })
}

/// The member path a place expression READS or WRITES, for the aliasing check:
/// `self.state.count` -> [self, state, count]; an INDEXED place resolves to its
/// collection's path (a write anywhere inside the collection aliases the whole
/// collection, nothing else). `None` for shapes the walk cannot name (treated
/// as opaque by callers).
pub(crate) fn written_place_path(
    proof_plan: &ProofPlan,
    expression: ExpressionHandle,
) -> Option<Vec<String>> {
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => written_place_path(proof_plan, inner.target),
        ExpressionNode::Indexed(indexed) => written_place_path(proof_plan, indexed.collection),
        ExpressionNode::Member(member) => {
            let mut path = written_place_path(proof_plan, member.receiver)?;
            path.push(member.member.as_str().to_owned());
            Some(path)
        }
        ExpressionNode::Name(path) => Some(
            proof_plan
                .program
                .expression_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str().to_owned())
                .collect(),
        ),
        _ => None,
    }
}

/// Collect the member paths of every Name/Member read inside `expression`
/// (guard conditions, assignment values). Unnameable reads (indexed elements)
/// contribute their COLLECTION path, so a write into the collection kills the
/// fact.
pub(crate) fn collect_read_place_paths(
    proof_plan: &ProofPlan,
    expression: ExpressionHandle,
    paths: &mut Vec<Vec<String>>,
) {
    if !expression.is_valid() {
        return;
    }
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            collect_read_place_paths(proof_plan, dispatch.subject, paths);
            for arm in proof_plan
                .program
                .expression_table
                .match_arms(dispatch.arms)
            {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    collect_read_place_paths(proof_plan, pattern, paths);
                }
                collect_read_place_paths(proof_plan, arm.value, paths);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_read_place_paths(proof_plan, binary.left, paths);
            collect_read_place_paths(proof_plan, binary.right, paths);
        }
        ExpressionNode::Unary(unary) => {
            collect_read_place_paths(proof_plan, unary.operand, paths);
        }
        ExpressionNode::Cast(cast) => collect_read_place_paths(proof_plan, cast.value, paths),
        ExpressionNode::Borrow(inner) => collect_read_place_paths(proof_plan, inner.target, paths),
        ExpressionNode::Indexed(indexed) => {
            collect_read_place_paths(proof_plan, indexed.collection, paths);
            collect_read_place_paths(proof_plan, indexed.index, paths);
        }
        ExpressionNode::Member(_) | ExpressionNode::Name(_) => {
            if let Some(path) = written_place_path(proof_plan, expression) {
                paths.push(path);
            }
        }
        _ => {}
    }
}

/// Two member paths may alias when one is a PREFIX of the other (a whole-struct
/// write aliases every field under it, and vice versa).
pub(crate) fn member_paths_may_alias(left: &[String], right: &[String]) -> bool {
    let shared = left.len().min(right.len());
    left[..shared] == right[..shared]
}

/// Whether any `Call` node appears in the expression tree (an opaque effect:
/// a value-machine call may mutate fields through `&mut self`).
pub(crate) fn expression_contains_call(
    proof_plan: &ProofPlan,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            expression_contains_call(proof_plan, dispatch.subject)
                || proof_plan
                    .program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        matches!(arm.pattern, typed_trees::expression::MatchPattern::Value(pattern)
                        if expression_contains_call(proof_plan, pattern))
                            || expression_contains_call(proof_plan, arm.value)
                    })
        }
        ExpressionNode::Call(_) => true,
        ExpressionNode::Binary(binary) => {
            expression_contains_call(proof_plan, binary.left)
                || expression_contains_call(proof_plan, binary.right)
        }
        ExpressionNode::Unary(unary) => expression_contains_call(proof_plan, unary.operand),
        ExpressionNode::Cast(cast) => expression_contains_call(proof_plan, cast.value),
        ExpressionNode::Borrow(inner) => expression_contains_call(proof_plan, inner.target),
        ExpressionNode::Indexed(indexed) => {
            expression_contains_call(proof_plan, indexed.collection)
                || expression_contains_call(proof_plan, indexed.index)
        }
        ExpressionNode::Member(member) => expression_contains_call(proof_plan, member.receiver),
        _ => false,
    }
}
