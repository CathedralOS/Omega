/*
    Whether fact contexts prove a contract proposition. The contract checks
    ask through three entries: `assembly` and `exits` use the first, `calls`
    the second, and `arrivals`, `calls` and `exits` the third.

    - `semantic_contexts_prove_boolean_expression`: a Boolean expression
      holds in some context, by `booleans` (conjunction and disjunction over
      the `direct` and `domains` provers).
    - `call_entry_contexts_prove_boolean_contract_expression`: a callee's
      Boolean contract expression at one call site. It tries, in order, a
      caller's case at the place a case test's subject names, Boolean field
      facts on the actual's
      referent (`field_actuals`), the instantiated expression in each entry
      context (`booleans`, skipped when `field_actuals` recognized the
      requirement), and last the expression evaluated with the call's
      arguments substituted (`evaluator`). Completed normal-call guarantees
      belong to `calls::captured_boolean` before its source-replay fence; these
      source-substitution fallbacks cannot replace a failed capture judgment.
    - `semantic_contexts_prove_contract_fact`: one contract fact, by payload
      kind: domain membership (a matching or covering fact, or
      `assigned_values`), carry permission, Boolean expression, proposition
      application or carry origin. Evidence and deferred payloads are never
      proved here.

    `assigned_values` (domain predicates over live assigned values) and
    `scalars` (scalar Boolean evaluation) are re-exported for the other
    contract checks, and `call_guarantees` is also used by `checks::borrows`.
*/

use crate::published::facts::boolean_predicates;
use crate::published::places::place_at;
use checked_trees::{FlowCallFact, FlowStateFact};
use facts::FactPayload;

mod assigned_values {
    /*
        Domain predicates over exact, live assignment values. Mutation invalidation
        owns their lifetime; this consumer neither replays a body nor assumes zeros.
    */
    use crate::flow::canonical_places_agree;
    use language_semantics::SemanticDomainId;
    use numerics::bignum::BigInt;
    use typed_trees::byte_predicates::literal_bytes;
    use validation::field_domains::ByteSequencePredicate;
    use validation::field_domains::domain_byte_predicate;
    use validation::field_domains::domain_is_subslice_preserving;
    use validation::field_domains::domain_is_vacuous;
    use validation::field_domains::domain_membership_implies;
    use validation::field_domains::string_literal_expression_grants_domain;
    use validation::{
        CanonicalPlace, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
        closed_collection_length_at_place, integer_domain_membership,
    };

    use crate::flow::expressions;
    use crate::flow::{
        canonical_place_from_semantic_place, relative_place_segments_from_expression,
    };
    use facts::{
        Fact, FactContextHandle, FactPayload, FactPlan, PlaceHandle, PlaceSegment, ProgramPoint,
        call_point_triple_pattern,
    };
    use symbols::SymbolHandle;
    use typed_trees::domain::{DomainDefinition, ProofFact};
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    use super::scalars::{self, ScalarValue};
    use crate::values::literal_at_place;
    pub(in crate::checks::contracts) use crate::values::scalar_value_at_place;

    pub(super) fn prove_domain(
        program: &dyn ::validation::CheckingInput,
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
        prove_domain_at_place(program, semantic, contexts, &subject, domain)
    }

    pub(in crate::checks::contracts) use crate::flow::segments_cover_subject;

    /*
        Range and dynamic-selector obligations prove every possible element in
        declaration-derived geometry.
        Recorded facts supply membership, never the collection's extent: a
        contiguous prefix of writes can still leave later cells uninitialized.
    */
    fn prove_domain_by_extent_enumeration(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        subject: &CanonicalPlace,
        domain: SymbolHandle,
    ) -> bool {
        let Some(position) = subject.segments.iter().position(|segment| {
            matches!(segment, PlaceSegment::FixedRange { .. })
                || matches!(segment, PlaceSegment::Index { expression }
                        if !matches!(program.expression_table.expression(*expression),
                            ExpressionNode::Range(_)))
        }) else {
            return false;
        };
        let (start, end) = match subject.segments[position] {
            PlaceSegment::FixedRange { start, end } => (start, end),
            PlaceSegment::Index { .. } => (0, usize::MAX),
            _ => return false,
        };
        let end = if start == 0 && end == usize::MAX {
            let mut collection = subject.clone();
            collection.segments.truncate(position);
            let Some(length) = contexts.iter().find_map(|context| {
                let point = semantic.contexts.get(*context).point;
                let state = match point {
                    ProgramPoint::State { state_symbol, .. }
                    | ProgramPoint::Statement { state_symbol, .. }
                    | call_point_triple_pattern!(state_symbol, ..)
                    | ProgramPoint::TransitionArm { state_symbol, .. }
                    | ProgramPoint::Exit { state_symbol, .. } => state_symbol,
                    _ => return None,
                };
                closed_collection_length_at_place(program, state, &collection)
                    .and_then(|length| usize::try_from(length).ok())
            }) else {
                return false;
            };
            length
        } else {
            end
        };
        start < end
            && (start..end).all(|index| {
                let mut element = subject.clone();
                element.segments[position] = PlaceSegment::FixedIndex { index };
                prove_domain_at_place(program, semantic, contexts, &element, domain)
            })
    }

    /*
        One walk over the live facts in the selected contexts: `accepts` judges the
        payload, `at_subject` the fact's own place. Membership, permission and
        carried-origin obligations are payload judgments over this one walk; none of
        them rebuilds the context/view/fact scan for itself.
    */
    pub(super) fn live_fact_at_place(
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        accepts: impl Fn(&Fact) -> bool,
        at_subject: impl Fn(PlaceHandle) -> bool,
    ) -> bool {
        contexts.iter().any(|context| {
            semantic
                .context_view(semantic.contexts.get(*context))
                .facts()
                .any(|candidate| {
                    accepts(candidate) && candidate.place.as_place().is_some_and(&at_subject)
                })
        })
    }

    /*
        The membership specialization of the same walk: the payload supplies the
        carried fact's declaration and interned instance, `satisfies` decides
        whether that membership answers the obligation at hand.
    */
    pub(super) fn carried_membership_at(
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        satisfies: impl Fn(SymbolHandle, SemanticDomainId) -> bool,
        at_subject: impl Fn(PlaceHandle) -> bool,
    ) -> bool {
        live_fact_at_place(
            semantic,
            contexts,
            |candidate| {
                candidate.payload.domain_membership().is_some_and(
                    |(_, _, candidate_symbol, candidate_instance)| {
                        satisfies(candidate_symbol, candidate_instance)
                    },
                )
            },
            at_subject,
        )
    }

    /*
        Whether a carried fact at `candidate` covers the `subject` elementwise:
        same canonical root, and at every position either equal segments or a
        containing `FixedRange` (the whole-extent `0..usize::MAX` row covers a
        runtime `Index`; a finite range covers contained indices -- see
        [`segments_cover_subject`]).
    */
    fn covers_canonical_subject(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        candidate: PlaceHandle,
        subject: &CanonicalPlace,
    ) -> bool {
        canonical_place_from_semantic_place(program, semantic, semantic.places.get(candidate))
            .is_some_and(|candidate| {
                candidate.root == subject.root
                    && segments_cover_subject(program, &candidate.segments, &subject.segments)
            })
    }

    /*
        Whether a carried membership fact on `subject` (a declared constraint or
        established contract evidence) already implies `domain`. This is the
        evidence arm of [`prove_domain_at_place`]: routed provenance and prior
        `ensures`/`in`-constrained declarations answer membership questions that
        predicate discharge cannot reach.
    */
    fn carried_domain_membership_at_place(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        subject: &CanonicalPlace,
        domain: SymbolHandle,
    ) -> bool {
        carried_membership_at(
            semantic,
            contexts,
            |candidate, _| domain_membership_implies(program, candidate, domain),
            |candidate| covers_canonical_subject(program, semantic, candidate, subject),
        )
    }

    pub(in crate::checks::contracts) fn prove_domain_at_place(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        subject: &CanonicalPlace,
        domain: SymbolHandle,
    ) -> bool {
        carried_domain_membership_at_place(program, semantic, contexts, subject, domain)
            || subslice_domain_at_place(program, semantic, contexts, subject, domain)
            || prove_domain_by_extent_enumeration(program, semantic, contexts, subject, domain)
            || AssignedValues {
                program,
                semantic,
                contexts,
            }
            .domain(subject, domain, SemanticDomainId::NULL, &mut Vec::new())
    }

    /*
        Membership of `subject` in one EXACT indexed (const-generic) instance.

        [`prove_domain_at_place`] answers membership questions by declaration
        symbol, which is the whole theory of an atomic domain but not of a family:
        `BelowIndex<4>` and `BelowIndex<9>` share a symbol and have no variance
        relationship, so a fact about one must not satisfy an obligation for the
        other. [`domain_membership_implies`] correctly
        refuses that reduction, so an indexed requirement matches the carried fact's
        own interned instance identity instead of its family symbol.

        Only live membership evidence is consulted. A declared constraint, a
        completed call's `ensures`, or an established initializer each record their
        exact instance; nothing here re-derives an instance from a type annotation
        or a predicate that could hold for several applications.
    */
    pub(in crate::checks::contracts) fn prove_indexed_domain_at_place(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        subject: &CanonicalPlace,
        domain: SymbolHandle,
        instance: SemanticDomainId,
    ) -> bool {
        instance.is_valid()
            && carried_membership_at(
                semantic,
                contexts,
                |candidate_domain, candidate_instance| {
                    candidate_domain == domain && candidate_instance == instance
                },
                |candidate| covers_canonical_subject(program, semantic, candidate, subject),
            )
    }

    /*
        Per-byte predicates survive a window; UTF-8 and nonempty do not. Strip
        exactly one authenticated range selector, then prove the live base through
        the same occurrence facts used for every other place. Bounds and custody
        remain obligations of the range operation.
    */
    fn subslice_domain_at_place(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        subject: &CanonicalPlace,
        domain: SymbolHandle,
    ) -> bool {
        if !domain_is_subslice_preserving(program, domain) {
            return false;
        }
        let Some((selection, segments)) = subject.segments.split_last() else {
            return false;
        };
        let is_window = match selection {
            PlaceSegment::FixedRange { .. } | PlaceSegment::OpaqueRange => true,
            PlaceSegment::Index { expression } => matches!(
                program.expression_table.expression(*expression),
                ExpressionNode::Range(_)
            ),
            _ => false,
        };
        is_window
            && prove_domain_at_place(
                program,
                semantic,
                contexts,
                &CanonicalPlace {
                    root: subject.root,
                    segments: segments.to_vec(),
                },
                domain,
            )
    }

    struct AssignedValues<'a> {
        program: &'a dyn ::validation::CheckingInput,
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

    /*
        Whether a live per-byte class fact at `subject` implies `required`. The one
        scan behind both consumers of minted `FactPayload::BytePredicate` evidence:
        a domain reduced to exactly one recognized byte predicate proves
        membership through it, and a `requires pred(place)` conjunct or a
        `pred(self)` initializer obligation discharges against it — identical
        place-equality plus `implies` semantics in every consumer.
    */
    pub(in crate::checks::contracts) fn byte_predicate_proven_at_place(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        subject: &CanonicalPlace,
        required: ByteSequencePredicate,
    ) -> bool {
        if literal_at_place(
            program,
            semantic,
            contexts
                .iter()
                .map(|context| semantic.contexts.get(*context)),
            subject,
        )
        .and_then(|literal| literal_bytes(program, literal))
        .is_some_and(|bytes| required.holds_for(&bytes))
        {
            return true;
        }
        contexts.iter().any(|context| {
            semantic
                .context_view(semantic.contexts.get(*context))
                .facts()
                .any(|fact| {
                    let FactPayload::BytePredicate { predicate } = fact.payload else {
                        return false;
                    };
                    let Some(place) = fact.place.as_place() else {
                        return false;
                    };
                    predicate.implies(required)
                        && canonical_place_from_semantic_place(
                            program,
                            semantic,
                            semantic.places.get(place),
                        )
                        .is_some_and(|candidate| {
                            canonical_places_agree(program, &candidate, subject)
                        })
                })
        })
    }

    impl AssignedValues<'_> {
        /*
            Whether a live per-byte class proved for the whole carrier entails the
            requested domain. An indexed write retires the exact value snapshot but
            leaves this class behind, which is how a text carrier stays provable
            across `buffer[i] = byte`. A domain that is not exactly one recognized
            byte predicate is never reached this way.
        */
        fn byte_predicate(&self, subject: &CanonicalPlace, symbol: SymbolHandle) -> bool {
            let Some(required) = domain_byte_predicate(self.program, symbol) else {
                return false;
            };
            byte_predicate_proven_at_place(
                self.program,
                self.semantic,
                self.contexts,
                subject,
                required,
            )
        }

        fn relative_subject(
            &self,
            subject: &CanonicalPlace,
            expression: ExpressionHandle,
            domain: &DomainDefinition,
        ) -> Option<CanonicalPlace> {
            let segments =
                relative_place_segments_from_expression(self.program, expression, domain)?;
            let mut selected = subject.clone();
            selected.extend_segments(&segments);
            Some(selected)
        }

        fn domain(
            &self,
            subject: &CanonicalPlace,
            symbol: SymbolHandle,
            instance: SemanticDomainId,
            active: &mut Vec<SymbolHandle>,
        ) -> bool {
            if !symbol.is_valid() || active.contains(&symbol) {
                return false;
            }
            let Some(domain) = self
                .program
                .domain_definition_for_instance(symbol, instance)
                .or_else(|| self.program.domain_definition(symbol))
            else {
                return false;
            };
            if let Some(literal) = self.literal(subject)
                && string_literal_expression_grants_domain(self.program, literal, symbol)
            {
                return true;
            }
            if self.byte_predicate(subject, symbol) {
                return true;
            }
            if !domain.type_parameters.is_empty() || !domain.index_arguments.is_empty() {
                return false;
            }
            if self
                .program
                .primitive_type_reference(domain.target_type)
                .is_some_and(|primitive| primitive.is_fixed_integer())
                && let Some(ScalarValue::Integer(value)) = scalar_value_at_place(
                    self.program,
                    self.semantic,
                    self.contexts
                        .iter()
                        .map(|context| self.semantic.contexts.get(*context)),
                    subject,
                )
            {
                match integer_domain_membership(
                    self.program,
                    symbol,
                    &[],
                    domain.semantic_id,
                    &ScopedArithmeticValue::Integer(value),
                ) {
                    StrictArithmeticImplicationJudgment::Proven => return true,
                    StrictArithmeticImplicationJudgment::Refuted => return false,
                    StrictArithmeticImplicationJudgment::Unknown => {}
                }
            }
            if !domain.establishment_routes.is_empty() {
                return false;
            }
            let facts = self.program.proof_facts.span_or_empty(domain.facts);
            if facts.is_empty() {
                return domain_is_vacuous(self.program, symbol, &mut Vec::new());
            }
            active.push(symbol);
            let proved = facts.iter().all(|fact| match fact {
                ProofFact::Expression(expression) => {
                    self.boolean(subject, domain, *expression) == Some(true)
                }
                ProofFact::Membership(membership) => self
                    .relative_subject(subject, membership.value, domain)
                    .is_some_and(|nested| {
                        carried_domain_membership_at_place(
                            self.program,
                            self.semantic,
                            self.contexts,
                            &nested,
                            membership.domain_symbol,
                        ) || self.domain(
                            &nested,
                            membership.domain_symbol,
                            membership.semantic_domain,
                            active,
                        )
                    }),
                ProofFact::Proposition(_) => false,
            });
            active.pop();
            proved
        }

        /*
            The place's installed nominal tag, when a live assignment fact has
            decided it. `let x = T::C{...}` mints one; nothing else does.
        */
        fn installed_case(&self, subject: &CanonicalPlace) -> Option<SymbolHandle> {
            self.contexts.iter().find_map(|context| {
                self.semantic
                    .context_view(self.semantic.contexts.get(*context))
                    .facts()
                    .find_map(|fact| {
                        let FactPayload::CaseValue {
                            variant,
                            value: true,
                        } = fact.payload
                        else {
                            return None;
                        };
                        let place = fact.place.as_place()?;
                        let candidate = canonical_place_from_semantic_place(
                            self.program,
                            self.semantic,
                            self.semantic.places.get(place),
                        )?;
                        canonical_places_agree(self.program, &candidate, subject).then_some(variant)
                    })
            })
        }

