/*
Fold deferred source constant evaluations under the admitted `Build` provider
plans and execution target (`SelectedConstantEvaluation`).

During `admission::checking::prepare_checked_build`, any `PreCheckEvaluation`
continuation that depends on target layout (`size_of`/`align_of`), target machine
bodies, or selected boundary providers is deferred until `Build` provider plans
are settled. `SelectedConstantEvaluation::prepare` binds each resolved operator
use to either its exact selected provider machine body
(`prepare_selected_build_time_provider_body`) or primitive float binary operator
(`derive_selected_primitive_float_binary_execution`), and `evaluate_pending`
folds the continuations in place inside `TypedTrees` before `CheckRequest` runs.
*/

use build_time_evaluation::validate_selected_provider_bodies;
use build_time_evaluation::{
    PreCheckEvaluation, SelectedBuildTimeBinaryOperator, SelectedBuildTimeOperators,
    SelectedBuildTimeProviderBody,
};
use checked_trees::CheckedProviderPlanCommitment;
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use provider_planning::target_machines::selected_constant_target_bodies;
use selected_dispatch::derive_selected_primitive_float_binary_execution;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelectedConstantEvaluation {
    operators: Vec<SelectedBuildTimeBinaryOperator>,
    provider_bodies: Vec<SelectedBuildTimeProviderBody>, // Selected uses bound to the provider's ordinary checked machine body.
    target_bodies: Vec<build_time_evaluation::SelectedBuildTimeTargetBody>,
    target_profile: target::TargetProfile, // Target profile under which deferred `size_of`/`align_of` layout queries fold.
    selection: typed_trees::references::DeclarationSelection, // Projected Build declaration roots that own folds.
    opaque_carriers: Vec<provider_planning::data_layout_queries::OpaqueCarrierSelection>, // Empty during Build admission.
}

impl SelectedConstantEvaluation {
    pub(crate) fn evaluate_pending(
        self,
        typed: &mut TypedTrees,
        pending: Vec<PreCheckEvaluation>,
    ) -> Result<(), Vec<Diagnostic>> {
        build_time_evaluation::SelectedConstantEvaluationRequest {
            program: typed,
            pending,
            operators: SelectedBuildTimeOperators {
                operators: &self.operators,
                provider_bodies: &self.provider_bodies,
            },
            target_bodies: &self.target_bodies,
            selection: &self.selection,
            layouts: &provider_planning::data_layout_queries::TargetLayoutQueries {
                target: self.target_profile,
                applications: None,
                opaque_representations: &self.opaque_carriers,
            },
        }
        .evaluate()
    }

    pub(crate) fn prepare(
        services: typed_trees::service_declarations::ServiceCatalog<'_>,
        target_profile: target::TargetProfile,
        opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
        facts: &checked_trees::CheckedOperatorFacts,
        plans: &SelectedProviderPlanFacts,
    ) -> Result<Self, Vec<Diagnostic>> {
        let context = services.context();
        let typed = context.typed;
        let uses = facts.resolved_expression_selections();
        let mut selected = Self {
            operators: Vec::new(),
            provider_bodies: Vec::new(),
            target_bodies: selected_constant_target_bodies(context, target_profile)?,
            target_profile,
            selection: typed_trees::references::DeclarationSelection::from_program(typed),
            opaque_carriers: opaque_representation_selections
                .iter()
                .map(
                    |selection| provider_planning::data_layout_queries::OpaqueCarrierSelection {
                        opaque: selection.opaque(),
                        carrier_type: selection.carrier_type(),
                    },
                )
                .collect(),
        };
        for (expression, origin, requirement, policy) in uses {
            let Some((_, plan)) =
                provider_planning::selected_requirement_plan(context, plans.plans(), requirement)
            else {
                continue;
            };
            if let Some(body) = provider_planning::prepare_selected_build_time_provider_body(
                services,
                target_profile,
                plan,
                expression,
                origin,
                requirement,
            )
            .map_err(|error| vec![error])?
            {
                selected.provider_bodies.push(body);
            } else if derive_selected_primitive_float_binary_execution(typed, plan, requirement)
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
            .and_then(|()| validate_selected_provider_bodies(typed, &selected.provider_bodies))
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        Ok(selected)
    }
}
