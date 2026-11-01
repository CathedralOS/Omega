//! Prepare one exact call and publish its completed result evidence.
//!
//! Inputs and reference bindings freeze before the call. An observed body
//! supplies its own returned value; an unavailable body retains ordinary
//! contract-only inference. Neither route re-evaluates the actual operands.

use super::*;

use super::super::selected_contracts::PreparedSelectedGuarantees;
pub(super) use super::call_preparation::PreparedNormalCall;

pub(super) struct CompletedBodyResult {
    pub(super) value: StructuralTerm,
    pub(super) integer_range: Option<checked_trees::IntegerRangeRecipe>,
}

pub(super) struct CapturedNormalCall {
    pub(super) result: Option<StructuralTerm>,
    pub(super) post_storage: Vec<PostCallStorage>,
}

pub(super) enum CompletedCallObservation<'program> {
    BodyPaths(Vec<NormalReturnPath<'program>>),
    Contract,
}

impl<'program> StructuralJudge<'program> {
    /// The ordinary return-range owner consumes the immutable scalar actuals
    /// already captured by this call. It owns complete return-path joins and
    /// effects; no caller initializer or post-call storage is reconstructed.
    fn completed_call_integer_range(
        &self,
        owner: &SelectedCallContract<'program>,
        call: &typed_trees::expression::TableCallExpression,
        parameters: &[typed_trees::signature::StateParameter],
        actuals: &[StructuralTerm],
        result_type: TypeReferenceHandle,
    ) -> Option<checked_trees::IntegerRangeRecipe> {
        let SelectedCallContract::Concrete(machine, entry) = owner else {
            return None;
        };
        if !call.selects_only_nominal_route()
            || !call.machine_arguments.is_empty()
            || !self.program.machine_type_parameters(machine).is_empty()
        {
            return None;
        }
        let primitive = self.program.primitive_type_reference(result_type)?;
        primitive.integer_bounds()?;
        let actuals = parameters
            .iter()
            .zip(actuals)
            .filter_map(|(parameter, value)| {
                if parameter.is_mutable || parameter.is_const {
                    return None;
                }
                let primitive = self
                    .program
                    .primitive_type_reference(parameter.type_reference)?;
                let (low, high) = primitive.integer_bounds()?;
                let value = StructuralTerm::IntegerEmbedding {
                    reference: parameter.type_reference,
                    value: Box::new(value.clone()),
                };
                Some((
                    parameter.symbol,
                    crate::proof_contracts::arithmetic_domains::Interval {
                        low: i64::try_from(low).ok(),
                        high: i64::try_from(high).ok(),
                    },
                    self.completed_integer_range_recipe(&value),
                ))
            })
            .collect::<Vec<_>>();
        crate::proof_contracts::arithmetic_domains::infer_return_interval(
            self.program,
            machine,
            entry,
            Some(primitive),
            &actuals,
        )
        .and_then(|(_, recipe)| recipe)
    }

