//! Actual D29 closed-application realization review projection.
//!
//! Selected dispatch performs the compiler-private join. This module removes
//! handles, attributes actual authored uses to the reviewed package, and
//! deduplicates only equal complete semantic rows. It creates no Terminal,
//! native, admission, or audit claim.

use super::super::semantics::declarations::nominal_identity;
use super::super::semantics::types::review_type_identity_with_binders;
use super::super::source::locations::canonical_source_span_location;
use super::intrinsics::project_compiler_intrinsic_execution;
use crate::record::{
    CheckedPackageBoundaryApplicationRealizationReview, PackageReviewBoundaryApplication,
    PackageReviewBoundaryApplicationArgument, PackageReviewBoundaryApplicationRealization,
    PackageReviewCanonicalRowSource, PackageReviewSourceLocation, PackageReviewSourceLocationOwner,
    PackageReviewSourceLocationRole,
};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(crate) struct ProjectedBoundaryApplicationRealizations {
    pub(crate) rows: Vec<CheckedPackageBoundaryApplicationRealizationReview>,
    pub(crate) sources: Vec<PackageReviewCanonicalRowSource>,
}

struct StagedRealization {
    row: CheckedPackageBoundaryApplicationRealizationReview,
    locations: Vec<PackageReviewSourceLocation>,
}

pub(crate) fn project_boundary_application_realizations(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<ProjectedBoundaryApplicationRealizations, Vec<Diagnostic>> {
    let derived =
        omega_selected_dispatch::derive_checked_nongeneric_operator_application_realizations(
            compilation,
            compilation.selected_provider_plans(),
        )?;
    let mut staged: Vec<StagedRealization> = Vec::new();

    for realization in derived {
        let psi_checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
            expression,
            ..
        } = realization.application_site
        else {
            return Err(vec![Diagnostic::error(
                "attached-Unit boundary application realization has no expression use site",
            )]);
        };
        let location = canonical_source_span_location(
            compilation,
            authored_application_source_span(
                compilation,
                expression,
                realization.authored_use_kind,
                realization.requirement_operator,
            )?,
            PackageReviewSourceLocationRole::BoundaryApplicationUse,
        )?;
        if location.owner != PackageReviewSourceLocationOwner::Package(package) {
            continue;
        }
        let row = CheckedPackageBoundaryApplicationRealizationReview {
            requirement_identity: realization.requirement_overload_identity,
            operator_declaration: nominal_identity(compilation, realization.requirement_operator)?,
            application: PackageReviewBoundaryApplication::Empty,
            selected_plan_digest: *realization.provider_plan_commitment.as_bytes(),
            realization: PackageReviewBoundaryApplicationRealization::NongenericCheckedBody {
                realization_machine: nominal_identity(
                    compilation,
                    realization.realization_machine,
                )?,
                realization_state: nominal_identity(compilation, realization.realization_state)?,
                realization_contract_commitment: realization
                    .realization_contract_commitment
                    .as_bytes(),
            },
        };
        let PackageReviewBoundaryApplicationRealization::NongenericCheckedBody {
            realization_contract_commitment,
            ..
        } = &row.realization
        else {
            unreachable!("newly constructed checked-body realization changed role")
        };
        if row.selected_plan_digest == [0; 32] || *realization_contract_commitment == [0; 32] {
            return Err(vec![Diagnostic::error(
                "boundary application realization lost a strong plan or contract commitment",
            )]);
        }
        stage_realization(&mut staged, row, location)?;
    }

    project_specialized_checked_body_applications(compilation, package, &mut staged)?;

    project_exact_compiler_intrinsic_applications(compilation, package, &mut staged)?;

    staged.sort_by(|left, right| application_key(&left.row).cmp(&application_key(&right.row)));
    let mut rows = Vec::with_capacity(staged.len());
    let mut sources = Vec::with_capacity(staged.len());
    for mut realization in staged {
        realization.locations.sort();
        realization.locations.dedup();
        rows.push(realization.row);
        sources.push(PackageReviewCanonicalRowSource::authored(
            realization.locations,
        ));
    }
    Ok(ProjectedBoundaryApplicationRealizations { rows, sources })
}

