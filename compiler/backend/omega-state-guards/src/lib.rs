mod model;
mod normalize;
mod operands;

pub use model::{
    StateGuard, StateGuardKind, StateGuardOperand, StateGuardOperandKind, StateGuardOperandStorage,
    StateGuardPlan,
};
use normalize::normalize_guard_expression;
use omega_checked_trees::Program;
use omega_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable, TableBinaryExpression,
};
use omega_checked_trees::machine::Machine;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_layout::LayoutPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_state_dispatch::{DispatchEdge, StateDispatchPlan};
use omega_state_values::simplify_expression;
pub use omega_target_operations::{StateGuardLowering, StateGuardOperator};
use operands::{GuardOperands, guard_operands};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StateGuardClause {
    pub lowering: StateGuardLowering,
    pub operator: StateGuardOperator,
    pub storage: StateGuardOperandStorage,
    pub byte_offset: usize,
    pub right_storage: StateGuardOperandStorage,
    pub right_byte_offset: usize,
    pub byte_size: usize,
    pub expected_value: i64,
    pub has_storage: bool,
    pub has_right_storage: bool,
}

const INLINE_STATE_GUARD_CLAUSE_COUNT: usize = 4;

pub struct StateGuardClauses {
    inline: [Option<StateGuardClause>; INLINE_STATE_GUARD_CLAUSE_COUNT],
    len: usize,
    overflow: Vec<StateGuardClause>,
}

