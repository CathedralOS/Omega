//! Exact selected execution for deferred source constant evaluation.
//! The ordinary selected ProviderPlan set is the authority. The folded result
//! retains its receiving type and source invocation, then final checking
//! independently rejoins operator facts and reexecutes that invocation.
use build_time_evaluation::{
    FoldedArrayLength, PreCheckEvaluation, SelectedBuildTimeBinaryOperator,
    SelectedBuildTimeOperators, SelectedBuildTimeProviderBody,
};
use checked_trees::{CheckedProviderPlanCommitment, CheckedTrees};
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectedConstantEvaluation {
    operators: Vec<SelectedBuildTimeBinaryOperator>,
    /// Selected uses whose meaning is the selected provider's ordinary checked
    /// machine body. Each row carries the exact provider entry the semantic
    /// evaluation owner rebinds to on its private evaluation copy.
    provider_bodies: Vec<SelectedBuildTimeProviderBody>,
    folds: Vec<FoldedArrayLength>,
}

impl SelectedConstantEvaluation {
    /// Resume pending Psi evaluation under the exact selected providers.
    /// Empty input leaves the typed program untouched. Otherwise, settle literal
    /// destinations, join provider execution once, and consume continuations in
    /// their original order. Retained folds stay with the selections used to
    /// produce them until final checked-program replay.
    pub fn evaluate_pending(
        typed: &mut TypedTrees,
        plans: &SelectedProviderPlanFacts,
        pending: Vec<PreCheckEvaluation>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if pending.is_empty() {
            return Ok(Self::default());
        }
        validation::land_float_literal_destinations(typed);
        let mut selected = Self::prepare(typed, plans)?;
        for continuation in pending {
            selected
                .folds
                .extend(continuation.evaluate_selected_operators(
                    typed,
                    SelectedBuildTimeOperators {
                        operators: &selected.operators,
                        provider_bodies: &selected.provider_bodies,
                    },
                )?);
        }
        Ok(selected)
    }

    /// Join provider authority once for every retained source use. Psi prepares
    /// semantic operands and independently admits both intrinsic and checked-body
    /// execution; the coordinator supplies only the selected realization.
    fn prepare(
        typed: &TypedTrees,
        plans: &SelectedProviderPlanFacts,
    ) -> Result<Self, Vec<Diagnostic>> {
        let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(typed);
        let uses = facts.resolved_expression_selections();
        let mut selected = Self::default();
        for (expression, origin, requirement, policy) in uses {
            let Some((_, plan)) =
                provider_planning::selected_requirement_plan(typed, plans.plans(), requirement)
            else {
                continue;
            };
            if plan.rows.iter().any(|row| {
                matches!(
                    row.binding,
                    effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                )
            }) {
                if crate::derive_selected_primitive_float_binary_execution(typed, plan, requirement)
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
            } else if let Some(provider) = provider_planning::selected_requirement_checked_adapter(
                provider_planning::ProviderPlanningContext::new(
                    typed,
                    source::DependencyScope::Product,
                ),
                plan,
                requirement,
            )
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

impl SelectedConstantEvaluation {
    /// Rejoin the final checked provider occurrences and ask Psi to replay the
    /// retained folds. Report coordinates never substitute for exact custody.
    pub fn validate(
        &self,
        checked: &CheckedTrees,
        plans: &SelectedProviderPlanFacts,
        packages: Option<&package_compilation::PackageCompilationInputs>,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.folds.is_empty() {
            return Ok(());
        }
        let current = Self::prepare(&checked.typed, plans)?;
        for selected in &self.operators {
            if current
                .operators
                .iter()
                .filter(|row| *row == selected)
                .count()
                != 1
            {
                return Err(vec![Diagnostic::error(
                    "folded operator differs from current selected provider execution",
                )]);
            }
            if joined_plan_commitment(checked, plans, selected.requirement, selected.origin)
                != Some(selected.provider)
            {
                return Err(vec![Diagnostic::error(
                    "folded operator lost its exact final provider custody",
                )]);
            }
        }
        for body in &self.provider_bodies {
            if current
                .provider_bodies
                .iter()
                .filter(|row| *row == body)
                .count()
                != 1
            {
                return Err(vec![Diagnostic::error(
                    "folded provider body differs from current selected provider execution",
                )]);
            }
            if joined_plan_commitment(checked, plans, body.requirement, body.origin)
                != Some(body.provider)
            {
                return Err(vec![Diagnostic::error(
                    "folded provider body lost its exact final provider custody",
                )]);
            }
        }
        let authority = packages.map(|packages| {
            std::sync::Arc::new(packages.clone())
                as std::sync::Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
        });
        build_time_evaluation::validate_folded_array_lengths(
            checked,
            &self.folds,
            &self.operators,
            &self.provider_bodies,
            authority,
        )
    }
}

/// The commitment of this target's selected plan serving one checked use,
/// joined by requirement identity.
fn joined_plan_commitment(
    checked: &CheckedTrees,
    plans: &SelectedProviderPlanFacts,
    requirement: symbols::SymbolHandle,
    origin: checked_trees::CheckedValueOrigin,
) -> Option<CheckedProviderPlanCommitment> {
    provider_planning::selected_use_plan(checked, plans.plans(), requirement, origin).map(
        |(_, plan)| CheckedProviderPlanCommitment::from_digest(*plan.identity_digest().as_bytes()),
    )
}
