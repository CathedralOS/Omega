//! Exact compiler-intrinsic application and selected-plan reconciliation.

use super::{
    StagedRealization, authored_application_source_span, canonical_source_span_location,
    expression_is_owned_by_package, nominal_identity, stage_realization, token_authored_use_kind,
};
use crate::capture::PackageReviewInput;
use crate::record::{
    CheckedPackageBoundaryApplicationRealizationReview, PackageReviewBoundaryApplication,
    PackageReviewBoundaryApplicationRealization, PackageReviewSourceLocationOwner,
    PackageReviewSourceLocationRole,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    package: PackageKeyIdentity,
    applications: &[&checked_trees::CheckedBoundaryOperatorApplicationDemand],
    staged: &mut Vec<StagedRealization>,
) -> Result<(), Vec<Diagnostic>> {
    let selected_execution = compilation.custody.execution();
    let selected_plans = selected_execution.selected_provider_plan_facts.plans();
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
        // A specialized machine instance is projected by the specialized
        // checked-body path — its plan binds the authored template slot and
        // its token selections resolve to the instance, both of which the
        // exact intrinsic join cannot see.
        if compilation
            .typed
            .symbols
            .authored_origin(application.requirement_symbol)
            != application.requirement_symbol
        {
            continue;
        }
        let expression = match application.site {
            checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                expression,
                ..
            }
            | checked_trees::CheckedBoundaryOperatorApplicationUseSite::MatchEquality {
                expression,
                ..
            } => expression,
            _ => continue,
        };
        // This projection describes the selected Product realization. Build
        // applications have their own checked instance and execution profile;
        // the Product's provider plans cannot attest those executions. Build
        // callable policy remains reviewed by capture/callables/policy.rs.
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
        // The authored selection ledger names what the use resolved to: a named
        // call selects the requirement's callable entry state, while a token
        // use's `Operator` occurrence selects the operator declaration itself.
        let selection_target = match actual_use.kind {
            selected_dispatch::CheckedOperatorAuthoredUseKind::Named => {
                provider_planning::IntrinsicRequirement::by_symbol(
                    compilation.checking_input,
                    application.requirement_symbol,
                )
                .map_or(application.requirement_symbol, |requirement| {
                    requirement.call_target
                })
            }
            selected_dispatch::CheckedOperatorAuthoredUseKind::FixedToken(_) => {
                application.requirement_symbol
            }
        };
        // A token use on a specialized machine member retains an `Operator`
        // selection resolved to the machine instance, not the member.
        let alternate_target = match actual_use.kind {
            selected_dispatch::CheckedOperatorAuthoredUseKind::FixedToken(_) => {
                specialized_machine_of_member(compilation, application.requirement_symbol)
                    .unwrap_or_default()
            }
            selected_dispatch::CheckedOperatorAuthoredUseKind::Named => {
                symbols::SymbolHandle::default()
            }
        };
        let implicit_source_span = match application.site {
            checked_trees::CheckedBoundaryOperatorApplicationUseSite::MatchEquality {
                source_arm,
                ..
            } => {
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
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "implicit application lost its exact source use",
                        )]
                    })?;
                source_use
                    .operands(compilation.checking_input)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "implicit application has invalid Match arm custody",
                        )]
                    })?;
                let typed_trees::expression::ExpressionNode::Match(dispatch) =
                    compilation.typed.expression_table.expression(expression)
                else {
                    return Err(vec![Diagnostic::error(
                        "implicit application lost its authored Match",
                    )]);
                };
                // operands() has established exact generation and membership in
                // this arm span. The arm owns an authored extent even when its
                // operand is a call whose expression extent is not retained.
                let ordinal = source_arm
                    .arena_index()
                    .checked_sub(dispatch.arms.start().arena_index())
                    .and_then(|ordinal| usize::try_from(ordinal).ok())
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "implicit application has invalid source arm ordinal",
                        )]
                    })?;
                let arm = compilation
                    .typed
                    .expression_table
                    .match_arms(dispatch.arms)
                    .get(ordinal)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "implicit application lost its source arm",
                        )]
                    })?;
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
                // The arm is authored; the implicit equality token is not.
                // Retain its parser-owned extent rather than inventing a binary
                // selection occurrence at the surrounding Match expression.
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
        // The selected plan joins the use by requirement identity. A
        // specialized machine instance binds the same slot its authored
        // template requirement binds — the instance carries no own schema.
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
            // A target that selects no provider for this requirement has no
            // realization of the application to attest: std's float code
            // checked for `uefi_x86_64`, which supplies no float provider. A
            // product that reaches such an application fails its own
            // realization; the portable package does not
            // (wiki/spec/build/configuration.md#multi-target-compilation). A
            // selected plan that binds the requirement but does not rejoin
            // this use is still inconsistent custody.
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
        if !matches!(
            row.binding,
            effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
        ) {
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
            **requirement_symbol,
            **realization_symbol,
            compilation
                .custody.providers().planning_target_profile
                .map(target::TargetProfile::target_name),
            **retained_execution,
        )?
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "actual boundary application at expression {expression:?} selects compiler intrinsic `{}`, but no closed execution identity exists",
                plan.name,
            ))]
        })?;
        // The demand names a boundary operator or a top-level boundary
        // requirement; the review row keys on the same overload coordinate
        // for either species.
        let requirement_identity = provider_planning::IntrinsicRequirement::by_symbol(
            compilation.checking_input,
            application.requirement_symbol,
        )
        .or_else(|| {
            let origin = compilation
                .typed
                .symbols
                .authored_origin(application.requirement_symbol);
            provider_planning::IntrinsicRequirement::by_symbol(compilation.checking_input, origin)
        })
        .map(|requirement| requirement.requirement_identity)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "actual compiler-intrinsic application names no boundary operator or top-level boundary requirement",
            )]
        })?;
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
    kind: selected_dispatch::CheckedOperatorAuthoredUseKind,
    origin: checked_trees::CheckedValueOrigin,
}