fn project_specialized_checked_body_applications(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
    staged: &mut Vec<StagedRealization>,
) -> Result<(), Vec<Diagnostic>> {
    let derived =
        omega_selected_dispatch::derive_checked_specialized_operator_application_realizations(
            compilation,
            compilation.selected_provider_plans(),
        )?;
    for realization in derived {
        let psi_checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
            expression,
            ..
        } = realization.application_site
        else {
            return Err(vec![Diagnostic::error(
                "specialized boundary application realization has no expression use site",
            )]);
        };
        let location = canonical_source_span_location(
            compilation,
            authored_application_source_span(
                compilation,
                expression,
                realization.authored_use_kind,
                realization.requirement_operator,
            )?,
            PackageReviewSourceLocationRole::BoundaryApplicationUse,
        )?;
        if location.owner != PackageReviewSourceLocationOwner::Package(package) {
            continue;
        }
        let application = project_exact_application(
            compilation,
            realization.requirement_operator,
            &realization.application_arguments,
        )?;
        let row = CheckedPackageBoundaryApplicationRealizationReview {
            requirement_identity: realization.requirement_overload_identity,
            operator_declaration: nominal_identity(compilation, realization.requirement_operator)?,
            application,
            selected_plan_digest: *realization.provider_plan_commitment.as_bytes(),
            realization: PackageReviewBoundaryApplicationRealization::SpecializedCheckedBody {
                realization_template: nominal_identity(
                    compilation,
                    realization.realization_template,
                )?,
                realization_machine: nominal_identity(
                    compilation,
                    realization.realization_machine,
                )?,
                realization_state: nominal_identity(compilation, realization.realization_state)?,
                specialization_commitment: realization.specialization_commitment.as_bytes(),
                realization_contract_commitment: realization
                    .realization_contract_commitment
                    .as_bytes(),
            },
        };
        if row.selected_plan_digest == [0; 32]
            || matches!(
                &row.realization,
                PackageReviewBoundaryApplicationRealization::SpecializedCheckedBody {
                    specialization_commitment,
                    realization_contract_commitment,
                    ..
                } if *specialization_commitment == [0; 32]
                    || *realization_contract_commitment == [0; 32]
            )
        {
            return Err(vec![Diagnostic::error(
                "specialized boundary application lost a strong plan, specialization, or contract commitment",
            )]);
        }
        stage_realization(staged, row, location)?;
    }
    Ok(())
}

fn project_exact_application(
    compilation: &CheckedCompilation,
    requirement: psi_symbols::SymbolHandle,
    arguments: &[psi_checked_trees::CheckedBoundaryOperatorApplicationArgument],
) -> Result<PackageReviewBoundaryApplication, Vec<Diagnostic>> {
    if arguments.is_empty() {
        return Err(vec![Diagnostic::error(
            "specialized checked-body projection received an empty application",
        )]);
    }
    let mut projected = Vec::with_capacity(arguments.len());
    for (ordinal, argument) in arguments.iter().enumerate() {
        let expected_ordinal = u32::try_from(ordinal).map_err(|_| {
            vec![Diagnostic::error(
                "boundary application telescope exceeds the review ordinal range",
            )]
        })?;
        match argument {
            psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
                binder_owner,
                binder_ordinal,
                type_reference,
                ..
            } if *binder_owner == requirement && *binder_ordinal == expected_ordinal => {
                projected.push(PackageReviewBoundaryApplicationArgument::Type {
                    binder_ordinal: *binder_ordinal,
                    type_identity: review_type_identity_with_binders(
                        compilation,
                        *type_reference,
                        &[],
                    )?,
                });
            }
            psi_checked_trees::CheckedBoundaryOperatorApplicationArgument::Const {
                binder_owner,
                binder_ordinal,
                declared_carrier,
                value,
                ..
            } if *binder_owner == requirement && *binder_ordinal == expected_ordinal => {
                psi_validation::validate_exact_const_value_encoding(
                    &compilation.typed,
                    *declared_carrier,
                    value.encoding.as_str(),
                )
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "boundary application const value has invalid canonical encoding: {reason}"
                    ))]
                })?;
                projected.push(PackageReviewBoundaryApplicationArgument::Const {
                    binder_ordinal: *binder_ordinal,
                    declared_carrier: review_type_identity_with_binders(
                        compilation,
                        *declared_carrier,
                        &[],
                    )?,
                    value_type: value.type_name.clone(),
                    value_encoding: value.encoding.clone(),
                });
            }
            _ => {
                return Err(vec![Diagnostic::error(
                    "boundary application does not rejoin its operator binder owner, category, and ordinal",
                )]);
            }
        }
    }
    Ok(PackageReviewBoundaryApplication::Exact(projected))
}

