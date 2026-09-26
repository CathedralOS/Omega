use super::rejected;
use crate::package_evidence::capture::PackageReviewInput;
use crate::package_evidence::capture::api::operators::project_operator_coordinate;
use crate::package_evidence::capture::semantics::conformances::policy_callable_identity;
use crate::package_evidence::capture::semantics::declarations::{
    nominal_identity, reviewed_package_owns,
};
use crate::package_evidence::record::{
    PackagePolicyBoundaryApplicationDemand, PackageReviewSymbolicBoundaryApplicationArgument,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    package: PackageKeyIdentity,
) -> Result<Vec<PackagePolicyBoundaryApplicationDemand>, Vec<Diagnostic>> {
    // Reuse the complete authored-use, binder-category and source-owner joins.
    // Iterate the original exact handles afterward: the legacy display-nominal
    // row may deduplicate two overloads with the same authored machine name.
    super::super::symbolic_demands::project_boundary_application_demands(compilation, package)?;
    let mut rows = Vec::new();
    for demand in &compilation.facts.operators.symbolic_boundary_applications {
        let machine = compilation
            .machines()
            .iter()
            .find(|machine| machine.symbol == demand.machine_symbol)
            .ok_or_else(|| rejected("a symbolic producer without its exact machine"))?;
        if !machine.is_public
            || !reviewed_package_owns(&nominal_identity(compilation, machine.symbol)?, package)?
        {
            continue;
        }
        let operator =
            symbol_resolved_trees_to_typed_trees::typed_trees::operator::declaration_by_symbol(
                &compilation.typed,
                demand.requirement_symbol,
            )
            .ok_or_else(|| rejected("a symbolic demand without its operator"))?;
        let arguments = demand
            .arguments
            .iter()
            .map(|argument| {
                let typed_trees_to_checked_trees::checked_trees::CheckedSymbolicBoundaryOperatorApplicationArgument::TypeBinder {
                    binder_ordinal,
                    machine_binder_ordinal,
                    ..
                } = argument;
                PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                    requirement_binder_ordinal: *binder_ordinal,
                    producer_binder_ordinal: *machine_binder_ordinal,
                }
            })
            .collect();
        rows.push(PackagePolicyBoundaryApplicationDemand {
            operator_coordinate: project_operator_coordinate(compilation, operator)?,
            producer_callable: policy_callable_identity(compilation, machine.symbol)?,
            arguments,
        });
    }
    rows.sort();
    rows.dedup();
    Ok(rows)
}