fn authored_use_kind(
    compilation: &PackageReviewInput<'_>,
    expression: typed_trees::expression::ExpressionHandle,
    requirement: symbols::SymbolHandle,
) -> selected_dispatch::CheckedOperatorAuthoredUseKind {
    token_authored_use_kind(
        compilation,
        expression,
        requirement,
        compilation.typed.symbols.authored_origin(requirement),
    )
    .or_else(|| {
        // A requirement member of a specialized machine instance: the token's
        // `Operator` selection resolves to the instance itself and its token
        // spelling lives on the authored template.
        let machine = specialized_machine_of_member(compilation, requirement)?;
        token_authored_use_kind(
            compilation,
            expression,
            machine,
            compilation.typed.symbols.authored_origin(machine),
        )
    })
    .unwrap_or(selected_dispatch::CheckedOperatorAuthoredUseKind::Named)
}

/// When `requirement` is a member generated inside a specialized machine
/// instance, returns that instance: the symbol a token-authored `Operator`
/// selection resolves to. Named uses and declared (non-generated) parents
/// return `None` so ordinary custody stays exact.
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

/// Whether this target selects any provider plan binding `requirement` or the
/// authored requirement a specialized member came from, the same two
/// identities the use join tries.
fn selects_requirement_provider(
    compilation: &PackageReviewInput<'_>,
    selected_plans: &[effects::provider_plan::ProviderPlan],
    requirement: symbols::SymbolHandle,
) -> bool {
    let context = provider_planning::ProviderPlanningContext::new(
        &compilation.program.typed,
        source::DependencyScope::Product,
    );
    let origin = compilation.typed.symbols.authored_origin(requirement);
    [requirement, origin].into_iter().any(|requirement| {
        provider_planning::selected_requirement_plan(context, selected_plans, requirement).is_some()
    })
}

fn exact_application_uses(
    compilation: &PackageReviewInput<'_>,
    site: checked_trees::CheckedBoundaryOperatorApplicationUseSite,
    requirement: symbols::SymbolHandle,
) -> Vec<ExactApplicationUse> {
    // Implicit Match equality keeps its operator occurrence even when the
    // requirement is authored as a token-bearing machine. Rejoin its exact
    // declaration view, not just the legacy operator declaration collection.
    let operator =
        typed_trees::operator::declaration_by_symbol(compilation.checking_input, requirement);
    compilation
        .facts
        .operators
        .named_uses
        .iter()
        .filter_map(|(_, operator_use)| {
            (site
                == checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
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
                        == checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
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
                        kind: selected_dispatch::CheckedOperatorAuthoredUseKind::FixedToken(
                            operator_use.spelling,
                        ),
                        origin: operator_use.origin,
                    })
                }),
        )
        .collect()
}