    pub(super) fn capture_prepared_normal_call(
        &mut self,
        prepared: &PreparedNormalCall<'program, '_>,
        prepared_guarantees: &PreparedSelectedGuarantees<'_>,
        body_result: Option<&CompletedBodyResult>,
        scope: ValueScope<'_>,
    ) -> Option<CapturedNormalCall> {
        let program = self.program;
        let expression = prepared.expression;
        let call = prepared.call;
        let owner = prepared.owner;
        let application = &prepared.application;
        let parameters = prepared.parameters.as_ref();
        let entry_bindings = &prepared.bindings.entry_bindings;
        let bindings = &prepared.bindings.bindings;
        let mut post_storage = prepared.bindings.post_storage.clone();
        let actuals = prepared.actuals;
        let callable = prepared.callable;
        // The applied result telescope borrows these immutable prepared rows;
        // this pure correspondence does not observe the actual operands again.
        let result_presentation = if let Some(reserved) = &prepared.result {
            let presentation = application.runtime_result_presentation(
                program,
                &owner,
                call,
                expression,
                reserved.receiving_type,
                crate::CallSite::Expression { expression, call }
                    .parameters(program, scope.0.symbol),
                callable,
                parameters,
                bindings,
            )?;
            (presentation.type_reference == reserved.type_reference).then_some(presentation)
        } else {
            None
        };
        let contracts = owner_contracts(program, &owner).collect::<Vec<_>>();
        let captured = result_presentation
            .as_ref()
            .zip(prepared.result.as_ref())
            .map(|(presentation, reserved)| StructuralTerm::CapturedValue {
                statement: arena::Handle::invalid(),
                expression,
                occurrence: reserved.occurrence,
                callable,
                result_type: presentation.type_reference,
                integer_range:
                    crate::proof_contracts::arithmetic_domains::deferred_integer_observation(
                        program,
                        scope.0.symbol,
                        expression,
                        program.primitive_type_reference(presentation.type_reference),
                    )
                    .or_else(|| match body_result {
                        Some(completed) => completed.integer_range.clone(),
                        None => self.completed_call_integer_range(
                            &owner,
                            call,
                            parameters,
                            actuals,
                            presentation.type_reference,
                        ),
                    })
                    .map(Arc::new),
            });
        if super::super::super::struct_trace() {
            eprintln!(
                "STRUCT normal-capture bindings expression={expression:?} entry={entry_bindings:?} post={bindings:?}"
            );
        }
        // An overwritten referent supplies no stable input observation, but
        // does not erase independent guarantees about this returned value.
        // Equations requiring an omitted formal fail exact substitution.
        if let Some(captured) = &captured {
            post_storage
                .extend(self.stage_completed_result_reads(scope.0, scope.1, expression, captured));
        }
        let guarantees =
            self.complete_selected_guarantees(prepared_guarantees, captured.as_ref(), 0)?;
        let mut equations = guarantees
            .equations
            .into_iter()
            .map(CapturedResultEquation::public)
            .collect::<Vec<_>>();
        // A completed occurrence keeps its identity. Both existing value-call
        // judgments establish repeatability: conditional primitive normal
        // returns, and the custody-free denotational call owner. Neither an
        // empty write frame nor a generic declaration's spelling establishes it.
        // Result presentation above has authenticated the complete applied
        // carrier; importing the raw generic return type here would lose that
        // correspondence. Actuals remain the completed input observations.
        if let (Some(captured), Some(presentation)) = (&captured, &result_presentation)
            && !crate::proof_contracts::quotients::type_has_forbidden_denotational_content(
                program,
                &crate::proof_only_classification(program),
                presentation.type_reference,
            )
        {
            let plans = crate::operand_call_plans(program, self.frame_resolver());
            let normal_identity =
                crate::machine_calls::denotational_calls::normal_return_call_candidate(
                    program, call, &plans.0, &plans.1,
                );
            let repeatable = normal_identity.is_ok()
                || crate::machine_calls::fact_call_projections::has_checked_denotational_call_identity(
                    program, call, &plans.0, &plans.1,
                );
            if super::super::super::struct_trace() && !repeatable {
                eprintln!(
                    "STRUCT normal-identity caller={} target={} failure={:?} static_arguments={} evidence_arguments={}",
                    scope.0.name,
                    call.target,
                    normal_identity.err(),
                    call.machine_arguments.len(),
                    call.evidence_arguments.len()
                );
            }
            if repeatable {
                equations.push(CapturedResultEquation::completed_call(
                    captured.clone(),
                    StructuralTerm::Application {
                        result_type: crate::expression_result_type_reference(
                            program, scope.0, scope.1, expression,
                        )?,
                        target: super::super::super::structural_terms::structural_call_target(
                            program,
                            scope.0.symbol,
                            call,
                        ),
                        selections: call.machine_arguments.to_vec(),
                        machine: super::super::structural_call_machine_name(
                            call.target.as_str(),
                            &call.machine_arguments,
                            &[],
                        ),
                        arguments: actuals.to_vec(),
                    },
                ));
                if let Some(source) = application.source_value_application(
                    program,
                    scope.0.symbol,
                    &owner,
                    call,
                    expression,
                    presentation.type_reference,
                    actuals,
                ) {
                    equations.push(CapturedResultEquation::completed_call(
                        captured.clone(),
                        source,
                    ));
                }
            }
        }
        Arc::make_mut(&mut self.normal_content_guarantees).extend(guarantees.content);
        // A saved local now resolves to this occurrence, so the callee's case
        // tags must classify it here; the citation route's saved-binding tags
        // never meet that environment term. Tags never become value equations.
        let case_guarantees =
            self.normal_call_case_guarantees(&owner, parameters, bindings, captured.as_ref());
        let selected_signature = match owner {
            SelectedCallContract::Signature(signature)
            | SelectedCallContract::DynamicSlot { signature, .. } => Some(signature),
            _ => None,
        };
        let qualification_contract =
            CallQualificationContract::prepare(program, call.target_symbol, selected_signature);
        for contract in contracts
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let ProofFact::Membership(guarantee) = fact {
                    if qualification_contract
                        .fact_source(contract.kind.clone(), fact)
                        .is_none()
                    {
                        // A checked adapter's inherited issuer obligation is
                        // not an ordinary-call guarantee. Use the same actual
                        // declaration gate as flow's call-contract evidence.
                        continue;
                    }
                    // A result qualification stated in `ensures` carries the
                    // same admission as one stated on the result type. Retain
                    // its exact detached occurrence, never a live caller name
                    // or the result's borrowed referent. Later writes change
                    // the observed subject and cannot reuse this snapshot.
                    if let Some(subject) = self.bound_contract_term(
                        guarantee.value,
                        callable,
                        parameters,
                        callable,
                        bindings,
                        &application.types,
                        captured.as_ref(),
                        0,
                    ) {
                        let mut root = &subject;
                        while let StructuralTerm::Projection { subject, .. } = root {
                            root = subject;
                        }
                        if Some(root) == captured.as_ref() && guarantee.semantic_domain.is_valid() {
                            Arc::make_mut(&mut self.normal_memberships).push(MembershipGuarantee {
                                subject,
                                domain: guarantee.domain_symbol,
                                semantic_domain: guarantee.semantic_domain,
                            });
                        }
                    }
                }
            }
        }
        if super::super::super::struct_trace() {
            eprintln!(
                "STRUCT normal-call expression={expression:?} public_owner={:?} result={captured:?} equations={equations:?}",
                owner_symbol(&owner),
            );
        }
        if body_result.is_none()
            && let (Some(captured), Some(_)) = (&captured, &result_presentation)
            && let SelectedCallContract::Concrete(callee, selected_state) = &owner
        {
            equations.extend(
                self.returned_field_equations(
                    callee,
                    selected_state,
                    parameters,
                    actuals,
                    captured,
                )
                .into_iter()
                .map(CapturedResultEquation::public),
            );
        }
        self.intake_normal_call_equations(equations);
        let completed_result = captured.clone();
        if let (Some(captured), Some(result_presentation)) = (captured, result_presentation) {
            let result_type = result_presentation.type_reference;
            self.intake_admitted_value_predicates(result_type, captured.clone());
            // A dependent result recipe keeps the same formal presentation used
            // by admission. Its operands are these completed invocation bindings;
            // neither slot alpha-renaming nor later writes can replay their source.
            self.intake_admitted_value_indices(
                result_type,
                &captured,
                result_presentation.parameter_owner,
                result_presentation.parameters,
                &result_presentation.bindings,
            );
            // A construction describes payload, never the identity or admission of
            // this call's result. Equal field geometry cannot merge linear roots.
            Arc::make_mut(&mut self.normal_results).push((expression, captured));
        }
        for guarantee in &case_guarantees {
            self.intake_case_guarantee(guarantee);
        }
        Some(CapturedNormalCall {
            result: completed_result,
            post_storage,
        })
    }

    /// Own call preparation once, regardless of whether its body can supply
    /// the result. Public-capture availability is independent of body admission.
    pub(super) fn complete_call_paths(
        &mut self,
        expression: ExpressionHandle,
        actuals: Option<&[StructuralTerm]>,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        writes: &NormalizedWriteFrame,
    ) -> Option<CompletedCallObservation<'program>> {
        let prepared = self.prepare_normal_call(expression, actuals, environment, scope, writes);
        let entry = self.clone();
        let guarantees = prepared
            .as_ref()
            .and_then(|prepared| entry.prepare_normal_call_guarantees(prepared));
        if let Some(paths) = self.observed_body_paths(
            expression,
            actuals,
            environment,
            scope,
            writes,
            prepared.as_ref().zip(guarantees.as_ref()),
        ) {
            return Some(CompletedCallObservation::BodyPaths(paths));
        }
        let completed =
            prepared
                .as_ref()
                .zip(guarantees.as_ref())
                .and_then(|(prepared, guarantees)| {
                    self.capture_prepared_normal_call(prepared, guarantees, None, scope)
                });
        self.observe_storage_effects(scope.1, writes.complete_paths())?;
        for storage in completed
            .into_iter()
            .flat_map(|completed| completed.post_storage)
        {
            self.install_post_call_storage(scope.1, storage);
        }
        Some(CompletedCallObservation::Contract)
    }

    /// Contract-only completion has no observed body result. Retain the same
    /// inference operation without rebuilding its captured call inputs.
    pub(super) fn capture_normal_call(
        &mut self,
        expression: ExpressionHandle,
        actuals: Option<&[StructuralTerm]>,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        writes: &NormalizedWriteFrame,
    ) -> Option<Vec<PostCallStorage>> {
        let prepared = self.prepare_normal_call(expression, actuals, environment, scope, writes)?;
        let entry = self.clone();
        let guarantees = entry.prepare_normal_call_guarantees(&prepared)?;
        self.capture_prepared_normal_call(&prepared, &guarantees, None, scope)
            .map(|completed| completed.post_storage)
    }
}
