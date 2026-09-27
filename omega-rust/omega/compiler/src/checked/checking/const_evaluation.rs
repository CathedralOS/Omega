//! Exact selected execution for deferred source constant evaluation.
//! The ordinary selected ProviderPlan set is the authority. The folded result
//! retains its receiving type and source invocation, then final checking
//! independently rejoins operator facts and reexecutes that invocation.
use build_time_evaluation::{
    FoldedArrayLength, SelectedBuildTimeBinaryOperator, SelectedBuildTimeProviderBody,
};
use checked_trees::{CheckedOperatorResolutionStatus, CheckedProviderPlanCommitment, CheckedTrees};
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SelectedConstEvaluation {
    pub(super) operators: Vec<SelectedBuildTimeBinaryOperator>,
    /// Selected uses whose meaning is the selected provider's ordinary checked
    /// machine body. Each row carries the exact provider entry the semantic
    /// evaluation owner rebinds to on its private evaluation copy.
    pub(super) provider_bodies: Vec<SelectedBuildTimeProviderBody>,
    pub(super) folds: Vec<FoldedArrayLength>,
}

pub(super) fn selected_operators(
    typed: &TypedTrees,
    plans: &SelectedProviderPlanFacts,
) -> Result<Vec<SelectedBuildTimeBinaryOperator>, Vec<Diagnostic>> {
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(typed);
    let mut selected = Vec::new();
    for fact in facts.uses_with_status(CheckedOperatorResolutionStatus::Resolved) {
        if fact.occurrence != checked_trees::CheckedOperatorOccurrence::Expression {
            continue;
        }
        let Some(operator) = typed
            .operators()
            .iter()
            .find(|operator| operator.symbol == fact.selected_operator_symbol)
        else {
            continue;
        };
        let Some((operation, format)) =
            typed_trees::operator::primitive_float_binary_semantics(typed, operator)
        else {
            continue;
        };
        let identity =
            typed_trees::operator::boundary_operator_requirement_identity(typed, operator);
        let matching: Vec<_> = plans
            .plans()
            .iter()
            .filter(|plan| plan.schema.trait_name == identity)
            .collect();
        let [plan] = matching.as_slice() else {
            continue;
        };
        let execution = selected_dispatch::derive_selected_primitive_float_binary_execution(
            typed,
            plan,
            operator.symbol,
        )
        .map_err(|error| vec![error])?;
        let expected =
            provider_planning::primitive_float_binary_intrinsic_execution_identity(typed, operator);
        if execution.is_none() || execution != expected {
            continue;
        }
        let typed_trees::expression::ExpressionNode::Binary(binary) =
            typed.expression_table.expression(fact.expression)
        else {
            continue;
        };
        selected.push(SelectedBuildTimeBinaryOperator {
            expression: fact.expression,
            origin: fact.origin,
            requirement: fact.selected_operator_symbol,
            operation,
            operands: [binary.left, binary.right],
            format,
            policy: fact.policy_adapter,
            provider: CheckedProviderPlanCommitment::from_digest(
                *plan.identity_digest().as_bytes(),
            ),
        });
    }
    build_time_evaluation::validate_selected_operators(typed, &selected)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    Ok(selected)
}

/// Resolve provider authority for retained source uses. Psi prepares and
/// independently checks each selected body's source signature and operands;
/// Omega supplies only the exact checked adapter and its plan commitment.
pub(super) fn selected_provider_bodies(
    typed: &TypedTrees,
    plans: &SelectedProviderPlanFacts,
) -> Result<Vec<SelectedBuildTimeProviderBody>, Vec<Diagnostic>> {
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(typed);
    let uses = facts
        .uses_with_status(CheckedOperatorResolutionStatus::Resolved)
        .filter(|fact| fact.occurrence == checked_trees::CheckedOperatorOccurrence::Expression)
        .map(|fact| (fact.expression, fact.origin, fact.selected_operator_symbol))
        .chain(
            facts
                .named_uses()
                .map(|fact| (fact.expression, fact.origin, fact.selected_operator_symbol)),
        )
        .chain(
            facts
                .named_requirement_uses()
                .map(|fact| (fact.expression, fact.origin, fact.requirement_symbol)),
        );
    let mut selected = Vec::new();
    for (expression, origin, requirement) in uses {
        let Some((_, plan)) =
            provider_planning::selected_requirement_plan(typed, plans.plans(), requirement)
        else {
            continue;
        };
        if let Some(row) = selected_provider_body_row(typed, plan, requirement, expression, origin)
            .map_err(|diagnostic| vec![diagnostic])?
        {
            selected.push(row);
        }
    }
    build_time_evaluation::validate_selected_provider_bodies(typed, &selected)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    Ok(selected)
}

