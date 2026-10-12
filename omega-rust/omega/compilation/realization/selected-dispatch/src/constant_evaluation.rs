//! Exact selected execution for deferred source constant evaluation.
//! The ordinary selected ProviderPlan set is the authority. Each fold lands
//! its value in the typed program once; final checking consumes that program
//! and the compile does not replay the folds against its own checked result
//! (pipeline.md, "Verification placement").
use build_time_evaluation::{
    PreCheckEvaluation, SelectedBuildTimeBinaryOperator, SelectedBuildTimeOperators,
    SelectedBuildTimeProviderBody,
};
use checked_trees::CheckedProviderPlanCommitment;
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedConstantEvaluation {
    operators: Vec<SelectedBuildTimeBinaryOperator>,
    /// Selected uses whose meaning is the selected provider's ordinary checked
    /// machine body. Each row carries the exact provider entry the semantic
    /// evaluation owner rebinds to on its private evaluation copy.
    provider_bodies: Vec<SelectedBuildTimeProviderBody>,
    /// The exact selected target the deferred folds are answered under — the
    /// explicit target custody a retained `size_of`/`align_of` query names its
    /// subject from. Never the host's facts: a `--target` build folds every
    /// query against this target's layout closure.
    native_target: target::NativeTarget,
    /// The opaque representation selections the layout authority consults for
    /// the same fold; Build admission has none and supplies an empty set.
    opaque_representation_selections: Vec<representation_planning::OpaqueRepresentationSelection>,
}

impl SelectedConstantEvaluation {
    /// Resume pending Psi evaluation under the exact selected providers.
    /// Consume continuations in their original order against canonical storage.
    /// Preparation may select an executable declaration projection of that same
    /// storage; only its occurrences acquire selected execution. Each fold
    /// lands its value in `typed`.
    ///
    /// Retained `size_of`/`align_of` queries fold first, under the selected
    /// target's custody: every typed call carrying a `layout_query` becomes
    /// the `u64` literal the layout authority reports for this target, in
    /// constant and runtime positions alike, before the deferred
    /// continuations' own evaluation observes the program.
    pub fn evaluate_pending(
        self,
        typed: &mut TypedTrees,
        pending: Vec<PreCheckEvaluation>,
        inputs: &mut Vec<build_time_evaluation::CheckedConstantInput>,
    ) -> Result<(), Vec<Diagnostic>> {
        provider_planning::data_layout_queries::fold_data_layout_queries(
            typed,
            self.native_target,
            &self.opaque_representation_selections,
            true,
        )?;
        for continuation in pending {
            continuation.evaluate_selected_operators(
                typed,
                SelectedBuildTimeOperators {
                    operators: &self.operators,
                    provider_bodies: &self.provider_bodies,
                    admission: Default::default(),
                },
                inputs,
            )?;
        }
        // The deferred evaluation installed the validated plan-laid layouts;
        // queries over `Policy<Schema>` operands fold now, and any operand the
        // program never instantiated reports the layout authority's own
        // missing-layout diagnostic.
        provider_planning::data_layout_queries::fold_data_layout_queries(
            typed,
            self.native_target,
            &self.opaque_representation_selections,
            false,
        )?;
        Ok(())
    }

    /// The provider rows this target's pending constant folds are evaluated
    /// under — the part of the selection every realized target must agree on,
    /// since each fold lands its value in the one checked program.
    /// `native_target` and the opaque representation selections are legitimate
    /// per-target layout custody and are not part of the agreement.
    pub fn pending_fold_providers(
        &self,
    ) -> (
        &[SelectedBuildTimeBinaryOperator],
        &[SelectedBuildTimeProviderBody],
    ) {
        (&self.operators, &self.provider_bodies)
    }

