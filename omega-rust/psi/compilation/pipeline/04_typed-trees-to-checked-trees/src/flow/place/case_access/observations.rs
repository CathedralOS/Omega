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
            FactPayload::CaseValue { variant, value } => {
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
                    selected: value,
                });
            }
            _ => {}
        }
    }
    // Normalize every retained observation in this same immutable consumer
    // scope once. Captured expression roots stay exact saved occurrences.
    for row in &mut observer.rows {
        normalize_place(
            program,
            semantic,
            contexts,
            state.symbol,
            statement_index,
            &mut row.place,
        );
    }
    observer
        .rows
        .sort_by(|left, right| left.place.compare_identity(&right.place));
    observer.rows
}

/// Give observations and queries the same coordinate representation. The
/// shared equality already compares Index with FixedIndex only when this same
/// expression-table constant query succeeds. Normalize those constants too,
/// so an unresolved Index can match only another unresolved Index.
pub(super) fn normalize_place(
    program: &dyn validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &mut CanonicalPlace,
) {
    super::super::narrow_pinned_index_segments(
        program,
        semantic,
        state_symbol,
        statement_index,
        contexts,
        &mut place.segments,
    );
    for segment in &mut place.segments {
        let facts::PlaceSegment::Index { expression } = *segment else {
            continue;
        };
        if !program.expression_table.expression_is_valid(expression) {
            continue;
        }
        if let Some(index) = program
            .expression_table
            .constant_integer_value(expression)
            .and_then(|value| usize::try_from(value).ok())
        {
            *segment = facts::PlaceSegment::FixedIndex { index };
        }
    }
}

pub(super) fn has_index(place: &CanonicalPlace) -> bool {
    place
        .segments
        .iter()
        .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
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
