//! Domain predicates over exact, live assignment values. Mutation invalidation
//! owns their lifetime; this consumer neither replays a body nor assumes zeros.

use validation::CanonicalPlace;

use crate::flow::{canonical_place_from_semantic_place, relative_place_segments_from_expression};
use facts::{FactContextHandle, FactPayload, FactPlan, PlaceHandle};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionHandle;

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

/// Range and dynamic-selector obligations prove every possible element in
/// declaration-derived geometry.
/// Recorded facts supply membership, never the collection's extent: a
/// contiguous prefix of writes can still leave later cells uninitialized.
fn prove_domain_by_extent_enumeration(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    subject: &CanonicalPlace,
    domain: SymbolHandle,
) -> bool {
    let Some(position) = subject.segments.iter().position(|segment| {
        matches!(segment, facts::PlaceSegment::FixedRange { .. })
            || matches!(segment, facts::PlaceSegment::Index { expression }
                    if !matches!(program.expression_table.expression(*expression),
                        typed_trees::expression::ExpressionNode::Range(_)))
    }) else {
        return false;
    };
    let (start, end) = match subject.segments[position] {
        facts::PlaceSegment::FixedRange { start, end } => (start, end),
        facts::PlaceSegment::Index { .. } => (0, usize::MAX),
        _ => return false,
    };
    let end = if start == 0 && end == usize::MAX {
        let mut collection = subject.clone();
        collection.segments.truncate(position);
        let Some(length) = contexts.iter().find_map(|context| {
            let point = semantic.contexts.get(*context).point;
            let state = match point {
                facts::ProgramPoint::State { state_symbol, .. }
                | facts::ProgramPoint::Statement { state_symbol, .. }
                | facts::call_point_triple_pattern!(state_symbol, ..)
                | facts::ProgramPoint::TransitionArm { state_symbol, .. }
                | facts::ProgramPoint::Exit { state_symbol, .. } => state_symbol,
                _ => return None,
            };
            validation::closed_collection_length_at_place(program, state, &collection)
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
            element.segments[position] = facts::PlaceSegment::FixedIndex { index };
            prove_domain_at_place(program, semantic, contexts, &element, domain)
        })
}

/// One walk over the live facts in the selected contexts: `accepts` judges the
/// payload, `at_subject` the fact's own place. Membership, permission and
/// carried-origin obligations are payload judgments over this one walk; none of
/// them rebuilds the context/view/fact scan for itself.
pub(super) fn live_fact_at_place(
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    accepts: impl Fn(&facts::Fact) -> bool,
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

/// The membership specialization of the same walk: the payload supplies the
/// carried fact's declaration and interned instance, `satisfies` decides
/// whether that membership answers the obligation at hand.
pub(super) fn carried_membership_at(
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    satisfies: impl Fn(SymbolHandle, language_semantics::SemanticDomainId) -> bool,
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

/// Whether a carried fact at `candidate` covers the `subject` elementwise:
/// same canonical root, and at every position either equal segments or a
/// containing `FixedRange` (the whole-extent `0..usize::MAX` row covers a
/// runtime `Index`; a finite range covers contained indices -- see
/// [`segments_cover_subject`]).
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

/// Whether a carried membership fact on `subject` (a declared constraint or
/// established contract evidence) already implies `domain`. This is the
/// evidence arm of [`prove_domain_at_place`]: routed provenance and prior
/// `ensures`/`in`-constrained declarations answer membership questions that
/// predicate discharge cannot reach.
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
        |candidate, _| {
            validation::field_domains::domain_membership_implies(program, candidate, domain)
        },
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
        .domain(subject, domain, &mut Vec::new())
}

/// Membership of `subject` in one EXACT indexed (const-generic) instance.
///
/// [`prove_domain_at_place`] answers membership questions by declaration
/// symbol, which is the whole theory of an atomic domain but not of a family:
/// `BelowIndex<4>` and `BelowIndex<9>` share a symbol and have no variance
/// relationship, so a fact about one must not satisfy an obligation for the
/// other. [`validation::field_domains::domain_membership_implies`] correctly
/// refuses that reduction, so an indexed requirement matches the carried fact's
/// own interned instance identity instead of its family symbol.
///
/// Only live membership evidence is consulted. A declared constraint, a
/// completed call's `ensures`, or an established initializer each record their
/// exact instance; nothing here re-derives an instance from a type annotation
/// or a predicate that could hold for several applications.
pub(in crate::checks::contracts) fn prove_indexed_domain_at_place(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    subject: &CanonicalPlace,
    domain: SymbolHandle,
    instance: language_semantics::SemanticDomainId,
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

/// Per-byte predicates survive a window; UTF-8 and nonempty do not. Strip
/// exactly one authenticated range selector, then prove the live base through
/// the same occurrence facts used for every other place. Bounds and custody
/// remain obligations of the range operation.
fn subslice_domain_at_place(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    subject: &CanonicalPlace,
    domain: SymbolHandle,
) -> bool {
    if !validation::field_domains::domain_is_subslice_preserving(program, domain) {
        return false;
    }
    let Some((selection, segments)) = subject.segments.split_last() else {
        return false;
    };
    let is_window = match selection {
        // A runtime-endpoint slice arrives as `OpaqueRange` once its
        // callee-local range expression is erased: still a window of the
        // carrier, so a subslice-preserving class survives it identically.
        facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => true,
        facts::PlaceSegment::Index { expression } => matches!(
            program.expression_table.expression(*expression),
            typed_trees::expression::ExpressionNode::Range(_)
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

/// Whether a live per-byte class fact at `subject` implies `required`. The one
/// scan behind both consumers of minted `FactPayload::BytePredicate` evidence:
/// a domain reduced to exactly one recognized byte predicate proves
/// membership through it, and a `requires pred(place)` conjunct or a
/// `pred(self)` initializer obligation discharges against it — identical
/// place-equality plus `implies` semantics in every consumer.
pub(in crate::checks::contracts) fn byte_predicate_proven_at_place(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    subject: &CanonicalPlace,
    required: validation::field_domains::ByteSequencePredicate,
) -> bool {
    // Exact literal contents and minted per-byte classes are two forms of
    // live place evidence. Both are retired by the ordinary write owner.
    if literal_at_place(
        program,
        semantic,
        contexts
            .iter()
            .map(|context| semantic.contexts.get(*context)),
        subject,
    )
    .and_then(|literal| typed_trees::byte_predicates::literal_bytes(program, literal))
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
                        crate::flow::canonical_places_agree(program, &candidate, subject)
                    })
            })
    })
}

