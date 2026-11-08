/*
We project exact compiler-intrinsic boundary applications and reconcile them
against selected provider plans.

Specialized machine instances are projected by the specialized checked-body
path because their plans bind the authored template slot and their token
selections resolve to the instance. Build applications have their own checked
instance and execution profile and are reviewed by callable policy rather than
Product provider plans. The authored selection ledger names what the use
resolved to: a named call selects the requirement's callable entry state,
while a token use's `Operator` occurrence selects the operator declaration
itself (or the specialized machine instance when the requirement is a member
generated inside one). Implicit `Match` equality retains its parser-owned arm
extent and operator occurrence even when the requirement is authored as a
token-bearing machine. A target that selects no provider for a requirement has
no realization of the application to attest unless a selected plan binds the
requirement without rejoining the use.
*/

use super::{
    StagedRealization, authored_application_source_span, canonical_source_span_location,
    expression_is_owned_by_package, nominal_identity, stage_realization, token_authored_use_kind,
};
use crate::capture::EvidenceOption;
use crate::capture::PackageReviewInput;
use crate::record::{
    CheckedPackageBoundaryApplicationRealizationReview, PackageReviewBoundaryApplication,
    PackageReviewBoundaryApplicationRealization, PackageReviewSourceLocationOwner,
    PackageReviewSourceLocationRole,
};
use checked_trees::CheckedBoundaryOperatorApplicationUseSite;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use provider_planning::IntrinsicRequirement;
use provider_planning::ProviderPlanningContext;
use selected_dispatch::CheckedOperatorAuthoredUseKind;
use semantic_vocabulary::PackageKeyIdentity;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    package: PackageKeyIdentity,
    applications: &[&checked_trees::CheckedBoundaryOperatorApplicationDemand],
    staged: &mut Vec<StagedRealization>,
) -> Result<(), Vec<Diagnostic>> {
    let selected_execution = compilation.custody.execution();
    let selected_plans = selected_execution.plans().plans();
    let provenance = selected_execution.selected_provider_provenance.as_slice();
    if selected_plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected-provider plans and intrinsic review provenance are not aligned",
        )]);
    }

    for application in applications {
        if !application.arguments.is_empty() {
            continue;
        }
        if compilation
            .typed
            .symbols
            .authored_origin(application.requirement_symbol)
            != application.requirement_symbol
        {
            continue;
        }
        let expression = match application.site {
            CheckedBoundaryOperatorApplicationUseSite::Expression { expression, .. }
            | CheckedBoundaryOperatorApplicationUseSite::MatchEquality { expression, .. } => {
                expression
            }
            _ => continue,
        };
        let source_span = compilation.typed.expression_table.source_span(expression);
        if compilation
            .typed
            .symbols
            .source_file(source_span)
            .is_some_and(|source| source.dependency_scope == source::DependencyScope::Build)
        {
            continue;
        }
        let uses = exact_application_uses(
            compilation,
            application.site,
            application.requirement_symbol,
        );
        let [actual_use] = uses.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "boundary application at expression {expression:?} retains {} exact selected uses; expected one",
                uses.len(),
            ))]);
        };
        let selection_target = match actual_use.kind {
            CheckedOperatorAuthoredUseKind::Named => IntrinsicRequirement::by_symbol(
                compilation.checking_input,
                application.requirement_symbol,
            )
            .map_or(application.requirement_symbol, |requirement| {
                requirement.call_target
            }),
            CheckedOperatorAuthoredUseKind::FixedToken(_) => application.requirement_symbol,
        };
        let alternate_target = match actual_use.kind {
            CheckedOperatorAuthoredUseKind::FixedToken(_) => {
                specialized_machine_of_member(compilation, application.requirement_symbol)
                    .unwrap_or_default()
            }
            CheckedOperatorAuthoredUseKind::Named => symbols::SymbolHandle::default(),
        };
        let implicit_source_span = match application.site {
            CheckedBoundaryOperatorApplicationUseSite::MatchEquality { source_arm, .. } => {
                let source_use = compilation
                    .facts
                    .operators
                    .uses
                    .iter()
                    .find(|(_, operator_use)| {
                        operator_use.application_site() == application.site
                            && operator_use.selected_operator_symbol
                                == application.requirement_symbol
                    })
                    .map(|(_, operator_use)| operator_use)
                    .or_evidence("implicit application lost its exact source use")?;
                source_use
                    .operands(compilation.checking_input)
                    .or_evidence("implicit application has invalid Match arm custody")?;
                let typed_trees::expression::ExpressionNode::Match(dispatch) =
                    compilation.typed.expression_table.expression(expression)
                else {
                    return Err(vec![Diagnostic::error(
                        "implicit application lost its authored Match",
                    )]);
                };
                let ordinal = source_arm
                    .arena_index()
                    .checked_sub(dispatch.arms.start().arena_index())
                    .and_then(|ordinal| usize::try_from(ordinal).ok())
                    .or_evidence("implicit application has invalid source arm ordinal")?;
                let arm = compilation
                    .typed
                    .expression_table
                    .match_arms(dispatch.arms)
                    .get(ordinal)
                    .or_evidence("implicit application lost its source arm")?;
                Some(arm.source_span)
            }
            _ => None,
        };
        if implicit_source_span.is_none()
            && !expression_is_owned_by_package(
                compilation,
                expression,
                actual_use.kind,
                selection_target,
                alternate_target,
                package,
            )?
        {
            continue;
        }
        let location = canonical_source_span_location(
            compilation,
            if let Some(pattern_span) = implicit_source_span {
                let root_span = compilation.typed.expression_table.source_span(expression);
                if pattern_span.source_id != root_span.source_id
                    || pattern_span.span.start < root_span.span.start
                    || pattern_span.span.end > root_span.span.end
                    || pattern_span.span.start >= pattern_span.span.end
                {
                    return Err(vec![Diagnostic::error(
                        "implicit application pattern escaped its authored Match source",
                    )]);
                }
                pattern_span
            } else {
                authored_application_source_span(
                    compilation,
                    expression,
                    actual_use.kind,
                    selection_target,
                    alternate_target,
                )?
            },
            PackageReviewSourceLocationRole::BoundaryApplicationUse,
        )?;
        if location.owner != PackageReviewSourceLocationOwner::Package(package) {
            continue;
        }
        let joined = provider_planning::selected_use_plan(
            compilation.program,
            source::DependencyScope::Product,
            selected_plans,
            application.requirement_symbol,
            actual_use.origin,
        )
        .or_else(|| {
            let origin = compilation
                .typed
                .symbols
                .authored_origin(application.requirement_symbol);
            (origin != application.requirement_symbol).then(|| {
                provider_planning::selected_use_plan(
                    compilation.program,
                    source::DependencyScope::Product,
                    selected_plans,
                    origin,
                    actual_use.origin,
                )
            })?
        })
        .and_then(|(index, plan)| {
            provenance
                .get(index)
                .filter(|retained| retained.plan == *plan)
                .map(|retained| (plan, retained))
        });
        let Some((plan, retained)) = joined else {
            if !selects_requirement_provider(
                compilation,
                selected_plans,
                application.requirement_symbol,
            ) {
                continue;
            }
            return Err(vec![Diagnostic::error(format!(
                "boundary application at expression {expression:?} rejoins no exact selected provider plan",
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
        if !matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
            if implicit_source_span.is_some() {
                return Err(vec![Diagnostic::error(
                    "implicit Match application has no supported intrinsic review realization",
                )]);
            }
            continue;
        }
        let execution = super::super::intrinsics::project_compiler_intrinsic_execution(
            compilation,
            plan,
            row,
            retained.provider.schema,
            retained.provider.provider_type,
            **requirement_symbol,
            realization_symbol.authored().ok_or_else(|| vec![Diagnostic::error("intrinsic application lost its authored realization")])?,
            compilation
                .custody.providers().planning_target_profile
                .map(target::TargetProfile::target_name),
            **retained_execution,
        )?
        .or_evidence(format!(
                "actual boundary application at expression {expression:?} selects compiler intrinsic `{}`, but no closed execution identity exists",
                plan.name,
            ))?;
        let requirement_identity = IntrinsicRequirement::by_symbol(
            compilation.checking_input,
            application.requirement_symbol,
        )
        .or_else(|| {
            let origin = compilation
                .typed
                .symbols
                .authored_origin(application.requirement_symbol);
            IntrinsicRequirement::by_symbol(compilation.checking_input, origin)
        })
        .map(|requirement| requirement.into_source().requirement_identity)
        .or_evidence("actual compiler-intrinsic application names no boundary operator or top-level boundary requirement",)?;
        if requirement_identity.is_empty() {
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
                selected_plan_digest: *plan.identity_digest().as_bytes(),
                realization: PackageReviewBoundaryApplicationRealization::ExactCompilerIntrinsic {
                    execution,
                },
            },
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct ExactApplicationUse {
    kind: CheckedOperatorAuthoredUseKind,
    origin: checked_trees::CheckedValueOrigin,
}

fn authored_use_kind(
    compilation: &PackageReviewInput<'_>,
    expression: typed_trees::expression::ExpressionHandle,
    requirement: symbols::SymbolHandle,
) -> CheckedOperatorAuthoredUseKind {
    token_authored_use_kind(
        compilation,
        expression,
        requirement,
        compilation.typed.symbols.authored_origin(requirement),
    )
    .or_else(|| {
        let machine = specialized_machine_of_member(compilation, requirement)?;
        token_authored_use_kind(
            compilation,
            expression,
            machine,
            compilation.typed.symbols.authored_origin(machine),
        )
    })
    .unwrap_or(CheckedOperatorAuthoredUseKind::Named)
}

fn specialized_machine_of_member(
    compilation: &PackageReviewInput<'_>,
    requirement: symbols::SymbolHandle,
) -> Option<symbols::SymbolHandle> {
    let parent = compilation.typed.symbols.get(requirement).parent;
    if parent.is_valid() && compilation.typed.symbols.authored_origin(parent) != parent {
        Some(parent)
    } else {
        None
    }
}

fn selects_requirement_provider(
    compilation: &PackageReviewInput<'_>,
    selected_plans: &[effects::provider_plan::ProviderPlan],
    requirement: symbols::SymbolHandle,
) -> bool {
    let context =
        ProviderPlanningContext::new(&compilation.program.typed, source::DependencyScope::Product);
    let origin = compilation.typed.symbols.authored_origin(requirement);
    [requirement, origin].into_iter().any(|requirement| {
        provider_planning::selected_requirement_plan(context, selected_plans, requirement).is_some()
    })
}

fn exact_application_uses(
    compilation: &PackageReviewInput<'_>,
    site: CheckedBoundaryOperatorApplicationUseSite,
    requirement: symbols::SymbolHandle,
) -> Vec<ExactApplicationUse> {
    let operator =
        typed_trees::operator::declaration_by_symbol(compilation.checking_input, requirement);
    compilation
        .facts
        .operators
        .named_uses
        .iter()
        .filter_map(|(_, operator_use)| {
            (site
                == CheckedBoundaryOperatorApplicationUseSite::Expression {
                    expression: operator_use.expression,
                    origin: operator_use.origin,
                }
                && operator_use.selected_operator_symbol == requirement)
                .then_some(ExactApplicationUse {
                    kind: authored_use_kind(compilation, operator_use.expression, requirement),
                    origin: operator_use.origin,
                })
        })
        .chain(
            compilation
                .facts
                .operators
                .named_requirement_uses
                .iter()
                .filter_map(|(_, requirement_use)| {
                    (site
                        == CheckedBoundaryOperatorApplicationUseSite::Expression {
                            expression: requirement_use.expression,
                            origin: requirement_use.origin,
                        }
                        && requirement_use.requirement_symbol == requirement)
                        .then_some(ExactApplicationUse {
                            kind: authored_use_kind(
                                compilation,
                                requirement_use.expression,
                                requirement,
                            ),
                            origin: requirement_use.origin,
                        })
                }),
        )
        .chain(
            compilation
                .facts
                .operators
                .uses
                .iter()
                .filter_map(|(_, operator_use)| {
                    (operator_use.application_site() == site
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
                        kind: CheckedOperatorAuthoredUseKind::FixedToken(operator_use.spelling),
                        origin: operator_use.origin,
                    })
                }),
        )
        .collect()
}
