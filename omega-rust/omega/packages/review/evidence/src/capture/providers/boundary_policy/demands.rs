use super::rejected;
use crate::capture::api::operators::project_operator_coordinate;
use crate::capture::semantics::conformances::policy_callable_identity;
use crate::capture::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::record::{
    PackagePolicyBoundaryApplicationDemand, PackageReviewSymbolicBoundaryApplicationArgument,
};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(super) fn project(
    compilation: &CheckedCompilation,
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
        let operator = psi_typed_trees::operator::declaration_by_symbol(
            &compilation.typed,
            demand.requirement_symbol,
        )
        .ok_or_else(|| rejected("a symbolic demand without its operator"))?;
        let arguments = demand.arguments.iter().map(|argument| {
            let psi_checked_trees::CheckedSymbolicBoundaryOperatorApplicationArgument::TypeBinder { binder_ordinal, machine_binder_ordinal, .. } = argument;
            PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder { requirement_binder_ordinal: *binder_ordinal, producer_binder_ordinal: *machine_binder_ordinal }
        }).collect();
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
