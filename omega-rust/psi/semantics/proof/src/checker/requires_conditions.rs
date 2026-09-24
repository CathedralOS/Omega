//! Authored `requires` conditions as assumptions at a statement.
//!
//! A machine's `requires` holds on arrival at its entry state, and a state's
//! own `requires` on arrival at that state. A condition remains an assumption
//! at a later statement only while the state prefix preserves every place it
//! and the refined value read. Bounded returns and bounded assignments refine
//! their values from the same surviving conditions; the return certificate
//! cites them as premises the kernel re-decides.

use crate::checker::arrival_stability;
use crate::checker::assignment_stability::collect_read_place_paths;
use crate::checker::guards::{apply_left_literal_guard, apply_right_literal_guard};
use crate::checker::integer_ranges::AssignmentRangeContext;
use crate::obligations::{BinaryValueOperands, IntegerRange, ProofPlan, integer_binary_range};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::SignatureContractKind;

/// The `requires` conditions that still hold at `statement_index` of the
/// state, for a value that reads `value` and, when it is a binary, its
/// operands.
pub(super) fn surviving_conditions(
    proof_plan: &ProofPlan<'_>,
    context: &AssignmentRangeContext<'_>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    value: ExpressionHandle,
    binary_operands: Option<&BinaryValueOperands>,
) -> Vec<ExpressionHandle> {
    let program = proof_plan.program;
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        return Vec::new();
    };
    let states = program.machine_states(machine);
    let Some(state) = states.iter().find(|state| state.symbol == state_symbol) else {
        return Vec::new();
    };
    let Some(call_frames) = context.call_frames() else {
        return Vec::new();
    };
    let is_entry = states
        .first()
        .is_some_and(|entry| entry.symbol == state.symbol);
    let mut conditions = Vec::new();
    for contract in program
        .machine_contracts(machine)
        .iter()
        .filter(|_| is_entry)
        .chain(program.state_contracts(state))
        .filter(|contract| contract.kind == SignatureContractKind::Requires)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(condition) = fact else {
                continue;
            };
            // Calls, indexing and unknown identities need their own evaluated
            // fact custody; they cannot become stable premises by spelling.
            if !stable_expression(proof_plan, *condition) {
                continue;
            }
            let mut premise_reads = Vec::new();
            collect_read_place_paths(proof_plan, *condition, &mut premise_reads);
            let mut value_reads = premise_reads.clone();
            collect_read_place_paths(proof_plan, value, &mut value_reads);
            if let Some(operands) = binary_operands {
                collect_read_place_paths(proof_plan, operands.left, &mut value_reads);
                collect_read_place_paths(proof_plan, operands.right, &mut value_reads);
            }
            if arrival_stability::prefix_preserves_reads(
                proof_plan,
                machine,
                state,
                statement_index,
                &premise_reads,
                &value_reads,
                call_frames,
            ) {
                conditions.push(*condition);
            }
        }
    }
    conditions
}

/// `range` narrowed by every condition's literal comparisons on `value`, and
/// by the operator fold of the narrowed operand ranges when `value` is a
/// binary. An empty intersection is no witness for a produced value, so it
/// leaves `range` unchanged.
pub(super) fn refine(
    proof_plan: &ProofPlan<'_>,
    range: IntegerRange,
    value: ExpressionHandle,
    binary_operands: Option<&BinaryValueOperands>,
    conditions: &[ExpressionHandle],
) -> IntegerRange {
    let narrow = |mut range: IntegerRange, value: ExpressionHandle| {
        for condition in conditions {
            range = apply_condition(proof_plan, range, value, *condition);
        }
        range
    };
    let mut refined = narrow(range.clone(), value);
    if let Some(operands) = binary_operands
        && let (Some(left), Some(right)) = (&operands.left_range, &operands.right_range)
    {
        let left = narrow(left.clone(), operands.left);
        let right = narrow(right.clone(), operands.right);
        if left.minimum <= left.maximum
            && right.minimum <= right.maximum
            && let Some(folded) = integer_binary_range(operands.operator, left, right)
        {
            refined.minimum = refined.minimum.max(folded.minimum);
            refined.maximum = refined.maximum.min(folded.maximum);
        }
    }
    if refined.minimum > refined.maximum {
        range
    } else {
        refined
    }
}

/// Every operand is a spelling-stable place or literal.
pub(super) fn stable_expression(proof_plan: &ProofPlan<'_>, value: ExpressionHandle) -> bool {
    let table = &proof_plan.program.expression_table;
    if !table.expression_is_valid(value) {
        return false;
    }
    match table.expression(value) {
        ExpressionNode::Name(path) => path.symbol.is_valid() && path.head_symbol.is_valid(),
        ExpressionNode::Member(member) => {
            member.member_symbol.is_valid() && stable_expression(proof_plan, member.receiver)
        }
        ExpressionNode::Binary(binary) => {
            stable_expression(proof_plan, binary.left)
                && stable_expression(proof_plan, binary.right)
        }
        ExpressionNode::Unary(unary) => stable_expression(proof_plan, unary.operand),
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
        _ => false,
    }
}

/// The condition side is a stable `Name` or `Member` place and the subject
/// spells the same stable place.
pub(super) fn same_place(
    proof_plan: &ProofPlan<'_>,
    condition_side: ExpressionHandle,
    subject: ExpressionHandle,
) -> bool {
    let table = &proof_plan.program.expression_table;
    matches!(
        table.expression(condition_side),
        ExpressionNode::Name(_) | ExpressionNode::Member(_)
    ) && stable_expression(proof_plan, condition_side)
        && stable_expression(proof_plan, subject)
        && table.expressions_structurally_equal(condition_side, subject)
}

/// One `== true` unwrap, `&&` splits, and a side spelling the value as the
/// same stable place owns the comparison. There is no `!` arm: conditions
/// are assumptions, not fall-through complements.
fn apply_condition(
    proof_plan: &ProofPlan<'_>,
    range: IntegerRange,
    value: ExpressionHandle,
    condition: ExpressionHandle,
) -> IntegerRange {
    let table = &proof_plan.program.expression_table;
    let ExpressionNode::Binary(binary) = table.expression(condition) else {
        return range;
    };
    if binary.operator == BinaryOperator::And {
        let range = apply_condition(proof_plan, range, value, binary.left);
        return apply_condition(proof_plan, range, value, binary.right);
    }
    if binary.operator == BinaryOperator::Equal {
        if matches!(
            table.expression(binary.right),
            ExpressionNode::Boolean(true)
        ) {
            return apply_condition(proof_plan, range, value, binary.left);
        }
        if matches!(table.expression(binary.left), ExpressionNode::Boolean(true)) {
            return apply_condition(proof_plan, range, value, binary.right);
        }
    }
    if same_place(proof_plan, binary.left, value) {
        apply_right_literal_guard(proof_plan, range, binary.operator, binary.right)
    } else if same_place(proof_plan, binary.right, value) {
        apply_left_literal_guard(proof_plan, range, binary.left, binary.operator)
    } else {
        range
    }
}
