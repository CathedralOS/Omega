//! Exact selected execution for deferred source constant evaluation.
//! The ordinary selected ProviderPlan set is the authority. The folded result
//! retains its receiving type and source invocation, then final checking
//! independently rejoins operator facts and reexecutes that invocation.
use build_time_evaluation::{FoldedArrayLength, SelectedBuildTimeBinaryOperator};
use checked_trees::{CheckedOperatorResolutionStatus, CheckedProviderPlanCommitment, CheckedTrees};
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SelectedConstEvaluation {
    pub(super) operators: Vec<SelectedBuildTimeBinaryOperator>,
    pub(super) folds: Vec<FoldedArrayLength>,
}

pub(super) fn selected_operators(
    typed: &TypedTrees,
    plans: &SelectedProviderPlanFacts,
) -> Result<Vec<SelectedBuildTimeBinaryOperator>, Vec<Diagnostic>> {
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(typed);
    let mut selected = Vec::new();
    for fact in facts.uses_with_status(CheckedOperatorResolutionStatus::Resolved) {
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
            provider_planning::plans::primitive_float_binary_intrinsic_execution_identity(
                typed, operator,
            );
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

pub(super) fn require_evaluated_array_lengths(
    program: &typed_trees::TypedTrees,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let pending = program
        .type_reference_table
        .fixed_array_lengths()
        .filter_map(|(_, length)| {
            let typed_trees::types::FixedArrayLength::ConstCall { name, source_span } = length
            else {
                return None;
            };
            Some(
                diagnostics::Diagnostic::error(format!(
                    "array length `{}` has an unresolved build-time evaluation dependency",
                    name.as_str()
                ))
                .with_source_span(*source_span),
            )
        })
        .collect::<Vec<_>>();
    if pending.is_empty() {
        Ok(())
    } else {
        Err(pending)
    }
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
            let matching: Vec<_> = checked
                .facts
                .operators
                .uses_with_status(CheckedOperatorResolutionStatus::Resolved)
                .filter(|fact| {
                    fact.expression == selected.expression && fact.origin == selected.origin
                })
                .collect();
            let [fact] = matching.as_slice() else {
                return Err(vec![Diagnostic::error(
                    "folded operator has no unique final checked occurrence",
                )]);
            };
            if fact.selected_operator_symbol != selected.requirement
                || fact.policy_adapter != selected.policy
                || fact.provider_plan_commitment != selected.provider
            {
                return Err(vec![Diagnostic::error(
                    "folded operator lost its exact final provider custody",
                )]);
            }
        }
        let authority = packages.map(|packages| {
            std::sync::Arc::new(packages.clone())
                as std::sync::Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
        });
        build_time_evaluation::validate_folded_array_lengths(
            &checked.typed,
            &self.folds,
            &self.operators,
            authority,
        )
    }
}

#[cfg(test)]
mod tests;
