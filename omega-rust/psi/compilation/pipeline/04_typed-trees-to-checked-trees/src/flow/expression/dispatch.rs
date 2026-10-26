//! Match effects fork after each comparison and meet at the result join.
//! A saved subject is not a promise about its storage after a pattern writes it;
//! only unchanged stable inputs can publish branch-local comparison evidence.
//!
//! Completed scalars, predicate-only domains and positive IEEE observations join
//! with the result. Scalars must agree when each arm finishes, before a later
//! pattern or sibling can mutate their former storage; unknown arms publish none.
//! Each covered arm
//! contributes the predicates already carried by the value it yields, and
//! the match result keeps only the consequences every arm carries. A fact
//! about some other place in the surrounding context does not move onto the
//! result. Routed domains keep their authorized provenance and are not joined
//! here; this publication is transport, not a new establishment.
use crate::flow::expression::Execution;
use crate::flow::state_values::qualifications::CarriedPredicate;
use arena::HandleSpan;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{FlowConstraintRef, FlowSemanticContextRef};
use facts::{FactPayload, ProgramPoint};
use typed_trees::expression::{MatchPattern, TableMatchExpression};

impl Execution<'_, '_, '_> {
    pub(super) fn dispatch(
        &mut self,
        expression: ExpressionHandle,
        dispatch: TableMatchExpression,
        destination: typed_trees::types::TypeReferenceHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> HandleSpan<checked_trees::FlowOperandAlternative> {
        let destination = if destination.is_valid() {
            destination
        } else {
            self.context
                .expression_result_type_at(self.program, self.machine, self.state, expression)
                .unwrap_or_default()
        };
        let structural = validation::structural_match_subject_type(
            self.program,
            self.machine,
            self.state,
            &dispatch,
        );
        let subject_operand = if structural.is_some() {
            self.evaluated_place(dispatch.subject, contexts, constraints)
                .capture
        } else {
            self.evaluated_operand(dispatch.subject, contexts, constraints)
                .capture
        };
        let outer_captures = self.match_subjects.len();
        if dispatch.capture.is_valid() {
            self.match_subjects
                .push((dispatch.capture, subject_operand));
        }
        let subject_writes = self.operand_writes.len();
        let mut joined = None;
        let mut completed_writes = Vec::new();
        let mut completed_reference_branches = Vec::new();
        let mut boolean_coverage = [false; 2];
        let mut case_coverage = Vec::new();
        let mut shared_predicates: Option<Vec<CarriedPredicate>> = None;
        let mut shared_scalar: Option<Option<facts::ScalarValue>> = None;
        let mut shared_members: Option<Vec<_>> = None;
        let mut value_coverage = false;
        let mut alternatives = HandleSpan::empty();
        for (ordinal, arm) in self
            .program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .enumerate()
        {
            let mut covered = matches!(arm.pattern, MatchPattern::Wildcard);
            if let Some(reference) = structural
                && let Some(case) = validation::structural_match_pattern_case(
                    self.program,
                    self.machine,
                    self.state,
                    &dispatch,
                    arm.pattern,
                )
                && match arm.pattern {
                    MatchPattern::Destructure(pattern) => {
                        !pattern.guard.is_valid()
                            && !self
                                .program
                                .expression_table
                                .match_fields(pattern.fields)
                                .iter()
                                .any(|field| {
                                    matches!(
                                        field.action,
                                        typed_trees::expression::MatchFieldAction::Equal(_)
                                    )
                                })
                    }
                    MatchPattern::Value(_) => true,
                    MatchPattern::Wildcard => false,
                }
            {
                if !case_coverage.contains(&case) {
                    case_coverage.push(case);
                }
                covered |= validation::structural_match_cases_are_covered(
                    self.program,
                    reference,
                    &case_coverage,
                );
            }
            if let MatchPattern::Value(pattern) = arm.pattern {
                let pattern_operand = self
                    .evaluated_operand(pattern, contexts, constraints)
                    .capture;
                if let Ok(ordinal) = u32::try_from(ordinal) {
                    let source_arm = dispatch.arms.handle_at(ordinal);
                    self.record_operator_invocation(
                        expression,
                        checked_trees::CheckedOperatorOccurrence::MatchEquality { source_arm },
                        &[subject_operand, pattern_operand],
                        *constraints,
                    );
                }
                if let Some(value) = self.program.expression_table.boolean_node(pattern) {
                    boolean_coverage[usize::from(*value)] = true;
                    covered = boolean_coverage.iter().all(|value| *value);
                }
            }
            let mut selected_contexts = *contexts;
            let mut selected_constraints = *constraints;
            if let MatchPattern::Destructure(pattern) = arm.pattern {
                self.evaluated_operand(pattern.condition, contexts, constraints);
                selected_contexts = *contexts;
                selected_constraints = *constraints;
                let prefix = self.write_prefix();
                for (matched, branch_contexts, branch_constraints) in [
                    (true, &mut selected_contexts, &mut selected_constraints),
                    (false, &mut *contexts, &mut *constraints),
                ] {
                    super::super::exits::append_predicate_context(
                        self.program,
                        self.semantic,
                        self.context,
                        self.state.symbol,
                        self.statement_index,
                        pattern.condition,
                        matched,
                        ProgramPoint::Statement {
                            machine_symbol: self.machine.symbol,
                            state_symbol: self.state.symbol,
                            statement_index: self.statement_index,
                        },
                        prefix.writes,
                        prefix.unknown,
                        branch_contexts,
                        branch_constraints,
                    );
                }
            }
            // Arm writes are absent on the fallthrough path. Retain their union
            // only after all alternatives, for later enclosing operands.
            let branch_prefix = self.write_prefix();
            if !matches!(arm.pattern, MatchPattern::Destructure(_))
                && self
                    .changed_operand_sources(&[(dispatch.subject, subject_writes)])
                    .is_empty()
                && let Ok(ordinal) = u32::try_from(ordinal)
            {
                let source_arm = dispatch.arms.handle_at(ordinal);
                let point = ProgramPoint::Statement {
                    machine_symbol: self.machine.symbol,
                    state_symbol: self.state.symbol,
                    statement_index: self.statement_index,
                };
                for (matched, branch_contexts, branch_constraints) in [
                    (true, &mut selected_contexts, &mut selected_constraints),
                    (false, &mut *contexts, &mut *constraints),
                ] {
                    super::super::exits::append_match_pattern_context(
                        self.program,
                        self.semantic,
                        self.context,
                        self.state.symbol,
                        self.statement_index,
                        FactPayload::MatchPattern {
                            expression,
                            arm: source_arm,
                            matched,
                        },
                        point,
                        branch_contexts,
                        branch_constraints,
                    );
                }
            }
            let completed = self.evaluated_operand_as(
                arm.value,
                destination,
                &mut selected_contexts,
                &mut selected_constraints,
            );
            let members = self.constructed_values(&[], destination, &completed);
            if let Some(shared) = &mut shared_members {
                shared.retain(|member| members.contains(member));
            } else {
                shared_members = Some(members);
            }
            let scalar = self.scalar_value_on_value(arm.value, &selected_contexts);
            shared_scalar = Some(match shared_scalar {
                None => scalar,
                Some(previous) => previous.filter(|previous| scalar.as_ref() == Some(previous)),
            });
            let value = self.context.control.operands.append(completed.capture);
            // A missing arm identity remains visible as an incomplete join;
            // never silently remove a successfully evaluated alternative.
            let source_arm = u32::try_from(ordinal)
                .map(|ordinal| dispatch.arms.handle_at(ordinal))
                .unwrap_or_default();
            self.context.control.operand_alternatives.append_to_span(
                &mut alternatives,
                checked_trees::FlowOperandAlternative { source_arm, value },
            );
            let yielded = self.predicates_on_value(arm.value, &selected_contexts);
            match shared_predicates.as_mut() {
                Some(shared) => shared.retain_mut(|predicate| {
                    let mut common = None;
                    for incoming in &yielded {
                        if let Some(candidate) = predicate.common_consequence(*incoming)
                            && common.is_none_or(|previous| candidate.implies(previous))
                        {
                            common = Some(candidate);
                        }
                    }
                    if let Some(common) = common {
                        *predicate = common;
                        true
                    } else {
                        false
                    }
                }),
                None => shared_predicates = Some(yielded),
            }
            completed_writes.extend(self.operand_writes[branch_prefix.length..].iter().cloned());
            completed_reference_branches.push(self.write_prefix());
            self.restore_write_prefix(branch_prefix);
            if let Some((previous_contexts, previous_constraints)) = joined {
                self.meet(
                    previous_contexts,
                    previous_constraints,
                    &mut selected_contexts,
                    &mut selected_constraints,
                );
            }
            joined = Some((selected_contexts, selected_constraints));
            if covered {
                value_coverage = true;
                break;
            }
        }
        // Later operands may observe any selected arm's effects. Each arm's
        // own capture retains only its first-match prefix, never a sibling's
        // writes or unknown-write flag.
        if !value_coverage {
            completed_reference_branches.push(self.write_prefix());
        }
        for writes in completed_writes {
            self.record_operand_writes(writes);
        }
        self.join_storage_prefixes(&completed_reference_branches);
        self.match_subjects.truncate(outer_captures);
        if let Some((joined_contexts, joined_constraints)) = joined {
            *contexts = joined_contexts;
            *constraints = joined_constraints;
        }
        let subject = self.context.control.operands.insert(subject_operand);
        self.retain_value_observation(
            expression,
            subject,
            subject_operand.referents,
            *contexts,
            *constraints,
        );
        if value_coverage {
            if let Some(members) = shared_members {
                self.publish_constructed_values(expression, members, contexts, constraints);
            }
            if let Some(shared) = shared_predicates {
                self.publish_joined_values(
                    expression,
                    &shared,
                    shared_scalar.flatten(),
                    contexts,
                    constraints,
                );
            }
        }
        // A partial result roster cannot authorize reference retention. Each
        // retained alternative keeps its own completed constraints and arm
        // identity; the joined context cannot substitute for missing coverage.
        if value_coverage {
            alternatives
        } else {
            HandleSpan::empty()
        }
    }

    /// Read this completed arm's owned scalar, preserving the selected source
    /// meaning. A reference arm carries a descriptor, not its referent's scalar;
    /// an authored indexing operation likewise needs its own result snapshot.
    fn scalar_value_on_value(
        &self,
        value: ExpressionHandle,
        contexts: &HandleSpan<FlowSemanticContextRef>,
    ) -> Option<facts::ScalarValue> {
        // Anonymous literals already own their exact denotation, even before
        // a declaration-backed carrier exists. A later destination checks
        // representability; inventing a source type here would move landing.
        if !crate::values::is_assignment_literal(self.program, value) {
            let reference = validation::expression_result_type_reference(
                self.program,
                self.machine,
                self.state,
                value,
            )?;
            let reference = validation::peel_declared_constraints(self.program, reference);
            if matches!(
                self.program.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::Reference { .. }
            ) || self.program.primitive_type_reference(reference).is_none()
            {
                return None;
            }
        }
        let source = self.completed_value_place(value)?;
        if matches!(source.root, facts::PlaceRoot::Symbol(_))
            && !validation::place_has_builtin_coordinates(
                self.program,
                self.machine,
                Some(self.state),
                value,
            )
        {
            return None;
        }
        crate::values::scalar_value_at_place(
            self.program,
            self.semantic,
            self.context
                .contexts
                .semantic_context_refs
                .span_or_empty(*contexts)
                .iter()
                .map(|reference| self.semantic.contexts.get(reference.context)),
            &source,
        )
    }

    /// Transportable predicates whose place is this arm value.
    ///
    /// Membership of a neighboring place stays where it is. A routed domain
    /// is absent here because copying it would drop the provenance the
    /// checker still has to rejoin.
    fn predicates_on_value(
        &self,
        value: ExpressionHandle,
        contexts: &HandleSpan<FlowSemanticContextRef>,
    ) -> Vec<CarriedPredicate> {
        let Some(source) = self.completed_value_place(value) else {
            return Vec::new();
        };
        let mut predicates = Vec::new();
        for fact in crate::flow::active_context_facts(self.context, self.semantic, *contexts) {
            let predicate = if let FactPayload::FloatPredicate { predicate } = fact.payload {
                CarriedPredicate::Float(predicate)
            } else if let Some((_, _, domain_symbol, semantic_domain)) =
                fact.payload.domain_membership()
            {
                if validation::field_domains::domain_requires_provenance(
                    self.program,
                    domain_symbol,
                ) {
                    continue;
                }
                let Some(domain) = self.program.domain_definition(domain_symbol) else {
                    continue;
                };
                if !domain.predicate_body.is_present() || domain.alias.is_some() {
                    continue;
                }
                CarriedPredicate::Domain {
                    // The joined row names the expression as its value; the
                    // authored domain spelling belongs to the captured carrier.
                    domain: HandleSpan::empty(),
                    domain_symbol,
                    semantic_domain,
                }
            } else {
                continue;
            };
            let Some(fact_place) = fact.place.as_place() else {
                continue;
            };
            let Some(fact_place) = crate::flow::canonical_place_from_semantic_place(
                self.program,
                self.semantic,
                self.semantic.places.get(fact_place),
            ) else {
                continue;
            };
            if !crate::flow::canonical_places_agree(self.program, &source, &fact_place) {
                continue;
            }
            if !predicates.contains(&predicate) {
                predicates.push(predicate);
            }
        }
        predicates
    }

    fn publish_joined_values(
        &mut self,
        expression: ExpressionHandle,
        domains: &[CarriedPredicate],
        scalar: Option<facts::ScalarValue>,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        if domains.is_empty() && scalar.is_none() {
            return;
        }
        let place = self.semantic.append_expression_place(expression);
        let point = ProgramPoint::Statement {
            machine_symbol: self.machine.symbol,
            state_symbol: self.state.symbol,
            statement_index: self.statement_index,
        };
        let mut refs = HandleSpan::empty();
        if let Some(scalar) = scalar {
            let value = self.semantic.scalar_values.append(scalar);
            let fact = self.semantic.append_fact(facts::Fact {
                place: facts::FactPlace::Place(place),
                point,
                origin: facts::FactOrigin::StatementTransfer,
                evidence: facts::QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::Propagated,
                    self.state.symbol,
                ),
                payload: FactPayload::AssignedScalarValue { value },
            });
            self.semantic.append_ref(&mut refs, fact);
        }
        for domain in domains {
            let fact = self.semantic.append_fact(facts::Fact {
                place: facts::FactPlace::Place(place),
                point,
                origin: facts::FactOrigin::StatementTransfer,
                evidence: facts::QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::Propagated,
                    self.state.symbol,
                ),
                payload: domain.payload(expression),
            });
            self.semantic.append_ref(&mut refs, fact);
        }
        let context = self.semantic.append_context(point, refs);
        crate::flow::reference_spans::append_flow_reference(
            &mut self.context.contexts.semantic_context_refs,
            contexts,
            FlowSemanticContextRef { context },
        );
        crate::flow::append_constraint_ref(
            &mut self.context.contexts.constraint_refs,
            constraints,
            checked_trees::FlowConstraintKind::SemanticContext { context },
        );
    }
}
