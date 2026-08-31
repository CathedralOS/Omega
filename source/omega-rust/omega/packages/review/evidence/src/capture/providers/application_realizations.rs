//! Actual D29 empty-application realization review projection.
//!
//! Selected dispatch performs the compiler-private join. This module removes
//! handles, attributes actual authored uses to the reviewed package, and
//! deduplicates only equal complete semantic rows. It creates no Terminal,
//! native, admission, or audit claim.

use super::super::semantics::declarations::nominal_identity;
use super::super::source::locations::canonical_source_span_location;
use crate::record::{
    CheckedPackageBoundaryApplicationRealizationReview, PackageReviewBoundaryApplication,
    PackageReviewBoundaryApplicationRealizationRole, PackageReviewCanonicalRowSource,
    PackageReviewSourceLocation, PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
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
            role: PackageReviewBoundaryApplicationRealizationRole::NongenericCheckedBody,
            realization_machine: nominal_identity(compilation, realization.realization_machine)?,
            realization_state: nominal_identity(compilation, realization.realization_state)?,
            realization_contract_commitment: realization.realization_contract_commitment.as_bytes(),
        };
        if row.selected_plan_digest == [0; 32] || row.realization_contract_commitment == [0; 32] {
            return Err(vec![Diagnostic::error(
                "boundary application realization lost a strong plan or contract commitment",
            )]);
        }

        if let Some(existing) = staged
            .iter_mut()
            .find(|existing| same_application_key(&existing.row, &row))
        {
            if existing.row != row {
                return Err(vec![Diagnostic::error(format!(
                    "boundary application `{}` has contradictory selected checked-body realizations",
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
    }

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
    PackageReviewBoundaryApplication,
) {
    (
        &row.operator_declaration,
        row.requirement_identity.as_str(),
        row.application,
    )
}