        fn boolean(
            &self,
            subject: &CanonicalPlace,
            domain: &DomainDefinition,
            expression: ExpressionHandle,
        ) -> Option<bool> {
            if let Some((member, case)) =
                crate::published::proof::exact_outcome_case_test(self.program, expression)
                && let Some(selected) = self.relative_subject(subject, member, domain)
            {
                return self
                    .installed_case(&selected)
                    .map(|variant| variant == case);
            }
            let value = scalars::evaluate(
                self.program,
                expression,
                &mut |expression| {
                    expressions(self.program)
                        .constant_integer_value(expression)
                        .map(|value| ScalarValue::Integer(BigInt::from_i64(value)))
                        .or_else(|| {
                            let selected = self.relative_subject(subject, expression, domain)?;
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
}
pub(super) use assigned_values::{
    byte_predicate_proven_at_place, prove_domain_at_place, prove_indexed_domain_at_place,
    scalar_value_at_place, segments_cover_subject,
};
pub(super) mod captured_values {
    /*
        Scalar observations of completed values share one copy/view law.
        Normal-result proofs project the retained operand tree, not source
        initializers. Copies keep their completion facts; references observe
        captured backing at the consumer's live frontier. Every possible captured
        value and referent must establish the same scalar independently.
    */
    use checked_trees::{CheckFacts, FlowConstraintRef, FlowOperandFact, FlowStateFact};

    /*
        `consumer_statement` is the first statement frontier after this completed
        consumption, including its nested calls. Its constraints exclude the goal.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::checks::contracts) fn scalar_at_path(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        capture: &FlowOperandFact,
        path: &[facts::PlaceSegment],
        consumer_statement: usize,
        invocation: arena::HandleSpan<FlowConstraintRef>,
        frames: &validation::CallFrameResolver<'_>,
    ) -> Option<facts::ScalarValue> {
        let values = crate::flow::captured_operand_values(
            program,
            facts.flow.view(),
            &facts.semantic,
            state,
            crate::flow::CapturedOperand::unrecorded(capture),
            capture.type_reference,
            path,
        )?;
        let mut agreed = None;
        for value in values {
            let scalar = [
                (
                    value.observation,
                    value.observation_type,
                    crate::flow::captured_observation_places(program, facts.flow.view(), &value),
                    invocation,
                    value.observation,
                    false,
                ),
                (
                    value.operand,
                    value.type_reference,
                    crate::flow::captured_value_places(program, facts.flow.view(), &value),
                    value.producer_invocation(invocation),
                    value.producer_epoch.unwrap_or(value.operand),
                    value.producer_epoch.is_some(),
                ),
            ]
            .into_iter()
            .find_map(|(operand, reference, places, invocation, epoch, frozen)| {
                let contexts = crate::flow::operand_contexts(
                    program,
                    &facts.flow.contexts,
                    &facts.flow.control,
                    &facts.semantic,
                    &operand,
                    reference,
                    invocation,
                );
                let detached = frozen || crate::flow::operand_binds_stable_copy(program, reference);
                let statement = if detached {
                    epoch.statement_index.checked_add(1)?
                } else {
                    consumer_statement
                };
                let predicates = crate::published::facts::boolean_predicates::retained(
                    program,
                    &facts.operators,
                    &facts.semantic,
                    &contexts,
                    facts::ProgramPoint::Statement {
                        machine_symbol: state.machine_symbol,
                        state_symbol: if detached {
                            epoch.state_symbol
                        } else {
                            state.state_symbol
                        },
                        statement_index: statement,
                    },
                );
                let mut candidate = None;
                for place in places? {
                    let scalar = crate::values::scalar_value_at_place(
                        program,
                        &facts.semantic,
                        contexts
                            .iter()
                            .map(|context| facts.semantic.contexts.get(*context)),
                        &place.projected,
                    )
                    .or_else(|| {
                        super::boolean_value_at_place(
                            program,
                            &facts.operators,
                            &facts.semantic,
                            &contexts,
                            facts::ProgramPoint::Statement {
                                machine_symbol: state.machine_symbol,
                                state_symbol: if detached {
                                    epoch.state_symbol
                                } else {
                                    state.state_symbol
                                },
                                statement_index: statement,
                            },
                            &place.projected,
                        )
                        .map(facts::ScalarValue::Boolean)
                    })
                    .or_else(|| {
                        predicates.iter().find_map(|predicate| {
                            predicate.constant_at_place(&place.projected, |operand| {
                                crate::values::scalar_value_at_place(
                                    program,
                                    &facts.semantic,
                                    contexts
                                        .iter()
                                        .map(|context| facts.semantic.contexts.get(*context)),
                                    operand,
                                )
                            })
                        })
                    })
                    .or_else(|| {
                        super::call_guarantees::scalar_at_place(
                            program,
                            facts,
                            state,
                            statement,
                            &contexts,
                            frames,
                            &place.projected,
                        )
                    })?;
                    if candidate
                        .as_ref()
                        .is_some_and(|previous| *previous != scalar)
                    {
                        return None;
                    }
                    candidate = Some(scalar);
                }
                candidate
            })?;
            if agreed.as_ref().is_some_and(|previous| *previous != scalar) {
                return None;
            }
            agreed = Some(scalar);
        }
        agreed
    }
}
mod scalars {
    /*
        The shared scalar predicate judgment, used both by source proof contexts
        and retained constant obligations when a selected target closes their leaves.
    */
    pub(in crate::checks::contracts) use checked_trees::checked_trees::facts::scalar_predicates::{
        ScalarValue, closed_boolean_value, evaluate, evaluate_for_atoms, has_builtin_operators,
    };
}
pub(super) use crate::values::evaluate_checked_scalar;
pub(super) use captured_values::scalar_at_path as captured_scalar_at_path;
pub(super) use scalars::{
    ScalarValue, closed_boolean_value, evaluate as evaluate_scalar, evaluate_for_atoms,
    has_builtin_operators,
};
mod booleans {
    use crate::published::facts::boolean_predicates::CasePremiseAvailability;
    use symbols::SymbolHandle;

    use super::super::labels::ContractTargetParameters;

    use super::super::direct::{
        direct_context_proves_boolean_expression,
        direct_context_proves_instantiated_boolean_expression,
    };
    use super::super::domains::{
        prove_boolean_expression_via_context_domain_membership,
        prove_instantiated_boolean_expression_via_context_domain_membership,
    };

    pub(super) fn semantic_context_proves_boolean_expression(
        program: &dyn ::validation::CheckingInput,
        semantic: &facts::FactPlan,
        context: &facts::FactContext,
        contexts: &[facts::FactContextHandle],
        point: facts::ProgramPoint,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        macro_rules! prove_boolean {
            ($expression:expr) => {
                semantic_context_proves_boolean_expression(
                    program,
                    semantic,
                    context,
                    contexts,
                    point,
                    $expression,
                )
            };
        }
        if direct_context_proves_boolean_expression(program, semantic, context, expression) {
            return true;
        }

        match program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Borrow(inner) => {
                prove_boolean!(inner.target)
            }
            typed_trees::expression::ExpressionNode::Binary(binary) => match binary.operator {
                typed_trees::expression::BinaryOperator::And => {
                    prove_boolean!(binary.left) && prove_boolean!(binary.right)
                }
                typed_trees::expression::BinaryOperator::Or => {
                    prove_boolean!(binary.left) || prove_boolean!(binary.right)
                }
                _ => prove_boolean_expression_via_context_domain_membership(
                    program, semantic, context, contexts, point, expression,
                ),
            },
            _ => prove_boolean_expression_via_context_domain_membership(
                program, semantic, context, contexts, point, expression,
            ),
        }
    }

    pub(super) fn semantic_context_proves_instantiated_boolean_expression<'premises>(
        program: &'premises dyn ::validation::CheckingInput,
        facts: &'premises checked_trees::CheckFacts,
        call_flow: &checked_trees::FlowCallFact,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        context: &facts::FactContext,
        contexts: &'premises [facts::FactContextHandle],
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call_site: &validation::CallSite<'_>,
        target_state: &(impl ContractTargetParameters + ?Sized),
        target_state_symbol: SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
        availability: &std::cell::OnceCell<Option<CasePremiseAvailability<'premises>>>,
        assignments: &mut crate::values::RootAssignmentObservations<'premises>,
    ) -> bool {
        let semantic = &facts.semantic;
        macro_rules! prove_instantiated {
            ($expression:expr) => {
                semantic_context_proves_instantiated_boolean_expression(
                    program,
                    facts,
                    call_flow,
                    call_frames,
                    context,
                    contexts,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    target_state_symbol,
                    $expression,
                    availability,
                    assignments,
                )
            };
        }
        if direct_context_proves_instantiated_boolean_expression(
            program,
            facts,
            call_flow,
            call_frames,
            context,
            contexts,
            caller_state_symbol,
            statement_index,
            call_site,
            target_state,
            target_state_symbol,
            expression,
            assignments,
        ) {
            return true;
        }

        match program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Borrow(inner) => {
                prove_instantiated!(inner.target)
            }
            typed_trees::expression::ExpressionNode::Binary(binary) => match binary.operator {
                typed_trees::expression::BinaryOperator::And => [binary.left, binary.right]
                    .iter()
                    .all(|operand| prove_instantiated!(*operand)),
                typed_trees::expression::BinaryOperator::Or => [binary.left, binary.right]
                    .iter()
                    .any(|operand| prove_instantiated!(*operand)),
                _ => prove_instantiated_boolean_expression_via_context_domain_membership(
                    program,
                    semantic,
                    context,
                    contexts,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    expression,
                    availability,
                ),
            },
            _ => prove_instantiated_boolean_expression_via_context_domain_membership(
                program,
                semantic,
                context,
                contexts,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                expression,
                availability,
            ),
        }
    }
}
mod place_predicates {
    /*
        Semantic observations over one exact place. The caller supplies the
        observation's value epoch; these queries never select or merge epochs.
    */
    use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
    use typed_trees::expression::ExpressionHandle;

    pub(in crate::checks::contracts) enum PlacePredicate {
        Bytes(validation::field_domains::ByteSequencePredicate),
        Case(symbols::SymbolHandle),
    }

    pub(in crate::checks::contracts) fn place_predicate(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<(PlacePredicate, ExpressionHandle)> {
        if let Some((predicate, subject)) =
            typed_trees::byte_predicates::byte_predicate_call(program, expression)
        {
            return Some((PlacePredicate::Bytes(predicate), subject));
        }
        crate::published::proof::exact_outcome_case_test(program, expression)
            .map(|(subject, case)| (PlacePredicate::Case(case), subject))
    }

    impl PlacePredicate {
        /*
            A descriptor guard describes the selected completed value. Rejoin that
            capture before trying its logical observation namespace; physical view
            candidates remain separate obligations when no such relation exists.
        */
        pub(in crate::checks::contracts) fn proven_for_capture(
            &self,
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            state: &FlowStateFact,
            call: &FlowCallFact,
            contexts: &[facts::FactContextHandle],
            captured: &crate::flow::CapturedValue<'_>,
        ) -> bool {
            let (source_state, source_statement) =
                captured.producer_frontier(state.state_symbol, call.statement_index);
            let point = facts::ProgramPoint::Statement {
                machine_symbol: state.machine_symbol,
                state_symbol: source_state,
                statement_index: source_statement,
            };
            if let Some(namespace) = crate::flow::descriptor_observation_namespace(
                program,
                &facts.flow.control,
                captured.operand.row(),
                contexts,
            )
            .and_then(|namespace| {
                crate::flow::project_captured_view_place(namespace, &captured.projection.remaining)
            }) && self.proven(program, facts, point, contexts, &namespace)
            {
                return true;
            }
            let proven = crate::flow::captured_value_places(program, facts.flow.view(), captured)
                .is_some_and(|places| {
                    !places.is_empty()
                        && places.iter().all(|place| {
                            self.proven(program, facts, point, contexts, &place.projected)
                        })
                });
            if !proven {
                match crate::checks::borrows::shared_reference_observation(
                    program, facts, state, call, captured,
                ) {
                    Ok(source) => {
                        let mut source_contexts = facts
                            .flow
                            .contexts
                            .semantic_constraint_contexts(source.operand.constraints)
                            .collect::<Vec<_>>();
                        let source_context_count = source_contexts.len();
                        source_contexts.retain(|context| {
                            crate::flow::descriptor_context_agrees(
                                program,
                                &facts.flow.control,
                                source.operand.row(),
                                *context,
                            )
                        });
                        if crate::flow::captured_value_places(program, facts.flow.view(), &source)
                            .is_some_and(|places| {
                                !places.is_empty()
                                    && places.iter().all(|place| {
                                        self.proven(
                                            program,
                                            facts,
                                            facts::ProgramPoint::Statement {
                                                machine_symbol: state.machine_symbol,
                                                state_symbol: source.operand.state_symbol,
                                                statement_index: source.operand.statement_index,
                                            },
                                            &source_contexts,
                                            &place.projected,
                                        )
                                    })
                            })
                        {
                            return true;
                        }
                        if ::validation::trace_flags::check_sites() {
                            eprintln!(
                                "CHECKSITE saved-reference-source-predicate-refusal source={:?} state={:?} statement={} source_context_count={} qualified_context_count={} qualified_contexts={:?}",
                                source.operand.expression,
                                source.operand.state_symbol,
                                source.operand.statement_index,
                                source_context_count,
                                source_contexts.len(),
                                &source_contexts[..source_contexts.len().min(8)],
                            );
                        }
                    }
                    Err(reason)
                        if matches!(
                            captured.operand.completion,
                            checked_trees::FlowOperandCompletion::Copied { .. }
                        ) && ::validation::trace_flags::check_sites() =>
                    {
                        eprintln!(
                            "CHECKSITE saved-reference-observation-refusal capture={:?} reason={reason}",
                            captured.operand.expression
                        );
                    }
                    Err(_) => {}
                }
            }
            if !proven && ::validation::trace_flags::check_sites() {
                self.trace_capture_refusal(facts, call, contexts, captured);
            }
            proven
        }

        // Keep the saved collection's identity and observation point. Replace
        // the callee's remaining index expressions with captured caller arguments.
        pub(in crate::checks::contracts) fn proven_for_call_projection(
            &self,
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            state: &FlowStateFact,
            call: &FlowCallFact,
            contexts: &[facts::FactContextHandle],
            captured: &crate::flow::CapturedValue<'_>,
            declared: &[facts::PlaceSegment],
            projection: &crate::checks::current_scalar_values::CallArgumentProjection,
        ) -> bool {
            let Self::Case(case) = self else {
                return false;
            };
            let trace_refusal = |stage: &'static str| {
                if validation::trace_flags::check_sites() {
                    eprintln!(
                        "CHECKSITE call-case-capture stage={} machine={:?} state={:?} statement={} call={} target={:?}",
                        stage,
                        state.machine_symbol,
                        state.state_symbol,
                        call.statement_index,
                        call.call_ordinal,
                        call.target_symbol,
                    );
                }
            };
            let (source_state, source_statement) =
                captured.producer_frontier(state.state_symbol, call.statement_index);
            let Some(evidence) = crate::flow::CaseEvidence::prepare(
                program,
                &facts.semantic,
                contexts,
                state.machine_symbol,
                source_state,
                source_statement,
            ) else {
                trace_refusal("evidence-point");
                return false;
            };
            let proven = crate::flow::captured_value_places(program, facts.flow.view(), captured)
                .is_some_and(|places| {
                    !places.is_empty()
                        && places.iter().all(|place| {
                            let Some(prefix) = place.projected.segments.strip_suffix(declared)
                            else {
                                trace_refusal("captured-path-suffix");
                                return false;
                            };
                            let prefix = validation::CanonicalPlace {
                                root: place.projected.root,
                                segments: prefix.to_vec(),
                            };
                            evidence.has_value_for_call_projection(&prefix, projection, *case, true)
                        })
                });
            if !proven {
                trace_refusal("captured-path-or-case");
            }
            proven
        }

        /*
            One failed obligation's retained cuts, without searching another value
            or evaluating a second proof. Missing diagnostic metadata stays absent.
        */
        fn trace_capture_refusal(
            &self,
            facts: &CheckFacts,
            call: &FlowCallFact,
            contexts: &[facts::FactContextHandle],
            captured: &crate::flow::CapturedValue<'_>,
        ) {
            let control = &facts.flow.control;
            let operand = captured.operand.row();
            fn path_prefix_and_count<T>(path: Option<&[T]>) -> Option<(&[T], usize)> {
                path.map(|path| (&path[..path.len().min(32)], path.len()))
            }
            let case = match self {
                Self::Case(case) => *case,
                Self::Bytes(_) => symbols::SymbolHandle::invalid(),
            };
            let cut = |row: &checked_trees::FlowOperandFact| {
                (
                    row.expression,
                    row.state_symbol,
                    row.statement_index,
                    row.constraints,
                    row.entry_origin_writes,
                    row.entry_origin_unknown_write,
                    row.entry_binding_writes,
                    row.entry_binding_unknown_write,
                )
            };
            let identity = (
                case,
                call.target_symbol,
                call.call_ordinal,
                captured.type_reference,
            );
            let projection = &captured.projection.remaining;
            eprintln!(
                "CHECKSITE captured-place-predicate identity(case,target,ordinal,type)={identity:?} cut={:?} projection={:?} projection_count={} completion={:?} contexts={:?} context_count={} call_entry={:?}",
                cut(operand),
                &projection[..projection.len().min(16)],
                projection.len(),
                operand.completion,
                &contexts[..contexts.len().min(8)],
                contexts.len(),
                call.entry_constraints,
            );
            if let checked_trees::FlowOperandCompletion::Copied { copy, .. } = operand.completion
                && control.copied_values.is_valid(copy)
            {
                let record = control.copied_values.get(copy);
                if control.operands.is_valid(record.source) {
                    eprintln!(
                        "CHECKSITE captured-place-predicate-copy copy={copy:?} source={:?} cut={:?} reference_leaves={:?}",
                        record.source,
                        cut(control.operands.get(record.source)),
                        record.references,
                    );
                    if let Some(leaves) = control.captured_reference_leaves.span(record.references)
                    {
                        for leaf in leaves.iter().take(4) {
                            let retention = control
                                .operand_retentions
                                .is_valid(leaf.retention)
                                .then(|| control.operand_retentions.get(leaf.retention));
                            eprintln!(
                                "CHECKSITE captured-place-predicate-saved-leaf leaf={leaf:?} path_prefix_and_count={:?} retention={retention:?}",
                                path_prefix_and_count(control.operand_child_paths.span(leaf.path)),
                            );
                        }
                    }
                }
            }
            if let Some(last_actual) = control
                .operands
                .span(call.operands)
                .and_then(|rows| rows.last())
            {
                eprintln!(
                    "CHECKSITE captured-place-predicate-last-actual cut={:?}",
                    cut(last_actual)
                );
                for (kind, prefix) in [
                    ("contents", last_actual.entry_origin_writes),
                    ("binding", last_actual.entry_binding_writes),
                ] {
                    if let Some(writes) = control.operand_origin_writes.span(prefix) {
                        for write in writes.iter().take(8) {
                            eprintln!(
                                "CHECKSITE captured-place-predicate-last-actual-write kind={kind} count={} write={write:?} segments_prefix_and_count={:?}",
                                writes.len(),
                                path_prefix_and_count(
                                    control.operand_referent_segments.span(write.segments)
                                ),
                            );
                        }
                    }
                }
            }
            if let Some(accesses) = facts.borrow.argument_accesses.span(call.accesses) {
                for access in accesses
                    .iter()
                    .filter(|access| access.expression == operand.expression)
                    .take(4)
                {
                    eprintln!("CHECKSITE captured-place-predicate-access {access:?}");
                }
            }
            let observation = captured.observation.row();
            let mut shown_resources = 0;
            for (_, resource) in facts
                .borrow
                .call_loan_resources
                .iter()
                .filter(|(_, resource)| {
                    control.calls.is_valid(resource.call)
                        && std::ptr::eq(control.calls.get(resource.call), call)
                        && control.operands.is_valid(resource.owner_operand)
                        && std::ptr::eq(control.operands.get(resource.owner_operand), observation)
                })
                .take(4)
            {
                shown_resources += 1;
                eprintln!(
                    "CHECKSITE captured-place-predicate-call-resource-physical-row-match resource={resource:?} owner_path_prefix_and_count={:?} capture_path_prefix_and_count={:?}",
                    path_prefix_and_count(facts.borrow.owner_segments.span(resource.owner_path)),
                    path_prefix_and_count(facts.borrow.call_loan_paths.span(resource.capture_path)),
                );
                if let checked_trees::CheckedCallBorrowLoanSource::CallAccess { parents, .. } =
                    resource.source
                    && let Some(parents) = facts.borrow.call_loan_parents.span(parents)
                {
                    for parent in parents.iter().take(4) {
                        if facts.borrow.loans.is_valid(*parent) {
                            eprintln!(
                                "CHECKSITE captured-place-predicate-call-parent handle={parent:?} loan={:?}",
                                facts.borrow.loans.get(*parent),
                            );
                        }
                    }
                }
            }
            eprintln!(
                "CHECKSITE captured-place-predicate-call-resource-physical-row-match-count shown={shown_resources} limit=4"
            );
        }

        pub(in crate::checks::contracts) fn proven(
            &self,
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            point: facts::ProgramPoint,
            contexts: &[facts::FactContextHandle],
            subject: &validation::CanonicalPlace,
        ) -> bool {
            match self {
                Self::Bytes(predicate) => super::byte_predicate_proven_at_place(
                    program,
                    &facts.semantic,
                    contexts,
                    subject,
                    *predicate,
                ),
                Self::Case(case) => {
                    let facts::ProgramPoint::Statement {
                        machine_symbol,
                        state_symbol,
                        statement_index,
                    } = point
                    else {
                        return false;
                    };
                    crate::flow::place_case_has_value(
                        program,
                        &facts.semantic,
                        contexts,
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        subject,
                        *case,
                        true,
                    )
                }
            }
        }
    }
}
pub(super) use place_predicates::place_predicate;
pub(in crate::checks) mod call_guarantees {
    /*
        A call guarantee and a local result binding have separate lifetimes. Join
        the surviving guarantee to surviving AssignedValue provenance, then compare
        predicates under exact declaration/call substitution. Never replay a local
        initializer or identify repeated calls by their printed arguments.
        Machine entries, named state arrivals, and requirement signatures share this
        substitution; `callable` retains their distinct parameter/result scopes.
        A public requirement guarantee is not a proof of its provider's body.
        A named arrival substitutes only its own state contract; it cannot revive
        machine-entry assumptions. Every input substitutes through its explicit
        argument position and exact declaration-owned projection.

        This consumes the existing flow evidence rather than copying predicates at
        each assignment. Storage dependencies retire changed inputs; assignment
        provenance retires changed results. Inputs retain their exact completed
        operand and declared carrier. Detached stable values can name opaque
        computations without replay; live views still owe storage preservation.
        Expression-root dependencies alone do not establish referent identity.
    */
    use super::super::call_values::actual_at;
    use super::super::call_values::captured_operand;
    use super::super::call_values::captured_values;
    use super::super::captured_predicates::retain_bindings;
    use super::super::captured_predicates::transport;
    use super::field_actuals::checked_place;
    use crate::flow::expressions;
    use crate::published::facts::boolean_predicates::{self, BoundPredicate};
    use crate::published::facts::contract_occurrences::append_expression_occurrences;
    use crate::published::places::place_at;
    use crate::published::semantic::facts::projected_argument_place;

    use validation::CallSite;
    use validation::CanonicalPlace;

    use checked_trees::{CheckedOperatorFacts, FlowCallFact, FlowStateFact};
    use facts::{FactPayload, FactPlan, PlaceRoot};
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;

    pub(in crate::checks) mod arithmetic {
        /*
            Arithmetic uses the same live invocation/result join as direct guarantee
            matching. Each proposition has its own declaration substitution; exact
            captured places become shared atoms only within this implication. No source
            initializer is replayed, and no callee precondition is imported as a premise.
            The existing scoped arithmetic engine owns entailment. This adapter owns
            field identity, current-value custody and selected builtin operation meaning.
            Polynomial terms retain Exact arithmetic. A separately marked completed
            unsigned remainder keeps its exact operand roster through policy erasure;
            the shared engine still needs independent positivity/nonnegativity premises
            before using its divisor-relative bound. This admits no general cast or
            syntax-only interpretation of an overridden operator.
            Imported return guarantees are optional premises: an ordinary constructor
            can project a required field directly to an already bounded caller value.
            An unrelated call-result sibling does not change that scalar relationship.
            The shared backward value-origin trace follows captured copies through
            stores while proving intervening write preservation; it does not execute
            initializers. Calls use the exact operand-evaluation frontier, whereas a
            stable return uses the exit statement boundary. Those frontiers cannot be
            interchanged when an earlier sibling operand mutates storage.
            A captured store also retains the predicates proved about its operand at
            evaluation. Those predicates bind historical value atoms, not the current
            contents of the operand's former storage. Following the same atoms through
            copies lets both later calls and returns use their bounds without reviving
            invalidated live facts or assuming that a callee's precondition held.
        */
        use super::callable::Callable;
        use super::{Invocation, actual_projection, bound_place, direct_place, invocation_fact};
        use crate::checks::contracts::return_values::completed_return_operand;
        use crate::checks::contracts::return_values::exit_return_call;
        use crate::checks::unsigned;
        use crate::flow::ValueObservation;
        use crate::published::facts::contract_occurrences::append_expression_occurrences;
        use checked_trees::{CheckFacts, FlowStateFact};
        use facts::{ContractFactKind, FactContextHandle, FactOrigin, FactPayload};
        use numerics::arithmetic::ArithmeticDomain;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
        use typed_trees::types::PrimitiveType;
        use validation::{
            ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
            ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
        };

        pub(super) fn proves(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            caller: &FlowStateFact,
            contexts: &[FactContextHandle],
            required: &Invocation<'_>,
            goal: ExpressionHandle,
            mut hypotheses: Vec<ScopedArithmeticHypothesis>,
            frames: &validation::CallFrameResolver<'_>,
        ) -> bool {
            let Some(goal) = at_invocation(program, facts, caller, required, goal, &[], frames)
            else {
                return false;
            };
            let Some(call) = invocation_fact(facts.view(), caller, required) else {
                return false;
            };
            append_store_hypotheses(
                program,
                facts,
                caller,
                required.statement,
                &mut hypotheses,
                frames,
            );
            proves_bound(
                program,
                facts,
                caller,
                required.statement,
                contexts,
                &goal.proposition,
                hypotheses,
                frames,
                Some(call),
            )
        }

        /*
            Normal-return propositions and their freshly written observations are one
            prepared proof scope. A consumer joins its outgoing values to these exact
            return frontiers before asking the arithmetic engine for an implication.
        */
        pub(in crate::checks) struct ExitArithmetic<'program, 'facts> {
            program: &'program dyn ::validation::CheckingInput,
            facts: &'facts CheckFacts,
            caller: &'facts FlowStateFact,
            exit: &'facts checked_trees::FlowExitFact,
            contexts: &'facts [FactContextHandle],
            frames: &'facts validation::CallFrameResolver<'program>,
            hypotheses: Vec<ScopedArithmeticHypothesis>,
            returned: Vec<ReturnedObservation>,
        }

        struct ReturnedObservation {
            statement: usize,
            ordinal: usize,
            place: validation::CanonicalPlace,
            primitive: PrimitiveType,
            length: bool,
            value: ScopedArithmeticValue,
        }

        pub(super) struct InvocationArithmetic {
            pub(super) proposition: ScopedArithmeticExpression,
            returned: Vec<ReturnedObservation>,
        }

        impl ExitArithmetic<'_, '_> {
            /*
                A post-call read may name the written home or a detached copy of its
                returned contents. Flow proves that correspondence and rejects later
                overlapping writes; a shared place spelling alone is never enough.
            */
            pub(in crate::checks) fn value(
                &self,
                place: validation::CanonicalPlace,
                primitive: PrimitiveType,
            ) -> Option<ScopedArithmeticValue> {
                let machine = self.program.machine_by_symbol(self.caller.machine_symbol)?;
                let storage = crate::flow::rebase_exact_local_place(
                    self.program,
                    self.caller.state_symbol,
                    self.exit.statement_index,
                    place.clone(),
                    Some(self.frames),
                )?;
                if !crate::flow::call_flows(&self.facts.flow.control, self.caller.calls)
                    .iter()
                    .filter(|call| call.statement_index == self.exit.statement_index)
                    .all(|call| {
                        crate::flow::entry_origin_is_stable(
                            self.program,
                            &self.facts.flow.control,
                            call.normal_return_writes,
                            !call.normal_return_writes_complete,
                            &storage,
                        )
                    })
                {
                    return None;
                }
                for returned in &self.returned {
                    if returned.primitive != primitive || returned.length {
                        continue;
                    }
                    let call = crate::flow::call_flows(&self.facts.flow.control, self.caller.calls)
                        .iter()
                        .find(|call| {
                            call.statement_index == returned.statement
                                && call.call_ordinal == returned.ordinal
                        })?;
                    if crate::flow::value_origin_matches_call_return(
                        self.program,
                        self.facts.flow.view(),
                        machine,
                        self.caller,
                        self.exit.statement_index,
                        place.clone(),
                        call,
                        &returned.place,
                        self.frames,
                    ) {
                        return Some(returned.value.clone());
                    }
                }
                value_origin(
                    self.program,
                    self.facts.view(),
                    self.caller,
                    self.exit.statement_index,
                    self.frames,
                    place,
                )
                .map(|place| atom(place, primitive))
            }

            pub(in crate::checks) fn proves(self, goal: &ScopedArithmeticExpression) -> bool {
                proves_bound(
                    self.program,
                    self.facts,
                    self.caller,
                    self.exit.statement_index,
                    self.contexts,
                    goal,
                    self.hypotheses,
                    self.frames,
                    None,
                )
            }
        }

        pub(in crate::checks) fn at_exit<'program, 'facts>(
            program: &'program dyn ::validation::CheckingInput,
            facts: &'facts CheckFacts,
            caller: &'facts FlowStateFact,
            exit: &'facts checked_trees::FlowExitFact,
            contexts: &'facts [FactContextHandle],
            frames: &'facts validation::CallFrameResolver<'program>,
        ) -> Option<ExitArithmetic<'program, 'facts>> {
            if caller.machine_symbol != exit.machine_symbol
                || caller.state_symbol != exit.state_symbol
            {
                return None;
            }
            let frontier = if completed_return_operand(program, facts, exit).is_some()
                || exit_return_call(program, facts, exit).is_some()
            {
                super::GuaranteeUse::EvaluatedContext(exit.statement_index)
            } else {
                super::GuaranteeUse::StatementEntry(exit.statement_index)
            };
            let mut hypotheses = Vec::new();
            let mut returned = Vec::new();
            for guarantee in
                super::available(program, facts.view(), caller, frontier, contexts, frames)
            {
                if let Some(writes) = guarantee.post_return_writes.as_deref()
                    && let Some(bound) = at_invocation(
                        program,
                        facts,
                        caller,
                        &guarantee.invocation,
                        guarantee.expression,
                        writes,
                        frames,
                    )
                {
                    returned.extend(bound.returned);
                    hypotheses.push(ScopedArithmeticHypothesis {
                        proposition: bound.proposition,
                        holds: true,
                    });
                }
            }
            returned.extend(append_store_hypotheses(
                program,
                facts,
                caller,
                exit.statement_index,
                &mut hypotheses,
                frames,
            ));
            Some(ExitArithmetic {
                program,
                facts,
                caller,
                exit,
                contexts,
                frames,
                hypotheses,
                returned,
            })
        }

        fn append_store_hypotheses(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            caller: &FlowStateFact,
            statement: usize,
            hypotheses: &mut Vec<ScopedArithmeticHypothesis>,
            frames: &validation::CallFrameResolver<'_>,
        ) -> Vec<ReturnedObservation> {
            let mut returned = Vec::new();
            for stored in facts
                .flow
                .control
                .statements
                .span_or_empty(caller.statements)
                .iter()
                .filter(|stored| stored.statement_index < statement)
            {
                if !facts.flow.control.operands.is_valid(stored.stored_value) {
                    continue;
                }
                let operand = facts.flow.control.operands.get(stored.stored_value);
                let entry_contexts = facts
                    .flow
                    .semantic_constraint_contexts(stored.entry_constraints)
                    .collect::<Vec<_>>();
                append_context_hypotheses(
                    program,
                    facts,
                    caller,
                    &entry_contexts,
                    hypotheses,
                    |place, observation| {
                        observation_origin(
                            program,
                            facts.view(),
                            caller,
                            stored.statement_index,
                            frames,
                            place,
                            observation,
                        )
                    },
                );
                let captured_contexts = facts
                    .flow
                    .semantic_constraint_contexts(stored.pre_store_constraints)
                    .collect::<Vec<_>>();
                for guarantee in super::available(
                    program,
                    facts.view(),
                    caller,
                    super::GuaranteeUse::EvaluatedContext(stored.statement_index),
                    &captured_contexts,
                    frames,
                ) {
                    if let Some(writes) = guarantee.post_return_writes.as_deref()
                        && let Some(bound) = at_invocation(
                            program,
                            facts,
                            caller,
                            &guarantee.invocation,
                            guarantee.expression,
                            writes,
                            frames,
                        )
                    {
                        returned.extend(bound.returned);
                        hypotheses.push(ScopedArithmeticHypothesis {
                            proposition: bound.proposition,
                            holds: true,
                        });
                    }
                }
                append_context_hypotheses(
                    program,
                    facts,
                    caller,
                    &captured_contexts,
                    hypotheses,
                    |place, observation| {
                        crate::flow::operand_entry_origin_is_stable(
                            program,
                            facts.flow.view(),
                            operand,
                            &place,
                        )
                        .then_some(())?;
                        observation_origin(
                            program,
                            facts.view(),
                            caller,
                            stored.statement_index,
                            frames,
                            place,
                            observation,
                        )
                    },
                );
            }
            returned
        }

        fn proves_bound(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            caller: &FlowStateFact,
            statement: usize,
            contexts: &[FactContextHandle],
            goal: &ScopedArithmeticExpression,
            mut hypotheses: Vec<ScopedArithmeticHypothesis>,
            frames: &validation::CallFrameResolver<'_>,
            call: Option<&checked_trees::FlowCallFact>,
        ) -> bool {
            let Some(machine) = program.machine_by_symbol(caller.machine_symbol) else {
                return false;
            };
            append_context_hypotheses(
                program,
                facts,
                caller,
                contexts,
                &mut hypotheses,
                |place, observation| {
                    if let Some(call) = call {
                        crate::flow::value_origin_at_call(
                            program,
                            &facts.flow,
                            machine,
                            caller,
                            call,
                            place,
                            Some(frames),
                            observation,
                        )
                    } else {
                        observation_origin(
                            program,
                            facts.view(),
                            caller,
                            statement,
                            frames,
                            place,
                            observation,
                        )
                    }
                },
            );
            validation::scoped_arithmetic_implication(program, machine, &hypotheses, goal)
                == StrictArithmeticImplicationJudgment::Proven
        }

        fn append_context_hypotheses(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            caller: &FlowStateFact,
            contexts: &[FactContextHandle],
            hypotheses: &mut Vec<ScopedArithmeticHypothesis>,
            origin: impl Fn(
                validation::CanonicalPlace,
                ValueObservation,
            ) -> Option<validation::CanonicalPlace>,
        ) {
            let Some(machine) = program.machine_by_symbol(caller.machine_symbol) else {
                return;
            };
            let Some(state) = validation::find_state_in_machine(
                program,
                caller.machine_symbol,
                caller.state_symbol,
            ) else {
                return;
            };
            for fact in contexts.iter().flat_map(|context| {
                facts
                    .semantic
                    .context_view(facts.semantic.contexts.get(*context))
                    .facts()
            }) {
                if matches!(
                    fact.origin,
                    FactOrigin::CallRequires | FactOrigin::CallEnsures
                ) {
                    continue;
                }
                let (expression, holds) = match fact.payload {
                    FactPayload::ContractBooleanExpression {
                        kind: ContractFactKind::Requires,
                        expression,
                        instantiated,
                        ..
                    } if !instantiated.is_valid() => (expression, true),
                    FactPayload::BooleanValue { expression, value } => (expression, value),
                    _ => continue,
                };
                if let Some(proposition) = proposition(
                    program,
                    facts,
                    &Callable::Machine { machine, state },
                    expression,
                    |occurrence, primitive| {
                        if let Some(length) = collection_length_value(
                            program,
                            &Callable::Machine { machine, state },
                            occurrence,
                            &origin,
                        ) {
                            return Some(length);
                        }
                        let place = direct_place(program, occurrence)?;
                        let place = origin(place, ValueObservation::Contents)?;
                        Some(atom(place, primitive))
                    },
                ) {
                    hypotheses.push(ScopedArithmeticHypothesis { proposition, holds });
                }
            }
        }

        /*
            Inputs use their operand frontier; a normal guarantee additionally carries
            the writes accepted by availability. Those observations have a new value
            identity at return and must never trace through the invocation's old stores.
        */
        pub(super) fn at_invocation(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            caller: &FlowStateFact,
            invocation: &Invocation<'_>,
            expression: ExpressionHandle,
            post_return_writes: &[validation::CanonicalPlace],
            frames: &validation::CallFrameResolver<'_>,
        ) -> Option<InvocationArithmetic> {
            let static_actuals =
                super::static_operands::bindings(program, facts, invocation, expression);
            let static_actuals = static_actuals?;
            let mut returned = Vec::new();
            let proposition = proposition_with_static_actuals(
                program,
                facts,
                &invocation.callable,
                expression,
                &static_actuals,
                |occurrence, primitive| {
                    if let Some((actual, remaining)) =
                        actual_projection(program, invocation, occurrence)
                        && remaining.is_empty()
                        && matches!(
                            program.expression_table.expression(actual),
                            ExpressionNode::Integer(_)
                        )
                    {
                        return Some(ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                            owner: caller.state_symbol,
                            expression: actual,
                            bindings: Vec::new(),
                        }));
                    }
                    let length =
                        collection_length_subject(program, &invocation.callable, occurrence)
                            .is_some();
                    let (observed, observation) = if length {
                        (
                            program.expression_table.member_node(occurrence)?.receiver,
                            collection_length_observation(
                                program,
                                &invocation.callable,
                                occurrence,
                            )?,
                        )
                    } else {
                        (occurrence, ValueObservation::Contents)
                    };
                    if !length
                        && let Some(result) = validation::reserved_result_place(program, observed)
                        && result.machine_symbol == invocation.callable.owner_symbol()
                    {
                        return call_result_atom(
                            program,
                            caller,
                            invocation_fact(facts.view(), caller, invocation)?,
                            &result.segments,
                            primitive,
                        );
                    }
                    let place = bound_place(program, invocation, observed)?;
                    if !post_return_writes.is_empty()
                        && matches!(place.root, facts::PlaceRoot::Symbol(_))
                    {
                        let storage = crate::flow::rebase_exact_local_place(
                            program,
                            invocation.caller_state,
                            invocation.statement,
                            place.clone(),
                            Some(frames),
                        )?;
                        if post_return_writes
                            .iter()
                            .any(|write| observation.changed_by(program, &storage, write))
                        {
                            let value = ScopedArithmeticValue::Atom {
                                identity: format!(
                                    "\0call-guarantee:return:{:?}:{}:{}:{primitive:?}:{length}:{storage:?}",
                                    invocation.caller_state,
                                    invocation.statement,
                                    invocation.ordinal,
                                ),
                                unsigned: matches!(
                                    primitive,
                                    typed_trees::unsigned_integer_primitive_pattern!()
                                ),
                            };
                            returned.push(ReturnedObservation {
                                statement: invocation.statement,
                                ordinal: invocation.ordinal,
                                place: storage,
                                primitive,
                                length,
                                value: value.clone(),
                            });
                            return Some(value);
                        }
                    }
                    let place = if matches!(place.root, facts::PlaceRoot::Expression(_)) {
                        place
                    } else {
                        let machine = program.machine_by_symbol(caller.machine_symbol)?;
                        let call = invocation_fact(facts.view(), caller, invocation)?;
                        crate::flow::value_origin_at_call(
                            program,
                            &facts.flow,
                            machine,
                            caller,
                            call,
                            place,
                            Some(frames),
                            observation,
                        )?
                    };
                    Some(if length {
                        length_atom(place)
                    } else {
                        atom(place, primitive)
                    })
                },
            )?;
            Some(InvocationArithmetic {
                proposition,
                returned,
            })
        }

        /*
            Normal results are fresh values of one exact invocation, including named
            terminal calls which intentionally have no fabricated expression handle.
            Consumers must establish completion/availability before using this identity.
        */
        pub(in crate::checks) fn call_result_atom(
            program: &dyn ::validation::CheckingInput,
            caller: &FlowStateFact,
            call: &checked_trees::FlowCallFact,
            segments: &[facts::PlaceSegment],
            primitive: typed_trees::types::PrimitiveType,
        ) -> Option<ScopedArithmeticValue> {
            if !call.authored_source_custody_valid {
                return None;
            }
            let reference = validation::call_target_return_type(program, call.target_symbol)?;
            if validation::project_type_reference_from_segments(program, reference, segments)
                .and_then(|reference| program.primitive_type_reference(reference))
                != Some(primitive)
            {
                return None;
            }
            let site = validation::find_call_site(
                program,
                caller.machine_symbol,
                caller.state_symbol,
                call.statement_index,
                call.call_ordinal,
            )?;
            match site {
                validation::CallSite::Expression {
                    expression,
                    call: selected,
                } if expression == call.authored_expression
                    && call.authored_site == checked_trees::FlowCallSite::Expression
                    && call.authored_target == selected.target_symbol =>
                {
                    Some(atom(
                        validation::CanonicalPlace {
                            root: facts::PlaceRoot::Expression(expression),
                            segments: segments.to_vec(),
                        },
                        primitive,
                    ))
                }
                validation::CallSite::TransitionNamed { path, .. }
                    if !call.authored_expression.is_valid()
                        && call.authored_site == checked_trees::FlowCallSite::NamedTransition
                        && call.authored_target == path.symbol
                        && call.target_symbol
                            == validation::call_target_state(program, path.symbol)
                                .map_or(path.symbol, |state| state.symbol) =>
                {
                    Some(ScopedArithmeticValue::Atom {
                        identity: format!(
                            "\0call-result:{:?}:{}:{}:{segments:?}:{primitive:?}",
                            caller.state_symbol, call.statement_index, call.call_ordinal
                        ),
                        unsigned: matches!(
                            primitive,
                            typed_trees::unsigned_integer_primitive_pattern!()
                        ),
                    })
                }
                _ => None,
            }
        }

        pub(in crate::checks) fn value_origin(
            program: &dyn ::validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            caller: &FlowStateFact,
            statement: usize,
            frames: &validation::CallFrameResolver<'_>,
            place: validation::CanonicalPlace,
        ) -> Option<validation::CanonicalPlace> {
            observation_origin(
                program,
                facts,
                caller,
                statement,
                frames,
                place,
                ValueObservation::Contents,
            )
        }

        pub(in crate::checks) fn observation_origin(
            program: &dyn ::validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            caller: &FlowStateFact,
            statement: usize,
            frames: &validation::CallFrameResolver<'_>,
            place: validation::CanonicalPlace,
            observation: ValueObservation,
        ) -> Option<validation::CanonicalPlace> {
            let machine = program.machine_by_symbol(caller.machine_symbol)?;
            if matches!(place.root, facts::PlaceRoot::Expression(expression)
                if matches!(program.expression_table.expression(expression), ExpressionNode::Call(_)))
            {
                return Some(place);
            }
            crate::flow::value_origin_before_statement(
                program,
                facts.flow,
                machine,
                caller,
                statement,
                place,
                Some(frames),
                observation,
            )
        }

        pub(in crate::checks) fn atom(
            place: validation::CanonicalPlace,
            primitive: PrimitiveType,
        ) -> ScopedArithmeticValue {
            ScopedArithmeticValue::Atom {
                identity: format!("\0call-guarantee:{primitive:?}:{place:?}"),
                unsigned: matches!(
                    primitive,
                    typed_trees::unsigned_integer_primitive_pattern!() | PrimitiveType::Addr
                ),
            }
        }

        /*
            Metadata is a scalar observation of a descriptor, not a stored field named
            `len`. The caller supplies exact descriptor origin/preservation at its own
            evaluation frontier. Its atom is distinct from every element/value atom.
        */
        pub(in crate::checks) fn collection_length_value(
            program: &dyn ::validation::CheckingInput,
            callable: &Callable<'_>,
            expression: ExpressionHandle,
            origin: impl FnOnce(
                validation::CanonicalPlace,
                ValueObservation,
            ) -> Option<validation::CanonicalPlace>,
        ) -> Option<ScopedArithmeticValue> {
            let place = collection_length_subject(program, callable, expression)?;
            let place = origin(
                place,
                collection_length_observation(program, callable, expression)?,
            )?;
            Some(length_atom(place))
        }

        pub(in crate::checks) fn length_atom(
            place: validation::CanonicalPlace,
        ) -> ScopedArithmeticValue {
            ScopedArithmeticValue::Atom {
                identity: format!("\0call-guarantee:length:{place:?}"),
                unsigned: true,
            }
        }

        fn collection_length_observation(
            program: &dyn ::validation::CheckingInput,
            callable: &Callable<'_>,
            expression: ExpressionHandle,
        ) -> Option<ValueObservation> {
            let member = program.expression_table.member_node(expression)?;
            let mut reference = callable.scalar_reference(program, member.receiver)?;
            let mut borrowed = false;
            loop {
                use typed_trees::types::TypeReferenceNode;
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    TypeReferenceNode::Reference { referee, .. } => {
                        borrowed = true;
                        reference = *referee;
                    }
                    _ => break,
                }
            }
            Some(
                if borrowed
                    && matches!(
                        program.type_reference_table.type_reference(reference),
                        typed_trees::types::TypeReferenceNode::Slice { .. }
                    )
                {
                    ValueObservation::Binding
                } else {
                    ValueObservation::Contents
                },
            )
        }