/// Actual machines own top-level requirement schemas, whether or not they
/// also declare a token spelling. Legacy operators retain their own schema.
fn provider_body_requirement_schema(
    typed: &TypedTrees,
    requirement: symbols::SymbolHandle,
) -> Option<effects::provider_plan::ServiceSchema> {
    if let Some(machine) = typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == requirement)
    {
        provider_planning::service_schema::from_typed_boundary_requirement(typed, machine)
    } else {
        let operator = typed
            .operators()
            .iter()
            .find(|operator| operator.symbol == requirement)?;
        provider_planning::service_schema::from_typed_operator(typed, operator)
    }
}

/// Resolve the selected plan's canonical overload to its exact checked
/// adapter. Interpretation owns the source shape and executable entry, not
/// this join between the requirement schema and provider authority.
fn selected_provider_body_row(
    typed: &TypedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    requirement: symbols::SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
    origin: checked_trees::CheckedValueOrigin,
) -> Result<Option<SelectedBuildTimeProviderBody>, Diagnostic> {
    let Some(schema) = provider_body_requirement_schema(typed, requirement) else {
        return Ok(None);
    };
    let [required_method] = schema.methods.as_slice() else {
        return Err(Diagnostic::error(
            "selected build-time requirement must retain exactly one schema method",
        ));
    };
    let overload_identity = &required_method.requirement_identity;
    if overload_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected build-time operator at expression {:?} has an empty canonical overload identity",
            expression,
        )));
    }
    let [method] = plan.schema.methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected build-time operator ProviderPlan `{}` must retain exactly one schema method",
            plan.name,
        )));
    };
    let [row] = plan.rows.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected build-time operator ProviderPlan `{}` must retain exactly one realization row",
            plan.name,
        )));
    };
    if plan.schema != schema || !plan.schema.row_binds_method(row, method) {
        return Err(Diagnostic::error(format!(
            "selected build-time operator ProviderPlan `{}` does not bind exact overload `{overload_identity}`",
            plan.name,
        )));
    }
    let effects::provider_plan::ProviderBinding::CheckedAdapter { .. } = &row.binding else {
        return Ok(None);
    };
    if plan.provider_type.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected build-time operator ProviderPlan `{}` has no nominal provider type",
            plan.name,
        )));
    }
    let provider = provider_planning::exact_checked_adapter(typed, plan, row)?;
    SelectedBuildTimeProviderBody::prepare(
        typed,
        expression,
        origin,
        requirement,
        provider,
        plan.provider_type.clone(),
        CheckedProviderPlanCommitment::from_digest(*plan.identity_digest().as_bytes()),
    )
    .map(Some)
    .map_err(Diagnostic::error)
}

impl SelectedConstEvaluation {
    pub(super) fn validate(
        &self,
        checked: &CheckedTrees,
        plans: &SelectedProviderPlanFacts,
        packages: Option<&package_compilation::PackageCompilationInputs>,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.folds.is_empty() {
            return Ok(());
        }
        let current = selected_operators(&checked.typed, plans)?;
        for selected in &self.operators {
            if current.iter().filter(|row| *row == selected).count() != 1 {
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
        let current_bodies = selected_provider_bodies(&checked.typed, plans)?;
        for body in &self.provider_bodies {
            if current_bodies.iter().filter(|row| *row == body).count() != 1 {
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
