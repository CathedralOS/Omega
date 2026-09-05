//! Domain predicates over exact, live assignment values. Mutation invalidation
//! owns their lifetime; this consumer neither replays a body nor assumes zeros.

use crate::flow::{
    CanonicalPlace, canonical_place_from_semantic_place, normalized_event_place_root,
    relative_place_segments_from_expression,
};
use psi_facts::{FactContextHandle, FactPayload, FactPlace, FactPlan, PlaceHandle};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

pub(super) fn prove_domain(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    place: PlaceHandle,
    domain: SymbolHandle,
) -> bool {
    let Some(subject) =
        canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
    else {
        return false;
    };
    AssignedValues {
        program,
        semantic,
        contexts,
    }
    .domain(&subject, domain, &mut Vec::new())
}

pub(in crate::checks::contracts) fn prove_domain_at_place(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    subject: &CanonicalPlace,
    domain: SymbolHandle,
) -> bool {
    contexts.iter().any(|context| {
        semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
            .any(|fact| {
                let candidate_domain = match fact.payload {
                    FactPayload::DomainMembership { domain_symbol, .. }
                    | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
                    _ => return false,
                };
                let FactPlace::Place(place) = fact.place else {
                    return false;
                };
                canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
                    .is_some_and(|candidate| {
                        normalized_event_place_root(program, candidate.root)
                            == normalized_event_place_root(program, subject.root)
                            && candidate.segments == subject.segments
                            && crate::field_domain::domain_membership_implies(
                                program,
                                candidate_domain,
                                domain,
                            )
                    })
            })
    }) || AssignedValues {
        program,
        semantic,
        contexts,
    }
    .domain(subject, domain, &mut Vec::new())
}

struct AssignedValues<'a> {
    program: &'a TypedTrees,
    semantic: &'a FactPlan,
    contexts: &'a [FactContextHandle],
}

impl AssignedValues<'_> {
    fn literal(&self, subject: &CanonicalPlace) -> Option<ExpressionHandle> {
        if let psi_facts::PlaceRoot::Expression(expression) = subject.root
            && subject.segments.is_empty()
            && matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::String(_)
            )
        {
            return Some(expression);
        }
        self.contexts.iter().find_map(|context| {
            self.semantic
                .context_view(self.semantic.contexts.get(*context))
                .facts()
                .find_map(|fact| {
                    let FactPayload::AssignedValue { value } = fact.payload else {
                        return None;
                    };
                    let FactPlace::Place(place) = fact.place else {
                        return None;
                    };
                    let candidate = canonical_place_from_semantic_place(
                        self.program,
                        self.semantic,
                        self.semantic.places.get(place),
                    )?;
                    (normalized_event_place_root(self.program, candidate.root)
                        == normalized_event_place_root(self.program, subject.root)
                        && candidate.segments == subject.segments)
                        .then_some(value)
                })
        })
    }

    fn relative_subject(
        &self,
        subject: &CanonicalPlace,
        expression: ExpressionHandle,
        type_symbol: Option<SymbolHandle>,
    ) -> Option<CanonicalPlace> {
        let segments =
            relative_place_segments_from_expression(self.program, expression, type_symbol)?;
        let mut selected = subject.clone();
        selected.extend_segments(&segments);
        Some(selected)
    }

    fn domain(
        &self,
        subject: &CanonicalPlace,
        symbol: SymbolHandle,
        active: &mut Vec<SymbolHandle>,
    ) -> bool {
        if !symbol.is_valid() || active.contains(&symbol) {
            return false;
        }
        let Some(domain) = self
            .program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == symbol)
        else {
            return false;
        };
        if let Some(literal) = self.literal(subject)
            && crate::field_domain::string_literal_expression_grants_domain(
                self.program,
                literal,
                symbol,
            )
        {
            return true;
        }
        if !domain.type_parameters.is_empty() || !domain.index_arguments.is_empty() {
            return false;
        }
        let facts = self.program.proof_facts.span_or_empty(domain.facts);
        if facts.is_empty() {
            return false;
        }
        let type_symbol = crate::lookup::machine_symbol_from_type_reference_handle(
            self.program,
            domain.target_type,
        );
        let type_symbol = type_symbol.is_valid().then_some(type_symbol);
        active.push(symbol);
        let proved = facts.iter().all(|fact| match fact {
            ProofFact::Expression(expression) => {
                self.boolean(subject, type_symbol, *expression) == Some(true)
            }
            ProofFact::Membership(membership) => self
                .relative_subject(subject, membership.value, type_symbol)
                .is_some_and(|nested| self.domain(&nested, membership.domain_symbol, active)),
            ProofFact::Proposition(_) => false,
        });
        active.pop();
        proved
    }

    fn integer(
        &self,
        subject: &CanonicalPlace,
        type_symbol: Option<SymbolHandle>,
        expression: ExpressionHandle,
    ) -> Option<i64> {
        self.program
            .expression_table
            .constant_integer_value(expression)
            .or_else(|| {
                let selected = self.relative_subject(subject, expression, type_symbol)?;
                self.program
                    .expression_table
                    .constant_integer_value(self.literal(&selected)?)
            })
    }

    fn boolean(
        &self,
        subject: &CanonicalPlace,
        type_symbol: Option<SymbolHandle>,
        expression: ExpressionHandle,
    ) -> Option<bool> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(*value),
            ExpressionNode::Binary(binary) => {
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    let left = self.boolean(subject, type_symbol, binary.left)?;
                    let right = self.boolean(subject, type_symbol, binary.right)?;
                    return Some(if binary.operator == BinaryOperator::And {
                        left && right
                    } else {
                        left || right
                    });
                }
                let left = self.integer(subject, type_symbol, binary.left)?;
                let right = self.integer(subject, type_symbol, binary.right)?;
                match binary.operator {
                    BinaryOperator::Equal => Some(left == right),
                    BinaryOperator::NotEqual => Some(left != right),
                    BinaryOperator::Less => Some(left < right),
                    BinaryOperator::LessOrEqual => Some(left <= right),
                    BinaryOperator::Greater => Some(left > right),
                    BinaryOperator::GreaterOrEqual => Some(left >= right),
                    _ => None,
                }
            }
            _ => {
                let selected = self.relative_subject(subject, expression, type_symbol)?;
                match self
                    .program
                    .expression_table
                    .expression(self.literal(&selected)?)
                {
                    ExpressionNode::Boolean(value) => Some(*value),
                    _ => None,
                }
            }
        }
    }
}
