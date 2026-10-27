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

/// Exact work and retained positions owned by one receiving capsule.
pub struct ConstantEvaluationInputs<'source> {
    pub declarations: &'source typed_trees::references::DeclarationSelection,
    pub queries: &'source std::collections::HashSet<typed_trees::expression::ExpressionHandle>,
    pub constants: &'source [typed_trees::constant_recipes::RetainedConstantPosition],
}

/// Physical query inputs shared by one invocation's private source jobs.
struct SelectedLayoutQueries {
    target: target::TargetProfile,
    opaque: Vec<representation_planning::OpaqueRepresentationSelection>,
}

impl build_time_evaluation::BuildTimeLayoutQueries for SelectedLayoutQueries {
    fn prepare(
        &self,
        program: &mut TypedTrees,
        selection: &typed_trees::references::DeclarationSelection,
    ) -> Result<(), Vec<Diagnostic>> {
        let requests =
            typed_trees::references::collect_selected_program_references(program, selection)
                .expressions;
        provider_planning::data_layout_queries::fold_data_layout_queries(
            program,
            self.target,
            &self.opaque,
            true,
            selection,
            &requests,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedConstantEvaluation {
    operators: Vec<SelectedBuildTimeBinaryOperator>,
    /// Selected uses whose meaning is the selected provider's ordinary checked
    /// machine body. Each row carries the exact provider entry the semantic
    /// evaluation owner rebinds to on its private evaluation copy.
    provider_bodies: Vec<SelectedBuildTimeProviderBody>,
    /// The exact selected target the deferred folds are answered under — the
    /// explicit target custody a retained `size_of`/`align_of` query names its
    /// subject from. Never the host's facts: demanded constant/proof queries
    /// fold against this target's layout closure.
    target_profile: target::TargetProfile,
    /// Only these declaration roots own folds; canonical arenas also retain
    /// Product descriptions during Build admission.
    selection: typed_trees::references::DeclarationSelection,
    query_requests: std::collections::HashSet<typed_trees::expression::ExpressionHandle>,
    constant_requests: Vec<typed_trees::constant_recipes::RetainedConstantPosition>,
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
    /// target's custody, only for the exact constant/proof dependency set.
    /// Runtime queries remain portable operations; the canonical catalog and
    /// its other target bodies remain available for lookup.
    pub fn evaluate_pending(
        self,
        typed: &mut TypedTrees,
        pending: Vec<PreCheckEvaluation>,
    ) -> Result<(), Vec<Diagnostic>> {
        provider_planning::data_layout_queries::fold_data_layout_queries(
            typed,
            self.target_profile,
            &self.opaque_representation_selections,
            true,
            &self.selection,
            &self.query_requests,
        )?;
        let layouts = build_time_evaluation::PreparedLayoutQueries::new(SelectedLayoutQueries {
            target: self.target_profile,
            opaque: self.opaque_representation_selections.clone(),
        });
        let mut evaluated = Vec::with_capacity(pending.len());
        for continuation in pending {
            evaluated.push(
                continuation
                    .select_work(typed, &self.selection)?
                    .with_constant_requests(&self.constant_requests)
                    .with_layout_queries(layouts.clone())
                    .evaluate_selected_operators(
                        typed,
                        SelectedBuildTimeOperators {
                            operators: &self.operators,
                            provider_bodies: &self.provider_bodies,
                        },
                    )?,
            );
        }
        // The deferred evaluation installed the validated plan-laid layouts;
        // queries over `Policy<Schema>` operands fold now, and any operand the
        // program never instantiated reports the layout authority's own
        // missing-layout diagnostic.
        provider_planning::data_layout_queries::fold_data_layout_queries(
            typed,
            self.target_profile,
            &self.opaque_representation_selections,
            false,
            &self.selection,
            &self.query_requests,
        )?;
        for continuation in evaluated {
            continuation.finish(typed)?;
        }
        Ok(())
    }

    /// The provider rows this target's pending constant folds are evaluated
    /// under — the part of the selection every realized target must agree on,
    /// since each fold lands its value in the one checked program.
    /// `target_profile` and the opaque representation selections are legitimate
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
    /// does not enter a source stage to reconstruct them. `target_profile` and
    /// `opaque_representation_selections` are the exact selection the deferred
    /// data-layout queries are answered under.
    pub fn prepare(
        context: provider_planning::ProviderPlanningContext<'_>,
        target_profile: target::TargetProfile,
        opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
        facts: &checked_trees::CheckedOperatorFacts,
        plans: &SelectedProviderPlanFacts,
        inputs: ConstantEvaluationInputs<'_>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let typed = context.typed;
        let selection = inputs.declarations;
        let uses = facts.resolved_expression_selections();
        let mut selected = Self {
            operators: Vec::new(),
            provider_bodies: Vec::new(),
            target_profile,
            selection: selection.clone(),
            query_requests: inputs.queries.clone(),
            constant_requests: inputs.constants.to_vec(),
            opaque_representation_selections: opaque_representation_selections.to_vec(),
        };
        let mut expressions =
            typed_trees::references::collect_selected_program_references(typed, selection)
                .expressions;
        for constant in inputs.constants {
            use typed_trees::constant_recipes::RetainedConstantPosition;
            match constant {
                RetainedConstantPosition::RangeEndpoint(root)
                | RetainedConstantPosition::ConstApplication { root, .. } => {
                    expressions.extend(
                        typed_trees::references::collect_expression_references(typed, *root)
                            .expressions,
                    );
                }
                RetainedConstantPosition::ArrayLength(_) | RetainedConstantPosition::None => {}
            }
        }
        for (expression, origin, requirement, policy) in uses {
            if !expressions.contains(&expression) {
                continue;
            }
            let Some((_, plan)) =
                provider_planning::selected_requirement_plan(context, plans.plans(), requirement)
            else {
                continue;
            };
            // Select this exact method, not the binding kind of a sibling
            // row in the same boundary-trait plan.
            if let Some((provider, declaration)) =
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
                        declaration,
                        plan.provider_type_package_identity,
                        target_profile.target_name(),
                        CheckedProviderPlanCommitment::from_digest(
                            *plan.identity_digest().as_bytes(),
                        ),
                    )
                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
                );
            } else if crate::derive_selected_primitive_float_binary_execution(
                typed,
                plan,
                requirement,
            )
            .map_err(|error| vec![error])?
            .is_some()
            {
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
