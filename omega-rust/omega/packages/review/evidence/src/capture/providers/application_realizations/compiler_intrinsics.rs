//! Exact compiler-intrinsic application and selected-plan reconciliation.

use super::{
    StagedRealization, authored_application_source_span, canonical_source_span_location,
    expression_is_owned_by_package, nominal_identity, stage_realization,
};
use crate::record::{
    CheckedPackageBoundaryApplicationRealizationReview, PackageReviewBoundaryApplication,
    PackageReviewBoundaryApplicationRealization, PackageReviewSourceLocationOwner,
    PackageReviewSourceLocationRole,
};
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;

pub(super) fn project(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
    staged: &mut Vec<StagedRealization>,
) -> Result<(), Vec<Diagnostic>> {
    let selected_plans = compilation.selected_provider_plans().plans();
    let provenance = compilation.selected_provider_provenance();
    if selected_plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected-provider plans and intrinsic review provenance are not aligned",
        )]);
    }

    for application in &compilation.facts.operators.boundary_applications {
        if !application.arguments.is_empty() {
            continue;
        }
        let checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
            expression,
            origin,
        } = application.site
        else {
            continue;
        };
        let uses = exact_application_uses(
            compilation,
            expression,
            origin,
            application.requirement_symbol,
        );
        let [actual_use] = uses.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "boundary application at expression {expression:?} retains {} exact selected uses; expected one",
                uses.len(),
            ))]);
        };
        if !expression_is_owned_by_package(
            compilation,
            expression,
            actual_use.kind,
            application.requirement_symbol,
            package,
        )? {
            continue;
        }
        let location = canonical_source_span_location(
            compilation,
            authored_application_source_span(
                compilation,
                expression,
                actual_use.kind,
                application.requirement_symbol,
            )?,
            PackageReviewSourceLocationRole::BoundaryApplicationUse,
        )?;
        if location.owner != PackageReviewSourceLocationOwner::Package(package) {
            continue;
        }
        let matching_plans = selected_plans
            .iter()
            .zip(provenance)
            .filter(|(plan, retained)| {
                retained.plan == **plan
                    && plan.report_fingerprint() == actual_use.plan_report_fingerprint
                    && plan.identity_digest().as_bytes() == actual_use.plan_commitment.as_bytes()
            })
            .collect::<Vec<_>>();
        let [(plan, retained)] = matching_plans.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "boundary application at expression {expression:?} rejoins {} exact selected provider plans; expected one",
                matching_plans.len(),
            ))]);
        };
        if retained.provider.row_requirements.len() != plan.rows.len()
            || retained.provider.row_realizations.len() != plan.rows.len()
            || retained.row_compiler_intrinsic_executions.len() != plan.rows.len()
        {
            return Err(vec![Diagnostic::error(format!(
                "selected provider plan `{}` has incomplete intrinsic application provenance",
                plan.name,
            ))]);
        }
        let matching_rows = plan
            .rows
            .iter()
            .zip(&retained.provider.row_requirements)
            .zip(&retained.provider.row_realizations)
            .zip(&retained.row_compiler_intrinsic_executions)
            .filter(|(((_, requirement), _), _)| **requirement == application.requirement_symbol)
            .collect::<Vec<_>>();
        let [(((row, requirement_symbol), realization_symbol), retained_execution)] =
            matching_rows.as_slice()
        else {
            return Err(vec![Diagnostic::error(format!(
                "selected provider plan `{}` has {} rows for actual boundary requirement {:?}; expected one",
                plan.name,
                matching_rows.len(),
                application.requirement_symbol,
            ))]);
        };
        if !matches!(
            row.binding,
            effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
        ) {
            continue;
        }
        let execution = super::super::intrinsics::project_compiler_intrinsic_execution(
            compilation,
            plan,
            row,
            retained.provider.schema,
            **requirement_symbol,
            **realization_symbol,
            compilation
                .selected_target_profile()
                .map(target::TargetProfile::target_name),
            **retained_execution,
        )?
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "actual boundary application at expression {expression:?} selects compiler intrinsic `{}`, but no closed execution identity exists",
                plan.name,
            ))]
        })?;
        let operator = compilation
            .typed
            .operators()
            .iter()
            .find(|operator| operator.symbol == application.requirement_symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "actual compiler-intrinsic application lost its operator declaration",
                )]
            })?;
        let requirement_identity = typed_trees::operator::boundary_operator_requirement_identity(
            &compilation.typed,
            operator,
        );
        if requirement_identity.is_empty() || actual_use.plan_commitment.is_empty() {
            return Err(vec![Diagnostic::error(
                "actual compiler-intrinsic application lost a strong semantic identity",
            )]);
        }
        stage_realization(
            staged,
            CheckedPackageBoundaryApplicationRealizationReview {
                requirement_identity,
                operator_declaration: nominal_identity(
                    compilation,
                    application.requirement_symbol,
                )?,
                application: PackageReviewBoundaryApplication::Empty,
                selected_plan_digest: *actual_use.plan_commitment.as_bytes(),
                realization: PackageReviewBoundaryApplicationRealization::ExactCompilerIntrinsic {
                    execution,
                },
            },
            location,
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ExactApplicationUse {
    kind: selected_dispatch::CheckedOperatorAuthoredUseKind,
    plan_report_fingerprint: u64,
    plan_commitment: checked_trees::CheckedProviderPlanCommitment,
}

fn exact_application_uses(
    compilation: &CheckedCompilation,
    expression: typed_trees::expression::ExpressionHandle,
    origin: checked_trees::CheckedValueOrigin,
    requirement: symbols::SymbolHandle,
) -> Vec<ExactApplicationUse> {
    let operator = compilation
        .typed
        .operators()
        .iter()
        .find(|operator| operator.symbol == requirement);
    compilation
        .facts
        .operators
        .named_uses
        .iter()
        .filter_map(|(_, operator_use)| {
            (operator_use.expression == expression
                && operator_use.origin == origin
                && operator_use.selected_operator_symbol == requirement)
                .then_some(ExactApplicationUse {
                    kind: selected_dispatch::CheckedOperatorAuthoredUseKind::Named,
                    plan_report_fingerprint: operator_use.provider_plan_report_fingerprint,
                    plan_commitment: operator_use.provider_plan_commitment,
                })
        })
        .chain(
            compilation
                .facts
                .operators
                .uses
                .iter()
                .filter_map(|(_, operator_use)| {
                    (operator_use.expression == expression
                        && operator_use.origin == origin
                        && operator_use.selected_operator_symbol == requirement
                        && operator_use.status
                            == checked_trees::CheckedOperatorResolutionStatus::Resolved
                        && operator.is_some_and(|operator| {
                            operator.is_boundary
                                && operator.spelling == Some(operator_use.spelling)
                                && compilation
                                    .facts
                                    .operators
                                    .selected_candidate(operator_use)
                                    .is_some_and(|candidate| {
                                        candidate.operator_symbol == requirement
                                            && candidate.is_boundary
                                    })
                        }))
                    .then_some(ExactApplicationUse {
                        kind: selected_dispatch::CheckedOperatorAuthoredUseKind::FixedToken(
                            operator_use.spelling,
                        ),
                        plan_report_fingerprint: operator_use.provider_plan_report_fingerprint,
                        plan_commitment: operator_use.provider_plan_commitment,
                    })
                }),
        )
        .collect()
}
