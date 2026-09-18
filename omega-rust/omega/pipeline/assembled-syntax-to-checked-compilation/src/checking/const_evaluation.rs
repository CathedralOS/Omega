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
use typed_trees::expression::ExpressionNode;

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

/// Derive one exact provider-body row per retained spelled-operator use whose
/// settled ProviderPlan binds an ordinary checked adapter. Omega keeps
/// provider-selection authority: each row names the exact provider machine and
/// entry state resolved from the plan, while Psi independently rejoins the
/// authored use, requirement, conformance, and executable entry before the
/// semantic evaluation owner rebinds it to an ordinary call.
pub(super) fn selected_provider_bodies(
    typed: &TypedTrees,
    plans: &SelectedProviderPlanFacts,
) -> Result<Vec<SelectedBuildTimeProviderBody>, Vec<Diagnostic>> {
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
        if !operator.is_boundary {
            continue;
        }
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
        let Some(row) =
            selected_provider_body_row(typed, plan, operator, SelectedOperatorUse::Spelled(fact))
                .map_err(|diagnostic| vec![diagnostic])?
        else {
            continue;
        };
        selected.push(row);
    }
    // A uniquely resolved named call to a boundary operator carries the same
    // execution obligation as its spelled twin: the name already selected the
    // requirement, so the settled plan's checked adapter owns the body.
    for fact in facts.named_uses() {
        let Some(operator) = typed
            .operators()
            .iter()
            .find(|operator| operator.symbol == fact.selected_operator_symbol)
        else {
            continue;
        };
        if !operator.is_boundary {
            continue;
        }
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
        let Some(row) =
            selected_provider_body_row(typed, plan, operator, SelectedOperatorUse::Named(fact))
                .map_err(|diagnostic| vec![diagnostic])?
        else {
            continue;
        };
        selected.push(row);
    }
    build_time_evaluation::validate_selected_provider_bodies(typed, &selected)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    Ok(selected)
}

/// One retained boundary-operator use whose settled plan may execute at build
/// time. A spelled use selects through its token spelling; a named use
/// already carries the exact requirement its call path resolved, so it has no
/// spelling to rejoin.
enum SelectedOperatorUse<'a> {
    Spelled(&'a checked_trees::CheckedOperatorUseFact),
    Named(&'a checked_trees::CheckedNamedOperatorUseFact),
}

/// Resolve one settled ProviderPlan row to its exact checked adapter body.
/// Mirrors the checked-side settlement's adapter resolution: the plan must
/// bind the requirement's canonical overload identity through its single
/// `realize` method, the binding must be a checked adapter owned by the
/// nominal provider type, and the machine must be a concrete checked body with
/// an executable entry state. Specializing applications stay rejected here --
/// their closed substitution belongs to the artifact-qualified path.
fn selected_provider_body_row(
    typed: &TypedTrees,
    plan: &effects::provider_plan::ProviderPlan,
    operator: &typed_trees::operator::OperatorDefinition,
    selected_use: SelectedOperatorUse<'_>,
) -> Result<Option<SelectedBuildTimeProviderBody>, Diagnostic> {
    let (expression, origin) = match selected_use {
        SelectedOperatorUse::Spelled(fact) => (fact.expression, fact.origin),
        SelectedOperatorUse::Named(fact) => (fact.expression, fact.origin),
    };
    let overload_identity =
        typed_trees::operator::boundary_operator_requirement_identity(typed, operator);
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
    if plan.schema.trait_name != overload_identity
        || method.name != "realize"
        || method.requirement_owner != overload_identity
        || method.requirement_identity != overload_identity
        || !plan.schema.row_binds_method(row, method)
    {
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
    if provider.attached_data.as_ref().map(|owner| owner.as_str())
        != Some(plan.provider_type.as_str())
    {
        return Err(Diagnostic::error(format!(
            "selected build-time provider `{}` does not belong to nominal provider `{}`",
            provider.name, plan.provider_type,
        )));
    }
    if provider.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
        return Err(Diagnostic::error(format!(
            "selected build-time provider `{}` is not a checked body",
            provider.name,
        )));
    }
    if !typed.machine_type_parameters(provider).is_empty()
        || !typed.operator_type_parameters(operator).is_empty()
    {
        return Err(Diagnostic::error(format!(
            "selected build-time operator `{}` at expression {:?} requires a specialized realization; symbolic application coverage is not part of this execution",
            operator_name(typed, operator),
            expression,
        )));
    }
    let operands = match selected_use {
        SelectedOperatorUse::Spelled(fact) => {
            if operator.spelling != Some(fact.spelling) {
                return Err(Diagnostic::error(format!(
                    "selected build-time operator at expression {:?} no longer owns its exact spelling",
                    expression,
                )));
            }
            let Some(operands) = fact.operands(typed) else {
                return Err(Diagnostic::error(format!(
                    "selected build-time operator expression {:?} lost its exact operand shape",
                    expression,
                )));
            };
            if let ExpressionNode::Indexed(indexed) = typed.expression_table.expression(expression)
                && matches!(
                    typed.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                )
            {
                return Err(Diagnostic::error(format!(
                    "selected range operator at expression {:?} has no fixed-token checked-adapter dispatch",
                    expression,
                )));
            }
            operands
        }
        // A named call's argument list is its authored operand tuple; the
        // requirement's exact path already selected it.
        SelectedOperatorUse::Named(_) => {
            let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
                return Err(Diagnostic::error(format!(
                    "selected build-time named operator expression {:?} lost its exact operand shape",
                    expression,
                )));
            };
            typed
                .expression_table
                .expression_handles(call.arguments)
                .to_vec()
        }
    };
    if typed.operator_parameters(operator).len() != operands.len() {
        return Err(Diagnostic::error(format!(
            "selected build-time operator at expression {:?} has {} requirement parameter(s), but its expression retains {} operand(s)",
            expression,
            typed.operator_parameters(operator).len(),
            operands.len(),
        )));
    }
    let Some(entry) = typed.machine_states(provider).first() else {
        return Err(Diagnostic::error(format!(
            "selected build-time provider `{}` has no executable entry state",
            provider.name,
        )));
    };
    Ok(Some(SelectedBuildTimeProviderBody {
        expression,
        origin,
        requirement: operator.symbol,
        operands,
        provider_machine: provider.symbol,
        provider_state: entry.symbol,
        provider_type: plan.provider_type.clone(),
        provider: CheckedProviderPlanCommitment::from_digest(*plan.identity_digest().as_bytes()),
    }))
}

