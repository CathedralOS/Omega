//! Separate proof-only package-review projection for bounded quotient rows.

use super::source::locations::canonical_typed_package_source_span_location;
use crate::record::{
    NonExecutableQuotientPackageReview, PackageReviewCanonicalRowSource,
    PackageReviewSourceLocationRole,
};
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::quotient_correspondence::{
    CanonicalQuotientCorrespondence, QuotientCorrespondenceOperationKind,
};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionNode, QuotientOperationKind};
use psi_typed_trees::statement::StatementNode;

/// Project the complete proof-only total-direct `define` batch into package
/// review without admitting any executable quotient operation.
///
/// Ordinary checked package projection remains unchanged and fail closed. This
/// separate entrance reruns the all-or-nothing source extractor and retains
/// only exact public operations owned by `package`.
pub fn project_non_executable_quotient_package_review(
    program: &TypedTrees,
    package: PackageKeyIdentity,
    target: TargetProfile,
) -> Result<NonExecutableQuotientPackageReview, Vec<Diagnostic>> {
    let extracted = psi_validation::extract_non_executable_quotient_correspondences(program)?
        .into_correspondences();
    project_replayed_batch(program, package, target, &extracted)
}

fn project_replayed_batch(
    program: &TypedTrees,
    package: PackageKeyIdentity,
    target: TargetProfile,
    supplied: &[CanonicalQuotientCorrespondence],
) -> Result<NonExecutableQuotientPackageReview, Vec<Diagnostic>> {
    let rederived = psi_validation::extract_non_executable_quotient_correspondences(program)?
        .into_correspondences();
    if supplied != rederived {
        return Err(vec![Diagnostic::error(
            "non-executable quotient package-review batch does not equal transactional source rederivation",
        )]);
    }
    if supplied.is_empty() {
        return Err(vec![Diagnostic::error(
            "non-executable quotient package review requires at least one total-direct `define` correspondence",
        )]);
    }

    let mut selected = Vec::new();
    let mut sources = Vec::new();
    for certificate in supplied {
        let machine = exact_operation(program, certificate)?;
        if program.symbols.symbol_package_identity(machine.symbol) != Some(package) {
            continue;
        }
        if certificate.operation_kind != QuotientCorrespondenceOperationKind::Define {
            return Err(vec![Diagnostic::error(
                "package review represents only the proof-only total-direct `Quotient::define` correspondence; lift remains unsupported",
            )]);
        }
        if !machine.is_public {
            return Err(vec![Diagnostic::error(
                "non-executable quotient package-review row names a private package callable",
            )]);
        }
        let [state] = program.machine_states(machine) else {
            return Err(vec![Diagnostic::error(
                "non-executable quotient package-review operation is not a one-state callable",
            )]);
        };
        if certificate.result_flow.state_position != 0 {
            return Err(vec![Diagnostic::error(
                "non-executable quotient package-review result names a noncanonical state position",
            )]);
        }
        let statement_position = usize::try_from(certificate.result_flow.statement_position)
            .map_err(|_| {
                vec![Diagnostic::error(
                    "non-executable quotient package-review statement position exceeds the host range",
                )]
            })?;
        let Some(StatementNode::Expression(expression)) = program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_position)
        else {
            return Err(vec![Diagnostic::error(
                "non-executable quotient package-review result does not name an exact expression statement",
            )]);
        };
        let ExpressionNode::Call(call) = program.expression_table.expression(*expression) else {
            return Err(vec![Diagnostic::error(
                "non-executable quotient package-review result does not name an exact call",
            )]);
        };
        if !matches!(
            call.quotient_operation.as_ref(),
            Some(request) if request.kind == QuotientOperationKind::Define
        ) {
            return Err(vec![Diagnostic::error(
                "non-executable quotient package-review source call is not an exact `Quotient::define` request",
            )]);
        }
        let location = canonical_typed_package_source_span_location(
            program,
            program
                .symbols
                .symbol_source_span(machine.symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "non-executable quotient package-review operation has no authored declaration span",
                    )]
                })?,
            PackageReviewSourceLocationRole::QuotientOperationDeclaration,
        )?;
        if location.owner() != crate::record::PackageReviewSourceLocationOwner::Package(package) {
            return Err(vec![Diagnostic::error(
                "non-executable quotient package-review source call belongs to another package",
            )]);
        }
        sources.push(PackageReviewCanonicalRowSource::authored(vec![location]));
        selected.push(certificate.clone());
    }

    if selected.is_empty() {
        return Err(vec![Diagnostic::error(
            "non-executable quotient package review contains no total-direct `define` correspondence owned by the requested package",
        )]);
    }

    Ok(NonExecutableQuotientPackageReview {
        package,
        target,
        correspondences: selected,
        row_sources: sources,
    })
}

fn exact_operation<'program>(
    program: &'program TypedTrees,
    certificate: &CanonicalQuotientCorrespondence,
) -> Result<&'program psi_typed_trees::machine::Machine, Vec<Diagnostic>> {
    let matches = program
        .machines()
        .iter()
        .filter(|machine| {
            program
                .normalized_hermetic_symbol_identity(machine.symbol)
                .is_ok_and(|identity| identity == certificate.public_operation.declaration)
                && program
                    .normalized_machine_overload_identity(machine)
                    .is_some_and(|identity| {
                        identity.identity() == certificate.public_operation.overload
                    })
        })
        .collect::<Vec<_>>();
    let [machine] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(
            "non-executable quotient package-review row does not resolve one exact callable",
        )]);
    };
    Ok(*machine)
}

#[cfg(test)]
mod tests;