impl StateGuardClauses {
    fn new() -> Self {
        Self {
            inline: [None; INLINE_STATE_GUARD_CLAUSE_COUNT],
            len: 0,
            overflow: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &StateGuardClause> {
        self.inline
            .iter()
            .take(self.len.min(INLINE_STATE_GUARD_CLAUSE_COUNT))
            .filter_map(Option::as_ref)
            .chain(self.overflow.iter())
    }

    fn push(&mut self, clause: StateGuardClause) {
        if self.len < INLINE_STATE_GUARD_CLAUSE_COUNT {
            self.inline[self.len] = Some(clause);
        } else {
            self.overflow.push(clause);
        }

        self.len += 1;
    }
}

pub fn build_state_guard_plan(
    program: &Program,
    state_dispatch: &StateDispatchPlan,
    control_flow: &ControlFlowPlan,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    entry_machine: SymbolHandle,
) -> StateGuardPlan {
    let mut plan = StateGuardPlan {
        guards: Arena::with_capacity(state_dispatch.edges.len()),
        operands: Arena::with_capacity(state_dispatch.edges.len().saturating_mul(2)),
        ..StateGuardPlan::default()
    };
    let mut normalized_expressions = ExpressionTable::new();

    for (_, state) in state_dispatch.states.iter() {
        let Some(machine) = machine_by_symbol(program, state.key.machine) else {
            continue;
        };
        let Some(edges) = state_dispatch.edges.span(state.edges) else {
            continue;
        };

        for (statement_order, edge) in edges.iter().enumerate() {
            if control_flow.state_by_key(state.key).is_none() {
                continue;
            }
            plan.guards.insert(build_state_guard(
                program,
                &control_flow.expressions,
                &mut plan.expressions,
                &mut plan.operands,
                layouts,
                runtime_storage,
                entry_machine,
                machine,
                &mut normalized_expressions,
                state.key,
                state.dispatch_index,
                statement_order,
                edge.statement_index,
                edge,
            ));
        }
    }

    plan
}

pub fn classify_transition_guard_expression(
    table: &ExpressionTable,
    guard: ExpressionHandle,
) -> StateGuardKind {
    if !guard.is_valid() {
        return StateGuardKind::Always;
    }

    match table.expression(guard) {
        ExpressionNode::Boolean(true) => StateGuardKind::Always,
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::Equal => StateGuardKind::RuntimeEquality,
            BinaryOperator::NotEqual => StateGuardKind::RuntimeInequality,
            BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual => StateGuardKind::RuntimeOrdering,
            BinaryOperator::Add
            | BinaryOperator::And
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::Multiply
            | BinaryOperator::Or
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::Subtract => StateGuardKind::RuntimeExpression,
        },
        _ => StateGuardKind::RuntimeExpression,
    }
}

fn build_state_guard(
    program: &Program,
    source_expressions: &ExpressionTable,
    guard_expressions: &mut ExpressionTable,
    operand_arena: &mut Arena<StateGuardOperand>,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    entry_machine: SymbolHandle,
    source_machine: &Machine,
    normalized_expressions: &mut ExpressionTable,
    source: StateKey,
    source_dispatch_index: u32,
    statement_order: usize,
    statement_index: usize,
    edge: &DispatchEdge,
) -> StateGuard {
    let source_guard = edge.expressions.guard;
    normalized_expressions.clear();
    let normalized_guard = normalized_guard_expression(
        program,
        source_machine,
        source_expressions,
        source_guard,
        normalized_expressions,
    );
    let kind = classify_transition_guard_expression(normalized_expressions, normalized_guard);
    let operator = normalized_guard
        .is_valid()
        .then(|| guard_operator(normalized_expressions, normalized_guard))
        .unwrap_or(StateGuardOperator::None);
    let expression = normalized_guard
        .is_valid()
        .then(|| guard_expressions.copy_from(normalized_expressions, normalized_guard))
        .unwrap_or_else(ExpressionHandle::invalid);
    let has_expression = normalized_guard.is_valid();
    let guard_operands = guard_operands(
        normalized_expressions,
        guard_expressions,
        layouts,
        runtime_storage,
        entry_machine,
        source,
        source_machine.symbol,
        source_dispatch_index,
        statement_index,
        normalized_guard,
    );
    let (kind, operator, lowering, operands) = if let Some(expected) = normalized_guard
        .is_valid()
        .then(|| normalized_guard)
        .and_then(|guard| match normalized_expressions.expression(guard) {
            ExpressionNode::Boolean(value) => Some(*value),
            _ => None,
        })
        && let Some(slot) = runtime_storage.transition_guard_result_slot(
            source_dispatch_index,
            source,
            statement_index,
        ) {
        let operands = GuardOperands {
            left: StateGuardOperand {
                expression,
                kind: StateGuardOperandKind::Place,
                storage: StateGuardOperandStorage::RuntimeFrame,
                byte_offset: slot.byte_offset,
                byte_size: slot.byte_size,
                resolved_value: 0,
                has_resolved_value: false,
            },
            right: StateGuardOperand {
                expression,
                kind: StateGuardOperandKind::Literal,
                storage: StateGuardOperandStorage::Unknown,
                byte_offset: 0,
                byte_size: slot.byte_size,
                resolved_value: i64::from(expected),
                has_resolved_value: true,
            },
        };
        (
            StateGuardKind::RuntimeEquality,
            StateGuardOperator::Equal,
            StateGuardLowering::CompareStaticValue,
            operands.insert_into(operand_arena),
        )
    } else {
        let lowering = guard_lowering(kind, operator, guard_operands.as_ref());
        let operands = guard_operands
            .map(|operands| operands.insert_into(operand_arena))
            .unwrap_or_default();
        (kind, operator, lowering, operands)
    };

    StateGuard {
        source,
        source_dispatch_index,
        target: edge.target.clone(),
        target_dispatch_index: edge.target_dispatch_index,
        continuation: edge.continuation.clone(),
        continuation_dispatch_index: edge.continuation_dispatch_index,
        statement_order,
        statement_index,
        kind,
        operator,
        lowering,
        expression,
        operands,
        has_expression,
        forms_cycle: edge.forms_cycle,
    }
}

fn normalized_guard_expression(
    program: &Program,
    source_machine: &Machine,
    source_expressions: &ExpressionTable,
    source_guard: ExpressionHandle,
    normalized_expressions: &mut ExpressionTable,
) -> ExpressionHandle {
    if !source_guard.is_valid() {
        return ExpressionHandle::invalid();
    }

    if let ExpressionNode::Boolean(value) = source_expressions.expression(source_guard) {
        return normalized_expressions.insert(ExpressionNode::Boolean(*value));
    }

    if source_expressions.expression_is_direct_place_path(source_guard) {
        let left = normalized_expressions.copy_from(source_expressions, source_guard);
        let right = normalized_expressions.insert(ExpressionNode::Boolean(true));
        return normalized_expressions.insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: BinaryOperator::Equal,
            right,
        }));
    }

    if let Some(normalized_guard) = normalized_direct_place_boolean_guard(
        source_expressions,
        source_guard,
        normalized_expressions,
    ) {
        return normalized_guard;
    }

    let simplified_guard = simplify_expression(
        program,
        source_machine,
        &source_expressions.to_tree(source_guard),
    );
    let normalized_guard = normalize_guard_expression(simplified_guard);
    normalized_expressions.insert_tree(&normalized_guard)
}

