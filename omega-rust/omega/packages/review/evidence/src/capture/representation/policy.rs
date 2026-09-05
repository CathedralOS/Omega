//! Receipt-free representation declarations, availability, selections, and uses.

mod availability;
mod selections;

pub(crate) use selections::rederive_selections;

use crate::capture::calling::project_checked_calling_policy;
use crate::capture::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::record::{PackagePolicyRepresentation, PackagePolicyRepresentationDemand};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

/// Project one package's representation policy in the checked activation.
/// The package owns declarations/producer availability independently of use;
/// the selecting package owns activation selections and actual demands.
pub fn project_checked_representation_policy(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<PackagePolicyRepresentation, Vec<Diagnostic>> {
    if !compilation
        .dependency_closure()
        .is_some_and(|closure| closure.packages().contains(&package))
    {
        return Err(rejected(
            "a package outside the exact checked source closure",
        ));
    }
    let target = super::physical_contract::project_representation_target(compilation)?;
    let selections = rederive_selections(compilation)?;
    let mut declarations = Vec::new();
    for definition in compilation.data_definitions().iter().filter(|definition| {
        definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque
    }) {
        let identity = nominal_identity(compilation, definition.symbol)?;
        if reviewed_package_owns(&identity, package)? {
            declarations.push(identity);
        }
    }
    declarations.sort();
    if declarations.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(rejected("opaque declarations repeat one exact identity"));
    }
    let producer_availability = availability::project(compilation, package)?;
    let selected_availability = selections::project(compilation, package, &selections)?;
    let mut demands = Vec::new();
    for realization in compilation.boundary_calling_plan_realizations() {
        let uses = realization
            .materialized_signature()
            .opaque_representation_uses();
        if uses.is_empty() || selected_availability.is_empty() {
            continue;
        }
        let calling = project_checked_calling_policy(compilation, realization)?;
        for selection in &selected_availability {
            let matches = calling
                .opaque_uses()
                .iter()
                .filter(|use_| use_.opaque() == &selection.opaque)
                .collect::<Vec<_>>();
            let [use_] = matches.as_slice() else {
                if matches.is_empty() {
                    continue;
                }
                return Err(rejected("demand has ambiguous exact opaque uses"));
            };
            if use_.carrier() != &selection.carrier
                || use_.selection_owner() != selection.selection_owner
                || use_.application() != &selection.application
                || use_.origin() != selection.origin
                || use_.lifecycle() != selection.lifecycle
                || use_.copy_disposition() != selection.copy_disposition
            {
                return Err(rejected(
                    "demand differs from the complete selected application",
                ));
            }
            demands.push(PackagePolicyRepresentationDemand {
                opaque: selection.opaque.clone(),
                calling: calling.clone(),
            });
        }
    }
    demands.sort_by(PackagePolicyRepresentationDemand::compare_application);
    demands.dedup();
    let policy = PackagePolicyRepresentation {
        package,
        target,
        declarations,
        producer_availability,
        selected_availability,
        demands,
    };
    policy.validate_canonical_structure().map_err(rejected)?;
    Ok(policy)
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "representation policy cannot retain {reason}"
    ))]
}