impl AssignedValues<'_> {
    /// Whether a live per-byte class proved for the whole carrier entails the
    /// requested domain. An indexed write retires the exact value snapshot but
    /// leaves this class behind, which is how a text carrier stays provable
    /// across `buffer[i] = byte`. A domain that is not exactly one recognized
    /// byte predicate is never reached this way.
    fn byte_predicate(&self, subject: &CanonicalPlace, symbol: SymbolHandle) -> bool {
        let Some(required) = validation::field_domains::domain_byte_predicate(self.program, symbol)
        else {
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
        domain: &typed_trees::domain::DomainDefinition,
    ) -> Option<CanonicalPlace> {
        let segments = relative_place_segments_from_expression(self.program, expression, domain)?;
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
        let Some(domain) = self.program.domain_definition(symbol) else {
            return false;
        };
        if let Some(literal) = self.literal(subject)
            && validation::field_domains::string_literal_expression_grants_domain(
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
        // Integer predicates share qualification's typed, declaration-scoped
        // judgment. The live value is captured independently of the requested
        // domain; substituting its binder must not invent routed provenance.
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
            match validation::integer_domain_membership(
                self.program,
                symbol,
                &[],
                domain.semantic_id,
                &validation::ScopedArithmeticValue::Integer(value),
            ) {
                validation::StrictArithmeticImplicationJudgment::Proven => return true,
                validation::StrictArithmeticImplicationJudgment::Refuted => return false,
                // Numerical evidence cannot supply a nested routed domain.
                // The place evidence below may already carry that membership.
                validation::StrictArithmeticImplicationJudgment::Unknown => {}
            }
        }
        // A route is an authorized origin no proof can replace. A semantic
        // role is not a membership obligation: binding qualifications, not
        // facts, select its operator meaning, so its predicates decide here.
        if !domain.establishment_routes.is_empty() {
            return false;
        }
        let facts = self.program.proof_facts.span_or_empty(domain.facts);
        if facts.is_empty() {
            // "A declaration with neither predicates nor routes adds no
            // membership obligation" (domains.md, declaration and membership):
            // every carrier value is a member, whatever is known about it.
            return validation::field_domains::domain_is_vacuous(
                self.program,
                symbol,
                &mut Vec::new(),
            );
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
                    ) || self.domain(&nested, membership.domain_symbol, active)
                }),
            ProofFact::Proposition(_) => false,
        });
        active.pop();
        proved
    }

    /// The place's installed nominal tag, when a live assignment fact has
    /// decided it. `let x = T::C{...}` mints one; nothing else does.
    fn installed_case(&self, subject: &CanonicalPlace) -> Option<SymbolHandle> {
        self.contexts.iter().find_map(|context| {
            self.semantic
                .context_view(self.semantic.contexts.get(*context))
                .facts()
                .find_map(|fact| {
                    let FactPayload::AssignedCase { variant } = fact.payload else {
                        return None;
                    };
                    let place = fact.place.as_place()?;
                    let candidate = canonical_place_from_semantic_place(
                        self.program,
                        self.semantic,
                        self.semantic.places.get(place),
                    )?;
                    crate::flow::canonical_places_agree(self.program, &candidate, subject)
                        .then_some(variant)
                })
        })
    }

    fn boolean(
        &self,
        subject: &CanonicalPlace,
        domain: &typed_trees::domain::DomainDefinition,
        expression: ExpressionHandle,
    ) -> Option<bool> {
        // `self in T::C` (and the `self == T::C` equality it lowers to) names
        // a nominal tag, not a scalar comparison: the assigned case at the
        // subject's place decides it.
        if let Some((member, case)) =
            crate::proof::exact_outcome_case_test(self.program, expression)
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
                // Preserve this domain prover's existing pure constant vocabulary.
                // The shared evaluator itself never picks an arithmetic domain.
                self.program
                    .expression_table
                    .constant_integer_value(expression)
                    .map(|value| ScalarValue::Integer(numerics::bignum::BigInt::from_i64(value)))
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
