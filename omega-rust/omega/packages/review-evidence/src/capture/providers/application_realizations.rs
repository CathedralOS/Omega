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
use crate::capture::PackageReviewInput;
use crate::record::{
    CheckedPackageBoundaryApplicationRealizationReview, PackageReviewBoundaryApplication,
    PackageReviewBoundaryApplicationArgument, PackageReviewBoundaryApplicationRealization,
    PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;

pub(super) struct StagedRealization {
    row: CheckedPackageBoundaryApplicationRealizationReview,
}

pub(crate) fn project_boundary_application_realizations(
    compilation: &PackageReviewInput<'_>,
    package: PackageKeyIdentity,
) -> Result<Vec<CheckedPackageBoundaryApplicationRealizationReview>, Vec<Diagnostic>> {
    // Select the executable population before deriving physical relationships:
    // an inactive generic demand must not require an active specialization plan.
    let mut applications = Vec::new();
    for application in &compilation.facts.operators.boundary_applications {
        if application_is_selected(compilation, application.site)? {
            applications.push(application);
        }
    }
    let derived = selected_dispatch::derive_checked_nongeneric_operator_application_realizations(
        compilation,
        &compilation.custody.execution().selected_provider_plan_facts,
        applications.iter().copied(),
    )?;
    let mut staged: Vec<StagedRealization> = Vec::new();

    for realization in derived {
        let checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
            expression, ..
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
            symbols::SymbolHandle::default(),
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
                symbols::SymbolHandle::default(),
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
            selected_plan_digest: realization.provider_plan_commitment.as_bytes(),
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
        stage_realization(&mut staged, row)?;
    }

    project_specialized_checked_body_applications(
        compilation,
        package,
        &applications,
        &mut staged,
    )?;

    compiler_intrinsics::project(compilation, package, &applications, &mut staged)?;

    staged.sort_by(|left, right| application_key(&left.row).cmp(&application_key(&right.row)));
    Ok(staged
        .into_iter()
        .map(|realization| realization.row)
        .collect())
}

fn project_specialized_checked_body_applications(
    compilation: &PackageReviewInput<'_>,
    package: PackageKeyIdentity,
    applications: &[&checked_trees::CheckedBoundaryOperatorApplicationDemand],
    staged: &mut Vec<StagedRealization>,
) -> Result<(), Vec<Diagnostic>> {
    let derived = selected_dispatch::derive_checked_specialized_operator_application_realizations(
        compilation,
        &compilation.custody.execution().selected_provider_plan_facts,
        applications.iter().copied(),
    )?;
    for realization in derived {
        let checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
            expression, ..
        } = realization.application_site
        else {
            return Err(vec![Diagnostic::error(
                "specialized boundary application realization has no expression use site",
            )]);
        };
        // A specialized clone carries two authored levels: a token-authored
        // use resolves its Operator selection to the specialized machine
        // instance, named calls resolve to the requirement member, and the
        // token spelling binds on the declared template. Custody therefore
        // matches the requirement member with the instance as the alternate
        // exact target; the realization's own symbols stay specialized.
        let use_kind = token_authored_use_kind(
            compilation,
            expression,
            realization.realization_machine,
            realization.realization_template,
        )
        .unwrap_or(realization.authored_use_kind);
        if !expression_is_owned_by_package(
            compilation,
            expression,
            use_kind,
            realization.requirement_operator,
            realization.realization_machine,
            package,
        )? {
            continue;
        }
        let location = canonical_source_span_location(
            compilation,
            authored_application_source_span(
                compilation,
                expression,
                use_kind,
                realization.requirement_operator,
                realization.realization_machine,
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
            selected_plan_digest: realization.provider_plan_commitment.as_bytes(),
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
        stage_realization(staged, row)?;
    }
    Ok(())
}

/// Closed realization rows describe the selected activation, not all checked
/// bodies. Inactive members retain callable contracts and external-supply
/// disclosure; they must neither demand nor borrow this activation's plan.
fn application_is_selected(
    compilation: &PackageReviewInput<'_>,
    site: checked_trees::CheckedBoundaryOperatorApplicationUseSite,
) -> Result<bool, Vec<Diagnostic>> {
    let origin = match site {
        checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression { origin, .. }
        | checked_trees::CheckedBoundaryOperatorApplicationUseSite::MatchEquality {
            origin, ..
        } => origin,
        // No known machine origin means no proof of inactive scope: keep the
        // ordinary exact application/plan checks, rather than omit the use.
        checked_trees::CheckedBoundaryOperatorApplicationUseSite::Statement(_) => return Ok(true),
    };
    let Some(machine) = origin.machine_symbol() else {
        return Ok(true);
    };
    let selected = compilation
        .authored_machine_target(machine)?
        .is_none_or(|authored| {
            compilation
                .custody
                .execution()
                .selected_target_profile
                .is_some_and(|selected| selected.target_name() == authored)
        });
    if !selected
        && !compilation
            .custody
            .facts
            .operators
            .boundary_applications
            .iter()
            .any(|original| original.site == site)
    {
        // A supplied fact cannot invent an inactive caller origin to hide an
        // active use. The exact expression/arm and origin were checked together.
        return Err(vec![Diagnostic::error(
            "inactive boundary application differs from checked occurrence custody",
        )]);
    }
    Ok(selected)
}

fn project_exact_application(
    compilation: &PackageReviewInput<'_>,
    requirement: symbols::SymbolHandle,
    arguments: &[checked_trees::CheckedBoundaryOperatorApplicationArgument],
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
            checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
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
            checked_trees::CheckedBoundaryOperatorApplicationArgument::Const {
                binder_owner,
                binder_ordinal,
                declared_carrier,
                value,
                ..
            } if *binder_owner == requirement && *binder_ordinal == expected_ordinal => {
                validation::validate_exact_const_value_encoding(
                    compilation.checking_input,
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
    compilation: &PackageReviewInput<'_>,
    expression: typed_trees::expression::ExpressionHandle,
    use_kind: selected_dispatch::CheckedOperatorAuthoredUseKind,
    requirement_operator: symbols::SymbolHandle,
    alternate_requirement: symbols::SymbolHandle,
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
            .authored_selection_occurrences(expression)
            .filter_map(|occurrence| {
                compilation
                    .authored_declaration_selections()
                    .get(occurrence)
            })
            .any(|selection| selection.kind() == expected_kind);
        if !has_authored_selection {
            return Ok(false);
        }
        authored_application_source_span(
            compilation,
            expression,
            use_kind,
            requirement_operator,
            alternate_requirement,
        )?
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
        source::SourceOrigin::Toolchain => Ok(false),
        source::SourceOrigin::User => source
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
    } else {
        staged.push(StagedRealization { row });
    }
    Ok(())
}

/// `requirement_operator` is the declaration an authored selection must
/// resolve to; `alternate_requirement` is a second accepted target (an invalid
/// handle means the requirement alone is exact). Specialized applications
/// supply their machine instance there because a token-authored use resolves
/// its Operator selection to the instance rather than the requirement member.
pub(super) fn authored_application_source_span(
    compilation: &PackageReviewInput<'_>,
    expression: typed_trees::expression::ExpressionHandle,
    use_kind: selected_dispatch::CheckedOperatorAuthoredUseKind,
    requirement_operator: symbols::SymbolHandle,
    alternate_requirement: symbols::SymbolHandle,
) -> Result<source::SourceSpan, Vec<Diagnostic>> {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionTarget,
    };

    let expected_kind = authored_application_selection_kind(use_kind);
    let selections = compilation
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
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
                || (alternate_requirement.is_valid()
                    && target.selected_symbol() == alternate_requirement)
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
    use_kind: selected_dispatch::CheckedOperatorAuthoredUseKind,
) -> language_semantics::declaration_selection::AuthoredDeclarationSelectionKind {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;

    match use_kind {
        selected_dispatch::CheckedOperatorAuthoredUseKind::Named => {
            AuthoredDeclarationSelectionKind::Call
        }
        selected_dispatch::CheckedOperatorAuthoredUseKind::FixedToken(_) => {
            AuthoredDeclarationSelectionKind::Operator
        }
    }
}

/// A token-authored application of a token-bound machine is rewritten into an
/// ordinary `Call` node (`bind_selected_operator_calls`); the rewritten node
/// keeps its authored `Operator` selection occurrence while the operator use
/// facts record the settled resolution as a named use. The named row names the
/// declaration the token selected, not the authored surface, so when the site
/// retains an `Operator` selection resolved to the requirement the use
/// classifies `FixedToken` under the token binding's spelling instead.
/// `selection_symbol` is the declaration the retained `Operator` selection
/// resolves to; `spelling_symbol` is a fallback symbol the token spelling may
/// bind on (an invalid handle disables it). A specialized machine instance
/// resolves the selection while its declared template holds the spelling.
pub(super) fn token_authored_use_kind(
    compilation: &PackageReviewInput<'_>,
    expression: typed_trees::expression::ExpressionHandle,
    selection_symbol: symbols::SymbolHandle,
    spelling_symbol: symbols::SymbolHandle,
) -> Option<selected_dispatch::CheckedOperatorAuthoredUseKind> {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
    };

    let authored_token = compilation
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .authored_declaration_selections()
                .get(occurrence)
        })
        .any(|selection| {
            selection.kind() == AuthoredDeclarationSelectionKind::Operator
                && matches!(
                    selection.target(),
                    AuthoredDeclarationSelectionTarget::Resolved(target)
                        if target.selected_symbol() == selection_symbol
                )
        });
    if !authored_token {
        return None;
    }
    compilation
        .typed
        .machine_token_bindings()
        .iter()
        .chain(compilation.typed.operators())
        .find(|operator| operator.symbol == selection_symbol)
        .or_else(|| {
            compilation
                .typed
                .machine_token_bindings()
                .iter()
                .chain(compilation.typed.operators())
                .find(|operator| spelling_symbol.is_valid() && operator.symbol == spelling_symbol)
        })
        .and_then(|operator| operator.spelling)
        .map(selected_dispatch::CheckedOperatorAuthoredUseKind::FixedToken)
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