        fn collection_length_subject(
            program: &dyn ::validation::CheckingInput,
            callable: &Callable<'_>,
            expression: ExpressionHandle,
        ) -> Option<validation::CanonicalPlace> {
            let member = program.expression_table.member_node(expression)?;
            let reference = callable.scalar_reference(program, member.receiver)?;
            if validation::collection_operations::collection_measure_member(
                program,
                member,
                validation::collection_operations::MeasureReceiver::Declared(reference),
            ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
            {
                return None;
            }
            direct_place(program, member.receiver)
        }

        pub(in crate::checks) fn proposition(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            callable: &Callable<'_>,
            expression: ExpressionHandle,
            value: impl FnMut(ExpressionHandle, PrimitiveType) -> Option<ScopedArithmeticValue>,
        ) -> Option<ScopedArithmeticExpression> {
            proposition_with_static_actuals(program, facts, callable, expression, &[], value)
        }

        fn proposition_with_static_actuals(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            callable: &Callable<'_>,
            expression: ExpressionHandle,
            static_actuals: &[ScopedArithmeticBinding],
            value: impl FnMut(ExpressionHandle, PrimitiveType) -> Option<ScopedArithmeticValue>,
        ) -> Option<ScopedArithmeticExpression> {
            if !super::super::has_builtin_operators(program, &facts.operators, expression)
                || !callable.builtin_meaning(program, expression)
                || !predicate(program, &facts.operators, callable, expression)
            {
                return None;
            }
            bind_occurrences(
                program,
                facts.view(),
                callable,
                expression,
                static_actuals,
                value,
            )
        }

        pub(in crate::checks) fn scalar_term(
            program: &dyn ::validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            callable: &Callable<'_>,
            expression: ExpressionHandle,
            value: impl FnMut(ExpressionHandle, PrimitiveType) -> Option<ScopedArithmeticValue>,
        ) -> Option<ScopedArithmeticValue> {
            let expression = admitted_scalar_expression(program, facts, callable, expression)?;
            let mut term = bind_occurrences(program, facts, callable, expression, &[], value)?;
            bind_completed_remainders(program, facts, callable, &mut term);
            Some(ScopedArithmeticValue::Term(term))
        }

        /*
            One admission owner for scalar denotations and captured call actuals.
            Selected builtin meaning and policy precede every polynomial interpretation.
        */
        fn admitted_scalar_expression(
            program: &dyn ::validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            callable: &Callable<'_>,
            expression: ExpressionHandle,
        ) -> Option<ExpressionHandle> {
            if !super::super::has_builtin_operators(program, facts.operators, expression)
                || !callable.builtin_meaning(program, expression)
                || !term(program, facts.operators, callable, expression)
            {
                return None;
            }
            Some(expression)
        }

        /*
            The completed-value owner grants the operation's normal denotation, not
            an enclosing cast's spelling. Unsigned remainder is unchanged by every
            arithmetic policy; its exact selected source retains that policy and the
            independent formation obligation. A proposition being proved does not call
            this operation-publication step.
        */
        pub(in crate::checks) fn completed_scalar_value(
            program: &dyn ::validation::CheckingInput,
            callable: &Callable<'_>,
            term: ScopedArithmeticExpression,
        ) -> ScopedArithmeticValue {
            if admitted_unsigned_remainder(program, callable, term.expression) {
                ScopedArithmeticValue::AdmittedIntegerRemainder(term)
            } else {
                ScopedArithmeticValue::Term(term)
            }
        }

        fn bind_completed_remainders(
            program: &dyn ::validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            callable: &Callable<'_>,
            term: &mut ScopedArithmeticExpression,
        ) {
            let mut operations = Vec::new();
            validation::expression_traversal::visit_post_order(
                program,
                term.expression,
                &mut Vec::new(),
                |expression, _| {
                    if admitted_unsigned_remainder(program, callable, expression)
                        && super::super::has_builtin_operators(program, facts.operators, expression)
                    {
                        operations.push(expression);
                    }
                },
            );
            for expression in operations {
                let operation = ScopedArithmeticExpression {
                    owner: term.owner,
                    expression,
                    bindings: term
                        .bindings
                        .iter()
                        .filter(|binding| {
                            binding.binder != ScopedArithmeticBinder::Occurrence(expression)
                        })
                        .cloned()
                        .collect(),
                };
                term.bindings.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Occurrence(expression),
                    value: completed_scalar_value(program, callable, operation),
                });
            }
        }

        fn bind_occurrences(
            program: &dyn ::validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            callable: &Callable<'_>,
            expression: ExpressionHandle,
            static_actuals: &[ScopedArithmeticBinding],
            mut value: impl FnMut(ExpressionHandle, PrimitiveType) -> Option<ScopedArithmeticValue>,
        ) -> Option<ScopedArithmeticExpression> {
            let mut occurrences = Vec::new();
            append_expression_occurrences(program, expression, &mut occurrences);
            let mut expressions = Vec::new();
            validation::expression_traversal::collect_expression_tree(
                program,
                expression,
                &mut expressions,
            );
            for expression in expressions {
                if collection_length_subject(program, callable, expression).is_some()
                    && !occurrences.contains(&expression)
                {
                    occurrences.push(expression);
                }
            }
            let mut bindings = Vec::new();
            for occurrence in occurrences {
                let Some(primitive) = contents_primitive(program, callable, occurrence)
                    .filter(|primitive| primitive.accepts_integer_literal())
                else {
                    continue;
                };
                let binder = match program.expression_table.expression(occurrence) {
                    ExpressionNode::Member(_) => ScopedArithmeticBinder::Occurrence(occurrence),
                    ExpressionNode::Indexed(_)
                        if validation::immutable_const_projection(program, occurrence)
                            .is_some() =>
                    {
                        ScopedArithmeticBinder::Occurrence(occurrence)
                    }
                    ExpressionNode::Name(path)
                        if validation::is_single_segment_name_path(program, path) =>
                    {
                        ScopedArithmeticBinder::Symbol(path.symbol)
                    }
                    ExpressionNode::Name(_)
                        if validation::reserved_result_place(program, occurrence).is_some_and(
                            |place| {
                                place.machine_symbol == callable.owner_symbol()
                                    && place.segments.is_empty()
                            },
                        ) =>
                    {
                        ScopedArithmeticBinder::Result
                    }
                    _ => return None,
                };
                let closed_field = matches!(
                    program.expression_table.expression(occurrence),
                    ExpressionNode::Member(_)
                )
                .then(|| program.closed_integer_value_in(occurrence, callable.owner_symbol()))
                .flatten()
                .filter(|closed| closed.primitive == Some(primitive))
                .map(|closed| closed.value);
                bindings.push(ScopedArithmeticBinding {
                    binder: binder.clone(),
                    value: match binder {
                        ScopedArithmeticBinder::Symbol(symbol)
                            if program.data_type_parameter_by_symbol(symbol).is_some_and(
                                |parameter| {
                                    matches!(
                                        parameter.kind,
                                        typed_trees::data::TypeParameterKind::Const { .. }
                                    )
                                },
                            ) =>
                        {
                            static_actuals
                                .iter()
                                .find(|binding| binding.binder == binder)
                                .map(|binding| binding.value.clone())
                                .unwrap_or_else(|| ScopedArithmeticValue::Atom {
                                    identity: format!("\0static-parameter:{symbol:?}"),
                                    unsigned: matches!(
                                        primitive,
                                        typed_trees::unsigned_integer_primitive_pattern!()
                                    ),
                                })
                        }
                        ScopedArithmeticBinder::Occurrence(_)
                            if validation::immutable_const_projection(program, occurrence)
                                .is_some() =>
                        {
                            static_actuals
                                .iter()
                                .find(|binding| binding.binder == binder)
                                .map(|binding| binding.value.clone())
                                .or_else(|| {
                                    super::static_operands::projection_atom(
                                        program, occurrence, primitive,
                                    )
                                })?
                        }
                        /*
                            A field of a record constant is a closed integer,
                            not a storage read: bind the value the constant's
                            initializer gave it, in the carrier this occurrence
                            is read at.
                        */
                        ScopedArithmeticBinder::Occurrence(_) if closed_field.is_some() => {
                            ScopedArithmeticValue::Integer(closed_field?)
                        }
                        _ => value(occurrence, primitive)?,
                    },
                });
            }
            let mut arguments = Vec::new();
            validation::expression_traversal::visit_post_order(
                program,
                expression,
                &mut Vec::new(),
                |_, node| {
                    if let ExpressionNode::Call(call) = node {
                        for argument in program.expression_table.expression_handles(call.arguments)
                        {
                            if !arguments.contains(argument) {
                                arguments.push(*argument);
                            }
                        }
                    }
                },
            );
            for argument in arguments {
                let Some(selected) = admitted_scalar_expression(program, facts, callable, argument)
                else {
                    continue;
                };
                let mut term = ScopedArithmeticExpression {
                    owner: callable.target_symbol(),
                    expression: selected,
                    bindings: bindings.clone(),
                };
                bind_completed_remainders(program, facts, callable, &mut term);
                bindings.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Occurrence(argument),
                    value: ScopedArithmeticValue::Term(term),
                });
            }
            Some(ScopedArithmeticExpression {
                owner: callable.target_symbol(),
                expression,
                bindings,
            })
        }

        fn predicate(
            program: &dyn ::validation::CheckingInput,
            operators: &checked_trees::CheckedOperatorFacts,
            callable: &Callable<'_>,
            expression: ExpressionHandle,
        ) -> bool {
            let Some(binary) = program.expression_table.binary_node(expression) else {
                return false;
            };
            match binary.operator {
                BinaryOperator::And => {
                    predicate(program, operators, callable, binary.left)
                        && predicate(program, operators, callable, binary.right)
                }
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual => {
                    term(program, operators, callable, binary.left)
                        && term(program, operators, callable, binary.right)
                }
                _ => false,
            }
        }

        fn term(
            program: &dyn ::validation::CheckingInput,
            operators: &checked_trees::CheckedOperatorFacts,
            callable: &Callable<'_>,
            expression: ExpressionHandle,
        ) -> bool {
            if !program.expression_table.expression_is_valid(expression) {
                return false;
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Integer(_) => true,
                ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                    contents_primitive(program, callable, expression)
                        .is_some_and(|primitive| primitive.accepts_integer_literal())
                }
                ExpressionNode::Indexed(_)
                    if validation::immutable_const_projection(program, expression).is_some() =>
                {
                    contents_primitive(program, callable, expression)
                        .is_some_and(|primitive| primitive.accepts_integer_literal())
                }
                ExpressionNode::Binary(binary)
                    if matches!(
                        binary.operator,
                        BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
                    ) =>
                {
                    [binary.left, binary.right].into_iter().all(|operand| {
                        term(program, operators, callable, operand)
                            && callable
                                .scalar_reference(program, operand)
                                .map(|reference| {
                                    program.arithmetic_domain_for_type_reference(reference)
                                        == ArithmeticDomain::Exact
                                })
                                .unwrap_or_else(|| proof_integer_term(program, operand))
                    })
                }
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Modulo => {
                    super::super::has_builtin_operators(program, operators, expression)
                        && callable.builtin_meaning(program, expression)
                        && builtin_unsigned_remainder(program, callable, binary.left, binary.right)
                        && term(program, operators, callable, binary.left)
                        && term(program, operators, callable, binary.right)
                }
                ExpressionNode::Cast(cast) => {
                    value_preserving_integer_coercion(program, callable, cast)
                        && term(program, operators, callable, cast.value)
                }
                ExpressionNode::Call(_) => {
                    if let Some((primitive, source)) =
                        validation::integer_embedding_argument(program, expression)
                    {
                        return primitive.accepts_integer_literal()
                            && term(program, operators, callable, source);
                    }
                    callable
                        .scalar_reference(program, expression)
                        .and_then(|reference| program.primitive_type_reference(reference))
                        .is_some_and(|primitive| primitive.accepts_integer_literal())
                }
                _ => false,
            }
        }

        /*
            Arithmetic propositions observe readable numeric contents, not reference
            addresses. Keep the declared type for operator/policy and access judgments;
            this projection supplies only the leaf's primitive carrier. Write-only
            references have no numeric observation, even beneath another shell.
        */
        fn contents_primitive(
            program: &dyn ::validation::CheckingInput,
            callable: &Callable<'_>,
            expression: ExpressionHandle,
        ) -> Option<PrimitiveType> {
            let declared = callable.scalar_reference(program, expression)?;
            let readable = validation::field_domains::readable_type_reference(program, declared)?;
            program.primitive_type_reference(readable)
        }

        /*
            A selected builtin unsigned remainder has one mathematical value in every
            integer policy once division-by-zero has been discharged. This adapter does
            not establish that obligation: the callee's `requires` and the independent
            admitted remainder certificate do. The operands retain their selected
            policies; no signed remainder or mixed carrier is interpreted as unsigned.
        */
        fn builtin_unsigned_remainder(
            program: &dyn ::validation::CheckingInput,
            callable: &Callable<'_>,
            left: ExpressionHandle,
            right: ExpressionHandle,
        ) -> bool {
            let Some(left) = callable.scalar_reference(program, left) else {
                return false;
            };
            let Some(right) = callable.scalar_reference(program, right) else {
                return false;
            };
            let Some(left_primitive) = program.primitive_type_reference(left) else {
                return false;
            };
            let Some(right_primitive) = program.primitive_type_reference(right) else {
                return false;
            };
            left_primitive == right_primitive && unsigned(left_primitive)
        }

        /* Rejoin the operation selected for this exact typed source occurrence. */
        fn admitted_unsigned_remainder(
            program: &dyn ::validation::CheckingInput,
            callable: &Callable<'_>,
            expression: ExpressionHandle,
        ) -> bool {
            let Some(binary) = program.expression_table.binary_node(expression) else {
                return false;
            };
            binary.operator == BinaryOperator::Modulo
                && callable.builtin_meaning(program, expression)
                && builtin_unsigned_remainder(program, callable, binary.left, binary.right)
        }

        /*
            An admitted authored integer coercion preserves its mathematical payload.
            Source formation independently proves any narrowing and target predicates;
            policy qualifications govern later operations. Recasts remain bit views,
            and floating conversions keep their separate denotation proof owner.
        */
        fn value_preserving_integer_coercion(
            program: &dyn ::validation::CheckingInput,
            callable: &Callable<'_>,
            cast: &typed_trees::expression::TableCastExpression,
        ) -> bool {
            !cast.form.is_recast()
                && callable
                    .scalar_reference(program, cast.value)
                    .and_then(|source| program.primitive_type_reference(source))
                    .is_some_and(|source| source.is_fixed_integer())
                && program
                    .primitive_type_reference(cast.target_type)
                    .is_some_and(|target| target.is_fixed_integer())
        }

        fn proof_integer_term(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> bool {
            if validation::integer_embedding_argument(program, expression).is_some() {
                return true;
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Integer(literal) => literal.landing().is_none(),
                ExpressionNode::Binary(binary)
                    if matches!(
                        binary.operator,
                        BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
                    ) =>
                {
                    proof_integer_term(program, binary.left)
                        && proof_integer_term(program, binary.right)
                }
                _ => false,
            }
        }
    }
    pub(in crate::checks) mod callable {
        /*
            Contract arithmetic retains its declaration's parameter identities: a
            machine, a bodyless requirement, or a transparent proposition. Neither a
            requirement nor a proposition acquires a manufactured executable body.
        */
        use symbols::SymbolHandle;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
        use typed_trees::machine::Machine;
        use typed_trees::proposition::PropositionDefinition;
        use typed_trees::signature::{SignatureContract, StateParameter, StateSignature};
        use typed_trees::state::State;
        use typed_trees::types::TypeReferenceHandle;
        use validation::expression_result_type_reference;
        use validation::find_machine_head;
        use validation::has_builtin_bound_expression_meaning;
        use validation::has_builtin_decomposed_guard_meaning;
        use validation::has_builtin_parameter_bound_expression_meaning;
        use validation::parameter_expression_result_type_reference;
        use validation::reserved_result_place;
        use validation::selected_requirement_signature;

        #[derive(Clone, Copy)]
        pub(in crate::checks) enum Callable<'program> {
            Machine {
                machine: &'program Machine,
                state: &'program State,
            },
            Requirement {
                signature: &'program StateSignature,
            },
            Proposition {
                declaration: &'program PropositionDefinition,
            },
        }

        impl<'program> Callable<'program> {
            pub(super) fn resolve(
                program: &'program dyn ::validation::CheckingInput,
                target: SymbolHandle,
            ) -> Option<Self> {
                if !target.is_valid() {
                    return None;
                }
                let machines = find_machine_head(program, target)
                    .map(|(machine, state)| Self::Machine { machine, state });
                let requirements = selected_requirement_signature(program, target)
                    .map(|(_, signature)| Self::Requirement { signature })
                    .or_else(|| {
                        program
                            .machine_parameter_signature(target)
                            .map(|(_, signature)| Self::Requirement { signature })
                    });
                let mut candidates = machines.into_iter().chain(requirements);
                let selected = candidates.next()?;
                let result_type = match selected {
                    Self::Machine { state, .. } => state.return_type,
                    Self::Requirement { signature } => signature.return_type,
                    Self::Proposition { .. } => TypeReferenceHandle::invalid(),
                };
                (candidates.next().is_none()
                    && (!result_type.is_valid()
                        || program
                            .type_reference_table
                            .contains_type_reference(result_type)))
                .then_some(selected)
            }

            pub(in crate::checks) fn owner_symbol(self) -> SymbolHandle {
                match self {
                    Self::Machine { machine, .. } => machine.symbol,
                    Self::Requirement { signature } => signature.symbol,
                    Self::Proposition { declaration } => declaration.symbol,
                }
            }

            pub(in crate::checks) fn target_symbol(self) -> SymbolHandle {
                match self {
                    Self::Machine { state, .. } => state.symbol,
                    Self::Requirement { signature } => signature.symbol,
                    Self::Proposition { declaration } => declaration.symbol,
                }
            }

            pub(in crate::checks) fn parameters(
                self,
                program: &'program dyn ::validation::CheckingInput,
            ) -> &'program [StateParameter] {
                match self {
                    Self::Machine { state, .. } => program.state_parameters(state),
                    Self::Requirement { signature } => {
                        program.state_signature_parameters(signature)
                    }
                    Self::Proposition { declaration } => {
                        program.proposition_parameters(declaration)
                    }
                }
            }

            pub(super) fn result_type(self) -> TypeReferenceHandle {
                match self {
                    Self::Machine { state, .. } => state.return_type,
                    Self::Requirement { signature } => signature.return_type,
                    Self::Proposition { .. } => TypeReferenceHandle::invalid(),
                }
            }

            pub(in crate::checks) fn contracts(
                self,
                program: &'program dyn ::validation::CheckingInput,
            ) -> impl Iterator<Item = &'program SignatureContract> {
                let (first, second): (&[SignatureContract], &[SignatureContract]) = match self {
                    Self::Machine { machine, state } => (
                        if program
                            .machine_states(machine)
                            .first()
                            .is_some_and(|entry| entry.symbol == state.symbol)
                        {
                            program.machine_contracts(machine)
                        } else {
                            &[]
                        },
                        program.state_contracts(state),
                    ),
                    Self::Requirement { signature } => {
                        (program.state_signature_contracts(signature), &[])
                    }
                    Self::Proposition { .. } => (&[], &[]),
                };
                first.iter().chain(second)
            }

            pub(in crate::checks) fn scalar_reference(
                self,
                program: &dyn ::validation::CheckingInput,
                expression: ExpressionHandle,
            ) -> Option<TypeReferenceHandle> {
                if let Some(result) = reserved_result_place(program, expression) {
                    return (result.machine_symbol == self.owner_symbol())
                        .then_some(result.type_reference);
                }
                match self {
                    Self::Machine { machine, state } => {
                        expression_result_type_reference(program, machine, state, expression)
                    }
                    Self::Proposition { declaration } => {
                        parameter_expression_result_type_reference(
                            program,
                            declaration.symbol,
                            program.proposition_parameters(declaration),
                            expression,
                        )
                    }
                    Self::Requirement { signature } => parameter_expression_result_type_reference(
                        program,
                        signature.symbol,
                        program.state_signature_parameters(signature),
                        expression,
                    ),
                }
            }

            pub(in crate::checks) fn builtin_meaning(
                self,
                program: &dyn ::validation::CheckingInput,
                expression: ExpressionHandle,
            ) -> bool {
                match self {
                    Self::Machine { machine, state } => has_builtin_bound_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        expression,
                    ),
                    Self::Proposition { declaration } => {
                        has_builtin_parameter_bound_expression_meaning(
                            program,
                            declaration.symbol,
                            program.proposition_parameters(declaration),
                            expression,
                        )
                    }
                    Self::Requirement { signature } => {
                        has_builtin_parameter_bound_expression_meaning(
                            program,
                            signature.symbol,
                            program.state_signature_parameters(signature),
                            expression,
                        )
                    }
                }
            }

            pub(super) fn decomposed_builtin_meaning(
                self,
                program: &dyn ::validation::CheckingInput,
                expression: ExpressionHandle,
            ) -> bool {
                match self {
                    Self::Machine { machine, state } => has_builtin_decomposed_guard_meaning(
                        program,
                        machine,
                        Some(state),
                        expression,
                    ),
                    Self::Requirement { .. } | Self::Proposition { .. } => {
                        if matches!(program.expression_table.expression(expression),
                            ExpressionNode::Binary(binary)
                                if matches!(binary.operator, BinaryOperator::And
                                    | BinaryOperator::Or))
                        {
                            return true;
                        }
                        self.builtin_meaning(program, expression)
                    }
                }
            }
        }
    }
    use callable::Callable;
    mod availability {
        /*
            Contract and borrow consumers share call establishment, source identity,
            and capture preservation. Availability comes from the exact use's live
            flow contexts, never a certificate's assertion of dominance.
        */
        use validation::CallSite;
        use validation::CanonicalPlace;

        use super::{
            Invocation, actual_projection, builtin_predicate, capture_preserved, captured_place,
            invocation, stable_predicate_inputs,
        };

        use checked_trees::FlowStateFact;
        use facts::{ContractFactKind, FactOrigin, FactPayload, FactPlan, PlaceRoot, ProgramPoint};
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::state::State;

        #[derive(Clone)]
        pub(in crate::checks) struct AvailableGuarantee<'program> {
            pub(in crate::checks) expression: ExpressionHandle,
            pub(in crate::checks) fact: arena::Handle<typed_trees::domain::ProofFact>,
            pub(super) invocation: Invocation<'program>,
            /* These subjects describe normal-return storage, not invocation inputs. */
            pub(super) post_return_writes: Option<Vec<CanonicalPlace>>,
        }

        impl AvailableGuarantee<'_> {
            pub(in crate::checks) fn coordinates(&self) -> (usize, usize) {
                (self.invocation.statement, self.invocation.ordinal)
            }

            pub(in crate::checks) fn has_builtin_meaning(
                &self,
                program: &dyn ::validation::CheckingInput,
                expression: ExpressionHandle,
            ) -> bool {
                self.invocation
                    .callable
                    .decomposed_builtin_meaning(program, expression)
            }

            /*
                The call expression's own handle when this guarantee's call site is a
                call nested inside a statement expression. Statement-site calls have
                no such operand — their results always bind a local name.
            */
            pub(in crate::checks) fn call_expression(&self) -> Option<ExpressionHandle> {
                match self.invocation.site {
                    CallSite::Expression { expression, .. } => Some(expression),
                    _ => None,
                }
            }

            /*
                The member-projection segments behind this operand when it resolves
                to this guarantee's own reserved result (`result` or `result.first`).
                A projection rooted at a different callable's result does not qualify.
            */
            pub(in crate::checks) fn result_segments(
                &self,
                program: &dyn ::validation::CheckingInput,
                expression: ExpressionHandle,
            ) -> Option<Vec<facts::PlaceSegment>> {
                let result = validation::reserved_result_place(program, expression)?;
                (result.machine_symbol == self.invocation.callable.owner_symbol())
                    .then_some(result.segments)
            }

            /*
                The argument expression behind one contract operand, with the
                projection segments the operand carries beyond the parameter's own
                place (`value.first` → the actual plus `[Field{first}]`). Literal
                operands carry no projection.
            */
            pub(in crate::checks) fn actual_projection(
                &self,
                program: &dyn ::validation::CheckingInput,
                expression: ExpressionHandle,
            ) -> Option<(ExpressionHandle, Vec<facts::PlaceSegment>)> {
                if matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::Integer(_)
                ) {
                    return Some((expression, Vec::new()));
                }
                actual_projection(program, &self.invocation, expression)
            }

            /*
                The initial assignment names an occurrence; it is never evaluated.
                Live provenance must independently join the binding to this exact
                call. A mutable local supplies the guarantee only while that
                provenance still pins it to the same occurrence the call produced;
                reassignment retires the assigned-value row, so the guarantee stops
                binding. Immutable copies normalize through the existing bound reader.
                Returns the binding symbol and whether that storage is mutable.
            */
            pub(in crate::checks) fn result_binding(
                &self,
                program: &dyn ::validation::CheckingInput,
                semantic: &FactPlan,
                contexts: &[facts::FactContextHandle],
                state: &State,
            ) -> Option<(SymbolHandle, bool)> {
                if state.symbol != self.invocation.caller_state {
                    return None;
                }
                let local = program
                    .statement_table
                    .statement_at(state, self.invocation.statement)?
                    .as_local_data()?;
                let CallSite::Expression { expression, .. } = self.invocation.site else {
                    return None;
                };
                if !local.symbol.is_valid() || local.initial_value != expression {
                    return None;
                }
                let place = captured_place(
                    program,
                    semantic,
                    contexts,
                    CanonicalPlace {
                        root: PlaceRoot::Symbol(local.symbol),
                        segments: Vec::new(),
                    },
                )?;
                (place.root == PlaceRoot::Expression(expression) && place.segments.is_empty())
                    .then_some((local.symbol, local.is_mutable))
            }
        }

        /*
            Statement entry excludes every call in that statement. An evaluated use
            may consume same-statement guarantees, but only those present in its exact
            retained flow context (for example the pre-store frontier after operands).
        */
        #[derive(Clone, Copy)]
        pub(crate) enum GuaranteeUse {
            StatementEntry(usize),
            EvaluatedContext(usize),
        }

        pub(in crate::checks) fn available<'program>(
            program: &'program dyn ::validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            caller: &FlowStateFact,
            use_point: GuaranteeUse,
            contexts: &[facts::FactContextHandle],
            frames: &validation::CallFrameResolver<'_>,
        ) -> Vec<AvailableGuarantee<'program>> {
            let Some(caller_machine) = program.machine_by_symbol(caller.machine_symbol) else {
                return Vec::new();
            };
            scan(
                facts,
                caller,
                use_point,
                contexts,
                |statement, ordinal, source, expression| {
                    admit(
                        program,
                        facts,
                        caller,
                        caller_machine,
                        frames,
                        statement,
                        ordinal,
                        source,
                        expression,
                    )
                },
                |_| false,
            )
            .0
        }

        /*
            Whether any available guarantee satisfies `accept`, visiting them in the
            order `available` returns them and stopping at the first that does.
            Later guarantees are never admitted; admission only reads its inputs,
            so skipping it changes no answer.
        */
        pub(in crate::checks) fn any_available<'program>(
            program: &'program dyn ::validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            caller: &FlowStateFact,
            use_point: GuaranteeUse,
            contexts: &[facts::FactContextHandle],
            frames: &validation::CallFrameResolver<'_>,
            accept: impl FnMut(&AvailableGuarantee<'program>) -> bool,
        ) -> bool {
            let Some(caller_machine) = program.machine_by_symbol(caller.machine_symbol) else {
                return false;
            };
            scan(
                facts,
                caller,
                use_point,
                contexts,
                |statement, ordinal, source, expression| {
                    admit(
                        program,
                        facts,
                        caller,
                        caller_machine,
                        frames,
                        statement,
                        ordinal,
                        source,
                        expression,
                    )
                },
                accept,
            )
            .1
        }

        /*
            One state's call guarantees, asked at a sequence of use points. Whether a
            call's guarantee is admitted (its declaration owns it, its inputs are
            stable and the call preserves them) does not depend on where it is used;
            only its presence in the use's live contexts does. Borrow checks ask at
            every statement, so re-deciding admission for every earlier call made
            each statement cost as much as all the calls before it.

            Each admission is decided at most once and kept here. That is sound
            because an admission is a function of the inputs this value holds by
            shared reference, which cannot change while it is borrowed (the program,
            the check facts, the caller state and the frame resolver), plus the
            call's coordinates, source fact and expression, which are the memo key.
            The use point and contexts, the only per-use inputs, are applied afresh
            on every question. A value lives for one state's check and is dropped.
        */
        pub(in crate::checks) struct StateGuarantees<'a, 'program> {
            program: &'program dyn ::validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'a>,
            caller: &'a FlowStateFact,
            frames: Option<&'a validation::CallFrameResolver<'a>>,
            /* Indexed by statement; one row per (ordinal, source, expression). */
            admitted: Vec<Vec<Admission<'program>>>,
            /*
                The same state's completed-call hypotheses, lent to each
                current-value replay; see `CompletedCallGuarantees`.
            */
            completed_calls: crate::checks::current_scalar_values::CompletedCallGuarantees,
        }

        struct Admission<'program> {
            ordinal: usize,
            source: arena::Handle<typed_trees::domain::ProofFact>,
            expression: ExpressionHandle,
            guarantee: Option<AvailableGuarantee<'program>>,
        }

        impl<'a, 'program> StateGuarantees<'a, 'program> {
            pub(in crate::checks) fn new(
                program: &'program dyn ::validation::CheckingInput,
                facts: checked_trees::CheckFactsRef<'a>,
                caller: &'a FlowStateFact,
                frames: Option<&'a validation::CallFrameResolver<'a>>,
            ) -> Self {
                Self {
                    program,
                    facts,
                    caller,
                    frames,
                    admitted: Vec::new(),
                    completed_calls: Default::default(),
                }
            }

            pub(in crate::checks) fn completed_calls(
                &mut self,
            ) -> &mut crate::checks::current_scalar_values::CompletedCallGuarantees {
                &mut self.completed_calls
            }

            pub(in crate::checks) fn caller(&self) -> &'a FlowStateFact {
                self.caller
            }

            /* The same guarantees, in the same order, as the free `available`. */
            pub(in crate::checks) fn available(
                &mut self,
                use_point: GuaranteeUse,
                contexts: &[facts::FactContextHandle],
            ) -> Vec<AvailableGuarantee<'program>> {
                self.walk(use_point, contexts, |_| false).0
            }

            /* The same answer as the free `any_available`. */
            pub(in crate::checks) fn any_available(
                &mut self,
                use_point: GuaranteeUse,
                contexts: &[facts::FactContextHandle],
                accept: impl FnMut(&AvailableGuarantee<'program>) -> bool,
            ) -> bool {
                self.walk(use_point, contexts, accept).1
            }

            fn walk(
                &mut self,
                use_point: GuaranteeUse,
                contexts: &[facts::FactContextHandle],
                stop: impl FnMut(&AvailableGuarantee<'program>) -> bool,
            ) -> (Vec<AvailableGuarantee<'program>>, bool) {
                let Some(frames) = self.frames else {
                    return (Vec::new(), false);
                };
                let program = self.program;
                let Some(caller_machine) = program.machine_by_symbol(self.caller.machine_symbol)
                else {
                    return (Vec::new(), false);
                };
                let (facts, caller, admitted) = (self.facts, self.caller, &mut self.admitted);
                scan(
                    facts,
                    caller,
                    use_point,
                    contexts,
                    |statement, ordinal, source, expression| {
                        if admitted.len() <= statement {
                            admitted.resize_with(statement + 1, Vec::new);
                        }
                        let row = &mut admitted[statement];
                        if let Some(known) = row.iter().find(|known| {
                            known.ordinal == ordinal
                                && known.source == source
                                && known.expression == expression
                        }) {
                            return known.guarantee.clone();
                        }
                        let guarantee = admit(
                            program,
                            facts,
                            caller,
                            caller_machine,
                            frames,
                            statement,
                            ordinal,
                            source,
                            expression,
                        );
                        row.push(Admission {
                            ordinal,
                            source,
                            expression,
                            guarantee: guarantee.clone(),
                        });
                        guarantee
                    },
                    stop,
                )
            }
        }

        /*
            The live call guarantees at one use, in context order and without
            duplicates. `admit` decides each call's use-independent admission;
            `stop` ends the walk early, and the second result says whether it did.
        */
        fn scan<'program>(
            facts: checked_trees::CheckFactsRef<'_>,
            caller: &FlowStateFact,
            use_point: GuaranteeUse,
            contexts: &[facts::FactContextHandle],
            mut admit: impl FnMut(
                usize,
                usize,
                arena::Handle<typed_trees::domain::ProofFact>,
                ExpressionHandle,
            ) -> Option<AvailableGuarantee<'program>>,
            mut stop: impl FnMut(&AvailableGuarantee<'program>) -> bool,
        ) -> (Vec<AvailableGuarantee<'program>>, bool) {
            let mut guarantees = Vec::new();
            for fact in contexts.iter().flat_map(|context| {
                facts
                    .semantic
                    .context_view(facts.semantic.contexts.get(*context))
                    .facts()
            }) {
                let FactPayload::ContractBooleanExpression {
                    kind: ContractFactKind::Ensures,
                    fact: source,
                    expression,
                    ..
                } = fact.payload
                else {
                    continue;
                };
                let ProgramPoint::CallEnsures {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    call_ordinal,
                } = fact.point
                else {
                    continue;
                };
                if fact.origin != FactOrigin::CallEnsures
                    || machine_symbol != caller.machine_symbol
                    || state_symbol != caller.state_symbol
                    || match use_point {
                        GuaranteeUse::StatementEntry(statement) => statement_index >= statement,
                        GuaranteeUse::EvaluatedContext(statement) => statement_index > statement,
                    }
                {
                    continue;
                }
                if guarantees.iter().any(|prior: &AvailableGuarantee<'_>| {
                    prior.fact == source && prior.coordinates() == (statement_index, call_ordinal)
                }) {
                    continue;
                }
                let Some(guarantee) = admit(statement_index, call_ordinal, source, expression)
                else {
                    continue;
                };
                if stop(&guarantee) {
                    return (guarantees, true);
                }
                guarantees.push(guarantee);
            }
            (guarantees, false)
        }

        /*
            Everything that decides whether one call's guarantee is available and
            does not depend on where it is used.
        */
        #[allow(clippy::too_many_arguments)]
        fn admit<'program>(
            program: &'program dyn ::validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            caller: &FlowStateFact,
            caller_machine: &typed_trees::machine::Machine,
            frames: &validation::CallFrameResolver<'_>,
            statement_index: usize,
            call_ordinal: usize,
            source: arena::Handle<typed_trees::domain::ProofFact>,
            expression: ExpressionHandle,
        ) -> Option<AvailableGuarantee<'program>> {
            let supplied = invocation(program, caller, statement_index, call_ordinal)?;
            /*
                Every test below must pass and none has effects, so their order only
                sets the cost of a rejection: the two that read the declaration
                alone run before the ones that walk call dependencies or flow.
            */
            if !owns_guarantee(program, &supplied, source, expression)
                || !builtin_predicate(program, facts.operators, &supplied, expression)
            {
                return None;
            }
            if !validation::independent_call_site(
                program,
                caller.machine_symbol,
                supplied.callable.owner_symbol(),
                &supplied.site,
            ) && !super::induction::strict_recursive_call(program, facts, caller, &supplied)
            {
                return None;
            }
            if !stable_predicate_inputs(program, facts, caller, &supplied, expression)
                || !capture_preserved(
                    program,
                    facts.operators,
                    facts.borrow,
                    caller_machine,
                    &supplied,
                    expression,
                    frames,
                )
            {
                return None;
            }
            let post_return_writes = super::invocation_fact(facts, caller, &supplied)
                .filter(|call| call.normal_return_writes_complete)
                .map(|call| {
                    facts
                        .flow
                        .control
                        .operand_origin_writes
                        .span_or_empty(call.normal_return_writes)
                        .iter()
                        .map(|write| {
                            crate::checks::places::place_or_root_in(
                                &facts.flow.control.operand_referent_segments,
                                write.root,
                                write.segments,
                            )
                        })
                        .collect()
                });
            Some(AvailableGuarantee {
                expression,
                fact: source,
                invocation: supplied,
                post_return_writes,
            })
        }

        fn owns_guarantee(
            program: &dyn ::validation::CheckingInput,
            supplied: &Invocation<'_>,
            source: arena::Handle<typed_trees::domain::ProofFact>,
            expression: ExpressionHandle,
        ) -> bool {
            source.is_valid()
                && matches!(program.proof_facts.get(source), typed_trees::domain::ProofFact::Expression(actual) if *actual == expression)
                && supplied
                    .callable
                    .contracts(program)
                    .filter(|contract| {
                        contract.kind == typed_trees::signature::SignatureContractKind::Ensures
                    })
                    .any(|contract| contract.facts.handles().any(|fact| fact == source))
        }
    }
    mod induction {
        /*
            A recursive guarantee is available only after independent strict descent
            at its exact call edge. Complete body admission, actual argument capture,
            premise checking and post-return preservation remain separate obligations.
        */
        use super::Invocation;
        use checked_trees::FlowStateFact;

        pub(super) fn strict_recursive_call(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            caller: &FlowStateFact,
            invocation: &Invocation<'_>,
        ) -> bool {
            if invocation.callable.owner_symbol() != caller.machine_symbol
                || invocation.caller_state != caller.state_symbol
                || invocation
                    .site
                    .is_internal_state_transfer(program, caller.machine_symbol)
            {
                return false;
            }
            let Some((machine, state)) =
                program.state_flow_site(caller.machine_symbol, caller.state_symbol)
            else {
                return false;
            };
            let plan = &facts.termination.rank_arrivals;
            let Some(judgment) = plan.judgment_for_body(machine.symbol, machine.states) else {
                return false;
            };
            let needs_range = machine
                .termination_plan
                .implementation_witness
                .as_ref()
                .is_some_and(|witness| witness.rank_range.is_some());
            if !judgment.local_terminates
                || !judgment.diagnostic.is_empty()
                || (needs_range && !judgment.entry_rank_formed)
            {
                return false;
            }
            let Some(correspondence) = plan.for_body(machine.symbol, machine.states) else {
                return false;
            };
            let Some(target) = validation::transition_call_target(
                program,
                machine,
                state,
                invocation.statement,
                invocation.ordinal,
            ) else {
                return false;
            };
            target.is_valid()
                && plan
                    .arrival_edges
                    .span_or_empty(correspondence.edges)
                    .iter()
                    .any(|edge| {
                        edge.source == state.symbol
                            && edge.destination == correspondence.entry_state
                            && usize::try_from(edge.statement_ordinal).ok()
                                == Some(invocation.statement)
                            && edge.target == target
                            && edge.strictly_decreases
                    })
        }
    }
    mod static_operands {
        /*
            Immutable static actuals use the occurrence's admitted declaration tuple.
            This joins retained operands, never infers a provider or rechecks a body.
            Runtime Value subjects keep their ordinary capture/preservation obligation.
        */
        mod projections {
            /*
                Project immutable aggregate actuals only after the parent joins the exact
                invocation tuple. Constructor children keep their supplying owner/roster;
                encoded values replay their entire schema before selecting any field.
                This never reads runtime storage or executes a const initializer.
            */
            use super::{Invocation, arithmetic};
            use language_semantics::const_value::{
                CanonicalConstValue, DecodedCanonicalConstValue,
            };
            use symbols::SymbolHandle;
            use typed_trees::data::TypeParameterKind;
            use typed_trees::expression::{ExpressionHandle, ExpressionNode};
            use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
            use validation::{
                ConstProjectionStep, ImmutableConstProjection, ScopedArithmeticValue,
            };

            pub(super) fn atom(
                program: &dyn validation::CheckingInput,
                projection: &ImmutableConstProjection,
                primitive: PrimitiveType,
            ) -> Option<ScopedArithmeticValue> {
                (program.primitive_type_reference(projection.carrier) == Some(primitive)).then(
                    || ScopedArithmeticValue::Atom {
                        identity: format!(
                            "\0static-parameter:{:?}:path:{:?}",
                            projection.root, projection.path
                        ),
                        unsigned: matches!(
                            primitive,
                            typed_trees::unsigned_integer_primitive_pattern!()
                        ),
                    },
                )
            }

            pub(super) fn value(
                program: &dyn validation::CheckingInput,
                facts: &checked_trees::CheckFacts,
                invocation: &Invocation<'_>,
                reference: TypeReferenceHandle,
                projection: &ImmutableConstProjection,
                primitive: PrimitiveType,
            ) -> Option<ScopedArithmeticValue> {
                let parameter = program.data_type_parameter_by_symbol(projection.root)?;
                let TypeParameterKind::Const {
                    type_reference: carrier,
                } = parameter.kind
                else {
                    return None;
                };
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::ConstExpression(expression) => expression_value(
                        program,
                        facts,
                        invocation,
                        *expression,
                        carrier,
                        &projection.path,
                        primitive,
                        &mut Vec::new(),
                    ),
                    TypeReferenceNode::Named { symbol, name } => {
                        if let Some(parameter) = program.data_type_parameter_by_symbol(*symbol) {
                            let TypeParameterKind::Const {
                                type_reference: actual,
                            } = parameter.kind
                            else {
                                return None;
                            };
                            if program.normalized_type_identity(actual)
                                != program.normalized_type_identity(carrier)
                            {
                                return None;
                            }
                            return atom(
                                program,
                                &ImmutableConstProjection {
                                    root: *symbol,
                                    path: projection.path.clone(),
                                    carrier: projection.carrier,
                                },
                                primitive,
                            );
                        }
                        let value = if symbol.is_valid() {
                            let declaration = program.const_declaration_by_symbol(*symbol)?;
                            if program.normalized_type_identity(declaration.declared_type)
                                != program.normalized_type_identity(carrier)
                            {
                                return None;
                            }
                            CanonicalConstValue::new(
                                program.normalized_type_identity_text(carrier),
                                declaration.evaluated_value.as_ref()?.encoding()?,
                                "",
                            )
                        } else {
                            CanonicalConstValue::from_atom(name.as_str())?
                        };
                        decoded_value(program, carrier, &projection.path, primitive, value)
                    }
                    _ => None,
                }
            }

            /*
                Walk a projection path through its declared carriers. Each field step
                names its declared field; each element step stays inside the fixed extent.
            */
            fn schema(
                program: &dyn validation::CheckingInput,
                mut carrier: TypeReferenceHandle,
                path: &[ConstProjectionStep],
            ) -> Option<TypeReferenceHandle> {
                for step in path {
                    carrier = step_carrier(program, carrier, *step)?;
                }
                Some(carrier)
            }

            fn step_carrier(
                program: &dyn validation::CheckingInput,
                carrier: TypeReferenceHandle,
                step: ConstProjectionStep,
            ) -> Option<TypeReferenceHandle> {
                match step {
                    ConstProjectionStep::Field(field) => {
                        field_step(program, carrier, field).map(|(_, carrier)| carrier)
                    }
                    ConstProjectionStep::Element(index) => {
                        let (element, length) = validation::fixed_array_geometry(program, carrier)?;
                        (index < length).then_some(element)
                    }
                }
            }

            /* The declared name and carrier of one exact record field of `carrier`. */
            fn field_step(
                program: &dyn validation::CheckingInput,
                carrier: TypeReferenceHandle,
                field: SymbolHandle,
            ) -> Option<(String, TypeReferenceHandle)> {
                let reference = program.unwrapped_type_reference(carrier)?;
                let symbol = program
                    .type_reference_table
                    .type_reference(reference)
                    .as_named_symbol()?;
                let definition = program.data_definition_by_symbol(symbol)?;
                let mut selected = program
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| member.as_field())
                    .filter(|candidate| candidate.symbol == field);
                let selected = selected.next().filter(|_| selected.next().is_none())?;
                Some((selected.name.as_str().to_owned(), selected.type_reference))
            }

            fn decoded_value(
                program: &dyn validation::CheckingInput,
                carrier: TypeReferenceHandle,
                path: &[ConstProjectionStep],
                primitive: PrimitiveType,
                value: CanonicalConstValue,
            ) -> Option<ScopedArithmeticValue> {
                if program.type_identity_by_text(&value.type_name)
                    != program.normalized_type_identity(carrier)
                {
                    return None;
                }
                validation::validate_exact_const_value_encoding(program, carrier, &value.encoding)
                    .ok()?;
                let terminal = schema(program, carrier, path)?;
                if program.primitive_type_reference(terminal) != Some(primitive) {
                    return None;
                }
                let mut decoded = value.decode_encoding()?;
                let mut current = carrier;
                for step in path {
                    decoded = match (step, decoded) {
                        (
                            ConstProjectionStep::Field(field),
                            DecodedCanonicalConstValue::Record { fields, .. },
                        ) => {
                            let (name, _) = field_step(program, current, *field)?;
                            fields.into_iter().find(|(field, _)| *field == name)?.1
                        }
                        (
                            ConstProjectionStep::Element(index),
                            DecodedCanonicalConstValue::Array { values, .. },
                        ) => values.into_iter().nth(*index)?,
                        _ => return None,
                    };
                    current = step_carrier(program, current, *step)?;
                }
                let DecodedCanonicalConstValue::Integer { type_name, value } = decoded else {
                    return None;
                };
                (type_name == primitive.name()).then(|| {
                    ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_i128(value))
                })
            }

            fn expression_value(
                program: &dyn validation::CheckingInput,
                facts: &checked_trees::CheckFacts,
                invocation: &Invocation<'_>,
                expression: ExpressionHandle,
                carrier: TypeReferenceHandle,
                path: &[ConstProjectionStep],
                primitive: PrimitiveType,
                active: &mut Vec<ExpressionHandle>,
            ) -> Option<ScopedArithmeticValue> {
                if !program.expression_table.expression_is_valid(expression)
                    || active.contains(&expression)
                {
                    return None;
                }
                if path.is_empty() {
                    let handle = match invocation.site {
                        validation::CallSite::Expression { call, .. } => {
                            call.authored_machine_application
                        }
                        validation::CallSite::Statement(call) => call.authored_machine_application,
                        validation::CallSite::TransitionNamed { .. } => return None,
                    };
                    let application = program.expression_table.static_machine_application(handle);
                    let machine = program.machine_by_symbol(application.lexical_owner)?;
                    let state = program.machine_states(machine).first()?;
                    return arithmetic::scalar_term(
                        program,
                        facts.view(),
                        &super::Callable::Machine { machine, state },
                        expression,
                        |_, _| None,
                    );
                }
                active.push(expression);
                let result = match program.expression_table.expression(expression) {
                    ExpressionNode::StructLiteral(literal)
                        if literal.case_symbol.is_none() && literal.case_name.is_none() =>
                    {
                        let ConstProjectionStep::Field(field) = path[0] else {
                            return None;
                        };
                        let reference = program.unwrapped_type_reference(carrier)?;
                        let symbol = program
                            .type_reference_table
                            .type_reference(reference)
                            .as_named_symbol()?;
                        if symbol != literal.type_symbol {
                            return None;
                        }
                        let definition = program.data_definition_by_symbol(symbol)?;
                        let declared = program
                            .data_members(definition)
                            .iter()
                            .map(|member| member.as_field())
                            .collect::<Option<Vec<_>>>()?;
                        let supplied = program.expression_table.struct_fields(literal.fields);
                        if declared.len() != supplied.len()
                            || declared.iter().any(|field| {
                                supplied
                                    .iter()
                                    .filter(|value| value.field_symbol == field.symbol)
                                    .count()
                                    != 1
                            })
                        {
                            return None;
                        }
                        let child_carrier = step_carrier(program, carrier, path[0])?;
                        let selected = supplied.iter().find(|value| value.field_symbol == field)?;
                        expression_value(
                            program,
                            facts,
                            invocation,
                            selected.value,
                            child_carrier,
                            &path[1..],
                            primitive,
                            active,
                        )
                    }
                    ExpressionNode::ArrayLiteral(elements) => {
                        let ConstProjectionStep::Element(index) = path[0] else {
                            return None;
                        };
                        let (_, length) = validation::fixed_array_geometry(program, carrier)?;
                        let elements = program.expression_table.expression_handles(*elements);
                        if elements.len() != length {
                            return None;
                        }
                        let child_carrier = step_carrier(program, carrier, path[0])?;
                        expression_value(
                            program,
                            facts,
                            invocation,
                            *elements.get(index)?,
                            child_carrier,
                            &path[1..],
                            primitive,
                            active,
                        )
                    }
                    ExpressionNode::Name(name) => {
                        if let Some(parameter) = program.data_type_parameter_by_symbol(name.symbol)
                        {
                            let TypeParameterKind::Const {
                                type_reference: actual,
                            } = parameter.kind
                            else {
                                return None;
                            };
                            if program.normalized_type_identity(actual)
                                != program.normalized_type_identity(carrier)
                            {
                                return None;
                            }
                            let terminal = schema(program, actual, path)?;
                            atom(
                                program,
                                &ImmutableConstProjection {
                                    root: parameter.symbol,
                                    path: path.to_vec(),
                                    carrier: terminal,
                                },
                                primitive,
                            )
                        } else {
                            let declaration = program.const_declaration_by_symbol(name.symbol)?;
                            if program.normalized_type_identity(declaration.declared_type)
                                != program.normalized_type_identity(carrier)
                            {
                                return None;
                            }
                            decoded_value(
                                program,
                                carrier,
                                path,
                                primitive,
                                CanonicalConstValue::new(
                                    program.normalized_type_identity_text(carrier),
                                    declaration.evaluated_value.as_ref()?.encoding()?,
                                    "",
                                ),
                            )
                        }
                    }
                    _ => None,
                };
                active.pop();
                result
            }
        }

        use super::{Callable, Invocation, arithmetic};
        use crate::flow::expressions;
        use crate::published::facts::contract_occurrences::append_expression_occurrences;
        use symbols::SymbolHandle;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
        use typed_trees::types::{PrimitiveType, TypeReferenceNode};
        use validation::{
            CallSite, ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticValue,
        };

        fn has_template(
            program: &dyn validation::CheckingInput,
            mut selected: SymbolHandle,
            declaration: SymbolHandle,
        ) -> bool {
            let mut seen = Vec::new();
            while selected != declaration {
                if seen.contains(&selected) {
                    return false;
                }
                seen.push(selected);
                match program.executable_application_template(selected) {
                    Ok(Some(template)) => selected = template,
                    _ => return false,
                }
            }
            true
        }

        fn tuple<'program>(
            program: &'program dyn validation::CheckingInput,
            invocation: &Invocation<'_>,
        ) -> Option<&'program [StaticMachineBinding]> {
            let (handle, arguments) = match invocation.site {
                CallSite::Expression { call, .. } => (
                    call.authored_machine_application,
                    &call.machine_arguments[..],
                ),
                CallSite::Statement(call) => (
                    call.authored_machine_application,
                    &call.machine_arguments[..],
                ),
                CallSite::TransitionNamed { .. } => return None,
            };
            if !handle.is_valid() {
                return None;
            }
            let application = program.expression_table.static_machine_application(handle);
            let declaration = program.machine_by_symbol(application.binding_declaration)?;
            let Callable::Machine { machine, .. } = invocation.callable else {
                return None;
            };
            let caller = program.machine_holding_state(invocation.caller_state)?;
            if !has_template(program, machine.symbol, declaration.symbol)
                || !has_template(program, caller.symbol, application.lexical_owner)
            {
                return None;
            }
            let parameters = program.machine_type_parameters(declaration);
            let bindings = expressions(program).static_machine_application_bindings(handle);
            if arguments.len() != parameters.len() + declaration.conformance_bounds.len()
                || bindings.len() != arguments.len()
                || expressions(program).static_machine_application_arguments(handle) != arguments
            {
                return None;
            }
            for (position, binding) in bindings.iter().enumerate() {
                if bindings[..position]
                    .iter()
                    .any(|prior| prior.parameter == binding.parameter)
                {
                    return None;
                }
            }
            for (parameter, argument) in parameters.iter().zip(arguments) {
                let binding = bindings
                    .iter()
                    .find(|binding| binding.parameter == parameter.symbol)?;
                if matches!(
                    binding.value,
                    StaticMachineBindingValue::RuntimeValue { .. }
                ) || !program.static_binding_matches_argument(
                    &parameter.kind,
                    &binding.value,
                    argument,
                    application.lexical_owner,
                    application.lexical_owner,
                ) {
                    return None;
                }
            }
            for (bound, argument) in declaration
                .conformance_bounds
                .iter()
                .zip(&arguments[parameters.len()..])
            {
                let binding = bindings
                    .iter()
                    .find(|binding| binding.parameter == bound.binder)?;
                if !matches!(&binding.value, StaticMachineBindingValue::Conformance(selected) if selected == argument)
                {
                    return None;
                }
            }
            Some(bindings)
        }

        pub(super) fn admitted(
            program: &dyn validation::CheckingInput,
            invocation: &Invocation<'_>,
        ) -> bool {
            let arguments_empty = match invocation.site {
                CallSite::Expression { call, .. } => call.machine_arguments.is_empty(),
                CallSite::Statement(call) => call.machine_arguments.is_empty(),
                CallSite::TransitionNamed { .. } => true,
            };
            let open = matches!(invocation.callable, Callable::Machine { machine, .. }
                if !program.machine_type_parameters(machine).is_empty() || !machine.conformance_bounds.is_empty());
            if arguments_empty && !open {
                return true;
            }
            tuple(program, invocation).is_some()
        }

        fn const_symbol(
            program: &dyn validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<SymbolHandle> {
            let path = program.expression_table.name_path_node(expression)?;
            if !validation::is_single_segment_name_path(program, path) {
                return None;
            }
            let parameter = program.data_type_parameter_by_symbol(path.symbol)?;
            matches!(parameter.kind, TypeParameterKind::Const { .. }).then_some(parameter.symbol)
        }

        pub(super) fn immutable(
            program: &dyn validation::CheckingInput,
            expression: ExpressionHandle,
        ) -> bool {
            const_symbol(program, expression).is_some()
                || validation::immutable_const_projection(program, expression).is_some()
        }

        pub(super) fn projection_atom(
            program: &dyn validation::CheckingInput,
            expression: ExpressionHandle,
            primitive: PrimitiveType,
        ) -> Option<ScopedArithmeticValue> {
            projections::atom(
                program,
                &validation::immutable_const_projection(program, expression)?,
                primitive,
            )
        }

        fn is_invocation_formal(
            program: &dyn validation::CheckingInput,
            invocation: &Invocation<'_>,
            symbol: SymbolHandle,
        ) -> Option<bool> {
            match invocation.callable {
                Callable::Machine { machine, .. } => {
                    let mut selected = machine.symbol;
                    let mut seen = Vec::new();
                    loop {
                        if seen.contains(&selected) {
                            return None;
                        }
                        seen.push(selected);
                        let declaration = program.machine_by_symbol(selected)?;
                        if program
                            .machine_type_parameters(declaration)
                            .iter()
                            .any(|parameter| parameter.symbol == symbol)
                        {
                            return Some(true);
                        }
                        match program.executable_application_template(selected).ok()? {
                            Some(template) => selected = template,
                            None => return Some(false),
                        }
                    }
                }
                Callable::Requirement { signature } => Some(
                    program
                        .requirement_invocation_contract_parameters(signature)
                        .iter()
                        .any(|parameter| parameter.symbol == symbol),
                ),
                Callable::Proposition { declaration } => {
                    Some(program.symbols.get(symbol).parent == declaration.symbol)
                }
            }
        }

        pub(super) fn bindings(
            program: &dyn validation::CheckingInput,
            facts: &checked_trees::CheckFacts,
            invocation: &Invocation<'_>,
            expression: ExpressionHandle,
        ) -> Option<Vec<ScopedArithmeticBinding>> {
            let tuple = tuple(program, invocation);
            let mut occurrences = Vec::new();
            append_expression_occurrences(program, expression, &mut occurrences);
            let mut result = Vec::<ScopedArithmeticBinding>::new();
            for occurrence in occurrences {
                if let Some(projection) =
                    validation::immutable_const_projection(program, occurrence)
                {
                    let formal = is_invocation_formal(program, invocation, projection.root)?;
                    let binding = tuple.and_then(|tuple| {
                        tuple
                            .iter()
                            .find(|binding| binding.parameter == projection.root)
                    });
                    let value = if let Some(binding) = binding {
                        let StaticMachineBindingValue::Const(reference) = binding.value else {
                            return None;
                        };
                        let primitive = invocation
                            .callable
                            .scalar_reference(program, occurrence)
                            .and_then(|reference| {
                            program.primitive_type_reference(reference)
                        })?;
                        projections::value(
                            program,
                            facts,
                            invocation,
                            reference,
                            &projection,
                            primitive,
                        )?
                    } else {
                        if formal {
                            return None;
                        }
                        let primitive = program.primitive_type_reference(projection.carrier)?;
                        projections::atom(program, &projection, primitive)?
                    };
                    result.push(ScopedArithmeticBinding {
                        binder: ScopedArithmeticBinder::Occurrence(occurrence),
                        value,
                    });
                    continue;
                }
                let Some(symbol) = const_symbol(program, occurrence) else {
                    continue;
                };
                let formal = is_invocation_formal(program, invocation, symbol)?;
                let binding = tuple
                    .and_then(|tuple| tuple.iter().find(|binding| binding.parameter == symbol));
                let Some(binding) = binding else {
                    if formal {
                        return None;
                    }
                    continue;
                };
                let StaticMachineBindingValue::Const(reference) = binding.value else {
                    return None;
                };
                let primitive = invocation
                    .callable
                    .scalar_reference(program, occurrence)
                    .and_then(|reference| program.primitive_type_reference(reference))?;
                let value = selected_value(program, facts, invocation, reference, primitive)?;
                let binder = ScopedArithmeticBinder::Symbol(symbol);
                if let Some(prior) = result.iter().find(|prior| prior.binder == binder) {
                    if prior.value != value {
                        return None;
                    }
                } else {
                    result.push(ScopedArithmeticBinding { binder, value });
                }
            }
            Some(result)
        }

        fn selected_value(
            program: &dyn validation::CheckingInput,
            facts: &checked_trees::CheckFacts,
            invocation: &Invocation<'_>,
            reference: typed_trees::types::TypeReferenceHandle,
            primitive: PrimitiveType,
        ) -> Option<ScopedArithmeticValue> {
            if let Some(literal) = program.static_const_literal_from_type_reference(reference) {
                return literal.value_bignum().map(ScopedArithmeticValue::Integer);
            }
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Named { symbol, .. } => {
                    let parameter = program.data_type_parameter_by_symbol(*symbol)?;
                    let TypeParameterKind::Const { type_reference } = parameter.kind else {
                        return None;
                    };
                    (program.primitive_type_reference(type_reference) == Some(primitive)).then(
                        || ScopedArithmeticValue::Atom {
                            identity: format!("\0static-parameter:{symbol:?}"),
                            unsigned: matches!(
                                primitive,
                                typed_trees::unsigned_integer_primitive_pattern!()
                            ),
                        },
                    )
                }
                TypeReferenceNode::ConstExpression(actual) => {
                    let (handle, _) = match invocation.site {
                        CallSite::Expression { call, .. } => (
                            call.authored_machine_application,
                            &call.machine_arguments[..],
                        ),
                        CallSite::Statement(call) => (
                            call.authored_machine_application,
                            &call.machine_arguments[..],
                        ),
                        CallSite::TransitionNamed { .. } => return None,
                    };
                    let application = program.expression_table.static_machine_application(handle);
                    let machine = program.machine_by_symbol(application.lexical_owner)?;
                    let state = program.machine_states(machine).first()?;
                    arithmetic::scalar_term(
                        program,
                        facts.view(),
                        &Callable::Machine { machine, state },
                        *actual,
                        |_, _| None,
                    )
                }
                _ => None,
            }
        }
    }
    pub(crate) use availability::GuaranteeUse;
    pub(in crate::checks) use availability::{
        AvailableGuarantee, StateGuarantees, any_available, available,
    };

    #[derive(Clone)]
    struct Invocation<'program> {
        site: CallSite<'program>,
        callable: Callable<'program>,
        caller_state: SymbolHandle,
        statement: usize,
        ordinal: usize,
    }

    fn invocation_fact<'facts>(
        facts: checked_trees::CheckFactsRef<'facts>,
        caller: &FlowStateFact,
        invocation: &Invocation<'_>,
    ) -> Option<&'facts checked_trees::FlowCallFact> {
        let mut calls = crate::flow::call_flows(facts.flow.control, caller.calls)
            .iter()
            .filter(|call| {
                call.statement_index == invocation.statement
                    && call.call_ordinal == invocation.ordinal
            });
        let call = calls.next()?;
        calls.next().is_none().then_some(call)
    }

    /*
        Declaration meaning and observation custody are independent. Invocation
        and exit consumers supply exact value bindings to the same matcher.
    */
    struct PredicateScope<'program, 'bindings> {
        callable: Callable<'program>,
        literal: &'bindings dyn Fn(ExpressionHandle) -> Option<facts::ScalarValue>,
        place: &'bindings dyn Fn(ExpressionHandle) -> Option<CanonicalPlace>,
    }

    /*
        Match a current-storage obligation with an independently available normal
        guarantee. The caller authenticates its declaration projections; the common
        matcher joins them through surviving completed-value provenance.
    */
    pub(in crate::checks) fn proves_current(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        caller: &FlowStateFact,
        statement: usize,
        contexts: &[facts::FactContextHandle],
        frames: &validation::CallFrameResolver<'_>,
        callable: Callable<'_>,
        expression: ExpressionHandle,
        place: &dyn Fn(ExpressionHandle) -> Option<CanonicalPlace>,
    ) -> bool {
        if !super::has_builtin_operators(program, &facts.operators, expression)
            || !callable.builtin_meaning(program, expression)
        {
            return false;
        }
        let required = PredicateScope {
            callable,
            literal: &|expression| crate::flow::literal(program, expression),
            place,
        };
        any_available(
            program,
            facts.view(),
            caller,
            GuaranteeUse::StatementEntry(statement),
            contexts,
            frames,
            |guarantee| {
                let supplied = PredicateScope {
                    callable: guarantee.invocation.callable,
                    literal: &|expression| {
                        bound_literal(program, &guarantee.invocation, expression)
                    },
                    place: &|expression| bound_place(program, &guarantee.invocation, expression),
                };
                predicates_match(
                    program,
                    &facts.semantic,
                    contexts,
                    &required,
                    expression,
                    &supplied,
                    guarantee.expression,
                    &mut Vec::new(),
                )
            },
        )
    }

    /*
        Read surviving normal guarantees in their authenticated caller namespace.
        Parameters observe current storage; results name completed occurrences.
        Availability owns declaration identity, actual capture and write preservation.
        Keep storage bindings intact for write retirement; completed-result joins
        belong to proof scratch after each consumer has selected its live contexts.
    */
    pub(crate) fn current_predicates(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        caller: &FlowStateFact,
        use_point: GuaranteeUse,
        contexts: &[facts::FactContextHandle],
        frames: &validation::CallFrameResolver<'_>,
    ) -> Vec<BoundPredicate> {
        available(program, facts.view(), caller, use_point, contexts, frames)
            .into_iter()
            .filter_map(|guarantee| {
                let invocation = &guarantee.invocation;
                let parameters = invocation.callable.parameters(program);
                let expression = crate::values::lower_callable_scalar_contract_predicate(
                    program,
                    &facts.operators,
                    invocation.callable.owner_symbol(),
                    parameters,
                    invocation.callable.result_type(),
                    guarantee.expression,
                    true,
                    &mut 4096,
                )?;
                let result = guarantee
                    .call_expression()
                    .map(|expression| CanonicalPlace {
                        root: PlaceRoot::Expression(expression),
                        segments: Vec::new(),
                    });
                let mut predicate = boolean_predicates::bind_parameters(
                    program,
                    invocation.callable.owner_symbol(),
                    invocation.callable.target_symbol(),
                    parameters,
                    result
                        .as_ref()
                        .map(|place| (place, invocation.callable.result_type())),
                    expression,
                )?;
                let arguments =
                    validation::call_site_argument_expressions(program, &invocation.site);
                let mut captures = Vec::new();
                for (binding_position, binding) in predicate.bindings.iter_mut().enumerate() {
                    if result
                        .as_ref()
                        .is_some_and(|result| binding.place.root == result.root)
                    {
                        continue;
                    }
                    let position = parameters.iter().position(|parameter| {
                        binding.place.root == PlaceRoot::Symbol(parameter.symbol)
                    })?;
                    let projection = binding.place.segments.clone();
                    let actual = *arguments.get(position)?;
                    let place = projected_argument_place(
                        program,
                        caller.state_symbol,
                        invocation.statement,
                        actual,
                        parameters[position].type_reference,
                        &projection,
                    )?;
                    binding.place = crate::flow::rebase_exact_local_place(
                        program,
                        caller.state_symbol,
                        invocation.statement,
                        place,
                        Some(frames),
                    )?;
                    if crate::flow::operand_binds_stable_copy(
                        program,
                        parameters[position].type_reference,
                    ) && let Some(call) = invocation_fact(facts.view(), caller, invocation)
                        && let Some(actual) = actual_at(program, caller, call, position, projection)
                        && let Some(capture) =
                            captured_operand(program, facts, caller, call, &actual)
                        && let Some(values) = captured_values(program, facts, caller, call, &actual)
                    {
                        for value in &values {
                            retain_bindings(
                                program,
                                facts,
                                value,
                                capture.row(),
                                std::slice::from_ref(binding),
                                binding_position,
                                &mut captures,
                            );
                        }
                    }
                }
                let transported =
                    transport(program, facts, caller, &mut predicate, &captures, &[], None);
                let mut predicates = transported.predicates;
                predicates.push(predicate);
                Some(predicates)
            })
            .flatten()
            .collect()
    }

    /*
        Join whole checked formulas through surviving completed-value provenance.
        A named receiver and its producer's result are the same observation only
        while the current contexts retain that exact assignment. This is proof
        scratch: stored predicates keep their storage namespace so later writes
        still invalidate them. In particular a replacement's incoming result cannot
        become the pre-store destination, and a retired assignment is never replayed.
    */
    pub(crate) fn proves_captured_predicates(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[facts::FactContextHandle],
        predicates: &[BoundPredicate],
        required: &BoundPredicate,
        mut ordinary: impl FnMut(&CanonicalPlace) -> Option<facts::ScalarValue>,
    ) -> bool {
        fn retain_value(
            values: &mut Vec<(CanonicalPlace, Option<facts::ScalarValue>)>,
            place: &CanonicalPlace,
            value: facts::ScalarValue,
        ) {
            if let Some((_, known)) = values.iter_mut().find(|(known, _)| known == place) {
                if *known != Some(value) {
                    *known = None;
                }
            } else {
                values.push((place.clone(), Some(value)));
            }
        }

        let mut values: Vec<(CanonicalPlace, Option<facts::ScalarValue>)> = Vec::new();
        let mut bind = |predicate: &BoundPredicate| {
            let mut predicate = predicate.clone();
            for binding in &mut predicate.bindings {
                let value = ordinary(&binding.place);
                if let Some(place) =
                    captured_place(program, semantic, contexts, binding.place.clone())
                {
                    binding.place = place;
                }
                if let Some(value) = value {
                    retain_value(&mut values, &binding.place, value);
                }
            }
            predicate
        };
        let predicates = predicates.iter().map(&mut bind).collect::<Vec<_>>();
        let required = bind(required);
        if values.iter().any(|(_, value)| value.is_none()) {
            return false;
        }
        let binding_count = predicates
            .iter()
            .map(|predicate| predicate.bindings.len())
            .sum::<usize>();
        for _ in 0..=binding_count {
            let prior = values.clone();
            for predicate in &predicates {
                for binding in &predicate.bindings {
                    if let Some(value) = predicate.constant_at_place(&binding.place, |place| {
                        prior
                            .iter()
                            .find(|(known, _)| known == place)
                            .and_then(|(_, value)| value.clone())
                    }) {
                        retain_value(&mut values, &binding.place, value);
                        if values.iter().any(|(_, value)| value.is_none()) {
                            return false;
                        }
                    }
                }
            }
            if values.len() == prior.len() {
                return boolean_predicates::proves(&predicates, &required, |place| {
                    values
                        .iter()
                        .find(|(known, _)| known == place)
                        .and_then(|(_, value)| value.clone())
                });
            }
        }
        false
    }

    /*
        Read an exact scalar conclusion after authenticating its declaration,
        argument substitution and surviving capture. This does not turn a raw
        `ensures` expression into a caller fact or choose a side of a disjunction.
    */
    pub(in crate::checks) fn scalar_at_place(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        caller: &FlowStateFact,
        statement: usize,
        contexts: &[facts::FactContextHandle],
        frames: &validation::CallFrameResolver<'_>,
        place: &CanonicalPlace,
    ) -> Option<facts::ScalarValue> {
        current_predicates(
            program,
            facts,
            caller,
            GuaranteeUse::StatementEntry(statement),
            contexts,
            frames,
        )
        .iter()
        .find_map(|predicate| {
            predicate.constant_at_place(place, |operand| {
                crate::values::scalar_value_at_place(
                    program,
                    &facts.semantic,
                    contexts
                        .iter()
                        .map(|context| facts.semantic.contexts.get(*context)),
                    operand,
                )
            })
        })
    }
    pub(in crate::checks::contracts) fn proves(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        guarantees: &mut StateGuarantees<'_, '_>,
        call: &FlowCallFact,
        contexts: &[facts::FactContextHandle],
        expression: ExpressionHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let caller = guarantees.caller();
        let operators = &facts.operators;
        let semantic = &facts.semantic;
        let Some(required) = invocation(program, caller, call.statement_index, call.call_ordinal)
        else {
            return false;
        };
        let Some(frames) = frames else {
            return false;
        };
        if !stable_predicate_inputs(program, facts.view(), caller, &required, expression)
            || !builtin_predicate(program, operators, &required, expression)
        {
            return false;
        }
        let mut arithmetic_hypotheses = Vec::new();
        let exact = guarantees.any_available(
            GuaranteeUse::EvaluatedContext(call.statement_index),
            contexts,
            |guarantee| {
                if let Some(post_return_writes) = guarantee.post_return_writes.as_deref()
                    && let Some(bound) = arithmetic::at_invocation(
                        program,
                        facts,
                        caller,
                        &guarantee.invocation,
                        guarantee.expression,
                        post_return_writes,
                        frames,
                    )
                {
                    arithmetic_hypotheses.push(validation::ScopedArithmeticHypothesis {
                        proposition: bound.proposition,
                        holds: true,
                    });
                }
                let required_scope = PredicateScope {
                    callable: required.callable,
                    literal: &|expression| bound_literal(program, &required, expression),
                    place: &|expression| {
                        let place = bound_place(program, &required, expression)?;
                        let machine = program.machine_by_symbol(caller.machine_symbol)?;
                        crate::flow::local_reference_storage_at_call(
                            program,
                            frames,
                            machine,
                            &facts.flow,
                            caller,
                            call,
                            place,
                            crate::flow::ValueObservation::Contents,
                        )
                    },
                };
                let supplied_scope = PredicateScope {
                    callable: guarantee.invocation.callable,
                    literal: &|expression| {
                        bound_literal(program, &guarantee.invocation, expression)
                    },
                    place: &|expression| bound_place(program, &guarantee.invocation, expression),
                };
                predicates_match(
                    program,
                    semantic,
                    contexts,
                    &required_scope,
                    expression,
                    &supplied_scope,
                    guarantee.expression,
                    &mut Vec::new(),
                )
            },
        );
        exact
            || arithmetic::proves(
                program,
                facts,
                caller,
                contexts,
                &required,
                expression,
                arithmetic_hypotheses,
                frames,
            )
    }

    fn capture_preserved(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        borrow: &checked_trees::BorrowFacts,
        caller: &Machine,
        supplied: &Invocation<'_>,
        guarantee: ExpressionHandle,
        frames: &validation::CallFrameResolver<'_>,
    ) -> bool {
        let Some((_, state)) = borrow.first_state_borrow(caller.symbol, supplied.caller_state)
        else {
            return false;
        };
        let callable_target = |target| {
            validation::call_target_state(program, target)
                .map(|state| state.symbol)
                .or_else(|| {
                    validation::selected_requirement_signature(program, target)
                        .map(|(_, signature)| signature.symbol)
                })
                .unwrap_or(target)
        };
        let target = callable_target(supplied.site.target_symbol(program));
        let Some(call) = borrow.calls.span_or_empty(state.calls).iter().find(|call| {
            call.statement_index == supplied.statement
                && call.call_ordinal == supplied.ordinal
                && callable_target(call.target_symbol) == target
        }) else {
            return false;
        };
        let Some(writes) = crate::flow::signature_ceiling_places(
            program,
            caller.symbol,
            supplied.caller_state,
            None,
            call,
            Some(frames),
        ) else {
            return false;
        };
        if let Some(reads) = logical_formal_reads(program, operators, supplied, guarantee) {
            let parameters = supplied.callable.parameters(program);
            let arguments = validation::call_site_argument_expressions(program, &supplied.site);
            return reads.iter().all(|formal| {
                let Some(position) = parameters
                    .iter()
                    .position(|parameter| formal.root == PlaceRoot::Symbol(parameter.symbol))
                else {
                    return false;
                };
                let Some(actual) = arguments.get(position) else {
                    return false;
                };
                if formal.segments.is_empty() && crate::flow::literal(program, *actual).is_some() {
                    return true;
                }
                let Some(place) = projected_argument_place(
                    program,
                    supplied.caller_state,
                    supplied.statement,
                    *actual,
                    parameters[position].type_reference,
                    &formal.segments,
                ) else {
                    return false;
                };
                let Some(place) = crate::flow::rebase_exact_local_place(
                    program,
                    supplied.caller_state,
                    supplied.statement,
                    place,
                    Some(frames),
                ) else {
                    return false;
                };
                !writes.iter().any(|write| {
                    write.root == place.root
                        && crate::flow::canonical_place_segments_may_overlap(
                            program,
                            &write.segments,
                            &place.segments,
                        )
                }) || crate::values::exclusive_reference(
                    program,
                    parameters[position].type_reference,
                )
            });
        }
        let mut occurrences = Vec::new();
        append_expression_occurrences(program, guarantee, &mut occurrences);
        occurrences.into_iter().all(|occurrence| {
            if validation::reserved_result_place(program, occurrence)
                .is_some_and(|result| result.machine_symbol == supplied.callable.owner_symbol())
                || bound_literal(program, supplied, occurrence).is_some()
                || static_operands::immutable(program, occurrence)
            {
                return true;
            }
            let Some(place) = bound_place(program, supplied, occurrence) else {
                return false;
            };
            let Some(place) = crate::flow::rebase_exact_local_place(
                program,
                supplied.caller_state,
                supplied.statement,
                place,
                Some(frames),
            ) else {
                return false;
            };
            if !writes.iter().any(|write| {
                (write.root) == (place.root)
                    && crate::flow::canonical_place_segments_may_overlap(
                        program,
                        &write.segments,
                        &place.segments,
                    )
            }) {
                return true;
            }
            let Some(formal) = formal_place(program, supplied, occurrence) else {
                return false;
            };
            supplied
                .callable
                .parameters(program)
                .iter()
                .any(|parameter| {
                    formal.root == PlaceRoot::Symbol(parameter.symbol)
                        && crate::values::exclusive_reference(program, parameter.type_reference)
                })
        })
    }

    /*
        Bind one declaration-scoped scalar guarantee through its authentic call
        actuals. By-value inputs retain operand completion; an exclusive-reference
        projection describes normal-return storage under the existing frame law.
        The observer supplies value versions, never a replay of the callee body.
    */
    pub(in crate::checks) fn guarantee_scalar<'facts>(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'facts>,
        caller: &FlowStateFact,
        call: &FlowCallFact,
        expression: ExpressionHandle,
        projection: &[facts::PlaceSegment],
        primitive: typed_trees::types::PrimitiveType,
        frames: &validation::CallFrameResolver<'_>,
        observe: &mut impl FnMut(
            &'facts checked_trees::FlowOperandFact,
            &[facts::PlaceSegment],
            typed_trees::types::PrimitiveType,
            bool,
        ) -> Option<validation::ScopedArithmeticValue>,
    ) -> Option<validation::ScopedArithmeticValue> {
        let supplied = invocation(program, caller, call.statement_index, call.call_ordinal)?;
        let bind = |occurrence,
                    extra: &[facts::PlaceSegment],
                    primitive,
                    observe: &mut dyn FnMut(
            &'facts checked_trees::FlowOperandFact,
            &[facts::PlaceSegment],
            typed_trees::types::PrimitiveType,
            bool,
        )
            -> Option<validation::ScopedArithmeticValue>| {
            let (actual, mut path, post_state) = if let Some(result) =
                validation::reserved_result_place(program, occurrence)
            {
                if result.machine_symbol != supplied.callable.owner_symbol() {
                    return None;
                }
                let CallSite::Expression { expression, .. } = supplied.site else {
                    return None;
                };
                (expression, result.segments, false)
            } else {
                let formal = formal_place(program, &supplied, occurrence)?;
                let post_state = supplied
                    .callable
                    .parameters(program)
                    .iter()
                    .any(|parameter| {
                        formal.root == PlaceRoot::Symbol(parameter.symbol)
                            && crate::values::exclusive_reference(program, parameter.type_reference)
                    });
                let (actual, path) = actual_projection(program, &supplied, occurrence)?;
                (actual, path, post_state)
            };
            path.extend_from_slice(extra);
            if post_state {
                let machine = program.machine_by_symbol(caller.machine_symbol)?;
                if matches!(supplied.site, CallSite::TransitionNamed { .. }) {
                    return None;
                }
                let stable = stable_predicate_inputs(program, facts, caller, &supplied, occurrence);
                let preserved = capture_preserved(
                    program,
                    facts.operators,
                    facts.borrow,
                    machine,
                    &supplied,
                    occurrence,
                    frames,
                );
                if !stable || !preserved {
                    return None;
                }
            }
            let operand =
                captured_actual_operand(program, facts, caller, call, actual, &path, post_state)?;
            observe(operand, &path, primitive, post_state)
        };
        if projection.is_empty() && primitive == typed_trees::types::PrimitiveType::Bool {
            if let Some(facts::ScalarValue::Boolean(value)) =
                crate::flow::literal(program, expression)
            {
                return Some(validation::ScopedArithmeticValue::Integer(
                    numerics::bignum::BigInt::from_u128(u128::from(value)),
                ));
            }
            return bind(expression, projection, primitive, observe);
        }
        if projection.is_empty() {
            arithmetic::scalar_term(
                program,
                facts,
                &supplied.callable,
                expression,
                |occurrence, primitive| bind(occurrence, &[], primitive, observe),
            )
        } else {
            bind(expression, projection, primitive, observe)
        }
    }

    /*
        Bind a declaration-owned place to the actual captured by this exact call.
        Reference effect queries consume the address, not the referent's numeric
        contents. Result projections and exclusive post-state values remain with
        their existing guarantee judgments.
    */
    pub(super) fn guarantee_input<'facts>(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'facts>,
        caller: &FlowStateFact,
        call: &FlowCallFact,
        expression: ExpressionHandle,
        suffix: &[facts::PlaceSegment],
    ) -> Option<(
        &'facts checked_trees::FlowOperandFact,
        Vec<facts::PlaceSegment>,
    )> {
        let supplied = invocation(program, caller, call.statement_index, call.call_ordinal)?;
        let formal = formal_place(program, &supplied, expression)?;
        let parameter = supplied
            .callable
            .parameters(program)
            .iter()
            .find(|parameter| formal.root == PlaceRoot::Symbol(parameter.symbol))?;
        if crate::values::exclusive_reference(program, parameter.type_reference) {
            return None;
        }
        let (actual, mut path) = actual_projection(program, &supplied, expression)?;
        path.extend_from_slice(suffix);
        let operand = captured_actual_operand(program, facts, caller, call, actual, &path, false)?;
        Some((operand, path))
    }
    fn captured_actual_operand<'facts>(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'facts>,
        caller: &FlowStateFact,
        call: &FlowCallFact,
        actual: ExpressionHandle,
        path: &[facts::PlaceSegment],
        post_state: bool,
    ) -> Option<&'facts checked_trees::FlowOperandFact> {
        let mut operands = facts
            .flow
            .control
            .operands
            .iter()
            .map(|(_, operand)| operand)
            .filter(|operand| {
                operand.state_symbol == caller.state_symbol
                    && operand.statement_index == call.statement_index
                    && operand.expression == actual
            });
        let operand = operands.next()?;
        let mut place = place_at(program, caller.state_symbol, call.statement_index, actual)
            .unwrap_or(CanonicalPlace {
                root: PlaceRoot::Expression(actual),
                segments: Vec::new(),
            });
        place.segments.extend_from_slice(path);
        if !operands.all(|other| {
            if post_state {
                crate::flow::operand_bindings_agree(program, facts.flow, operand, other, &place)
            } else {
                crate::flow::operand_observations_agree(program, facts.flow, operand, other, &place)
            }
        }) {
            return None;
        }
        Some(operand)
    }

    fn invocation<'program>(
        program: &'program dyn ::validation::CheckingInput,
        caller: &FlowStateFact,
        statement: usize,
        ordinal: usize,
    ) -> Option<Invocation<'program>> {
        let site = validation::find_call_site(
            program,
            caller.machine_symbol,
            caller.state_symbol,
            statement,
            ordinal,
        )?;
        let target = match &site {
            CallSite::Expression { call, .. } => call.target_symbol,
            CallSite::Statement(call) => call.target_symbol,
            CallSite::TransitionNamed { path, .. } => path.symbol,
        };
        let requirement = site.static_requirement_dispatch().is_some()
            || validation::selected_requirement_signature(program, target).is_some();
        let callable = if requirement {
            let (_, signature) =
                validation::requirement_contract_signature(program, caller.machine_symbol, &site)?;
            Callable::Requirement { signature }
        } else {
            match &site {
                CallSite::TransitionNamed { .. } => validation::find_state(program, target)
                    .filter(|(machine, _)| machine.symbol == caller.machine_symbol)
                    .map(|(machine, state)| Callable::Machine { machine, state })
                    .or_else(|| Callable::resolve(program, target))?,
                _ => Callable::resolve(program, target)?,
            }
        };
        Some(Invocation {
            site,
            callable,
            caller_state: caller.state_symbol,
            statement,
            ordinal,
        })
    }

    fn ordinary_arguments(
        program: &dyn ::validation::CheckingInput,
        invocation: &Invocation<'_>,
    ) -> bool {
        let parameters = invocation.callable.parameters(program);
        let arguments = validation::call_site_argument_expressions(program, &invocation.site);
        let ordinary = match &invocation.site {
            CallSite::Expression { call, .. } => {
                static_operands::admitted(program, invocation)
                    && call.evidence_arguments.is_empty()
                    && (call.static_requirement_dispatch.is_none()
                        || matches!(invocation.callable, Callable::Requirement { .. }))
            }
            CallSite::Statement(call) => {
                static_operands::admitted(program, invocation)
                    && call.evidence_arguments.is_empty()
                    && (call.static_requirement_dispatch.is_none()
                        || matches!(invocation.callable, Callable::Requirement { .. }))
            }
            CallSite::TransitionNamed {
                evidence_arguments, ..
            } => evidence_arguments.is_empty(),
        };
        ordinary
            && parameters.len() == arguments.len()
            && parameters.iter().all(|parameter| !parameter.is_const)
    }

    /*
        A proof query consumes only the actuals its predicate names. Requiring every
        sibling actual to be a place rejected `cell(grid, y, x + 1)` even for the
        independent clause `y <= grid.max_y`. Effects of every operand still run
        through the call-entry contexts and capture-preservation checks; this is
        only admission to the exact-value vocabulary, not an effect exemption.
    */
    fn stable_predicate_inputs(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        caller: &FlowStateFact,
        invocation: &Invocation<'_>,
        expression: ExpressionHandle,
    ) -> bool {
        if !ordinary_arguments(program, invocation) {
            return false;
        }
        if let Some(reads) = logical_formal_reads(program, facts.operators, invocation, expression)
        {
            let parameters = invocation.callable.parameters(program);
            let arguments = validation::call_site_argument_expressions(program, &invocation.site);
            return reads.iter().all(|place| {
                let Some(position) = parameters
                    .iter()
                    .position(|formal| place.root == PlaceRoot::Symbol(formal.symbol))
                else {
                    return false;
                };
                let Some(actual) = arguments.get(position) else {
                    return false;
                };
                stable_captured_projection(
                    program,
                    facts,
                    caller,
                    invocation,
                    position,
                    *actual,
                    &place.segments,
                )
            });
        }
        let mut occurrences = Vec::new();
        append_expression_occurrences(program, expression, &mut occurrences);
        occurrences.into_iter().all(|occurrence| {
            if let Some(result) = validation::reserved_result_place(program, occurrence) {
                return result.machine_symbol == invocation.callable.owner_symbol();
            }
            if static_operands::immutable(program, occurrence) {
                return true;
            }
            let Some((actual, path)) = actual_projection(program, invocation, occurrence) else {
                return false;
            };
            let Some(formal) = formal_place(program, invocation, occurrence) else {
                return false;
            };
            let Some(position) = invocation
                .callable
                .parameters(program)
                .iter()
                .position(|parameter| formal.root == PlaceRoot::Symbol(parameter.symbol))
            else {
                return false;
            };
            stable_captured_projection(program, facts, caller, invocation, position, actual, &path)
        })
    }

    /*
        Logical matches carry their subject binding inside this exact contract.
        Return only the formal observations admitted by the common contract owner;
        ambient captures and unsupported patterns cannot enter this route.
    */
    fn logical_formal_reads(
        program: &dyn validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        invocation: &Invocation<'_>,
        expression: ExpressionHandle,
    ) -> Option<Vec<CanonicalPlace>> {
        let mut tree = Vec::new();
        validation::expression_traversal::collect_expression_tree(program, expression, &mut tree);
        if !tree.iter().any(|node| {
            matches!(
                program.expression_table.expression(*node),
                ExpressionNode::Match(_)
            )
        }) || !validation::expression_has_closed_value_captures(program, expression)
        {
            return None;
        }
        let parameters = invocation.callable.parameters(program);
        let predicate = crate::values::lower_callable_scalar_contract_predicate(
            program,
            operators,
            invocation.callable.owner_symbol(),
            parameters,
            invocation.callable.result_type(),
            expression,
            true,
            &mut 4096,
        )?;
        let result = CanonicalPlace {
            root: PlaceRoot::Expression(expression),
            segments: Vec::new(),
        };
        let bound = boolean_predicates::bind_parameters(
            program,
            invocation.callable.owner_symbol(),
            invocation.callable.target_symbol(),
            parameters,
            Some((&result, invocation.callable.result_type())),
            predicate,
        )?;
        Some(
            bound
                .bindings
                .into_iter()
                .filter_map(|binding| (binding.place.root != result.root).then_some(binding.place))
                .collect(),
        )
    }

    fn stable_captured_projection(
        program: &dyn ::validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        caller: &FlowStateFact,
        invocation: &Invocation<'_>,
        position: usize,
        actual: ExpressionHandle,
        path: &[facts::PlaceSegment],
    ) -> bool {
        let Some(call) = invocation_fact(facts, caller, invocation) else {
            return false;
        };
        let Some((operands, receiver_offset)) =
            crate::flow::captured_call_operands(program, facts.flow, caller, call)
        else {
            return false;
        };
        let Some(operand) = operands.get(receiver_offset + position) else {
            return false;
        };
        if operand.expression != actual
            || validation::project_type_reference_from_segments(
                program,
                operand.type_reference,
                path,
            )
            .is_none()
        {
            return false;
        }
        stable_owned_projection(program, operand.type_reference, path)
            || stable_value(program, actual, &mut Vec::new())
    }

    fn stable_owned_projection(
        program: &dyn ::validation::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
        path: &[facts::PlaceSegment],
    ) -> bool {
        let mut selected = reference;
        for length in 0..=path.len() {
            let Some(projected) = validation::project_type_reference_from_segments(
                program,
                reference,
                &path[..length],
            ) else {
                return false;
            };
            if crate::flow::operand_binds_live_view(program, projected) {
                return false;
            }
            selected = projected;
        }
        crate::flow::operand_binds_stable_copy(program, selected)
    }

    fn stable_value(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        pending: &mut Vec<ExpressionHandle>,
    ) -> bool {
        if pending.contains(&expression) {
            return false;
        }
        pending.push(expression);
        let stable = if let ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression(expression)
        {
            expressions(program)
                .struct_fields(literal.fields)
                .iter()
                .all(|field| stable_value(program, field.value, pending))
        } else {
            crate::flow::literal(program, expression).is_some()
                || direct_place(program, expression)
                    .or_else(|| exclusive_borrow_place(program, expression))
                    .is_some_and(|place| matches!(place.root, PlaceRoot::Symbol(_)))
        };
        pending.pop();
        stable
    }

    /*
        An exclusive-borrow actual resolves through canonical place spelling: the
        attached-field identity the write frame carries, which the authored member
        gate in `checked_place` deliberately does not keep.
    */
    fn exclusive_borrow_place(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        let borrow = program.expression_table.borrow_expression(expression)?;
        if !borrow.access.is_exclusive() {
            return None;
        }
        let place = validation::canonical_place_from_expression(program, expression)?;
        (matches!(place.root, PlaceRoot::Symbol(symbol) if symbol.is_valid())
            && !place.segments.iter().any(|segment| {
                matches!(segment, facts::PlaceSegment::Index { .. })
                    || crate::flow::place_segment_has_unresolved_identity(*segment)
            }))
        .then_some(place)
    }

    pub(in crate::checks) fn direct_place(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        let mut current = expression;
        let mut visited = Vec::new();
        loop {
            if visited.contains(&current) {
                return None;
            }
            visited.push(current);
            current = match program.expression_table.expression(current) {
                ExpressionNode::Name(_) => break,
                ExpressionNode::Member(member) => member.receiver,
                ExpressionNode::Borrow(borrow) => borrow.target,
                _ => return None,
            };
        }
        checked_place(program, expression)
    }

    fn builtin_predicate(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        invocation: &Invocation<'_>,
        expression: ExpressionHandle,
    ) -> bool {
        super::has_builtin_operators(program, operators, expression)
            && invocation.callable.builtin_meaning(program, expression)
    }

    fn predicates_match(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[facts::FactContextHandle],
        required: &PredicateScope<'_, '_>,
        left: ExpressionHandle,
        supplied: &PredicateScope<'_, '_>,
        right: ExpressionHandle,
        pending: &mut Vec<(ExpressionHandle, ExpressionHandle)>,
    ) -> bool {
        if pending.contains(&(left, right)) {
            return false;
        }
        pending.push((left, right));
        macro_rules! match_child {
            ($left:expr, $right:expr $(,)?) => {
                predicates_match(
                    program, semantic, contexts, required, $left, supplied, $right, pending,
                )
            };
        }
        let matched = match (
            program.expression_table.expression(left),
            program.expression_table.expression(right),
        ) {
            (ExpressionNode::Binary(left_binary), ExpressionNode::Binary(right_binary)) => {
                left_binary.operator == right_binary.operator
                    && same_scalar_type(
                        program,
                        &required.callable,
                        left_binary.left,
                        &supplied.callable,
                        right_binary.left,
                    )
                    && same_scalar_type(
                        program,
                        &required.callable,
                        left_binary.right,
                        &supplied.callable,
                        right_binary.right,
                    )
                    && match_child!(left_binary.left, right_binary.left)
                    && match_child!(left_binary.right, right_binary.right)
            }
            (ExpressionNode::Unary(left_unary), ExpressionNode::Unary(right_unary)) => {
                left_unary.operator == right_unary.operator
                    && same_scalar_type(
                        program,
                        &required.callable,
                        left_unary.operand,
                        &supplied.callable,
                        right_unary.operand,
                    )
                    && match_child!(left_unary.operand, right_unary.operand)
            }
            (ExpressionNode::Cast(left_cast), ExpressionNode::Cast(right_cast)) => {
                same_scalar_type(program, &required.callable, left, &supplied.callable, right)
                    && match_child!(left_cast.value, right_cast.value)
            }
            _ => match ((required.literal)(left), (supplied.literal)(right)) {
                (Some(left), Some(right)) => left == right,
                _ => (required.place)(left)
                    .zip((supplied.place)(right))
                    .is_some_and(|(left, right)| {
                        captured_place(program, semantic, contexts, left)
                            .zip(captured_place(program, semantic, contexts, right))
                            .is_some_and(|(left, right)| left == right)
                    }),
            },
        };
        let matched = matched
            || expressions(program)
                .binary_node(right)
                .filter(|binary| binary.operator == typed_trees::expression::BinaryOperator::And)
                .is_some_and(|binary| {
                    match_child!(left, binary.left) || match_child!(left, binary.right)
                });
        pending.pop();
        matched
    }

    fn same_scalar_type(
        program: &dyn ::validation::CheckingInput,
        left_owner: &Callable<'_>,
        left: ExpressionHandle,
        right_owner: &Callable<'_>,
        right: ExpressionHandle,
    ) -> bool {
        let reference =
            |owner: &Callable<'_>, expression| owner.scalar_reference(program, expression);
        if let (Some(left), Some(right)) = (
            crate::flow::literal(program, left),
            crate::flow::literal(program, right),
        ) {
            return left == right;
        }
        match (reference(left_owner, left), reference(right_owner, right)) {
            (Some(left), Some(right)) => {
                program.primitive_type_reference(left).is_some()
                    && program.primitive_type_reference(left)
                        == program.primitive_type_reference(right)
                    && program.arithmetic_domain_for_type_reference(left)
                        == program.arithmetic_domain_for_type_reference(right)
            }
            _ => false,
        }
    }

    fn actual_projection(
        program: &dyn ::validation::CheckingInput,
        invocation: &Invocation<'_>,
        expression: ExpressionHandle,
    ) -> Option<(ExpressionHandle, Vec<facts::PlaceSegment>)> {
        formal_place(program, invocation, expression)?;
        crate::published::semantic::places::call_contract_argument_projection(
            program,
            invocation.callable.parameters(program),
            validation::call_site_argument_expressions(program, &invocation.site),
            expression,
        )
    }

    /*
        A contract projection belongs to its declaration and selected builtin
        operations. Literal indexes have exact coordinates; runtime selectors need
        capture evidence this static projection cannot supply.
    */
    fn formal_place(
        program: &dyn ::validation::CheckingInput,
        invocation: &Invocation<'_>,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        let parameters = invocation.callable.parameters(program);
        let (position, path, _) =
            crate::values::structural_parameter_place(program, parameters, expression)?;
        let (root, segments, _, _) =
            crate::values::resolve_structural_parameter_path(program, parameters, position, &path)?;
        Some(CanonicalPlace {
            root: PlaceRoot::Symbol(root),
            segments,
        })
    }

    fn bound_literal(
        program: &dyn ::validation::CheckingInput,
        invocation: &Invocation<'_>,
        expression: ExpressionHandle,
    ) -> Option<facts::ScalarValue> {
        crate::flow::literal(program, expression).or_else(|| {
            let (value, remaining) = actual_projection(program, invocation, expression)?;
            if !remaining.is_empty() {
                return None;
            }
            crate::flow::literal(program, value)
        })
    }

    fn bound_place(
        program: &dyn ::validation::CheckingInput,
        invocation: &Invocation<'_>,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        if let Some(result) = validation::reserved_result_place(program, expression) {
            if result.machine_symbol != invocation.callable.owner_symbol() {
                return None;
            }
            let CallSite::Expression { expression, .. } = invocation.site else {
                return None;
            };
            return Some(CanonicalPlace {
                root: PlaceRoot::Expression(expression),
                segments: result.segments,
            });
        }
        let (actual, remaining) = actual_projection(program, invocation, expression)?;
        let mut place = place_at(
            program,
            invocation.caller_state,
            invocation.statement,
            actual,
        )?;
        place.segments.extend(remaining);
        Some(place)
    }

    pub(crate) fn captured_place(
        program: &dyn ::validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[facts::FactContextHandle],
        mut place: CanonicalPlace,
    ) -> Option<CanonicalPlace> {
        let mut source = None;
        let mut non_call_value = false;
        let mut prefix_length = None;
        for fact in contexts.iter().flat_map(|context| {
            semantic
                .context_view(semantic.contexts.get(*context))
                .facts()
        }) {
            if !matches!(
                fact.payload,
                FactPayload::AssignedValue { .. }
                    | FactPayload::AssignedScalarValue { .. }
                    | FactPayload::AssignedValueCapture { .. }
            ) {
                continue;
            }
            let Some(destination) = fact.place.as_place() else {
                continue;
            };
            let destination = semantic.places.get(destination);
            let segments = semantic.place_segments.span_or_empty(destination.segments);
            if destination.root != place.root || !place.segments.starts_with(segments) {
                continue;
            }
            if prefix_length.is_some_and(|previous| previous > segments.len()) {
                continue;
            }
            if prefix_length != Some(segments.len()) {
                prefix_length = Some(segments.len());
                source = None;
                non_call_value = false;
            }
            match fact.payload {
                FactPayload::AssignedValue { value }
                    if matches!(
                        program.expression_table.expression(value),
                        ExpressionNode::Call(_)
                    ) =>
                {
                    if source.is_some_and(|prior| prior != value) {
                        return None;
                    }
                    source = Some(value);
                }
                _ => non_call_value = true,
            }
        }
        if let Some(source) = source {
            if non_call_value {
                return None;
            }
            place.root = PlaceRoot::Expression(source);
            place.segments.drain(..prefix_length?);
        }
        Some(place)
    }
}
mod current_boolean {
    /*
        Determine whether a stored Boolean is true or false at this program point.
        Use only the facts the caller supplies as current; an old initializer is not
        evidence about a value that may have changed. Call checks and crash handling
        use this same query.
    */
    use crate::flow::canonical_place_from_semantic_place;
    use crate::published::facts::boolean_predicates::{BoundPredicate, inferred_boolean_at_place};
    use crate::published::facts::contract_occurrences::append_expression_occurrences;
    use crate::published::places::place_at;
    use crate::published::semantic::places::call_contract_argument_projection;
    use std::cell::OnceCell;
    use validation::CanonicalPlace;