    /// Join provider authority once for every retained source use. Psi prepares
    /// semantic operands and independently admits both intrinsic and checked-body
    /// execution. Its coordinator supplies the operator selections; realization
    /// does not enter a source stage to reconstruct them. `native_target` and
    /// `opaque_representation_selections` are the exact selection the deferred
    /// data-layout queries are answered under.
    pub fn prepare(
        context: provider_planning::ProviderPlanningContext<'_>,
        native_target: target::NativeTarget,
        opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
        facts: &checked_trees::CheckedOperatorFacts,
        plans: &SelectedProviderPlanFacts,
        provenance: &[provider_planning::SelectedProviderReviewProvenance],
        selected_target_profile: Option<target::TargetProfile>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if plans.plans().len() != provenance.len()
            || plans
                .plans()
                .iter()
                .zip(provenance)
                .any(|(plan, retained)| *plan != retained.plan)
        {
            return Err(vec![Diagnostic::error(
                "selected constant evaluation lost original provider-plan provenance",
            )]);
        }
        let typed = context.typed;
        let uses = facts.resolved_expression_selections();
        let mut selected = Self {
            operators: Vec::new(),
            provider_bodies: Vec::new(),
            native_target,
            opaque_representation_selections: opaque_representation_selections.to_vec(),
        };
        for (expression, origin, requirement, policy) in uses {
            if typed
                .public_trait_requirement_signature(requirement)
                .is_some_and(|public| public.owner.is_boundary)
            {
                if let Some(body) = selected_trait_requirement_body(
                    context,
                    provenance,
                    expression,
                    origin,
                    requirement,
                )
                .map_err(|error| vec![error])?
                {
                    selected.provider_bodies.push(body);
                }
                continue;
            }
            let Some((plan_index, plan)) =
                provider_planning::selected_requirement_plan(context, plans.plans(), requirement)
            else {
                continue;
            };
            if plan.rows.iter().any(|row| {
                matches!(
                    row.binding,
                    effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                )
            }) {
                let execution = match &provenance[plan_index].selected_by {
                    provider_planning::ProviderSelectionProvenance::CompilerCatalog(choice) => {
                        let retained = &provenance[plan_index].provider;
                        let ([original], [realization]) = (
                            retained.row_requirements.as_slice(),
                            retained.row_realizations.as_slice(),
                        ) else {
                            return Err(vec![Diagnostic::error(
                                "automatic constant execution lost its original row",
                            )]);
                        };
                        if *original != requirement {
                            return Err(vec![Diagnostic::error(
                                "automatic constant execution has a different original requirement",
                            )]);
                        }
                        let identity = choice
                            .row_execution(
                                plan,
                                *original,
                                *realization,
                                selected_target_profile.map(target::TargetProfile::target_name),
                            )
                            .map_err(|error| vec![error])?;
                        match identity {
                            identity @ effects::CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary { .. } => Some(identity),
                            _ => None,
                        }
                    }
                    _ => crate::derive_selected_primitive_float_binary_execution(
                        typed,
                        plan,
                        requirement,
                    )
                    .map_err(|error| vec![error])?,
                };
                if execution.is_some() {
                    selected.operators.push(
                        SelectedBuildTimeBinaryOperator::prepare(
                            typed,
                            expression,
                            origin,
                            requirement,
                            policy,
                            CheckedProviderPlanCommitment::from_digest(
                                *plan.identity_digest().as_bytes(),
                            ),
                        )
                        .map_err(|reason| vec![Diagnostic::error(reason)])?,
                    );
                }
            } else if let Some(provider) =
                provider_planning::selected_requirement_checked_adapter(context, plan, requirement)
                    .map_err(|error| vec![error])?
            {
                selected.provider_bodies.push(
                    SelectedBuildTimeProviderBody::prepare(
                        typed,
                        expression,
                        origin,
                        requirement,
                        provider,
                        plan.provider_type.clone(),
                        CheckedProviderPlanCommitment::from_digest(
                            *plan.identity_digest().as_bytes(),
                        ),
                    )
                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
                );
            }
        }
        build_time_evaluation::validate_selected_operators(typed, &selected.operators)
            .and_then(|()| {
                build_time_evaluation::validate_selected_provider_bodies(
                    typed,
                    &selected.provider_bodies,
                )
            })
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        Ok(selected)
    }
}

