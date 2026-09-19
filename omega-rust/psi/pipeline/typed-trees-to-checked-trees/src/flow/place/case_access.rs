//! Case payload access consumes live tag evidence for the exact saved subject.
//! Conditional field contracts may move with a whole sum; extracting a payload
//! must discharge every case crossed by the source place, regardless of its
//! multiplicity. No initializer or call is replayed to recover an old tag.

use super::{
    CanonicalPlace, canonical_place_from_expression_in_state, canonical_place_from_semantic_place,
};
use facts::{FactContextHandle, FactPayload, FactPlace, FactPlan, PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator},
};

pub(crate) fn place_cases_are_selected(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: &CanonicalPlace,
) -> bool {
    subject
        .segments
        .iter()
        .enumerate()
        .all(|(position, segment)| {
            let PlaceSegment::Case { variant } = segment else {
                return true;
            };
            place_case_has_value(
                program,
                semantic,
                contexts,
                machine_symbol,
                state_symbol,
                statement_index,
                &CanonicalPlace {
                    root: subject.root,
                    segments: subject.segments[..position].to_vec(),
                },
                *variant,
                true,
            )
        })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn place_case_has_value(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: &CanonicalPlace,
    case: SymbolHandle,
    required: bool,
) -> bool {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        return false;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
    else {
        return false;
    };
    let query = CaseAccess {
        program,
        machine,
        state,
        statement_index,
    };
    if !stable_subject(subject) || !case.is_valid() {
        return false;
    }
    contexts.iter().any(|context| {
        semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
            .any(|fact| match fact.payload {
                FactPayload::BooleanValue { expression, value } => {
                    query.predicate(expression, value, subject, case, required, 0)
                }
                FactPayload::BooleanExpression(expression)
                | FactPayload::ContractBooleanExpression { expression, .. } => {
                    query.predicate(expression, true, subject, case, required, 0)
                }
                FactPayload::AssignedCase { variant } => {
                    let FactPlace::Place(place) = fact.place else {
                        return false;
                    };
                    canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic.places.get(place),
                    )
                    .as_ref()
                        == Some(subject)
                        && variant.is_valid()
                        && program.symbols.get(variant).kind == symbols::SymbolKind::Variant
                        && program.symbols.get(case).kind == symbols::SymbolKind::Variant
                        && program.symbols.get(variant).parent == program.symbols.get(case).parent
                        && (variant == case) == required
                }
                _ => false,
            })
    })
}

fn stable_subject(subject: &CanonicalPlace) -> bool {
    matches!(subject.root, PlaceRoot::Symbol(symbol) if symbol.is_valid())
        && subject.segments.iter().all(|segment| {
            matches!(segment, PlaceSegment::Field { symbol } if symbol.is_valid())
                || matches!(segment, PlaceSegment::Case { variant } if variant.is_valid())
                || matches!(segment, PlaceSegment::FixedIndex { .. })
        })
}

struct CaseAccess<'program> {
    program: &'program TypedTrees,
    machine: &'program typed_trees::machine::Machine,
    state: &'program typed_trees::state::State,
    statement_index: usize,
}

impl CaseAccess<'_> {
    fn predicate(
        &self,
        expression: ExpressionHandle,
        value: bool,
        subject: &CanonicalPlace,
        case: SymbolHandle,
        required: bool,
        depth: usize,
    ) -> bool {
        if depth >= 128
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return false;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                self.predicate(unary.operand, !value, subject, case, required, depth + 1)
            }
            ExpressionNode::Binary(binary)
                if (binary.operator == BinaryOperator::And && value)
                    || (binary.operator == BinaryOperator::Or && !value) =>
            {
                self.predicate(binary.left, value, subject, case, required, depth + 1)
                    || self.predicate(binary.right, value, subject, case, required, depth + 1)
            }
            ExpressionNode::Binary(binary)
                if binary.operator == BinaryOperator::Equal
                    && validation::has_builtin_decomposed_guard_meaning(
                        self.program,
                        self.machine,
                        Some(self.state),
                        expression,
                    )
                    && matches!(
                        self.program.expression_table.expression(binary.right),
                        ExpressionNode::Boolean(true)
                    ) =>
            {
                self.predicate(binary.left, value, subject, case, required, depth + 1)
            }
            ExpressionNode::Binary(binary)
                if binary.operator == BinaryOperator::Equal
                    && validation::has_builtin_decomposed_guard_meaning(
                        self.program,
                        self.machine,
                        Some(self.state),
                        expression,
                    )
                    && matches!(
                        self.program.expression_table.expression(binary.left),
                        ExpressionNode::Boolean(true)
                    ) =>
            {
                self.predicate(binary.right, value, subject, case, required, depth + 1)
            }
            ExpressionNode::Binary(binary)
                if validation::has_exact_case_membership_meaning(
                    self.program,
                    self.machine,
                    Some(self.state),
                    expression,
                    binary,
                ) =>
            {
                let ExpressionNode::Name(observed) =
                    self.program.expression_table.expression(binary.right)
                else {
                    return false;
                };
                canonical_place_from_expression_in_state(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    binary.left,
                )
                .as_ref()
                    == Some(subject)
                    && self.program.symbols.get(observed.symbol).parent
                        == self.program.symbols.get(case).parent
                    && if value {
                        (observed.symbol == case) == required
                    } else {
                        observed.symbol == case && !required
                    }
            }
            _ => false,
        }
    }
}
