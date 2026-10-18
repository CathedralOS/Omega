//! A completed result keeps its checked scalar guarantees at evaluation.
//!
//! Reuse validation's destination-independent result query: it joins all Match
//! arms by normalized type identity and does not copy operand refinements onto
//! arithmetic results. Publish after child effects and branch meets, never into
//! a callee's requires context or an unselected arm. The subject is this result
//! occurrence, not storage read while producing it.
//!
//! Checked domain qualifications and enforced floating ranges describe the
//! completed result, not its operands or a receiving annotation. Range
//! predicates join the same captured occurrence as tags; invocation checking
//! need not reconstruct a source call
//! or skip its preconditions. This is transport, not establishment authority.
//! The normalized instance travels with the declaration through every fact copy.
//! Equality of exact instances is not a license to equate different open index
//! expressions; those still require their selected normalization/equality proof.
//! Normal-return integer bounds use the selected declaration's contract, not
//! body evaluation or a later read of mutable actuals. Publishing them at the
//! expression occurrence lets constructors and assignments consume one common
//! snapshot; a later sibling write invalidates source storage, not this result.
use crate::flow::append_constraint_ref;
use crate::flow::expression::Execution;
use crate::flow::reference_spans;
use arena::HandleSpan;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
use facts::{Fact, FactOrigin, FactPayload, FactPlace, ProgramPoint, QualificationEvidence};

