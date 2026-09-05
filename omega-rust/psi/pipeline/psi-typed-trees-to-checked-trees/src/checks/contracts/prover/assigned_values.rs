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
use psi_typed_trees::expression::ExpressionHandle;

use super::scalars::{self, ScalarValue};
use crate::values::literal_at_place;
pub(in crate::checks::contracts) use crate::values::scalar_value_at_place;

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
        literal_at_place(
            self.program,
            self.semantic,
            self.contexts
                .iter()
                .map(|context| self.semantic.contexts.get(*context)),
            subject,
        )
    }
}

impl AssignedValues<'_> {
    /// Whether a live per-byte class proved for the whole carrier entails the
    /// requested domain. An indexed write retires the exact value snapshot but
    /// leaves this class behind, which is how a text carrier stays provable
    /// across `buffer[i] = byte`. A domain that is not exactly one recognized
    /// byte predicate is never reached this way.
    fn byte_predicate(&self, subject: &CanonicalPlace, symbol: SymbolHandle) -> bool {
        let Some(required) = crate::field_domain::domain_byte_predicate(self.program, symbol)
        else {
            return false;
        };
        self.contexts.iter().any(|context| {
            self.semantic
                .context_view(self.semantic.contexts.get(*context))
                .facts()
                .any(|fact| {
                    let FactPayload::BytePredicate { predicate } = fact.payload else {
                        return false;
                    };
                    let FactPlace::Place(place) = fact.place else {
                        return false;
                    };
                    predicate.implies(required)
                        && canonical_place_from_semantic_place(
                            self.program,
                            self.semantic,
                            self.semantic.places.get(place),
                        )
                        .is_some_and(|candidate| {
                            normalized_event_place_root(self.program, candidate.root)
                                == normalized_event_place_root(self.program, subject.root)
                                && candidate.segments == subject.segments
                        })
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
        if self.byte_predicate(subject, symbol) {
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

    fn boolean(
        &self,
        subject: &CanonicalPlace,
        type_symbol: Option<SymbolHandle>,
        expression: ExpressionHandle,
    ) -> Option<bool> {
        let value = scalars::evaluate_with_comparisons(
            self.program,
            expression,
            &mut |expression| {
                // Preserve this domain prover's existing pure constant vocabulary.
                // The shared evaluator itself never picks an arithmetic domain.
                self.program
                    .expression_table
                    .constant_integer_value(expression)
                    .map(|value| {
                        ScalarValue::Integer(psi_numerics::bignum::BigInt::from_i64(value))
                    })
                    .or_else(|| {
                        let selected = self.relative_subject(subject, expression, type_symbol)?;
                        scalar_value_at_place(
                            self.program,
                            self.semantic,
                            self.contexts
                                .iter()
                                .map(|context| self.semantic.contexts.get(*context)),
                            &selected,
                        )
                    })
            },
            &|left, right| {
                matches!(
                    (left, right),
                    (ScalarValue::Integer(_), ScalarValue::Integer(_))
                )
            },
        )?;
        match value {
            ScalarValue::Boolean(value) => Some(value),
            ScalarValue::Integer(_) | ScalarValue::Unknown => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_facts::{Fact, PlaceRoot, ProgramPoint};
    use psi_typed_trees::expression::{ExpressionNode, TableBinaryExpression};

    #[test]
    fn scalar_lookup_requires_live_exact_literal_evidence_not_initializers() {
        let tokens = psi_source_files_to_tokens::Lexer::new(
            "machine main() { let stored: u64 = 7; let other: u64 = 7; }",
        )
        .tokenize()
        .expect("tokenize");
        let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve");
        let mut program =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type");
        let state = &program.machine_states(&program.machines()[0])[0];
        let locals: Vec<_> = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .filter_map(|statement| match statement {
                psi_typed_trees::statement::StatementNode::LocalData(local) => {
                    Some((local.symbol, local.initial_value))
                }
                _ => None,
            })
            .collect();
        let subject = CanonicalPlace {
            root: PlaceRoot::Symbol(locals[0].0),
            segments: Vec::new(),
        };
        let other = CanonicalPlace {
            root: PlaceRoot::Symbol(locals[1].0),
            segments: Vec::new(),
        };
        let literal = locals[0].1;
        let mut semantic = FactPlan::default();
        let place = semantic.append_symbol_place(locals[0].0);
        let append = |semantic: &mut FactPlan, payload| {
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                payload,
                ..Fact::default()
            });
            let mut references = psi_arena::HandleSpan::empty();
            semantic.append_ref(&mut references, fact);
            semantic.append_context(ProgramPoint::Global, references)
        };
        let live = append(&mut semantic, FactPayload::AssignedValue { value: literal });
        assert_eq!(
            scalar_value_at_place(&program, &semantic, [], &subject),
            None
        );
        assert_eq!(
            scalar_value_at_place(&program, &semantic, [semantic.contexts.get(live)], &subject),
            Some(ScalarValue::Integer(
                psi_numerics::bignum::BigInt::from_i64(7)
            ))
        );
        assert_eq!(
            scalar_value_at_place(&program, &semantic, [semantic.contexts.get(live)], &other),
            None
        );
        let arithmetic =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: literal,
                    operator: psi_typed_trees::expression::BinaryOperator::Add,
                    right: literal,
                }));
        let nonliteral = append(
            &mut semantic,
            FactPayload::AssignedValue { value: arithmetic },
        );
        assert_eq!(
            scalar_value_at_place(
                &program,
                &semantic,
                [semantic.contexts.get(nonliteral)],
                &subject
            ),
            None
        );
        for value in [
            ExpressionHandle::invalid(),
            ExpressionHandle::from_parts(literal.arena_index(), literal.generation() + 1),
        ] {
            let invalid = append(&mut semantic, FactPayload::AssignedValue { value });
            assert_eq!(
                scalar_value_at_place(
                    &program,
                    &semantic,
                    [semantic.contexts.get(invalid)],
                    &subject
                ),
                None
            );
            assert_eq!(
                literal_at_place(
                    &program,
                    &semantic,
                    [semantic.contexts.get(invalid)],
                    &subject
                ),
                None
            );
        }
        let seven = semantic.scalar_values.append(ScalarValue::Integer(
            psi_numerics::bignum::BigInt::from_i64(7),
        ));
        let snapshot = append(
            &mut semantic,
            FactPayload::AssignedScalarValue { value: seven },
        );
        assert_eq!(
            scalar_value_at_place(
                &program,
                &semantic,
                [semantic.contexts.get(live), semantic.contexts.get(snapshot)],
                &subject
            ),
            Some(semantic.scalar_values.get(seven).clone())
        );
        let eight = semantic.scalar_values.append(ScalarValue::Integer(
            psi_numerics::bignum::BigInt::from_i64(8),
        ));
        let conflict = append(
            &mut semantic,
            FactPayload::AssignedScalarValue { value: eight },
        );
        assert_eq!(
            scalar_value_at_place(
                &program,
                &semantic,
                [
                    semantic.contexts.get(snapshot),
                    semantic.contexts.get(conflict)
                ],
                &subject
            ),
            None
        );
        for value in [
            psi_arena::Handle::invalid(),
            psi_arena::Handle::from_parts(seven.arena_index(), seven.generation() + 1),
        ] {
            let invalid = append(&mut semantic, FactPayload::AssignedScalarValue { value });
            assert_eq!(
                scalar_value_at_place(
                    &program,
                    &semantic,
                    [semantic.contexts.get(invalid)],
                    &subject
                ),
                None
            );
            assert_eq!(
                scalar_value_at_place(
                    &program,
                    &semantic,
                    [
                        semantic.contexts.get(snapshot),
                        semantic.contexts.get(invalid)
                    ],
                    &subject
                ),
                None
            );
        }
    }
}
