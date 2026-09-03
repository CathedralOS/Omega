//! Artifact-qualified D29 symbolic-demand review projection.
//!
//! These rows preserve open generic applications exported by public package
//! callables. They are composition inputs only: no row in this module claims a
//! selected realization, semantic coverage, Terminal custody, or native code.

use super::super::api::operators::project_operator_coordinate;
use super::super::semantics::declarations::{nominal_identity, reviewed_package_owns};
use super::super::source::locations::canonical_source_span_location;
use super::application_realizations::authored_application_source_span;
use crate::record::{
    CheckedPackageBoundaryApplicationDemandReview, PackageReviewCanonicalRowSource,
    PackageReviewSourceLocation, PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
    PackageReviewSymbolicBoundaryApplicationArgument,
};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(crate) struct ProjectedBoundaryApplicationDemands {
    pub(crate) rows: Vec<CheckedPackageBoundaryApplicationDemandReview>,
    pub(crate) sources: Vec<PackageReviewCanonicalRowSource>,
}

struct StagedDemand {
    row: CheckedPackageBoundaryApplicationDemandReview,
    locations: Vec<PackageReviewSourceLocation>,
}

pub(crate) fn project_boundary_application_demands(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<ProjectedBoundaryApplicationDemands, Vec<Diagnostic>> {
    let mut staged: Vec<StagedDemand> = Vec::new();
    for demand in &compilation.facts.operators.symbolic_boundary_applications {
        let Some(operator) = psi_typed_trees::operator::declaration_by_symbol(
            &compilation.typed,
            demand.requirement_symbol,
        )
        .filter(|operator| operator.is_boundary) else {
            return Err(vec![Diagnostic::error(
                "symbolic boundary application lost its boundary operator declaration",
            )]);
        };
        let Some(machine) = compilation
            .machines()
            .iter()
            .find(|machine| machine.symbol == demand.machine_symbol)
        else {
            return Err(vec![Diagnostic::error(
                "symbolic boundary application lost its producer machine",
            )]);
        };
        let producer_callable = nominal_identity(compilation, machine.symbol)?;
        if !reviewed_package_owns(&producer_callable, package)? {
            continue;
        }
        if !machine.is_public {
            continue;
        }

        let operator_parameters = compilation.operator_type_parameters(operator);
        let machine_parameters = compilation.machine_type_parameters(machine);
        if operator_parameters.len() != demand.arguments.len() || demand.arguments.is_empty() {
            return Err(vec![Diagnostic::error(
                "symbolic boundary application does not match its operator telescope",
            )]);
        }
        let mut arguments = Vec::with_capacity(demand.arguments.len());
        for (ordinal, (argument, operator_parameter)) in
            demand.arguments.iter().zip(operator_parameters).enumerate()
        {
            let expected_ordinal = u32::try_from(ordinal).map_err(|_| {
                vec![Diagnostic::error(
                    "symbolic boundary application exceeds the review ordinal range",
                )]
            })?;
            let psi_checked_trees::CheckedSymbolicBoundaryOperatorApplicationArgument::TypeBinder {
                binder_owner,
                binder_ordinal,
                binder_symbol,
                machine_binder_ordinal,
                machine_binder_symbol,
            } = argument;
            let machine_parameter = usize::try_from(*machine_binder_ordinal)
                .ok()
                .and_then(|ordinal| machine_parameters.get(ordinal));
            if *binder_owner != operator.symbol
                || *binder_ordinal != expected_ordinal
                || *binder_symbol != operator_parameter.symbol
                || !matches!(
                    operator_parameter.kind,
                    psi_typed_trees::data::TypeParameterKind::Type
                )
                || machine_parameter.is_none_or(|parameter| {
                    parameter.symbol != *machine_binder_symbol
                        || !matches!(
                            parameter.kind,
                            psi_typed_trees::data::TypeParameterKind::Type
                        )
                })
            {
                return Err(vec![Diagnostic::error(
                    "symbolic boundary application does not rejoin its operator and producer binders",
                )]);
            }
            arguments.push(
                PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                    requirement_binder_ordinal: *binder_ordinal,
                    producer_binder_ordinal: *machine_binder_ordinal,
                },
            );
        }

        let psi_checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
            expression,
            origin,
        } = demand.site
        else {
            return Err(vec![Diagnostic::error(
                "symbolic boundary application has an unsupported statement use site",
            )]);
        };
        if origin.machine_symbol() != Some(machine.symbol) {
            return Err(vec![Diagnostic::error(
                "symbolic boundary application use does not belong to its producer callable",
            )]);
        }
        let location = canonical_source_span_location(
            compilation,
            authored_application_source_span(
                compilation,
                expression,
                omega_selected_dispatch::CheckedOperatorAuthoredUseKind::Named,
                operator.symbol,
            )?,
            PackageReviewSourceLocationRole::BoundaryApplicationUse,
        )?;
        if location.owner != PackageReviewSourceLocationOwner::Package(package) {
            return Err(vec![Diagnostic::error(
                "symbolic boundary application producer and authored use have different package owners",
            )]);
        }

        let requirement_identity =
            psi_typed_trees::operator::boundary_operator_requirement_identity(
                &compilation.typed,
                operator,
            );
        if requirement_identity.is_empty() {
            return Err(vec![Diagnostic::error(
                "symbolic boundary application has no stable requirement identity",
            )]);
        }
        let row = CheckedPackageBoundaryApplicationDemandReview {
            requirement_identity,
            operator_coordinate: project_operator_coordinate(compilation, operator)?,
            producer_callable,
            arguments,
        };
        if let Some(existing) = staged.iter_mut().find(|existing| existing.row == row) {
            existing.locations.push(location);
        } else {
            staged.push(StagedDemand {
                row,
                locations: vec![location],
            });
        }
    }

    staged.sort_by(|left, right| demand_key(&left.row).cmp(&demand_key(&right.row)));
    let mut rows = Vec::with_capacity(staged.len());
    let mut sources = Vec::with_capacity(staged.len());
    for mut demand in staged {
        demand.locations.sort();
        demand.locations.dedup();
        rows.push(demand.row);
        sources.push(PackageReviewCanonicalRowSource::authored(demand.locations));
    }
    Ok(ProjectedBoundaryApplicationDemands { rows, sources })
}

fn demand_key(
    row: &CheckedPackageBoundaryApplicationDemandReview,
) -> (
    &crate::record::PackageReviewOperatorCoordinate,
    &str,
    &crate::record::PackageReviewNominalIdentity,
    &[PackageReviewSymbolicBoundaryApplicationArgument],
) {
    (
        &row.operator_coordinate,
        row.requirement_identity.as_str(),
        &row.producer_callable,
        &row.arguments,
    )
}