impl Execution<'_, '_, '_> {
    fn cast_predicate_observations(
        &self,
        expression: ExpressionHandle,
        target: typed_trees::types::TypeReferenceHandle,
        contexts: HandleSpan<FlowSemanticContextRef>,
    ) -> Vec<(symbols::SymbolHandle, language_semantics::SemanticDomainId)> {
        let ExpressionNode::Cast(cast) = self.program.expression_table.expression(expression)
        else {
            return Vec::new();
        };
        if cast.form.is_recast() {
            return Vec::new();
        }
        let Some(source_type) = validation::expression_result_type_reference(
            self.program,
            self.machine,
            self.state,
            cast.value,
        ) else {
            return Vec::new();
        };
        if !validation::declared_carriers_match(self.program, source_type, target) {
            return Vec::new();
        }
        let Some(source) = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state.symbol,
            self.statement_index,
            cast.value,
        ) else {
            return Vec::new();
        };
        let handles = crate::flow::active_context_handles(self.context, contexts);
        if !crate::flow::place_cases_are_selected(
            self.program,
            self.semantic,
            &handles,
            self.machine.symbol,
            self.state.symbol,
            self.statement_index,
            &source,
        ) {
            return Vec::new();
        }
        crate::flow::active_context_facts(self.context, self.semantic, contexts)
            .into_iter()
            .filter_map(|fact| {
                let (_, _, symbol, identity) = fact.payload.domain_membership()?;
                let domain = self.program.domain_definition(symbol)?;
                if !domain.predicate_body.is_present()
                    || domain.alias.is_some()
                    || validation::field_domains::domain_requires_provenance(self.program, symbol)
                {
                    return None;
                }
                // A value cast snapshots owned contents, never predicates on
                // a borrowed view or a referent hidden inside an owned value.
                // Reuse the invalidation owner's exact dependency paths.
                let dependency = self.domains.dependency_fact(symbol)?;
                if dependency.dependencies.is_empty()
                    || !self.domains.dependency_paths(dependency).all(|path| {
                        (0..=path.len()).all(|length| {
                            validation::project_type_reference_from_segments(
                                self.program,
                                source_type,
                                &path[..length],
                            )
                            .is_some_and(|reference| {
                                !crate::flow::operand_binds_live_view(self.program, reference)
                            })
                        })
                    })
                {
                    return None;
                }
                let place = fact.place.as_place()?;
                let place = crate::flow::canonical_place_from_semantic_place(
                    self.program,
                    self.semantic,
                    self.semantic.places.get(place),
                )?;
                crate::flow::canonical_places_agree(self.program, &source, &place)
                    .then_some((symbol, identity))
            })
            .collect()
    }

    pub(super) fn append_result_facts(
        &mut self,
        expression: ExpressionHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        if !self.program.expression_table.expression_is_valid(expression)
            // Storage-backed values already have invalidation-aware facts. A
            // declared place type cannot restore a promise lost to a write.
            || matches!(self.program.expression_table.expression(expression),
                ExpressionNode::Name(_) | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_) | ExpressionNode::Borrow(_))
        {
            return;
        }
        let Some(reference) = validation::expression_result_type_reference(
            self.program,
            self.machine,
            self.state,
            expression,
        ) else {
            return;
        };
        // Source and flow qualification judgments discharge these producer
        // obligations; a cast's own judgment reads its operand, never the
        // provisional result fact. The completed result owns every declared atom,
        // not just predicate-free tags. Omitting predicate domains here made
        // a checked cast lose its membership at an inline call or return.
        // This does not establish a new domain on a bare operand or storage.
        // A structural carrier is no different: a checked record
        // qualification describes its completed value exactly as a scalar
        // one does.
        let mut domains =
            validation::field_domains::domain_constraint_identities(self.program, reference);
        // Semantic erasure cannot change a predicate's truth on an unchanged
        // carrier. Keep only predicate-only observations from the evaluated
        // operand; erased semantic tags and routed authority never follow.
        for observed in self.cast_predicate_observations(expression, reference, *contexts) {
            if !domains.contains(&observed) {
                domains.push(observed);
            }
        }
        let mut predicates = Vec::new();
        crate::values::float_predicates::declared_type_predicates(
            self.program,
            reference,
            &mut predicates,
        );
        if let Some(call) = self.program.expression_table.call_node(expression) {
            validation::normal_return_float_predicates(self.program, call, &mut predicates);
        }
        let integer_bounds = self
            .program
            .expression_table
            .call_node(expression)
            .and_then(|call| validation::normal_return_integer_bounds(self.program, call))
            .and_then(|(minimum, maximum)| {
                Some(facts::IntegerRange {
                    minimum: numerics::bignum::BigInt::from_i64(minimum?),
                    maximum: numerics::bignum::BigInt::from_i64(maximum?),
                })
            });
        let zero_fields = if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::StructLiteral(_)
        ) {
            validation::field_domains::declared_owned_field_domain_identities(self.program, reference)
                .into_iter()
                .filter(|(path, domain, _)| {
                    crate::flow::literal_value_projections(self.program, expression, reference, path, false)
                        .is_some_and(|projections| !projections.is_empty() && projections.iter().all(|projection| {
                            matches!(projection.source, crate::flow::LiteralValueSource::OmittedZero { type_reference, .. }
                                if validation::field_domains::domain_admits_zero_byte_carrier(self.program, *domain, type_reference))
                        }))
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let field_bounds = validation::declared_float_field_bounds(self.program, reference);
        if domains.is_empty()
            && predicates.is_empty()
            && integer_bounds.is_none()
            && zero_fields.is_empty()
            && field_bounds.is_empty()
        {
            return;
        }
        let place = self.semantic.append_expression_place(expression);
        let point = ProgramPoint::Statement {
            machine_symbol: self.machine.symbol,
            state_symbol: self.state.symbol,
            statement_index: self.statement_index,
        };
        let mut refs = HandleSpan::empty();
        if let Some(bounds) = integer_bounds {
            let bounds = self.semantic.integer_ranges.append(bounds);
            let fact = self.semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::Propagated,
                    self.state.symbol,
                ),
                payload: FactPayload::AssignedIntegerBounds { bounds },
            });
            self.semantic.append_ref(&mut refs, fact);
        }
        // Zero formation establishes only predicates true of this exact
        // inline image. Publish on the completed constructor so calls, stores
        // and returns consume ordinary occurrence evidence. Declaration-only
        // membership and a return-specific zero proof would bypass this owner.
        for (path, domain_symbol, semantic_domain) in zero_fields {
            let field = crate::semantic::places::append_place(
                self.semantic,
                facts::PlaceRoot::Expression(expression),
                &path,
            );
            let fact = self.semantic.append_fact(Fact {
                place: FactPlace::Place(field),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::CheckedValidation,
                    self.state.symbol,
                ),
                payload: FactPayload::DomainMembership {
                    value: expression,
                    domain: HandleSpan::empty(),
                    domain_symbol,
                    semantic_domain,
                },
            });
            self.semantic.append_ref(&mut refs, fact);
        }
        for (domain_symbol, semantic_domain) in domains {
            let fact = self.semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::Propagated,
                    self.state.symbol,
                ),
                payload: FactPayload::DomainMembership {
                    value: expression,
                    domain: HandleSpan::empty(),
                    domain_symbol,
                    semantic_domain,
                },
            });
            self.semantic.append_ref(&mut refs, fact);
        }
        // A checked complete data value owns its field predicates. Publish
        // them on this occurrence so ordinary stores and projections snapshot
        // the value; no declaration reread may restore a mutated storage fact.
        for bound in field_bounds {
            let Some(predicate) = bound.predicate() else {
                continue;
            };
            let field = self.semantic.append_expression_place(expression);
            for symbol in bound.path {
                self.semantic
                    .push_place_segment(field, facts::PlaceSegment::Field { symbol });
            }
            let fact = self.semantic.append_fact(Fact {
                place: FactPlace::Place(field),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::CheckedValidation,
                    self.state.symbol,
                ),
                payload: FactPayload::FloatPredicate { predicate },
            });
            self.semantic.append_ref(&mut refs, fact);
        }
        for predicate in predicates {
            let fact = self.semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::StatementTransfer,
                evidence: QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::Propagated,
                    self.state.symbol,
                ),
                payload: FactPayload::FloatPredicate { predicate },
            });
            self.semantic.append_ref(&mut refs, fact);
        }
        let context = self.semantic.append_context(point, refs);
        reference_spans::append_flow_reference(
            &mut self.context.contexts.semantic_context_refs,
            contexts,
            FlowSemanticContextRef { context },
        );
        append_constraint_ref(
            &mut self.context.contexts.constraint_refs,
            constraints,
            FlowConstraintKind::SemanticContext { context },
        );
    }
}