fn project_exact_compiler_intrinsic_applications(
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
        let psi_checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
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
            omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
        ) {
            continue;
        }
        let execution = project_compiler_intrinsic_execution(
            compilation,
            plan,
            row,
            retained.provider.schema,
            **requirement_symbol,
            **realization_symbol,
            compilation
                .selected_target_profile()
                .map(omega_target::TargetProfile::target_name),
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
        let requirement_identity =
            psi_typed_trees::operator::boundary_operator_requirement_identity(
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
    kind: omega_selected_dispatch::CheckedOperatorAuthoredUseKind,
    plan_report_fingerprint: u64,
    plan_commitment: psi_checked_trees::CheckedProviderPlanCommitment,
}

fn exact_application_uses(
    compilation: &CheckedCompilation,
    expression: psi_typed_trees::expression::ExpressionHandle,
    origin: psi_checked_trees::CheckedValueOrigin,
    requirement: psi_symbols::SymbolHandle,
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
                    kind: omega_selected_dispatch::CheckedOperatorAuthoredUseKind::Named,
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
                            == psi_checked_trees::CheckedOperatorResolutionStatus::Resolved
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
                        kind: omega_selected_dispatch::CheckedOperatorAuthoredUseKind::FixedToken(
                            operator_use.spelling,
                        ),
                        plan_report_fingerprint: operator_use.provider_plan_report_fingerprint,
                        plan_commitment: operator_use.provider_plan_commitment,
                    })
                }),
        )
        .collect()
}

fn stage_realization(
    staged: &mut Vec<StagedRealization>,
    row: CheckedPackageBoundaryApplicationRealizationReview,
    location: PackageReviewSourceLocation,
) -> Result<(), Vec<Diagnostic>> {
    if let Some(existing) = staged
        .iter_mut()
        .find(|existing| same_application_key(&existing.row, &row))
    {
        if existing.row != row {
            return Err(vec![Diagnostic::error(format!(
                "boundary application `{}` has contradictory selected semantic realizations",
                row.requirement_identity,
            ))]);
        }
        existing.locations.push(location);
    } else {
        staged.push(StagedRealization {
            row,
            locations: vec![location],
        });
    }
    Ok(())
}

fn authored_application_source_span(
    compilation: &CheckedCompilation,
    expression: psi_typed_trees::expression::ExpressionHandle,
    use_kind: omega_selected_dispatch::CheckedOperatorAuthoredUseKind,
    requirement_operator: psi_symbols::SymbolHandle,
) -> Result<psi_source::SourceSpan, Vec<Diagnostic>> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelectionTarget,
    };

    let expected_kind = match use_kind {
        omega_selected_dispatch::CheckedOperatorAuthoredUseKind::Named => {
            AuthoredDeclarationSelectionKind::Call
        }
        omega_selected_dispatch::CheckedOperatorAuthoredUseKind::FixedToken(_) => {
            AuthoredDeclarationSelectionKind::Operator
        }
    };
    let selections = compilation
        .typed
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .typed
                .authored_declaration_selections()
                .get(occurrence)
        })
        .filter(|selection| selection.kind() == expected_kind)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "boundary application expression {expression:?} retains {} matching authored selection occurrences; expected one",
            selections.len(),
        ))]);
    };
    let target_is_exact = matches!(
        selection.target(),
        AuthoredDeclarationSelectionTarget::Resolved(target)
            if target.selected_symbol() == requirement_operator
    );
    if selection.exposure() != AuthoredDeclarationSelectionExposure::PrivateImplementation
        || !target_is_exact
        || selection.source_span().span.start >= selection.source_span().span.end
    {
        return Err(vec![
            Diagnostic::error("boundary application has invalid authored selection custody")
                .with_source_span(selection.source_span()),
        ]);
    }
    Ok(selection.source_span())
}

fn same_application_key(
    left: &CheckedPackageBoundaryApplicationRealizationReview,
    right: &CheckedPackageBoundaryApplicationRealizationReview,
) -> bool {
    application_key(left) == application_key(right)
}

fn application_key(
    row: &CheckedPackageBoundaryApplicationRealizationReview,
) -> (
    &crate::record::PackageReviewNominalIdentity,
    &str,
    &PackageReviewBoundaryApplication,
) {
    (
        &row.operator_declaration,
        row.requirement_identity.as_str(),
        &row.application,
    )
}
