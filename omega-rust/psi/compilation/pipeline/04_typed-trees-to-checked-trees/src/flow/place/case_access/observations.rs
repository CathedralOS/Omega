//! Authenticate the tag observations already live in one consumer scope.
//! Preparation does not inspect initializers or recover retired observations.
//! Each Boolean leaf keeps its selected operation, exact place and polarity.

use facts::{FactContextHandle, FactPayload, FactPlan};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use validation::{CanonicalPlace, canonical_place_from_expression_in_state};

pub(super) struct CaseObservation {
    pub place: CanonicalPlace,
    pub variant: SymbolHandle,
    pub selected: bool,
}

pub(super) fn collect(
    program: &dyn validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Vec<CaseObservation> {
    let mut observer = Observer {
        program,
        machine,
        state,
        statement_index,
        rows: Vec::new(),
    };
    for handle in semantic.unique_context_facts(contexts.iter().copied()) {
        let fact = semantic.facts.get(handle);
        match fact.payload {
            FactPayload::BooleanValue { expression, value } => {
                observer.predicate(expression, value, 0)
            }
            FactPayload::BooleanExpression(expression)
            | FactPayload::ContractBooleanExpression { expression, .. } => {
                observer.predicate(expression, true, 0)
            }
            FactPayload::AssignedCase { variant } => {
                if !variant.is_valid()
                    || program.symbols.get(variant).kind != symbols::SymbolKind::Variant
                {
                    continue;
                }
                let Some(place) = fact.place.as_place().and_then(|place| {
                    super::canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic.places.get(place),
                    )
                }) else {
                    continue;
                };
                observer.rows.push(CaseObservation {
                    place,
                    variant,
                    selected: true,
                });
            }
            _ => {}
        }
    }
    observer
        .rows
        .sort_by(|left, right| left.place.compare_identity(&right.place));
    observer.rows
}

struct Observer<'program> {
    program: &'program dyn validation::CheckingInput,
    machine: &'program typed_trees::machine::Machine,
    state: &'program typed_trees::state::State,
    statement_index: usize,
    rows: Vec<CaseObservation>,
}

impl Observer<'_> {
    fn predicate(&mut self, expression: ExpressionHandle, value: bool, depth: usize) {
        if self
            .program
            .expression_table
            .expression_at_depth(expression, depth)
            .is_none()
        {
            return;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                self.predicate(unary.operand, !value, depth + 1);
            }
            ExpressionNode::Binary(binary)
                if (binary.operator == BinaryOperator::And && value)
                    || (binary.operator == BinaryOperator::Or && !value) =>
            {
                self.predicate(binary.left, value, depth + 1);
                self.predicate(binary.right, value, depth + 1);
            }
            ExpressionNode::Binary(binary)
                if binary.operator == BinaryOperator::Equal
                    && matches!(
                        self.program.expression_table.expression(binary.right),
                        ExpressionNode::Boolean(true)
                    )
                    && validation::has_builtin_decomposed_guard_meaning(
                        self.program,
                        self.machine,
                        Some(self.state),
                        expression,
                    ) =>
            {
                self.predicate(binary.left, value, depth + 1);
            }
            ExpressionNode::Binary(binary)
                if binary.operator == BinaryOperator::Equal
                    && matches!(
                        self.program.expression_table.expression(binary.left),
                        ExpressionNode::Boolean(true)
                    )
                    && validation::has_builtin_decomposed_guard_meaning(
                        self.program,
                        self.machine,
                        Some(self.state),
                        expression,
                    ) =>
            {
                self.predicate(binary.right, value, depth + 1);
            }
            ExpressionNode::Binary(binary) => {
                let Some(variant) = validation::applied_case_membership_variant(
                    self.program,
                    self.machine,
                    Some(self.state),
                    expression,
                    binary,
                ) else {
                    return;
                };
                let Some(place) = canonical_place_from_expression_in_state(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    binary.left,
                ) else {
                    return;
                };
                self.rows.push(CaseObservation {
                    place,
                    variant,
                    selected: value,
                });
            }
            _ => {}
        }
    }
}