    use facts::{FactOrigin, FactPayload, FactPlan, ProgramPoint, ScalarValue};

    /*
        Check several Boolean fields without rebuilding the same conditions for each.
        Prepare those conditions only when a recorded value cannot answer a query.
        All queries borrow one unchanged set of facts at the same program point;
        each field still receives its own freshness and inference checks. A
        query takes `&mut self` because it fills the batch's assignment cache.
    */
    pub(crate) struct BooleanValueQueries<'scope> {
        program: &'scope dyn validation::CheckingInput,
        operators: &'scope checked_trees::CheckedOperatorFacts,
        semantic: &'scope FactPlan,
        contexts: &'scope [facts::FactContextHandle],
        point: ProgramPoint,
        predicates: OnceCell<Vec<BoundPredicate>>,
        /*
            Each root's assignments in these contexts, read once for every query.
            The borrowed plan cannot change while this batch exists.
        */
        assignments: crate::values::RootAssignmentObservations<'scope>,
        /*
            The Boolean facts of these contexts with their reconstructed places, in
            context order. Every query used to re-scan all facts and rebuild every
            Boolean fact's place; a whole-aggregate transfer asks once per member.
        */
        boolean_facts: OnceCell<Vec<(Option<CanonicalPlace>, &'scope facts::Fact)>>,
    }