/// The original call application selects an existing public row of one already
/// chosen whole plan. This neither searches visible satisfiers nor uses a
/// runtime ABI signature to recover another requirement or receiver scope.
fn selected_trait_requirement_body(
    context: provider_planning::ProviderPlanningContext<'_>,
    selections: &[provider_planning::SelectedProviderReviewProvenance],
    expression: typed_trees::expression::ExpressionHandle,
    origin: checked_trees::CheckedValueOrigin,
    requirement: symbols::SymbolHandle,
) -> Result<Option<SelectedBuildTimeProviderBody>, Diagnostic> {
    let program = context.typed;
    let call = program
        .expression_table
        .call_node(expression)
        .ok_or_else(|| {
            Diagnostic::error("selected constant trait requirement lost its original call")
        })?;
    let public = program
        .selected_public_trait_requirement_signature(requirement, call.authored_machine_application)
        .ok_or_else(|| {
            Diagnostic::error(
                "selected constant trait requirement lost its complete original application",
            )
        })?;
    if !public.owner.is_boundary || !context.contains(public.declaration.symbol) {
        return Err(Diagnostic::error(
            "selected constant trait requirement is outside its boundary declaration context",
        ));
    }
    let interface = if public.selected_interface.is_valid() {
        public.selected_interface
    } else {
        public.owner.symbol
    };
    let mut candidates = Vec::new();
    for selection in selections {
        for (ordinal, row) in selection.plan.rows.iter().enumerate() {
            if selection.provider.row_requirements.get(ordinal) != Some(&public.declaration.symbol)
            {
                continue;
            }
            let scope = selection
                .provider
                .row_receiver_scopes
                .get(ordinal)
                .and_then(|scope| scope.as_ref())
                .and_then(|scope| scope.bind(program))
                .and_then(|scope| scope.declaring_scope(program, interface))
                .ok_or_else(|| {
                    Diagnostic::error("selected constant row lost its original receiving scope")
                })?;
            if !program.requirement_call_application_matches_scope(
                requirement,
                call.authored_machine_application,
                &scope,
            ) {
                continue;
            }
            candidates.push((selection, ordinal, row));
        }
    }
    let (selection, ordinal, row) = match candidates.as_slice() {
        [] => return Ok(None), // Discovery alone never chooses an implementation.
        [candidate] => *candidate,
        _ => {
            return Err(Diagnostic::error(
                "selected constant public application has multiple selected whole provider rows",
            ));
        }
    };
    if !matches!(
        row.binding,
        effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
    ) {
        return Ok(None); // External mechanisms require their own execution admission.
    }
    let selecting_map = selection
        .provider
        .provider_type
        .and_then(|symbol| program.conformance_by_symbol(symbol))
        .ok_or_else(|| {
            Diagnostic::error("selected constant public row lost its named implementation")
        })?;
    let adapter = provider_planning::exact_checked_adapter(context, &selection.plan, row)?;
    if selection.provider.row_realizations.get(ordinal) != Some(&adapter.symbol)
        || !provider_planning::provider_trait_requirement_matches(
            context,
            &selection.plan,
            row,
            adapter,
            public.owner.symbol,
            public.declaration.symbol,
        )
    {
        return Err(Diagnostic::error(
            "selected constant public row changed its original realizing declaration",
        ));
    }
    let provider = if public.signature.symbol == public.declaration.symbol {
        adapter
    } else {
        let mut instances =
            program
                .instantiated_requirement_realizations
                .iter()
                .filter(|(_, instance)| {
                    instance.named_conformance.is_valid()
                        && program.conformances.get(instance.named_conformance).symbol
                            == selecting_map.symbol
                        && instance.requirement_template == public.declaration.symbol
                        && instance.requirement_instance == public.signature.symbol
                        && instance.provider_template == adapter.symbol
                });
        let (_, instance) = instances.next().ok_or_else(|| {
            Diagnostic::error("selected constant application has no produced closed realization")
        })?;
        if instances.next().is_some() {
            return Err(Diagnostic::error(
                "selected constant application has multiple produced closed realizations",
            ));
        }
        program
            .machine_by_symbol(instance.provider_instance)
            .ok_or_else(|| {
                Diagnostic::error("selected constant application lost its produced executable body")
            })?
    };
    SelectedBuildTimeProviderBody::prepare(
        program,
        expression,
        origin,
        requirement,
        provider,
        selection.plan.provider_type.clone(),
        CheckedProviderPlanCommitment::from_digest(*selection.plan.identity_digest().as_bytes()),
    )
    .map(|body| Some(body.with_named_implementation(selecting_map.symbol)))
    .map_err(|reason| Diagnostic::error(reason))
}
