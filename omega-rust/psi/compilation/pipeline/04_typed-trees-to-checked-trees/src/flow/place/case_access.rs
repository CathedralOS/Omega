//! Case payload access consumes live tag evidence for the exact saved subject.
//! Conditional field contracts may move with a whole sum; extracting a payload
//! must discharge every case crossed by the source place, regardless of its
//! multiplicity. No initializer or call is replayed to recover an old tag.

use validation::{CanonicalPlace, canonical_place_from_expression_in_state};

use super::canonical_place_from_semantic_place;
use facts::{FactContextHandle, FactPayload, FactPlan, PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};

/// `place_case_has_value` for the Case segment at `position`, evaluated on
/// the subject truncated just before that segment.
#[allow(clippy::too_many_arguments)]
fn case_segment_value(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: &CanonicalPlace,
    position: usize,
    variant: SymbolHandle,
    expected: bool,
) -> bool {
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
        variant,
        expected,
    )
}

pub(crate) fn place_cases_are_selected(
    program: &dyn ::validation::CheckingInput,
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
            case_segment_value(
                program,
                semantic,
                contexts,
                machine_symbol,
                state_symbol,
                statement_index,
                subject,
                position,
                *variant,
                true,
            )
        })
}

/// A whole value owes a payload invariant only when its enclosing cases are
/// active. Discharge the conditional obligation from a live contrary tag, not
/// from a missing payload fact. This grants no permission to read the payload:
/// reads still require every enclosing case to be positively selected.
pub(crate) fn place_case_path_is_inactive(
    program: &dyn ::validation::CheckingInput,
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
        .any(|(position, segment)| {
            let PlaceSegment::Case { variant } = segment else {
                return false;
            };
            case_segment_value(
                program,
                semantic,
                contexts,
                machine_symbol,
                state_symbol,
                statement_index,
                subject,
                position,
                *variant,
                false,
            )
        })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn place_case_has_value(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: &CanonicalPlace,
    case: SymbolHandle,
    required: bool,
) -> bool {
    let Some(machine) = program.machine_by_symbol(machine_symbol) else {
        return false;
    };
    let Some(state) = program.state_in_machine(machine, state_symbol) else {
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
    let has_case_evidence = |case, required| {
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
                        let Some(place) = fact.place.as_place() else {
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
                            && program.symbols.get(variant).parent
                                == program.symbols.get(case).parent
                            && (variant == case) == required
                    }
                    _ => false,
                })
        })
    };
    if has_case_evidence(case, required) {
        return true;
    }
    if !required || program.symbols.get(case).kind != symbols::SymbolKind::Variant {
        return false;
    }
    let Some(definition) = program.data_definition_by_symbol(program.symbols.get(case).parent)
    else {
        return false;
    };
    // An established finite sum has exactly one active case. Excluding every
    // other declared case establishes this one, but each exclusion must come
    // from the same live subject. Missing evidence is not a negative fact.
    // Reuse the direct judgment rather than recursively inferring exclusions.
    let mut declared = false;
    let mut excluded_alternative = false;
    for member in program.data_members(definition) {
        let typed_trees::data::DataMember::Variant(alternative) = member else {
            continue;
        };
        if alternative.symbol == case {
            declared = true;
        } else {
            if !has_case_evidence(alternative.symbol, false) {
                return false;
            }
            excluded_alternative = true;
        }
    }
    declared && excluded_alternative
}

fn stable_subject(subject: &CanonicalPlace) -> bool {
    // An evaluated temporary is an identity too. Live facts name its exact
    // occurrence; matching them neither replays its producer nor transfers a
    // selected tag to another equal-looking call or constructor.
    (matches!(subject.root, PlaceRoot::Symbol(symbol) if symbol.is_valid())
        || matches!(subject.root, PlaceRoot::Expression(expression) if expression.is_valid()))
        && subject.segments.iter().all(|segment| {
            matches!(segment, PlaceSegment::Field { symbol } if symbol.is_valid())
                || matches!(segment, PlaceSegment::Case { variant } if variant.is_valid())
                || matches!(segment, PlaceSegment::FixedIndex { .. })
        })
}

struct CaseAccess<'program> {
    program: &'program dyn ::validation::CheckingInput,
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
        if self
            .program
            .expression_table
            .expression_at_depth(expression, depth)
            .is_none()
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
                let Some(observed) = self.program.expression_table.name_path_node(binary.right)
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