    impl<'scope> BooleanValueQueries<'scope> {
        pub(crate) fn new(
            program: &'scope dyn validation::CheckingInput,
            operators: &'scope checked_trees::CheckedOperatorFacts,
            semantic: &'scope FactPlan,
            contexts: &'scope [facts::FactContextHandle],
            point: ProgramPoint,
        ) -> Self {
            Self {
                program,
                operators,
                semantic,
                contexts,
                point,
                predicates: OnceCell::new(),
                assignments: crate::values::RootAssignmentObservations::new(
                    program, semantic, contexts,
                ),
                boolean_facts: OnceCell::new(),
            }
        }

        /*
            The recorded or proved Boolean at `actual`, from this batch's prepared
            inputs. It borrows only the assignment cache and the Boolean facts, so
            the predicate inference in `at_place` can call it while it holds the
            prepared predicates.
        */
        fn ordinary_at_place(
            program: &'scope dyn validation::CheckingInput,
            semantic: &'scope FactPlan,
            contexts: &'scope [facts::FactContextHandle],
            assignments: &mut crate::values::RootAssignmentObservations<'scope>,
            boolean_facts: &OnceCell<Vec<(Option<CanonicalPlace>, &'scope facts::Fact)>>,
            actual: &CanonicalPlace,
        ) -> Option<bool> {
            let scalar = assignments.scalar_value(actual);
            let facts = boolean_facts
                .get_or_init(|| boolean_facts_of(program, semantic, contexts).collect());
            ordinary_boolean_value_from(
                program,
                scalar,
                facts.iter().map(|(place, fact)| (place.as_ref(), *fact)),
                actual,
            )
        }

        pub(crate) fn at_place(&mut self, actual: &CanonicalPlace) -> Option<bool> {
            let program = self.program;
            let semantic = self.semantic;
            let contexts = self.contexts;
            let point = self.point;
            let scope = match point {
                ProgramPoint::State { state_symbol, .. } => Some((state_symbol, 0)),
                ProgramPoint::Statement {
                    state_symbol,
                    statement_index,
                    ..
                }
                | ProgramPoint::Call {
                    state_symbol,
                    statement_index,
                    ..
                }
                | ProgramPoint::CallRequires {
                    state_symbol,
                    statement_index,
                    ..
                }
                | ProgramPoint::CallEnsures {
                    state_symbol,
                    statement_index,
                    ..
                }
                | ProgramPoint::TransitionArm {
                    state_symbol,
                    statement_index,
                    ..
                }
                | ProgramPoint::Exit {
                    state_symbol,
                    statement_index,
                    ..
                } => Some((state_symbol, statement_index)),
                _ => None,
            };
            if let Some(reference) = scope
                .and_then(|(state, statement)| {
                    validation::canonical_place_type_reference(program, state, statement, actual)
                })
                .and_then(|reference| {
                    validation::field_domains::readable_type_reference(program, reference)
                })
                && program
                    .primitive_type_reference(reference)
                    .is_some_and(|primitive| primitive != typed_trees::types::PrimitiveType::Bool)
            {
                return None;
            }
            let Self {
                operators,
                predicates,
                assignments,
                boolean_facts,
                ..
            } = self;
            if let Some(value) = Self::ordinary_at_place(
                program,
                semantic,
                contexts,
                assignments,
                boolean_facts,
                actual,
            ) {
                return Some(value);
            }
            let predicates = predicates.get_or_init(|| {
                crate::published::facts::boolean_predicates::retained(
                    program, operators, semantic, contexts, point,
                )
            });
            inferred_boolean_at_place(predicates, actual, |place| {
                Self::ordinary_at_place(
                    program,
                    semantic,
                    contexts,
                    assignments,
                    boolean_facts,
                    place,
                )
                .map(ScalarValue::Boolean)
                .or_else(|| assignments.scalar_value(place))
            })
        }
    }