fn normalized_direct_place_boolean_guard(
    source_expressions: &ExpressionTable,
    source_guard: ExpressionHandle,
    normalized_expressions: &mut ExpressionTable,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Binary(binary) = source_expressions.expression(source_guard) else {
        return None;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        return None;
    }

    let (place, expected) = match (
        source_expressions.expression(binary.left),
        source_expressions.expression(binary.right),
    ) {
        (_, ExpressionNode::Boolean(value))
            if source_expressions.expression_is_direct_place_path(binary.left) =>
        {
            (binary.left, positive_branch(binary.operator, *value))
        }
        (ExpressionNode::Boolean(value), _)
            if source_expressions.expression_is_direct_place_path(binary.right) =>
        {
            (binary.right, positive_branch(binary.operator, *value))
        }
        _ => return None,
    };

    let left = normalized_expressions.copy_from(source_expressions, place);
    let right = normalized_expressions.insert(ExpressionNode::Boolean(expected));
    Some(
        normalized_expressions.insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: BinaryOperator::Equal,
            right,
        })),
    )
}

fn positive_branch(operator: BinaryOperator, flag: bool) -> bool {
    match operator {
        BinaryOperator::Equal => flag,
        BinaryOperator::NotEqual => !flag,
        _ => true,
    }
}

fn machine_by_symbol<'program>(
    program: &'program Program,
    machine_symbol: SymbolHandle,
) -> Option<&'program Machine> {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
}

fn guard_lowering(
    kind: StateGuardKind,
    operator: StateGuardOperator,
    operands: Option<&GuardOperands>,
) -> StateGuardLowering {
    if kind == StateGuardKind::Always {
        return StateGuardLowering::NoOp;
    }

    if !matches!(
        operator,
        StateGuardOperator::Equal
            | StateGuardOperator::NotEqual
            | StateGuardOperator::Greater
            | StateGuardOperator::GreaterOrEqual
            | StateGuardOperator::Less
            | StateGuardOperator::LessOrEqual
    ) {
        return StateGuardLowering::NeedsRuntimeExpression;
    }

    let Some(operands) = operands else {
        return StateGuardLowering::NeedsRuntimeExpression;
    };
    let left = &operands.left;
    let right = &operands.right;

    if left.kind == StateGuardOperandKind::Place && right.has_resolved_value {
        return StateGuardLowering::CompareStaticValue;
    }

    if left.kind == StateGuardOperandKind::Place && right.kind == StateGuardOperandKind::Place {
        return StateGuardLowering::CompareRuntimeValue;
    }

    StateGuardLowering::NeedsRuntimeExpression
}

fn guard_operator(table: &ExpressionTable, expression: ExpressionHandle) -> StateGuardOperator {
    let ExpressionNode::Binary(binary) = table.expression(expression) else {
        return StateGuardOperator::None;
    };

    match binary.operator {
        BinaryOperator::Equal => StateGuardOperator::Equal,
        BinaryOperator::NotEqual => StateGuardOperator::NotEqual,
        BinaryOperator::Greater => StateGuardOperator::Greater,
        BinaryOperator::GreaterOrEqual => StateGuardOperator::GreaterOrEqual,
        BinaryOperator::Less => StateGuardOperator::Less,
        BinaryOperator::LessOrEqual => StateGuardOperator::LessOrEqual,
        BinaryOperator::Add => StateGuardOperator::Add,
        BinaryOperator::And => StateGuardOperator::And,
        BinaryOperator::Or => StateGuardOperator::Or,
        BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract => StateGuardOperator::None,
    }
}

