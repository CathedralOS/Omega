use super::*;
use typed_trees::domain::ProofFact;
use typed_trees::signature::SignatureContractKind;

/// Refine a bounded return using joined arrivals and authored assumptions. This
/// proves the body under those assumptions, not the assumptions themselves:
/// checked call/transition validation must still discharge every arrival.
/// The arithmetic query binds source arguments to exact target parameters and
/// crosses the consuming state's writes before publishing a range.
pub(super) fn integer_range(
    proof_plan: &ProofPlan<'_>,
    obligation: &BoundedStateReturnObligation,
    context: &AssignmentRangeContext<'_>,
) -> Option<IntegerRange> {
    let program = proof_plan.program;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == obligation.machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == obligation.state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    if !std::ptr::eq(program, context.program)
        || !matches!(statements.get(obligation.statement_index),
            Some(StatementNode::Expression(value)) if *value == obligation.value)
    {
        return None;
    }
    if let Some((minimum, maximum)) = validation::arrival_integer_expression_bounds(
        program,
        obligation.machine_symbol,
        obligation.state_symbol,
        obligation.statement_index,
        obligation.value,
    ) {
        return Some(IntegerRange {
            minimum: BigInt::from_i64(minimum),
            maximum: BigInt::from_i64(maximum),
        });
    }
    let declared = integer_range_for_return_value(proof_plan, obligation)?;
    let is_entry = program
        .machine_states(machine)
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
            collect_read_place_paths(proof_plan, obligation.value, &mut value_reads);
            if let Some(operands) = &obligation.binary_operands {
                collect_read_place_paths(proof_plan, operands.left, &mut value_reads);
                collect_read_place_paths(proof_plan, operands.right, &mut value_reads);
            }
            let Some(call_frames) = context.call_frames() else {
                continue;
            };
            if arrival_stability::prefix_preserves_reads(
                proof_plan,
                machine,
                state,
                obligation.statement_index,
                &premise_reads,
                &value_reads,
                call_frames,
            ) {
                conditions.push(*condition);
            }
        }
    }
    let refine = |mut range: IntegerRange, value: ExpressionHandle| {
        for condition in &conditions {
            range = apply_condition(proof_plan, range, value, *condition);
        }
        range
    };
    let mut range = refine(declared.clone(), obligation.value);
    if let Some(operands) = &obligation.binary_operands
        && let (Some(left), Some(right)) = (&operands.left_range, &operands.right_range)
    {
        let left = refine(left.clone(), operands.left);
        let right = refine(right.clone(), operands.right);
        if left.minimum <= left.maximum
            && right.minimum <= right.maximum
            && let Some(folded) = integer_binary_range(operands.operator, left, right)
        {
            range.minimum = range.minimum.max(folded.minimum);
            range.maximum = range.maximum.min(folded.maximum);
        }
    }
    // An empty intersection is not a range witness for a produced value.
    if range.minimum > range.maximum {
        Some(declared)
    } else {
        Some(range)
    }
}

fn stable_expression(proof_plan: &ProofPlan<'_>, value: ExpressionHandle) -> bool {
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

fn same_place(proof_plan: &ProofPlan<'_>, left: ExpressionHandle, right: ExpressionHandle) -> bool {
    let table = &proof_plan.program.expression_table;
    matches!(
        table.expression(left),
        ExpressionNode::Name(_) | ExpressionNode::Member(_)
    ) && stable_expression(proof_plan, left)
        && stable_expression(proof_plan, right)
        && table.expressions_structurally_equal(left, right)
}

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