    pub(crate) fn boolean_value_at_place(
        program: &dyn validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &FactPlan,
        contexts: &[facts::FactContextHandle],
        point: ProgramPoint,
        actual: &CanonicalPlace,
    ) -> Option<bool> {
        BooleanValueQueries::new(program, operators, semantic, contexts, point).at_place(actual)
    }

    /*
        The Boolean facts of `contexts`, in context order, each with the place it
        is stored at. Facts without a place cannot describe any queried place and
        are left out; a place that does not reconstruct stays `None` and matches
        nothing.
    */
    fn boolean_facts_of<'plan>(
        program: &'plan dyn ::validation::CheckingInput,
        semantic: &'plan FactPlan,
        contexts: &'plan [facts::FactContextHandle],
    ) -> impl Iterator<Item = (Option<CanonicalPlace>, &'plan facts::Fact)> + 'plan {
        contexts
            .iter()
            .flat_map(move |context| {
                semantic
                    .context_view(semantic.contexts.get(*context))
                    .facts()
            })
            .filter(|fact| {
                matches!(
                    fact.payload,
                    FactPayload::BooleanExpression(_)
                        | FactPayload::ContractBooleanExpression { .. }
                        | FactPayload::BooleanValue { .. }
                )
            })
            .filter_map(move |fact| {
                let place = fact.place.as_place()?;
                Some((
                    canonical_place_from_semantic_place(
                        program,
                        semantic,
                        semantic.places.get(place),
                    ),
                    fact,
                ))
            })
    }

    /*
        The value a stored Boolean is recorded or proved to hold at `actual`.
        `scalar` is the current-value snapshot of `actual`; `facts` are the
        contexts' Boolean facts with their places, in context order.
    */
    fn ordinary_boolean_value_from<'plan>(
        program: &dyn ::validation::CheckingInput,
        scalar: Option<ScalarValue>,
        facts: impl IntoIterator<Item = (Option<&'plan CanonicalPlace>, &'plan facts::Fact)>,
        actual: &CanonicalPlace,
    ) -> Option<bool> {
        let mut retained = match scalar {
            Some(ScalarValue::Boolean(value)) => Some(value),
            Some(_) => return None,
            None => None,
        };
        for (place, fact) in facts {
            let (expression, value) = match fact.payload {
                FactPayload::BooleanExpression(expression)
                | FactPayload::ContractBooleanExpression { expression, .. } => (expression, true),
                FactPayload::BooleanValue { expression, value } => (expression, value),
                _ => continue,
            };
            if place != Some(actual) {
                continue;
            }
            let matches_subject = |candidate| {
                let Some(place) = super::field_actuals::checked_place(program, candidate) else {
                    return false;
                };
                if fact.origin == FactOrigin::CallEnsures
                    && matches!(fact.point, ProgramPoint::CallEnsures { .. })
                {
                    postcondition_place(program, fact.point, candidate).as_ref() == Some(actual)
                } else {
                    place == *actual
                }
            };
            let mut occurrences = Vec::new();
            append_expression_occurrences(program, expression, &mut occurrences);
            for leaf in occurrences
                .into_iter()
                .filter(|leaf| matches_subject(*leaf))
            {
                for required_value in [false, true] {
                    if !super::super::direct::guard_value_proves_with(
                        program,
                        expression,
                        value,
                        leaf,
                        required_value,
                        &|candidate, required| required == leaf && matches_subject(candidate),
                    ) {
                        continue;
                    }
                    if retained.is_some_and(|previous| previous != required_value) {
                        return None;
                    }
                    retained = Some(required_value);
                }
            }
        }
        retained
    }

    fn postcondition_place(
        program: &dyn ::validation::CheckingInput,
        point: ProgramPoint,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        let ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } = point
        else {
            return None;
        };
        let site = validation::find_call_site(
            program,
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        )?;
        let target = match &site {
            validation::CallSite::Statement(call) => call.target_symbol,
            validation::CallSite::Expression { call, .. } => call.target_symbol,
            validation::CallSite::TransitionNamed { path, .. } => path.symbol,
        };
        let parameters = validation::call_target_parameters(program, target)?;
        let (value, remaining) = call_contract_argument_projection(
            program,
            parameters,
            validation::call_site_argument_expressions(program, &site),
            expression,
        )?;
        super::field_actuals::checked_place(program, value)?;
        let mut actual = place_at(program, state_symbol, statement_index, value)?;
        actual.segments.extend(remaining);
        Some(actual)
    }
}
mod field_actuals {
    /*
        Exact referent substitution for supplied Boolean field facts at call entry.
    */
    use crate::published::places::place_at;
    use validation::CanonicalPlace;

    use facts::{FactPlan, PlaceRoot};
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::signature::StateParameter;

    /*
        None leaves unrelated computed-value proof paths unchanged. A recognized
        place requirement returns Some(false) on mismatch and cannot use labels.
    */
    pub(super) fn proves(
        program: &dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &FactPlan,
        contexts: &[facts::FactContextHandle],
        caller_state: SymbolHandle,
        statement_index: usize,
        call: &validation::CallSite<'_>,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<bool> {
        if !matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Member(_)
        ) {
            return None;
        }
        match has_explicit_formal_root(program, parameters, expression) {
            Some(true) => {}
            Some(false) => return None,
            None => return Some(false),
        }
        let Some(mut required) = checked_place(program, expression) else {
            return Some(false);
        };
        let formal = required.root.as_symbol()?;
        let position = typed_trees::signature::state_parameter_position(parameters, formal)?;
        let Some(argument) = validation::call_site_argument_expressions(program, call)
            .get(position)
            .copied()
        else {
            return Some(false);
        };
        let Some(raw_actual) = checked_place(program, argument) else {
            return Some(false);
        };
        if !matches!(raw_actual.root, PlaceRoot::Symbol(_)) {
            return None;
        }
        let Some(mut actual) = place_at(program, caller_state, statement_index, argument) else {
            return Some(false);
        };
        actual.segments.append(&mut required.segments);
        let machine = program.machine_holding_state(caller_state)?;
        Some(
            super::boolean_value_at_place(
                program,
                operators,
                semantic,
                contexts,
                facts::ProgramPoint::Statement {
                    machine_symbol: machine.symbol,
                    state_symbol: caller_state,
                    statement_index,
                },
                &actual,
            ) == Some(true),
        )
    }

    fn has_explicit_formal_root(
        program: &dyn ::validation::CheckingInput,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> Option<bool> {
        let mut current = expression;
        let mut visited = Vec::new();
        while program.expression_table.expression_is_valid(current) && !visited.contains(&current) {
            visited.push(current);
            current = match program.expression_table.expression(current) {
                ExpressionNode::Member(member) => member.receiver,
                ExpressionNode::Borrow(borrow) => borrow.target,
                ExpressionNode::Name(name) => {
                    if !name.head_symbol.is_valid() || !name.symbol.is_valid() {
                        return None;
                    }
                    return Some(parameters.iter().any(|parameter| {
                        parameter.symbol == name.head_symbol || parameter.symbol == name.symbol
                    }));
                }
                _ => return Some(false),
            };
        }
        None
    }

    pub(super) fn checked_place(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        crate::flow::projected_leaf_place(program, expression)
    }
}
pub(crate) use current_boolean::{BooleanValueQueries, boolean_value_at_place};

use self::booleans::{
    semantic_context_proves_boolean_expression,
    semantic_context_proves_instantiated_boolean_expression,
};
use super::evaluator::call_site_proves_boolean_contract_expression;

pub(super) fn semantic_contexts_prove_boolean_expression(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    entry_contexts: &[facts::FactContextHandle],
    point: facts::ProgramPoint,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        typed_trees::expression::ExpressionNode::Boolean(true)
    ) || entry_contexts.iter().any(|entry_context| {
        semantic_context_proves_boolean_expression(
            program,
            semantic,
            semantic.contexts.get(*entry_context),
            entry_contexts,
            point,
            expression,
        )
    })
}

/*
    Evaluate selected scalar operations over the live storage observations at
    one source point. Unlike symbolic entry propositions, these snapshots have
    already survived the flow owner's write and call invalidation.
*/
pub(super) fn live_contexts_prove_boolean_expression(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    state: &typed_trees::state::State,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    live_scalar_value(
        program,
        facts.view(),
        state,
        statement_index,
        contexts,
        expression,
        typed_trees::types::PrimitiveType::Bool,
    ) == Some(ScalarValue::Boolean(true))
        || current_bound_boolean_is_proven(
            program,
            facts,
            state,
            statement_index,
            contexts,
            expression,
        )
        || facts
            .flow
            .control
            .state_by_symbol(state.symbol)
            .and_then(|flow| {
                validation::machine_state_by_symbols(program, flow.machine_symbol, state.symbol)
            })
            .and_then(|(machine, _)| {
                let values = crate::checks::current_scalar_values::CurrentScalarValues::at(
                    program,
                    machine,
                    state,
                    statement_index,
                    typed_trees::statement::TransitionTargetHandle::invalid(),
                    None,
                )?;
                let goal = values.current(expression)?;
                Some(values.proves(program, machine, &goal))
            })
            == Some(true)
}

/*
    Form the requested current predicate as a goal only. Its observations are
    resolved against the same invalidation-adjusted contexts as scalar reads;
    neither the goal nor its match subject contributes an assumed premise.
*/
fn current_bound_boolean_is_proven(
    program: &dyn validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    state: &typed_trees::state::State,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let Some(expression) = boolean_predicates::lower_current_guard(
        program,
        &facts.operators,
        state.symbol,
        statement_index,
        expression,
    ) else {
        return false;
    };
    let Some(required) = boolean_predicates::bind_current_expression(
        program,
        state.symbol,
        expression,
        true,
        |operand| {
            boolean_predicates::declared_observation(
                program,
                state.symbol,
                statement_index,
                operand,
            )
        },
    ) else {
        return false;
    };
    let point = facts::ProgramPoint::Statement {
        machine_symbol: required.machine,
        state_symbol: state.symbol,
        statement_index,
    };
    let retained =
        boolean_predicates::retained(program, &facts.operators, &facts.semantic, contexts, point);
    boolean_predicates::proves(&retained, &required, |place| {
        scalar_value_at_place(
            program,
            &facts.semantic,
            contexts
                .iter()
                .map(|context| facts.semantic.contexts.get(*context)),
            place,
        )
    })
}