pub fn lower_guard_conjunction(
    plan: &StateGuardPlan,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    entry_machine: SymbolHandle,
    source_key: StateKey,
    source_machine: SymbolHandle,
    source_dispatch_index: u32,
    statement_order: usize,
) -> StateGuardClauses {
    let Some(guard) = plan.guard_for_dispatch(source_dispatch_index, statement_order) else {
        return StateGuardClauses::new();
    };
    let expression = guard.expression;
    if !expression.is_valid() {
        return StateGuardClauses::new();
    }

    let mut clauses = StateGuardClauses::new();
    let mut scratch_expressions = ExpressionTable::new();
    if lower_guard_conjunction_expression(
        plan,
        layouts,
        runtime_storage,
        entry_machine,
        source_key,
        source_machine,
        source_dispatch_index,
        guard.statement_index,
        expression,
        &mut scratch_expressions,
        &mut clauses,
    )
    .is_none()
    {
        return StateGuardClauses::new();
    }

    clauses
}

fn lower_guard_conjunction_expression(
    plan: &StateGuardPlan,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    entry_machine: SymbolHandle,
    source_key: StateKey,
    source_machine: SymbolHandle,
    source_dispatch_index: u32,
    statement_index: usize,
    expression: ExpressionHandle,
    scratch_expressions: &mut ExpressionTable,
    clauses: &mut StateGuardClauses,
) -> Option<()> {
    match plan.expressions.expression(expression) {
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
            lower_guard_conjunction_expression(
                plan,
                layouts,
                runtime_storage,
                entry_machine,
                source_key,
                source_machine,
                source_dispatch_index,
                statement_index,
                binary.left,
                scratch_expressions,
                clauses,
            )?;
            lower_guard_conjunction_expression(
                plan,
                layouts,
                runtime_storage,
                entry_machine,
                source_key,
                source_machine,
                source_dispatch_index,
                statement_index,
                binary.right,
                scratch_expressions,
                clauses,
            )
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
            ) =>
        {
            lower_guard_leaf(
                plan,
                layouts,
                runtime_storage,
                entry_machine,
                source_key,
                source_machine,
                source_dispatch_index,
                statement_index,
                expression,
                scratch_expressions,
                clauses,
            )
        }
        _ => None,
    }
}

fn lower_guard_leaf(
    plan: &StateGuardPlan,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    entry_machine: SymbolHandle,
    source_key: StateKey,
    source_machine: SymbolHandle,
    source_dispatch_index: u32,
    statement_index: usize,
    expression: ExpressionHandle,
    scratch_expressions: &mut ExpressionTable,
    clauses: &mut StateGuardClauses,
) -> Option<()> {
    let kind = classify_transition_guard_expression(&plan.expressions, expression);
    let operator = guard_operator(&plan.expressions, expression);
    scratch_expressions.clear();
    let operands = guard_operands(
        &plan.expressions,
        scratch_expressions,
        layouts,
        runtime_storage,
        entry_machine,
        source_key,
        source_machine,
        source_dispatch_index,
        statement_index,
        expression,
    )?;
    let lowering = guard_lowering(kind, operator, Some(&operands));
    if matches!(
        lowering,
        StateGuardLowering::NeedsRuntimeExpression | StateGuardLowering::NoOp
    ) {
        return None;
    }

    let mut place_operands = [&operands.left, &operands.right]
        .into_iter()
        .filter(|operand| {
            operand.kind == StateGuardOperandKind::Place
                && operand.storage != StateGuardOperandStorage::Unknown
        });
    let place_operand = place_operands.next()?;
    let right_place_operand = place_operands.next();
    let expected_value = [&operands.left, &operands.right]
        .into_iter()
        .find(|operand| operand.has_resolved_value)
        .map(|operand| operand.resolved_value)
        .unwrap_or(0);

    clauses.push(StateGuardClause {
        lowering,
        operator,
        storage: place_operand.storage,
        byte_offset: place_operand.byte_offset,
        right_storage: right_place_operand
            .map(|operand| operand.storage)
            .unwrap_or(StateGuardOperandStorage::Unknown),
        right_byte_offset: right_place_operand
            .map(|operand| operand.byte_offset)
            .unwrap_or(0),
        byte_size: place_operand.byte_size,
        expected_value,
        has_storage: true,
        has_right_storage: right_place_operand.is_some(),
    });

    Some(())
}