fn operator_name(
    typed: &TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
) -> String {
    typed
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

/// A deferred range-endpoint mark that survives selected settlement means its
/// owning pre-check continuation was lost before it could fold -- a custody
/// break, never coverage. The evaluator clears each mark as it substitutes
/// the landed integer, so any remainder names an endpoint no continuation
/// evaluated.
pub(super) fn require_evaluated_range_endpoints(
    program: &typed_trees::TypedTrees,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let pending = program
        .pending_const_range_endpoints
        .iter()
        .map(|expression| {
            Diagnostic::error(
                "range endpoint has an unresolved build-time evaluation dependency".to_owned(),
            )
            .with_source_span(program.expression_table.source_span(*expression))
        })
        .collect::<Vec<_>>();
    if pending.is_empty() {
        Ok(())
    } else {
        Err(pending)
    }
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
                    fact.expression == selected.expression
                        && fact.origin == selected.origin
                        && fact.occurrence == checked_trees::CheckedOperatorOccurrence::Expression
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
        let current_bodies = selected_provider_bodies(&checked.typed, plans)?;
        for body in &self.provider_bodies {
            if current_bodies.iter().filter(|row| *row == body).count() != 1 {
                return Err(vec![Diagnostic::error(
                    "folded provider body differs from current selected provider execution",
                )]);
            }
            // The fold's checked occurrence is whichever of the two use
            // arenas retains this expression and origin: a spelled use or a
            // uniquely resolved named call. Exactly one must exist.
            let spelled: Vec<_> = checked
                .facts
                .operators
                .uses_with_status(CheckedOperatorResolutionStatus::Resolved)
                .filter(|fact| {
                    fact.expression == body.expression
                        && fact.origin == body.origin
                        && fact.occurrence == checked_trees::CheckedOperatorOccurrence::Expression
                })
                .collect();
            let named: Vec<_> = checked
                .facts
                .operators
                .named_uses()
                .filter(|fact| fact.expression == body.expression && fact.origin == body.origin)
                .collect();
            let (requirement, commitment) = match (spelled.as_slice(), named.as_slice()) {
                ([fact], []) => (fact.selected_operator_symbol, fact.provider_plan_commitment),
                ([], [fact]) => (fact.selected_operator_symbol, fact.provider_plan_commitment),
                _ => {
                    return Err(vec![Diagnostic::error(
                        "folded provider body has no unique final checked occurrence",
                    )]);
                }
            };
            if requirement != body.requirement || commitment != body.provider {
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
            &checked.typed,
            &self.folds,
            &self.operators,
            &self.provider_bodies,
            authority,
        )
    }
}