fn live_scalar_value(
    program: &dyn validation::CheckingInput,
    facts: checked_trees::CheckFactsRef<'_>,
    state: &typed_trees::state::State,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    expression: typed_trees::expression::ExpressionHandle,
    primitive: typed_trees::types::PrimitiveType,
) -> Option<ScalarValue> {
    let selected = crate::values::lower_unit_scalar_argument(
        program,
        facts.operators,
        state,
        statement_index,
        expression,
        primitive,
    )?;
    let symbols = crate::values::unit_scalar_binding_symbols(program, state, statement_index)?;
    let semantic = &facts.semantic;
    let mut values = crate::values::PlaceScalarValues {
        program,
        parameters: program.state_parameters(state),
        symbols: &symbols,
        value_at_place: |place: &validation::CanonicalPlace, _observation| {
            scalar_value_at_place(
                program,
                semantic,
                contexts
                    .iter()
                    .map(|context| semantic.contexts.get(*context)),
                place,
            )
        },
    };
    crate::values::evaluate_checked_scalar(&selected, &mut values)
}

pub(super) fn call_entry_contexts_prove_boolean_contract_expression(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[facts::FactContextHandle],
    expression: typed_trees::expression::ExpressionHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let operators = &facts.operators;
    let semantic = &facts.semantic;
    let Some(call_site) = validation::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return false;
    };
    let Some(target_parameters) =
        validation::call_target_parameters(program, call_flow.target_symbol)
    else {
        return false;
    };

    if let Some((predicate, subject)) = place_predicate(program, expression)
        && let Some((value, remaining)) =
            case_test_subject_actual(program, &call_site, target_parameters, subject)
        && let Some(mut place) = place_at(
            program,
            state_flow.state_symbol,
            call_flow.statement_index,
            value,
        )
    {
        place.segments.extend(remaining);
        if predicate.proven(
            program,
            facts,
            facts::ProgramPoint::Statement {
                machine_symbol: state_flow.machine_symbol,
                state_symbol: state_flow.state_symbol,
                statement_index: call_flow.statement_index,
            },
            entry_contexts,
            &place,
        ) {
            return true;
        }
    }

    let field_proof = field_actuals::proves(
        program,
        operators,
        semantic,
        entry_contexts,
        state_flow.state_symbol,
        call_flow.statement_index,
        &call_site,
        target_parameters,
        expression,
    );
    if field_proof == Some(true) {
        return true;
    }
    (field_proof.is_none() && {
        // Share the unchanged entry roster's case and assignment inputs across
        // contexts and Boolean leaves. Check each subject and scalar agreement.
        let availability = std::cell::OnceCell::new();
        let mut assignments =
            crate::values::RootAssignmentObservations::new(program, semantic, entry_contexts);
        entry_contexts.iter().any(|entry_context| {
            let context = semantic.contexts.get(*entry_context);
            semantic_context_proves_instantiated_boolean_expression(
                program,
                facts,
                call_flow,
                call_frames,
                context,
                entry_contexts,
                state_flow.state_symbol,
                call_flow.statement_index,
                &call_site,
                target_parameters,
                call_flow.target_symbol,
                expression,
                &availability,
                &mut assignments,
            )
        })
    }) || call_site_proves_boolean_contract_expression(
        program,
        operators,
        state_flow,
        call_flow,
        &call_site,
        call_flow.target_symbol,
        target_parameters,
        expression,
    )
}

pub(super) fn semantic_contexts_prove_contract_fact(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    point: facts::ProgramPoint,
    entry_contexts: &[facts::FactContextHandle],
    fact: &facts::Fact,
) -> bool {
    let semantic = &facts.semantic;
    if !contract_membership_place_is_accessible(program, semantic, entry_contexts, fact) {
        return false;
    }
    match fact.payload {
        FactPayload::InstructionEffect { .. } | FactPayload::CallCompletion { .. } => false,
        facts::domain_membership_pattern!(domain_symbol, semantic_domain) => {
            let Some(place) = fact.place.as_place() else {
                return false;
            };
            if indexed_membership(program, fact.payload) {
                return semantic_domain.is_valid()
                    && assigned_values::carried_membership_at(
                        semantic,
                        entry_contexts,
                        |candidate_symbol, candidate_instance| {
                            candidate_symbol == domain_symbol
                                && candidate_instance == semantic_domain
                        },
                        |candidate| semantic.places_match(program, candidate, place),
                    );
            }
            entry_contexts.iter().any(|entry_context| {
                semantic
                    .context_view(semantic.contexts.get(*entry_context))
                    .proves_place_domain_membership_in_program(program, place, domain_symbol)
            }) || assigned_values::carried_membership_at(
                semantic,
                entry_contexts,
                |candidate_symbol, _| {
                    validation::field_domains::domain_membership_implies(
                        program,
                        candidate_symbol,
                        domain_symbol,
                    )
                },
                |candidate| {
                    semantic.places_match(program, candidate, place)
                        || place_covers_subject(program, semantic, candidate, place)
                },
            ) || assigned_values::prove_domain(
                program,
                semantic,
                entry_contexts,
                place,
                domain_symbol,
            )
        }
        facts::carry_permission_pair_pattern!(permission, ..) => {
            let Some(place) = fact.place.as_place() else {
                return false;
            };
            assigned_values::live_fact_at_place(
                semantic,
                entry_contexts,
                |candidate| candidate.payload.carry_permission() == Some(permission),
                |candidate| semantic.places_match(program, candidate, place),
            )
        }
        FactPayload::BooleanExpression(expression)
        | FactPayload::ContractBooleanExpression { expression, .. } => {
            semantic_contexts_prove_boolean_expression(
                program,
                semantic,
                entry_contexts,
                point,
                expression,
            )
        }
        FactPayload::PropositionApplication { .. }
        | FactPayload::ContractPropositionApplication { .. } => {
            let Some(required_label) = semantic.proposition_fact_label(program, fact) else {
                return false;
            };
            contexts_prove_proposition_label(semantic, program, entry_contexts, &required_label)
                || instruction_effects::proves_contract(program, facts, point, entry_contexts, fact)
        }
        FactPayload::CarryOrigin { .. } => {
            let Some(place) = fact.place.as_place() else {
                return false;
            };
            assigned_values::live_fact_at_place(
                semantic,
                entry_contexts,
                |candidate| matches!(candidate.payload, FactPayload::CarryOrigin { .. }),
                |candidate| semantic.places_match(program, candidate, place),
            )
        }
        FactPayload::BooleanPredicate { .. }
        | FactPayload::CaseValue { .. }
        | FactPayload::AssignedValue { .. }
        | FactPayload::AssignedIntegerBounds { .. }
        | FactPayload::FloatPredicate { .. }
        | FactPayload::AssignedScalarValue { .. }
        | FactPayload::AssignedValueCapture { .. }
        | FactPayload::StorageDependency { .. }
        | FactPayload::BytePredicate { .. }
        | FactPayload::BooleanValue { .. }
        | FactPayload::MatchPattern { .. }
        | FactPayload::TypeConstraint { .. }
        | FactPayload::ProofObligation { .. }
        | FactPayload::Contract { .. } => false,
    }
}

/*
    Explicit call membership requirements consume their projected subject.
    Nominal field contracts are checked separately and retain conditional paths
    below a whole-sum actual instead of extracting all its alternatives.
*/
pub(super) fn contract_membership_place_is_accessible(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    contexts: &[facts::FactContextHandle],
    fact: &facts::Fact,
) -> bool {
    if !matches!(
        fact.payload,
        FactPayload::DomainMembership { .. } | FactPayload::ContractDomainMembership { .. }
    ) {
        return true;
    }
    let facts::ProgramPoint::CallRequires {
        machine_symbol,
        state_symbol,
        statement_index,
        ..
    } = fact.point
    else {
        return true;
    };
    let Some(place) = fact.place.as_place() else {
        return false;
    };
    let Some(subject) = crate::flow::canonical_place_from_semantic_place(
        program,
        semantic,
        semantic.places.get(place),
    ) else {
        return false;
    };
    crate::flow::place_cases_are_selected(
        program,
        semantic,
        contexts,
        machine_symbol,
        state_symbol,
        statement_index,
        &subject,
    )
}

/*
    Whether a live fact at `candidate` covers the required `subject`
    elementwise: same place root, and at every position either equal segments
    or a containing `FixedRange` (the whole-extent `0..usize::MAX` row covers
    a runtime `Index`; a finite range covers contained indices -- see
    `assigned_values::segments_cover_subject`).
*/
fn place_covers_subject(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    candidate: facts::PlaceHandle,
    subject: facts::PlaceHandle,
) -> bool {
    let candidate = semantic.places.get(candidate);
    let subject = semantic.places.get(subject);
    candidate.root == subject.root
        && assigned_values::segments_cover_subject(
            program,
            semantic.place_segments.span_or_empty(candidate.segments),
            semantic.place_segments.span_or_empty(subject.segments),
        )
}

pub(super) fn indexed_membership(
    program: &dyn ::validation::CheckingInput,
    payload: FactPayload,
) -> bool {
    let (facts::domain_membership_pattern!(domain_symbol)) = payload else {
        return false;
    };
    program.domain_definitions().iter().any(|domain| {
        domain.symbol == domain_symbol
            && !typed_trees::domain::index_parameters(program, domain).is_empty()
    })
}

/*
    The caller actual a case test's subject names: the positional argument for
    an ordinary formal, or the receiver for `self`.
*/
fn case_test_subject_actual(
    program: &dyn ::validation::CheckingInput,
    call_site: &validation::CallSite<'_>,
    parameters: &[typed_trees::signature::StateParameter],
    subject: typed_trees::expression::ExpressionHandle,
) -> Option<(
    typed_trees::expression::ExpressionHandle,
    Vec<facts::PlaceSegment>,
)> {
    crate::published::semantic::places::call_contract_argument_projection(
        program,
        parameters,
        validation::call_site_argument_expressions(program, call_site),
        subject,
    )
}

/*
    Whether any selected context proves `required_label` — directly, or through
    a retained boolean fact whose label matches the `boolean:` suffix form.
    Call `requires` resolution uses the same admission as the contract prover.
*/
pub(crate) fn contexts_prove_proposition_label(
    semantic: &facts::FactPlan,
    program: &dyn ::validation::CheckingInput,
    contexts: &[facts::FactContextHandle],
    required_label: &str,
) -> bool {
    contexts.iter().any(|context| {
        let context = semantic.contexts.get(*context);
        semantic
            .context_view(context)
            .proves_proposition_label(program, required_label)
            || required_label
                .strip_prefix("boolean:")
                .is_some_and(|required_boolean| {
                    semantic.context_view(context).facts().any(|candidate| {
                        semantic
                            .boolean_fact_label(program, candidate)
                            .is_some_and(|label| label == required_boolean)
                    })
                })
    })
}

mod propositions {
    /*
        Substitute ordinary proposition arguments into selected scalar plans. State
        arguments read live flow observations; alias arguments read their immutable
        parameter telescope. Both use the shared checked arithmetic evaluator.
    */
    use super::{ScalarValue, live_scalar_value};
    use typed_trees::proposition::{
        PropositionApplication, PropositionBody, PropositionDefinition, PropositionFormula,
    };
    use typed_trees::types::PrimitiveType;

    struct PropositionProof<'program> {
        program: &'program dyn validation::CheckingInput,
        facts: &'program checked_trees::CheckFacts,
        state: &'program typed_trees::state::State,
        statement: usize,
        contexts: &'program [facts::FactContextHandle],
        active: Vec<symbols::SymbolHandle>,
    }
    struct PropositionArguments<'program> {
        declaration: &'program PropositionDefinition,
        values: Vec<(symbols::SymbolHandle, Option<ScalarValue>)>,
    }
    pub(in crate::checks::contracts) fn live_contexts_prove_proposition_application(
        program: &dyn validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        state: &typed_trees::state::State,
        statement: usize,
        contexts: &[facts::FactContextHandle],
        application: &PropositionApplication,
    ) -> bool {
        PropositionProof {
            program,
            facts,
            state,
            statement,
            contexts,
            active: Vec::new(),
        }
        .application(application, None)
            == Some(true)
    }
    impl<'program> PropositionProof<'program> {
        fn application(
            &mut self,
            application: &PropositionApplication,
            environment: Option<&PropositionArguments<'program>>,
        ) -> Option<bool> {
            if self.active.contains(&application.proposition) {
                return None;
            }
            let declaration = self
                .program
                .proposition_definition_by_symbol(application.proposition)?;
            let PropositionBody::Transparent { proposition } = &declaration.body else {
                return None;
            };
            let parameters = self.program.proposition_parameters(declaration);
            let arguments = self
                .program
                .expression_table
                .expression_handles(application.arguments);
            if parameters.len() != arguments.len() {
                return None;
            }
            let mut bound = PropositionArguments {
                declaration,
                values: Vec::with_capacity(parameters.len()),
            };
            for (parameter, argument) in parameters.iter().zip(arguments) {
                let value = self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    .and_then(|primitive| match environment {
                        Some(environment) => {
                            self.parameter_value(environment, *argument, primitive)
                        }
                        None => live_scalar_value(
                            self.program,
                            self.facts.view(),
                            self.state,
                            self.statement,
                            self.contexts,
                            *argument,
                            primitive,
                        ),
                    });
                bound.values.push((parameter.symbol, value));
            }
            self.active.push(application.proposition);
            let result = match proposition {
                PropositionFormula::Application(application) => {
                    self.application(application, Some(&bound))
                }
                PropositionFormula::BooleanExpression(expression) => {
                    match self.parameter_value(&bound, *expression, PrimitiveType::Bool) {
                        Some(ScalarValue::Boolean(value)) => Some(value),
                        _ => None,
                    }
                }
            };
            self.active.pop();
            result
        }
        fn parameter_value(
            &self,
            environment: &PropositionArguments<'program>,
            expression: typed_trees::expression::ExpressionHandle,
            primitive: PrimitiveType,
        ) -> Option<ScalarValue> {
            let parameters = self.program.proposition_parameters(environment.declaration);
            let plan = crate::values::lower_parameter_scalar_expression(
                self.program,
                &self.facts.operators,
                environment.declaration.symbol,
                parameters,
                expression,
                primitive,
            )?;
            let symbols = parameters
                .iter()
                .filter(|parameter| {
                    crate::values::scalar::occupies_scalar_position(self.program, parameter)
                })
                .map(|parameter| parameter.symbol)
                .collect::<Vec<_>>();
            crate::values::evaluate_checked_scalar(
                &plan,
                &mut crate::values::BoundScalarValues {
                    symbols: &symbols,
                    value_at_symbol: |symbol| {
                        environment
                            .values
                            .iter()
                            .find(|(parameter, _)| *parameter == symbol)
                            .and_then(|(_, value)| value.clone())
                    },
                },
            )
        }
    }
}
pub(super) use propositions::live_contexts_prove_proposition_application;

mod instruction_effects {
    /*
        Match sealed instruction-model propositions to completed flow events.
        Values are compared at their own operand-completion frontiers. Unknown
        scalars can retain the same place only through the flow owner's complete
        write footprints in the admitted control order; no source expression or
        call initializer is replayed to recover a value.
    */
    use crate::flow::{call_flows, expressions, state_statements};
    use crate::values::resolve_structural_parameter_path;
    use crate::values::structural_parameter_place;
    use checked_trees::CheckFacts;
    use facts::{FactPayload, ProgramPoint};
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::proposition::PropositionApplication;
    use typed_trees::types::PrimitiveType;
    mod atomic {
        /*
            Release-store proofs bind the performed atomic event to its completed
            destination and scalar operand. Generic type arguments retain the exact
            atomic cell/value pair; a same-sized integer or another cell is not enough.
        */
        use super::{QueryArgument, QueryArguments};
        use crate::published::places::place_at;
        use checked_trees::{FlowOperandFact, FlowStateFact};
        use typed_trees::types::PrimitiveType;
        use validation::CanonicalPlace;

        fn query_primitive(
            program: &dyn validation::CheckingInput,
            arguments: &QueryArguments<'_>,
        ) -> Option<PrimitiveType> {
            let frame = arguments.frames.len().checked_sub(1)?;
            let cell = arguments.binder_symbol(program, frame, 0)?;
            let value = arguments.binder_symbol(program, frame, 1)?;
            if !program.symbols.builtin_type_atom(cell)?.is_atomic()
                || program.symbols.builtin_type_atom(value)?.is_atomic()
            {
                return None;
            }
            let cell = program
                .type_reference_table
                .find_named_type_reference(cell)?;
            let value = program
                .type_reference_table
                .find_named_type_reference(value)?;
            let primitive = program.primitive_type_reference(cell)?;
            (program.primitive_type_reference(value) == Some(primitive)).then_some(primitive)
        }

        fn receiver_places(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            flow: &FlowStateFact,
            state: &typed_trees::state::State,
            statement: usize,
            arguments: &QueryArguments<'_>,
            required: QueryArgument,
        ) -> Option<Vec<CanonicalPlace>> {
            let (expression, suffix) = arguments.source_projection(program, required, &[])?;
            let mut place = place_at(program, state.symbol, statement, expression)?;
            if !suffix.is_empty() {
                let reference = validation::canonical_place_type_reference(
                    program,
                    state.symbol,
                    statement,
                    &place,
                )?;
                place
                    .segments
                    .extend(crate::values::applied_projection_segments(
                        program, reference, &suffix,
                    )?);
            }
            crate::flow::stored_views::address_before_statement(
                program,
                facts.semantic,
                facts.flow.control,
                state,
                flow.statements,
                statement,
                place,
            )
        }

        fn captured_places(
            facts: checked_trees::CheckFactsRef<'_>,
            operand: &FlowOperandFact,
        ) -> Option<Vec<CanonicalPlace>> {
            let rows = facts
                .flow
                .control
                .operand_referents
                .span(operand.referents)?;
            if rows.is_empty() {
                return None;
            }
            Some(
                rows.iter()
                    .map(|row| {
                        crate::checks::places::place_or_root_in(
                            &facts.flow.control.operand_referent_segments,
                            row.root,
                            row.segments,
                        )
                    })
                    .collect(),
            )
        }

        /*
            A destination reached through a loaded descriptor (for example the
            second reference hop of `envelope.holder.word.value`) is captured
            under that load's expression. Name it by the place the descriptor
            was loaded from, resolved before the event's own statement, so it
            meets the requested receiver in the same canonical form. Resolving
            at the event, not at the query, keeps a later rebind of that
            descriptor distinct; an unresolvable load fails the match.
        */
        fn event_addresses(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            flow: &FlowStateFact,
            state: &typed_trees::state::State,
            event_statement: usize,
            captured: Vec<CanonicalPlace>,
        ) -> Option<Vec<CanonicalPlace>> {
            let mut addresses = Vec::new();
            for place in captured {
                let facts::PlaceRoot::Expression(load) = place.root else {
                    addresses.push(place);
                    continue;
                };
                let source = place_at(program, state.symbol, event_statement, load)?;
                for mut resolved in crate::flow::stored_views::address_before_statement(
                    program,
                    facts.semantic,
                    facts.flow.control,
                    state,
                    flow.statements,
                    event_statement,
                    source,
                )? {
                    resolved.segments.extend_from_slice(&place.segments);
                    if !addresses.contains(&resolved) {
                        addresses.push(resolved);
                    }
                }
            }
            Some(addresses)
        }

        pub(super) fn matches_store(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            flow: &FlowStateFact,
            state: &typed_trees::state::State,
            statement: usize,
            contexts: &[facts::FactContextHandle],
            event: &facts::Fact,
            arguments: &QueryArguments<'_>,
            required: &[QueryArgument],
        ) -> bool {
            let result = store_matches(
                program, facts, flow, state, statement, contexts, event, arguments, required,
            );
            result.unwrap_or(false)
        }
        fn store_matches(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            flow: &FlowStateFact,
            state: &typed_trees::state::State,
            statement: usize,
            contexts: &[facts::FactContextHandle],
            event: &facts::Fact,
            arguments: &QueryArguments<'_>,
            required: &[QueryArgument],
        ) -> Option<bool> {
            let facts::ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            } = event.point
            else {
                return None;
            };
            if machine_symbol != flow.machine_symbol || state_symbol != state.symbol {
                return None;
            }
            let order = program
                .statement_table
                .admitted_execution_order(state.statement_nodes);
            if !order.precedes(statement_index, statement)
                && !(statement
                    == crate::flow::state_statements(program, state.statement_nodes).len()
                    && order.position(statement_index).is_some())
            {
                return None;
            }
            let machine = program.machine_by_symbol(machine_symbol)?;
            let authored = crate::flow::state_statements(program, state.statement_nodes)
                .get(statement_index)?;
            let carrier =
                crate::flow::atomic_effects::release_store(program, machine, state, authored)?;
            let row = facts.flow.state_statement(flow, statement_index)?;
            let destination = facts.flow.control.operands.get(row.store_target);
            let actual = event_addresses(
                program,
                facts,
                flow,
                state,
                statement_index,
                captured_places(facts, destination)?,
            )?;
            let [receiver, value] = required else {
                return None;
            };
            let requested =
                receiver_places(program, facts, flow, state, statement, arguments, *receiver)?;
            if actual.is_empty()
                || requested.is_empty()
                || actual.len() != requested.len()
                || actual.iter().any(|place| !requested.contains(place))
            {
                return Some(false);
            }
            let assignment = authored.as_assignment()?;
            let mut captures =
                facts
                    .flow
                    .control
                    .atomic_executions
                    .iter()
                    .filter_map(|(_, row)| {
                        (row.state_symbol == state.symbol
                            && row.statement_index == statement_index
                            && row.expression == assignment.value)
                            .then_some(row)
                    });
            let capture = captures.next()?;
            if captures.next().is_some() {
                return None;
            }
            let [operand] = facts.flow.control.operands.span(capture.operands)? else {
                return None;
            };
            let [expression] = carrier.operands.as_slice() else {
                return None;
            };
            if operand.expression != *expression {
                return None;
            }
            let primitive = query_primitive(program, arguments)?;
            let cell_type =
                validation::declared_place_type_raw(program, machine, Some(state), carrier.place)?;
            if program.primitive_type_reference(cell_type) != Some(primitive) {
                return Some(false);
            }
            if program.primitive_type_reference(operand.type_reference) != Some(primitive) {
                return Some(false);
            }
            Some(super::operands::operand_matches(
                program, facts, flow, state, statement, contexts, operand, arguments, *value,
                primitive,
            ))
        }

