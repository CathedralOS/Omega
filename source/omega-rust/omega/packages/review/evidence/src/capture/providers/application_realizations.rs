//! Actual D29 closed-application realization review projection.
//!
//! Selected dispatch performs the compiler-private join. This module removes
//! handles, attributes actual authored uses to the reviewed package, and
//! deduplicates only equal complete semantic rows. It creates no Terminal,
//! native, admission, or audit claim.

mod compiler_intrinsics;

use super::super::semantics::declarations::nominal_identity;
use super::super::semantics::types::review_type_identity_with_binders;
use super::super::source::locations::canonical_source_span_location;
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

pub(super) struct StagedRealization {
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
        if !expression_is_owned_by_package(
            compilation,
            expression,
            realization.authored_use_kind,
            realization.requirement_operator,
            package,
        )? {
            continue;
        }
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

    compiler_intrinsics::project(compilation, package, &mut staged)?;

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
        if !expression_is_owned_by_package(
            compilation,
            expression,
            realization.authored_use_kind,
            realization.requirement_operator,
            package,
        )? {
            continue;
        }
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

fn expression_is_owned_by_package(
    compilation: &CheckedCompilation,
    expression: psi_typed_trees::expression::ExpressionHandle,
    use_kind: omega_selected_dispatch::CheckedOperatorAuthoredUseKind,
    requirement_operator: psi_symbols::SymbolHandle,
    package: PackageKeyIdentity,
) -> Result<bool, Vec<Diagnostic>> {
    let span = compilation.typed.expression_table.source_span(expression);
    // This projection records actual authored applications. Checked lowering
    // may synthesize applications with an empty provenance span and no
    // authored-selection custody while deriving contracts; those are compiler
    // derivations, not package uses. Test and generated fixtures can retain an
    // empty expression span beside a real authored selection, so the ledger is
    // the deciding distinction.
    let ownership_span = if span.span.start >= span.span.end {
        let expected_kind = authored_application_selection_kind(use_kind);
        let has_authored_selection = compilation
            .typed
            .expression_table
            .authored_selection_occurrences(expression)
            .filter_map(|occurrence| {
                compilation
                    .typed
                    .authored_declaration_selections()
                    .get(occurrence)
            })
            .any(|selection| selection.kind() == expected_kind);
        if !has_authored_selection {
            return Ok(false);
        }
        authored_application_source_span(compilation, expression, use_kind, requirement_operator)?
    } else {
        span
    };
    let source = compilation
        .typed
        .symbols
        .source_file(ownership_span)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "boundary application expression {expression:?} has no retained source file",
            ))]
        })?;
    match source.origin {
        psi_source::SourceOrigin::Toolchain => Ok(false),
        psi_source::SourceOrigin::User => source
            .package_identity
            .map(|owner| owner == package)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "boundary application source `{}` has no reconciled package identity",
                    source.path.display(),
                ))]
            }),
    }
}

pub(super) fn stage_realization(
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

pub(super) fn authored_application_source_span(
    compilation: &CheckedCompilation,
    expression: psi_typed_trees::expression::ExpressionHandle,
    use_kind: omega_selected_dispatch::CheckedOperatorAuthoredUseKind,
    requirement_operator: psi_symbols::SymbolHandle,
) -> Result<psi_source::SourceSpan, Vec<Diagnostic>> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionTarget,
    };

    let expected_kind = authored_application_selection_kind(use_kind);
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
        let expression_span = compilation.typed.expression_table.source_span(expression);
        let source = compilation.typed.symbols.source_file(expression_span);
        return Err(vec![Diagnostic::error(format!(
            "boundary application expression {expression:?} for `{}` ({use_kind:?}) at {}:{}..{} retains {} matching authored selection occurrences; expected one",
            compilation
                .typed
                .symbols
                .display_path(requirement_operator, "::"),
            source
                .map(|source| source.path.display().to_string())
                .unwrap_or_else(|| "<unknown source>".to_owned()),
            expression_span.span.start,
            expression_span.span.end,
            selections.len(),
        ))
        .with_source_span(expression_span)]);
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

fn authored_application_selection_kind(
    use_kind: omega_selected_dispatch::CheckedOperatorAuthoredUseKind,
) -> psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind {
    use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;

    match use_kind {
        omega_selected_dispatch::CheckedOperatorAuthoredUseKind::Named => {
            AuthoredDeclarationSelectionKind::Call
        }
        omega_selected_dispatch::CheckedOperatorAuthoredUseKind::FixedToken(_) => {
            AuthoredDeclarationSelectionKind::Operator
        }
    }
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
