//! Reharvest the complete authoritative activation before projecting any choice.

use super::rejected;
use crate::capture::representation::physical_contract::{
    project_representation_copy_disposition, project_representation_lifecycle,
    project_representation_origin,
};
use crate::capture::semantics::conformances::project_checked_conformance_policy;
use crate::capture::semantics::declarations::{nominal_identity, nominal_owner};
use crate::record::{PackagePolicyRepresentationSelection, PackageReviewNominalOwner};
use omega_compiler::CheckedCompilation;
use omega_representation_planning::OpaqueRepresentationSelection;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(crate) fn rederive_selections(
    compilation: &CheckedCompilation,
) -> Result<Vec<OpaqueRepresentationSelection>, Vec<Diagnostic>> {
    let selected = compilation.selected_build_machine_symbol();
    if let Some(symbol) = selected {
        let machines = compilation
            .machines()
            .iter()
            .filter(|machine| machine.symbol == symbol)
            .collect::<Vec<_>>();
        let [machine] = machines.as_slice() else {
            return Err(rejected(
                "selection lacks its unique authoritative build machine",
            ));
        };
        if !machine.lifetime_parameters.is_empty()
            || !compilation.machine_type_parameters(machine).is_empty()
        {
            return Err(rejected(
                "authoritative build has a nonempty static or lifetime telescope",
            ));
        }
    }
    // Use the independently retained selected build even when the selection
    // collection is empty: otherwise omission of an unused choice is invisible.
    let derived = omega_representation_planning::rederive_opaque_representation_selections(
        &compilation.typed,
        selected,
        compilation.opaque_representation_selections(),
    )?;
    if derived
        .iter()
        .any(|selection| !selection.application().lifetime_arguments.is_empty())
    {
        return Err(rejected(
            "selected application has no closed build lifetime context",
        ));
    }
    Ok(derived)
}

pub(super) fn project(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
    selections: &[OpaqueRepresentationSelection],
) -> Result<Vec<PackagePolicyRepresentationSelection>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for selection in selections {
        let selection_owner = nominal_owner(compilation, selection.selecting_machine())?;
        if selection_owner == PackageReviewNominalOwner::Unresolved {
            return Err(rejected("an unresolved selecting package"));
        }
        if selection_owner != PackageReviewNominalOwner::Package(package) {
            continue;
        }
        rows.push(PackagePolicyRepresentationSelection {
            opaque: nominal_identity(compilation, selection.opaque())?,
            carrier: nominal_identity(compilation, selection.carrier())?,
            selection_owner,
            application: project_checked_conformance_policy(
                compilation,
                selection.application(),
                &[],
            )?,
            origin: project_representation_origin(selection.origin()),
            lifecycle: project_representation_lifecycle(selection.lifecycle()),
            copy_disposition: project_representation_copy_disposition(selection.copy_disposition()),
        });
    }
    rows.sort();
    Ok(rows)
}