        pub(super) fn guarantee_matches(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            flow: &FlowStateFact,
            state: &typed_trees::state::State,
            statement: usize,
            contexts: &[facts::FactContextHandle],
            call: &checked_trees::FlowCallFact,
            candidate: &typed_trees::proposition::PropositionApplication,
            arguments: &QueryArguments<'_>,
            required: &[QueryArgument],
        ) -> bool {
            let Some(contract) = validation::instruction_effect_contract(program, candidate) else {
                return false;
            };
            if contract.operation != facts::InstructionEffectOperation::AtomicReleaseStore {
                return false;
            }
            let Some(supplied) = QueryArguments::new(program, &contract) else {
                return false;
            };
            let Some(primitive) = query_primitive(program, arguments) else {
                return false;
            };
            if query_primitive(program, &supplied) != Some(primitive) {
                return false;
            }
            let Some(frame) = supplied.frames.last() else {
                return false;
            };
            let ([actual_receiver, actual_value], [receiver, value]) =
                (frame.arguments.as_slice(), required)
            else {
                return false;
            };
            let Some((expression, suffix)) =
                supplied.source_projection(program, *actual_receiver, &[])
            else {
                return false;
            };
            let input = super::super::call_guarantees::guarantee_input(
                program, facts, flow, call, expression, &suffix,
            );
            let Some((operand, path)) = input else {
                return false;
            };
            let Some(mut actual) = captured_places(facts, operand) else {
                return false;
            };
            for place in &mut actual {
                place.segments.extend_from_slice(&path);
            }
            let Some(requested) =
                receiver_places(program, facts, flow, state, statement, arguments, *receiver)
            else {
                return false;
            };
            if actual.is_empty()
                || actual.len() != requested.len()
                || actual.iter().any(|place| !requested.contains(place))
            {
                return false;
            }
            super::operands::guarantee_value_matches(
                program,
                facts,
                flow,
                state,
                statement,
                contexts,
                call,
                &supplied,
                *actual_value,
                arguments,
                *value,
                primitive,
            )
        }
    }
    mod operands {
        /*
            Instruction captures and current proposition terms share the existing
            scoped arithmetic judge. Value origins and operand-prefix preservation
            remain flow-owned; neither aliases nor initializers are executed here.
        */
        use super::super::call_guarantees::{arithmetic, callable::Callable};
        use super::{QueryArgument, QueryArguments};
        use crate::published::places::place_at;
        use crate::values::applied_projection_segments;
        use checked_trees::{FlowOperandFact, FlowStateFact};
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
        use typed_trees::types::PrimitiveType;
        use validation::{
            CanonicalPlace, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
        };

        struct Capture<'facts> {
            frontier: CaptureFrontier<'facts>,
            primitive: PrimitiveType,
            place: CanonicalPlace,
            origin: Option<CanonicalPlace>,
            value: ScopedArithmeticValue,
        }

        enum CaptureFrontier<'facts> {
            Operand(&'facts FlowOperandFact),
            NormalReturn(&'facts checked_trees::FlowCallFact),
        }

        #[derive(Clone, Copy)]
        struct Values<'scope> {
            program: &'scope dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'scope>,
            flow: &'scope FlowStateFact,
            state: &'scope typed_trees::state::State,
            statement: usize,
            contexts: &'scope [facts::FactContextHandle],
            frames: &'scope validation::CallFrameResolver<'scope>,
        }
        impl<'scope> Values<'scope> {
            fn captured(
                &self,
                operand: &'scope FlowOperandFact,
                projection: &[facts::PlaceSegment],
                primitive: PrimitiveType,
                captures: &mut Vec<Capture<'scope>>,
            ) -> Option<ScopedArithmeticValue> {
                let Self {
                    program,
                    facts,
                    flow,
                    state,
                    frames,
                    ..
                } = *self;
                let machine = program.machine_by_symbol(flow.machine_symbol)?;
                let callable = Callable::Machine { machine, state };
                let mut capture = |expression, primitive| {
                    let observed = if expression == operand.expression {
                        operand
                    } else {
                        let mut observations = facts
                            .flow
                            .control
                            .operands
                            .iter()
                            .map(|(_, value)| value)
                            .filter(|value| {
                                value.state_symbol == operand.state_symbol
                                    && value.statement_index == operand.statement_index
                                    && value.expression == expression
                            });
                        if let Some(first) = observations.next() {
                            let place = value_place(
                                program,
                                first.state_symbol,
                                first.statement_index,
                                expression,
                                projection,
                            )?;
                            if !observations.all(|other| {
                                crate::flow::operand_observations_agree(
                                    program, facts.flow, first, other, &place,
                                )
                            }) {
                                return None;
                            }
                            first
                        } else {
                            let place = value_place(
                                program,
                                operand.state_symbol,
                                operand.statement_index,
                                expression,
                                projection,
                            )?;
                            if !crate::flow::operand_entry_origin_is_stable(
                                program, facts.flow, operand, &place,
                            ) {
                                return None;
                            }
                            operand
                        }
                    };
                    let place = value_place(
                        program,
                        observed.state_symbol,
                        observed.statement_index,
                        expression,
                        projection,
                    )?;
                    let observed_contexts = facts
                        .flow
                        .semantic_constraint_contexts(observed.constraints)
                        .collect::<Vec<_>>();
                    let origin = crate::flow::operand_entry_origin_is_stable(
                        program, facts.flow, observed, &place,
                    )
                    .then(|| {
                        arithmetic::value_origin(
                            program,
                            facts,
                            flow,
                            observed.statement_index,
                            frames,
                            place.clone(),
                        )
                    })
                    .flatten();
                    let value = known_integer(
                        program,
                        facts,
                        state,
                        observed.statement_index,
                        &observed_contexts,
                        expression,
                        projection,
                        primitive,
                    )
                    .map(ScopedArithmeticValue::Integer)
                    .unwrap_or_else(|| {
                        origin
                            .clone()
                            .map(|origin| arithmetic::atom(origin, primitive))
                            .unwrap_or_else(|| ScopedArithmeticValue::Atom {
                                identity: format!(
                                    "\0instruction-capture:{:?}:{}:{:?}:{:?}:{primitive:?}:{place:?}",
                                    observed.state_symbol,
                                    observed.statement_index,
                                    observed.expression,
                                    observed.entry_origin_writes
                                ),
                                unsigned: !primitive.is_signed_integer(),
                            })
                    });
                    captures.push(Capture {
                        frontier: CaptureFrontier::Operand(observed),
                        primitive,
                        place,
                        origin,
                        value: value.clone(),
                    });
                    Some(value)
                };
                if primitive == PrimitiveType::Bool
                    || !projection.is_empty()
                    || matches!(
                        program.expression_table.expression(operand.expression),
                        ExpressionNode::Call(_)
                    )
                {
                    capture(operand.expression, primitive)
                } else {
                    arithmetic::scalar_term(
                        program,
                        facts,
                        &callable,
                        operand.expression,
                        &mut capture,
                    )
                }
            }
            fn current(
                &self,
                expression: ExpressionHandle,
                projection: &[facts::PlaceSegment],
                primitive: PrimitiveType,
                captures: &[Capture<'scope>],
            ) -> Option<ScopedArithmeticValue> {
                let Self {
                    program,
                    facts,
                    flow,
                    state,
                    statement,
                    contexts,
                    frames,
                } = *self;
                let machine = program.machine_by_symbol(flow.machine_symbol)?;
                let callable = Callable::Machine { machine, state };
                let observe = |expression, remaining: &[facts::PlaceSegment], primitive| {
                    if let Some(integer) = known_integer(
                        program, facts, state, statement, contexts, expression, remaining,
                        primitive,
                    ) {
                        return Some(ScopedArithmeticValue::Integer(integer));
                    }
                    let place =
                        value_place(program, state.symbol, statement, expression, remaining)?;
                    let origin = arithmetic::value_origin(
                        program,
                        facts,
                        flow,
                        statement,
                        frames,
                        place.clone(),
                    );
                    for capture in captures {
                        if capture.primitive != primitive {
                            continue;
                        }
                        let query_preserves =
                            crate::flow::call_flows(facts.flow.control, flow.calls)
                                .iter()
                                .filter(|call| call.statement_index == statement)
                                .all(|call| {
                                    crate::flow::entry_origin_is_stable(
                                        program,
                                        facts.flow.control,
                                        call.normal_return_writes,
                                        !call.normal_return_writes_complete,
                                        &place,
                                    )
                                });
                        let same_place = capture.place == place
                            && match capture.frontier {
                                CaptureFrontier::Operand(operand) => {
                                    crate::flow::operand_observation_before_statement_agrees(
                                        program,
                                        facts.flow,
                                        operand,
                                        state.symbol,
                                        statement,
                                        &place,
                                    )
                                }
                                CaptureFrontier::NormalReturn(call) => {
                                    crate::flow::call_return_before_statement_agrees(
                                        program, facts.flow, flow, call, statement, &place,
                                    )
                                }
                            };
                        let copied_return = match capture.frontier {
                            CaptureFrontier::NormalReturn(call) => {
                                crate::flow::value_origin_matches_call_return(
                                    program,
                                    facts.flow,
                                    machine,
                                    flow,
                                    statement,
                                    place.clone(),
                                    call,
                                    &capture.place,
                                    frames,
                                )
                            }
                            CaptureFrontier::Operand(_) => false,
                        };
                        if query_preserves
                            && (same_place
                                || copied_return
                                || capture
                                    .origin
                                    .as_ref()
                                    .zip(origin.as_ref())
                                    .is_some_and(|(left, right)| left == right))
                        {
                            return Some(capture.value.clone());
                        }
                    }
                    Some(ScopedArithmeticValue::Atom {
                        identity: format!(
                            "\0instruction-current:{:?}:{statement}:{primitive:?}:{place:?}",
                            state.symbol
                        ),
                        unsigned: !primitive.is_signed_integer(),
                    })
                };
                if primitive == PrimitiveType::Bool
                    || !projection.is_empty()
                    || matches!(
                        program.expression_table.expression(expression),
                        ExpressionNode::Call(_)
                    )
                {
                    observe(expression, projection, primitive)
                } else {
                    arithmetic::scalar_term(
                        program,
                        facts,
                        &callable,
                        expression,
                        |expression, primitive| observe(expression, &[], primitive),
                    )
                }
            }
            fn equal(
                &self,
                captured: &ScopedArithmeticValue,
                current: &ScopedArithmeticValue,
            ) -> bool {
                let Some(machine) = self.program.machine_by_symbol(self.flow.machine_symbol) else {
                    return false;
                };
                validation::scoped_arithmetic_values_relation(
                    self.program,
                    machine,
                    &[],
                    BinaryOperator::Equal,
                    captured,
                    current,
                ) == StrictArithmeticImplicationJudgment::Proven
            }
        }
        pub(super) fn operand_matches(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            flow: &FlowStateFact,
            state: &typed_trees::state::State,
            statement: usize,
            contexts: &[facts::FactContextHandle],
            operand: &FlowOperandFact,
            arguments: &QueryArguments<'_>,
            required: QueryArgument,
            primitive: PrimitiveType,
        ) -> bool {
            let Some(frames) = validation::CallFrameResolver::new(program) else {
                return false;
            };
            let values = Values {
                program,
                facts,
                flow,
                state,
                statement,
                contexts,
                frames: &frames,
            };
            let mut captures = Vec::new();
            let capture_result = values.captured(operand, &[], primitive, &mut captures);
            let Some(captured) = capture_result else {
                return false;
            };
            let current = arguments.scoped_value(
                program,
                facts,
                required,
                &[],
                primitive,
                &mut |expression, path, primitive| {
                    values.current(expression, path, primitive, &captures)
                },
            );
            current.is_some_and(|current| values.equal(&captured, &current))
        }

        pub(super) fn guarantee_matches(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            flow: &FlowStateFact,
            state: &typed_trees::state::State,
            statement: usize,
            contexts: &[facts::FactContextHandle],
            call: &checked_trees::FlowCallFact,
            candidate: &typed_trees::proposition::PropositionApplication,
            arguments: &QueryArguments<'_>,
            required: &[QueryArgument],
            operation: language_core::inline_assembly::AsmRegisterOperation,
        ) -> bool {
            let Some(contract) = validation::instruction_effect_contract(program, candidate) else {
                return false;
            };
            if contract.operation != facts::InstructionEffectOperation::Register(operation) {
                return false;
            }
            let Some(supplied) = QueryArguments::new(program, &contract) else {
                return false;
            };
            let Some(frame) = supplied.frames.last() else {
                return false;
            };
            if frame.arguments.len() != required.len() {
                return false;
            }
            frame
                .arguments
                .iter()
                .zip(required)
                .zip(operation.input_carriers())
                .all(|((supplied_argument, required), carrier)| {
                    let Some(primitive) = PrimitiveType::from_name(carrier.name()) else {
                        return false;
                    };
                    guarantee_value_matches(
                        program,
                        facts,
                        flow,
                        state,
                        statement,
                        contexts,
                        call,
                        &supplied,
                        *supplied_argument,
                        arguments,
                        *required,
                        primitive,
                    )
                })
        }

        pub(super) fn guarantee_value_matches(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            flow: &FlowStateFact,
            state: &typed_trees::state::State,
            statement: usize,
            contexts: &[facts::FactContextHandle],
            call: &checked_trees::FlowCallFact,
            supplied: &QueryArguments<'_>,
            supplied_argument: QueryArgument,
            arguments: &QueryArguments<'_>,
            required: QueryArgument,
            primitive: PrimitiveType,
        ) -> bool {
            let Some(frames) = validation::CallFrameResolver::new(program) else {
                return false;
            };
            let values = Values {
                program,
                facts,
                flow,
                state,
                statement,
                contexts,
                frames: &frames,
            };
            let mut captures = Vec::new();
            let Some(captured) = supplied.scoped_value(
                program,
                facts,
                supplied_argument,
                &[],
                primitive,
                &mut |expression, projection, primitive| {
                    super::super::call_guarantees::guarantee_scalar(
                        program,
                        facts,
                        flow,
                        call,
                        expression,
                        projection,
                        primitive,
                        &frames,
                        &mut |operand, path, primitive, post_state| {
                            if post_state {
                                if !post_binding_preserved(
                                    program,
                                    facts,
                                    operand,
                                    state.symbol,
                                    statement,
                                    &frames,
                                ) {
                                    return None;
                                }
                                let place = value_place(
                                    program,
                                    state.symbol,
                                    statement,
                                    operand.expression,
                                    path,
                                )?;
                                let returned_contexts = facts
                                    .flow
                                    .semantic_constraint_contexts(call.exit_constraints)
                                    .collect::<Vec<_>>();
                                let scalar = super::super::scalar_value_at_place(
                                    program,
                                    facts.semantic,
                                    returned_contexts
                                        .iter()
                                        .map(|context| facts.semantic.contexts.get(*context)),
                                    &place,
                                );
                                let value = if let Some(facts::ScalarValue::Integer(value)) = scalar {
                                    ScopedArithmeticValue::Integer(value)
                                } else {
                                    ScopedArithmeticValue::Atom {
                                        identity: format!(
                                            "\0instruction-return:{:?}:{}:{}:{primitive:?}:{place:?}",
                                            state.symbol, call.statement_index, call.call_ordinal
                                        ),
                                        unsigned: !primitive.is_signed_integer(),
                                    }
                                };
                                captures.push(Capture {
                                    frontier: CaptureFrontier::NormalReturn(call),
                                    primitive,
                                    place,
                                    origin: None,
                                    value: value.clone(),
                                });
                                Some(value)
                            } else {
                                values.captured(operand, path, primitive, &mut captures)
                            }
                        },
                    )
                },
            ) else {
                return false;
            };
            let Some(current) = arguments.scoped_value(
                program,
                facts,
                required,
                &[],
                primitive,
                &mut |expression, path, primitive| {
                    values.current(expression, path, primitive, &captures)
                },
            ) else {
                return false;
            };
            values.equal(&captured, &current)
        }

        fn post_binding_preserved(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            operand: &FlowOperandFact,
            state: symbols::SymbolHandle,
            statement: usize,
            frames: &validation::CallFrameResolver<'_>,
        ) -> bool {
            let Some(caller) = facts
                .flow
                .control
                .states
                .iter()
                .map(|(_, state)| state)
                .find(|state| state.state_symbol == operand.state_symbol)
            else {
                return false;
            };
            let Some(machine) = program.machine_by_symbol(caller.machine_symbol) else {
                return false;
            };
            let Some(source_state) = program.state_by_symbol(operand.state_symbol) else {
                return false;
            };
            if frames.expression_is_fixed_incoming_reference(source_state, operand.expression) {
                return true;
            }

            let Some(actual_type) = validation::expression_result_type_reference(
                program,
                machine,
                source_state,
                operand.expression,
            ) else {
                return false;
            };
            let actual_type = validation::peel_declared_constraints(program, actual_type);
            let typed_trees::types::TypeReferenceNode::Reference { referee, .. } =
                program.type_reference_table.type_reference(actual_type)
            else {
                return false;
            };
            let fresh = || CanonicalPlace {
                root: facts::PlaceRoot::Expression(operand.expression),
                segments: Vec::new(),
            };
            let binding = if let Some(place) = place_at(
                program,
                operand.state_symbol,
                operand.statement_index,
                operand.expression,
            ) {
                let Some(place_type) = validation::canonical_place_type_reference(
                    program,
                    operand.state_symbol,
                    operand.statement_index,
                    &place,
                ) else {
                    return false;
                };
                let place_type = validation::peel_declared_constraints(program, place_type);
                if program.package_qualified_type_identity(actual_type)
                    == program.package_qualified_type_identity(place_type)
                {
                    place
                } else if program.package_qualified_type_identity(
                    validation::peel_declared_constraints(program, *referee),
                ) == program.package_qualified_type_identity(place_type)
                {
                    fresh()
                } else {
                    return false;
                }
            } else {
                fresh()
            };
            crate::flow::operand_binding_before_statement_agrees(
                program, facts.flow, operand, state, statement, &binding,
            )
        }
        fn value_place(
            program: &dyn validation::CheckingInput,
            state: symbols::SymbolHandle,
            statement: usize,
            expression: ExpressionHandle,
            projection: &[facts::PlaceSegment],
        ) -> Option<CanonicalPlace> {
            let mut place =
                place_at(program, state, statement, expression).unwrap_or(CanonicalPlace {
                    root: facts::PlaceRoot::Expression(expression),
                    segments: Vec::new(),
                });
            if !projection.is_empty() {
                let reference =
                    validation::canonical_place_type_reference(program, state, statement, &place)?;
                place
                    .segments
                    .extend(applied_projection_segments(program, reference, projection)?);
            }
            Some(place)
        }
        fn known_integer(
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            state: &typed_trees::state::State,
            statement: usize,
            contexts: &[facts::FactContextHandle],
            expression: ExpressionHandle,
            projection: &[facts::PlaceSegment],
            primitive: PrimitiveType,
        ) -> Option<numerics::bignum::BigInt> {
            if !projection.is_empty() {
                let place = value_place(program, state.symbol, statement, expression, projection)?;
                return match super::super::scalar_value_at_place(
                    program,
                    facts.semantic,
                    contexts
                        .iter()
                        .map(|context| facts.semantic.contexts.get(*context)),
                    &place,
                )? {
                    facts::ScalarValue::Integer(value) => Some(value),
                    facts::ScalarValue::Boolean(value) if primitive == PrimitiveType::Bool => {
                        Some(numerics::bignum::BigInt::from_u128(u128::from(value)))
                    }
                    _ => None,
                };
            }
            if !matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Name(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Integer(_)
                    | ExpressionNode::Boolean(_)
            ) {
                return None;
            }
            match super::super::live_scalar_value(
                program, facts, state, statement, contexts, expression, primitive,
            )? {
                facts::ScalarValue::Integer(value) => Some(value),
                facts::ScalarValue::Boolean(value) if primitive == PrimitiveType::Bool => {
                    Some(numerics::bignum::BigInt::from_u128(u128::from(value)))
                }
                _ => None,
            }
        }
    }
    use operands::operand_matches;

    pub(super) fn proves_contract(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        point: ProgramPoint,
        contexts: &[facts::FactContextHandle],
        fact: &facts::Fact,
    ) -> bool {
        let (FactPayload::PropositionApplication { fact: source, .. }
        | FactPayload::ContractPropositionApplication { fact: source, .. }) = fact.payload
        else {
            return false;
        };
        let Some(application) = program.proof_facts.get(source).as_proposition() else {
            return false;
        };
        let ProgramPoint::Statement {
            state_symbol,
            statement_index,
            ..
        } = point
        else {
            return false;
        };
        proves(
            program,
            facts,
            state_symbol,
            statement_index,
            contexts,
            application,
        )
    }
    pub(in crate::checks::contracts) fn proves(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state_symbol: SymbolHandle,
        statement: usize,
        contexts: &[facts::FactContextHandle],
        application: &PropositionApplication,
    ) -> bool {
        evidence(
            program,
            facts.view(),
            state_symbol,
            statement,
            contexts,
            application,
            &[],
        )
        .is_some()
    }

    /*
        An invocation path distinguishes a helper's new instruction from a premise
        it merely preserves. Every normal callee exit contributes its own child;
        mutually exclusive exits may select different instructions. Two receipts
        overlap when any execution alternative would spend the same physical event.
    */
    #[derive(Clone)]
    pub(in crate::checks) struct InstructionEffectEvidence {
        pub(in crate::checks) event: facts::FactHandle,
        contract: arena::Handle<checked_trees::ContractProofFact>,
        pub(in crate::checks) children: Vec<InstructionEffectEvidence>,
        observation: ProgramPoint,
    }
    impl InstructionEffectEvidence {
        pub(in crate::checks) fn publish(
            &self,
            arena: &mut arena::PagedArenaSet<checked_trees::flow::FlowInstructionEffectReceiptFact>,
        ) -> arena::Handle<checked_trees::flow::FlowInstructionEffectReceiptFact> {
            let record = self.record(arena);
            arena.append(record)
        }
        pub(in crate::checks) fn observe_at(&mut self, point: ProgramPoint) {
            self.observation = point;
        }

        fn record(
            &self,
            arena: &mut arena::PagedArenaSet<checked_trees::flow::FlowInstructionEffectReceiptFact>,
        ) -> checked_trees::flow::FlowInstructionEffectReceiptFact {
            let children = self
                .children
                .iter()
                .map(|child| child.record(arena))
                .collect::<Vec<_>>();
            let children = arena.insert_many(children);
            checked_trees::flow::FlowInstructionEffectReceiptFact {
                event: self.event,
                contract: self.contract,
                observation: self.observation,
                children,
            }
        }

        pub(in crate::checks) fn overlaps(
            &self,
            facts: checked_trees::CheckFactsRef<'_>,
            other: &Self,
        ) -> bool {
            if facts.semantic.facts.get(self.event).point
                != facts.semantic.facts.get(other.event).point
            {
                return false;
            }
            match (self.children.is_empty(), other.children.is_empty()) {
                (true, true) => true,
                (false, false) => self.children.iter().any(|left| {
                    other.children.iter().any(|right| {
                        left.observation == right.observation && left.overlaps(facts, right)
                    })
                }),
                _ => false,
            }
        }
    }
    pub(in crate::checks) fn evidence(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        state_symbol: SymbolHandle,
        statement: usize,
        contexts: &[facts::FactContextHandle],
        application: &PropositionApplication,
        spent: &[InstructionEffectEvidence],
    ) -> Option<InstructionEffectEvidence> {
        evidence_inner(
            program,
            facts,
            state_symbol,
            statement,
            contexts,
            application,
            spent,
            &mut Vec::new(),
        )
    }
    fn evidence_inner(
        program: &dyn validation::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        state_symbol: SymbolHandle,
        statement: usize,
        contexts: &[facts::FactContextHandle],
        application: &PropositionApplication,
        spent: &[InstructionEffectEvidence],
        active: &mut Vec<SymbolHandle>,
    ) -> Option<InstructionEffectEvidence> {
        let contract = validation::instruction_effect_contract(program, application)?;
        let operation = contract.operation;
        let flow = facts.flow.control.state_by_symbol(state_symbol)?;
        let (_, state) = program.state_flow_site(flow.machine_symbol, state_symbol)?;
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let observation = ProgramPoint::Statement {
            machine_symbol: flow.machine_symbol,
            state_symbol,
            statement_index: statement,
        };
        let arguments = QueryArguments::new(program, &contract)?;
        let required = arguments.frames.last()?.arguments.clone();
        let carriers = match operation {
            facts::InstructionEffectOperation::Register(operation) => operation.input_carriers(),
            facts::InstructionEffectOperation::AtomicReleaseStore => &[],
        };
        let arity = if operation == facts::InstructionEffectOperation::AtomicReleaseStore {
            2
        } else {
            carriers.len()
        };
        if required.len() != arity {
            return None;
        }
        for context in contexts {
            for reference in facts
                .semantic
                .refs
                .span_or_empty(facts.semantic.contexts.get(*context).facts)
            {
                let event = facts.semantic.facts.get(reference.fact);
                if operation == facts::InstructionEffectOperation::AtomicReleaseStore
                    && matches!(
                        event.payload,
                        FactPayload::InstructionEffect {
                            operation: facts::InstructionEffectOperation::AtomicReleaseStore
                        }
                    )
                {
                    if atomic::matches_store(
                        program, facts, flow, state, statement, contexts, event, &arguments,
                        &required,
                    ) {
                        let receipt = InstructionEffectEvidence {
                            event: reference.fact,
                            contract: Default::default(),
                            children: Vec::new(),
                            observation,
                        };
                        if !spent.iter().any(|other| receipt.overlaps(facts, other)) {
                            return Some(receipt);
                        }
                    }
                    continue;
                }
                let ProgramPoint::CallEnsures {
                    machine_symbol,
                    state_symbol: event_state,
                    statement_index,
                    call_ordinal,
                } = event.point
                else {
                    continue;
                };
                if machine_symbol != flow.machine_symbol
                    || event_state != state_symbol
                    || !(order.precedes(statement_index, statement)
                        || (statement == state_statements(program, state.statement_nodes).len()
                            && order.position(statement_index).is_some()))
                {
                    continue;
                }
                let Some(call) = call_flows(facts.flow.control, flow.calls)
                    .iter()
                    .find(|call| {
                        call.statement_index == statement_index && call.call_ordinal == call_ordinal
                    })
                else {
                    continue;
                };
                if matches!(event.payload, FactPayload::CallCompletion { target } if target == call.target_symbol)
                {
                    for ensured in facts.proof.contract_fact_refs.span_or_empty(call.ensures) {
                        let guarantee = facts.proof.contract_facts.get(ensured.fact);
                        if guarantee.kind != checked_trees::ContractProofFactKind::Ensures {
                            continue;
                        }
                        let Some(candidate) =
                            program.proof_facts.get(guarantee.fact).as_proposition()
                        else {
                            continue;
                        };
                        let matches = match operation {
                            facts::InstructionEffectOperation::Register(register_operation) => {
                                operands::guarantee_matches(
                                    program,
                                    facts,
                                    flow,
                                    state,
                                    statement,
                                    contexts,
                                    call,
                                    candidate,
                                    &arguments,
                                    &required,
                                    register_operation,
                                )
                            }
                            facts::InstructionEffectOperation::AtomicReleaseStore => {
                                atomic::guarantee_matches(
                                    program, facts, flow, state, statement, contexts, call,
                                    candidate, &arguments, &required,
                                )
                            }
                        };
                        if !matches {
                            continue;
                        }
                        let Some(callee_flow) = facts
                            .flow
                            .control
                            .states
                            .iter()
                            .map(|(_, flow)| flow)
                            .find(|flow| flow.state_symbol == call.target_symbol)
                        else {
                            continue;
                        };
                        let Some(callee) = program.machine_by_symbol(callee_flow.machine_symbol)
                        else {
                            continue;
                        };
                        if active.contains(&callee.symbol) {
                            continue;
                        }
                        let exits = facts
                            .flow
                            .control
                            .exits
                            .iter()
                            .filter_map(|(_, exit)| {
                                (exit.machine_symbol == callee.symbol).then_some(exit)
                            })
                            .collect::<Vec<_>>();
                        if exits.is_empty() {
                            continue;
                        }
                        active.push(callee.symbol);
                        let children = exits
                            .iter()
                            .map(|exit| {
                                let contexts = facts
                                    .flow
                                    .semantic_constraint_contexts(exit.entry_constraints)
                                    .collect::<Vec<_>>();
                                let observation = ProgramPoint::Exit {
                                    machine_symbol: exit.machine_symbol,
                                    state_symbol: exit.state_symbol,
                                    statement_index: exit.statement_index,
                                    transition_target: exit.transition_target,
                                };
                                let spent_here = spent
                                    .iter()
                                    .filter(|receipt| {
                                        facts.semantic.facts.get(receipt.event).point == event.point
                                    })
                                    .flat_map(|receipt| &receipt.children)
                                    .filter(|receipt| receipt.observation == observation)
                                    .cloned()
                                    .collect::<Vec<_>>();
                                evidence_inner(
                                    program,
                                    facts,
                                    exit.state_symbol,
                                    exit.statement_index,
                                    &contexts,
                                    candidate,
                                    &spent_here,
                                    active,
                                )
                                .map(|mut receipt| {
                                    receipt.observation = observation;
                                    receipt
                                })
                            })
                            .collect::<Option<Vec<_>>>();
                        active.pop();
                        if let Some(children) = children {
                            let receipt = InstructionEffectEvidence {
                                event: reference.fact,
                                contract: ensured.fact,
                                children,
                                observation,
                            };
                            if !spent.iter().any(|other| receipt.overlaps(facts, other)) {
                                return Some(receipt);
                            }
                        }
                    }
                    continue;
                }
                if !matches!(event.payload, FactPayload::InstructionEffect { operation: recorded } if recorded == operation)
                {
                    continue;
                }
                let operands = facts.flow.control.operands.span_or_empty(call.operands);
                if operands.len() == required.len()
                    && operands.iter().zip(&required).zip(carriers).all(
                        |((operand, required), carrier)| {
                            let Some(primitive) = PrimitiveType::from_name(carrier.name()) else {
                                return false;
                            };
                            operand_matches(
                                program, facts, flow, state, statement, contexts, operand,
                                &arguments, *required, primitive,
                            )
                        },
                    )
                {
                    let receipt = InstructionEffectEvidence {
                        event: reference.fact,
                        contract: Default::default(),
                        children: Vec::new(),
                        observation,
                    };
                    if !spent.iter().any(|other| receipt.overlaps(facts, other)) {
                        return Some(receipt);
                    }
                }
            }
        }
        None
    }

    #[derive(Clone, Copy)]
    enum QueryArgument {
        Source(ExpressionHandle),
        Parameter {
            frame: usize,
            expression: ExpressionHandle,
        },
    }
    struct QueryFrame<'program> {
        declaration: &'program typed_trees::proposition::PropositionDefinition,
        application: &'program PropositionApplication,
        arguments: Vec<QueryArgument>,
    }
    struct QueryArguments<'program> {
        frames: Vec<QueryFrame<'program>>,
    }
    impl<'program> QueryArguments<'program> {
        fn new(
            program: &'program dyn validation::CheckingInput,
            contract: &validation::InstructionEffectContract<'program>,
        ) -> Option<Self> {
            let mut frames: Vec<QueryFrame<'program>> = Vec::new();
            for application in &contract.applications {
                let declaration =
                    program.proposition_definition_by_symbol(application.proposition)?;
                let actuals = expressions(program).expression_handles(application.arguments);
                if actuals.len() != program.proposition_parameters(declaration).len() {
                    return None;
                }
                let mut arguments = Vec::with_capacity(actuals.len());
                for expression in actuals {
                    let argument = if let Some(previous) = frames.last() {
                        let selected = expressions(program)
                            .name_path_node(*expression)
                            .and_then(|name| {
                                program
                                    .proposition_parameters(previous.declaration)
                                    .iter()
                                    .position(|parameter| parameter.symbol == name.symbol)
                            })
                            .and_then(|position| previous.arguments.get(position))
                            .copied();
                        selected.unwrap_or(QueryArgument::Parameter {
                            frame: frames.len() - 1,
                            expression: *expression,
                        })
                    } else {
                        QueryArgument::Source(*expression)
                    };
                    arguments.push(argument);
                }
                frames.push(QueryFrame {
                    declaration,
                    application,
                    arguments,
                });
            }
            Some(Self { frames })
        }
        fn scoped_value(
            &self,
            program: &dyn validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            argument: QueryArgument,
            projection: &[facts::PlaceSegment],
            primitive: PrimitiveType,
            source: &mut impl FnMut(
                ExpressionHandle,
                &[facts::PlaceSegment],
                PrimitiveType,
            ) -> Option<validation::ScopedArithmeticValue>,
        ) -> Option<validation::ScopedArithmeticValue> {
            let QueryArgument::Parameter { frame, expression } = argument else {
                let QueryArgument::Source(expression) = argument else {
                    unreachable!()
                };
                return source(expression, projection, primitive);
            };
            let frame = self.frames.get(frame)?;
            let parameters = program.proposition_parameters(frame.declaration);
            let project = |occurrence| {
                let (position, path, _) =
                    structural_parameter_place(program, parameters, occurrence)?;
                let (_, segments, _, _) =
                    resolve_structural_parameter_path(program, parameters, position, &path)?;
                Some((position, segments))
            };
            if let Some((position, mut segments)) = project(expression) {
                segments.extend_from_slice(projection);
                return self.scoped_value(
                    program,
                    facts,
                    *frame.arguments.get(position as usize)?,
                    &segments,
                    primitive,
                    source,
                );
            }
            super::call_guarantees::arithmetic::scalar_term(
                program,
                facts,
                &super::call_guarantees::callable::Callable::Proposition {
                    declaration: frame.declaration,
                },
                expression,
                |occurrence, primitive| {
                    if let Some((position, segments)) = project(occurrence) {
                        self.scoped_value(
                            program,
                            facts,
                            *frame.arguments.get(position as usize)?,
                            &segments,
                            primitive,
                            source,
                        )
                    } else {
                        program
                            .closed_integer_value_in(occurrence, frame.declaration.symbol)
                            .map(|value| validation::ScopedArithmeticValue::Integer(value.value))
                    }
                },
            )
        }
    }

    impl QueryArguments<'_> {
        fn source_projection(
            &self,
            program: &dyn validation::CheckingInput,
            argument: QueryArgument,
            suffix: &[facts::PlaceSegment],
        ) -> Option<(ExpressionHandle, Vec<facts::PlaceSegment>)> {
            match argument {
                QueryArgument::Source(expression) => Some((expression, suffix.to_vec())),
                QueryArgument::Parameter { frame, expression } => {
                    let frame = self.frames.get(frame)?;
                    let parameters = program.proposition_parameters(frame.declaration);
                    let (position, path, _) =
                        structural_parameter_place(program, parameters, expression)?;
                    let (_, mut segments, _, _) =
                        resolve_structural_parameter_path(program, parameters, position, &path)?;
                    segments.extend_from_slice(suffix);
                    self.source_projection(
                        program,
                        *frame.arguments.get(position as usize)?,
                        &segments,
                    )
                }
            }
        }
        fn binder_symbol(
            &self,
            program: &dyn validation::CheckingInput,
            frame: usize,
            position: usize,
        ) -> Option<SymbolHandle> {
            let current = self.frames.get(frame)?;
            let argument = current.application.binder_arguments.get(position)?;
            if argument.kind != typed_trees::proposition::PropositionBinderArgumentKind::Type {
                return None;
            }
            if let Some(previous) = frame.checked_sub(1) {
                let declaration = self.frames.get(previous)?.declaration;
                if let Some(position) = program
                    .proposition_binders(declaration)
                    .iter()
                    .position(|binder| binder.symbol == argument.symbol)
                {
                    return self.binder_symbol(program, previous, position);
                }
            }
            argument.symbol.is_valid().then_some(argument.symbol)
        }
    }
}
pub(super) use instruction_effects::proves as instruction_effect_proven;

pub(in crate::checks) use instruction_effects::evidence as instruction_effect_evidence;

pub(in crate::checks) use instruction_effects::InstructionEffectEvidence;
