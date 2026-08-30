use super::super::callables::{project_callable, project_private_external_executable_supply};
use super::super::semantics::declarations::nominal_identity;
use super::super::source::contracts::project_contract_source_locations;
use super::super::source::invocations::project_machine_invocation_source_locations;
use super::super::source::parameters::{
    collect_callable_parameter_source_locations, collect_type_parameter_source_locations,
};
use super::super::source::service_reach::project_machine_service_reach_source_locations;
use super::super::source::suspension::project_machine_operational_source_locations;
use crate::capture::source::{ProjectedNestedSourceLocation, ProjectedReviewRow};
use crate::record::{
    CheckedPackageCallableReview, PackageReviewCallableRole, PackageReviewExternalExecutableSupply,
    PackageReviewNominalOwner, PackageReviewSourceLocationRole,
};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::MachineSupplyMode;

pub(super) struct ProjectedPackageCallables {
    pub(super) callables: Vec<ProjectedReviewRow<CheckedPackageCallableReview>>,
    pub(super) external_executable_supply:
        Vec<ProjectedReviewRow<PackageReviewExternalExecutableSupply>>,
}

pub(super) fn project_package_callables(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<ProjectedPackageCallables, Vec<Diagnostic>> {
    let build_machine = compilation.selected_build_machine_symbol();
    let mut callables = Vec::new();
    let mut external_executable_supply = Vec::new();
    let mut projected_build_machine = false;

    for machine in compilation.machines() {
        let role = if Some(machine.symbol) == build_machine {
            Some(PackageReviewCallableRole::Build)
        } else if machine.supply_mode.is_boundary_declaration() {
            Some(PackageReviewCallableRole::Boundary)
        } else if machine.is_public {
            Some(PackageReviewCallableRole::Public)
        } else {
            None
        };
        let Some(role) = role else {
            continue;
        };
        let owner = nominal_identity(compilation, machine.symbol)?;
        match owner.owner {
            PackageReviewNominalOwner::Package(owner) if owner == package => {}
            PackageReviewNominalOwner::Package(_)
            | PackageReviewNominalOwner::ToolchainSource(_) => continue,
            PackageReviewNominalOwner::Unresolved => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` has no managed package owner",
                    owner.path
                ))]);
            }
        }

        let (callable, executable_supply) = project_callable(compilation, machine, role, owner)?;
        let mut contract_locations =
            project_contract_source_locations(compilation, compilation.machine_contracts(machine))?;
        contract_locations.extend(project_machine_invocation_source_locations(
            compilation,
            machine,
        )?);
        contract_locations.extend(project_machine_service_reach_source_locations(
            compilation,
            machine,
        )?);
        contract_locations.extend(project_machine_operational_source_locations(
            compilation,
            machine,
        )?);
        collect_type_parameter_source_locations(
            compilation,
            compilation.machine_type_parameters(machine),
            &mut contract_locations,
        )?;
        let entry = compilation.machine_states(machine).first().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{}` has no canonical entry signature",
                compilation.typed.symbols.display_path(machine.symbol, "::")
            ))]
        })?;
        collect_callable_parameter_source_locations(
            compilation,
            compilation.state_parameters(entry),
            "reviewed callable parameter",
            &mut contract_locations,
        )?;
        contract_locations.extend(
            psi_typed_trees_to_checked_trees::derive_checked_body_call_source_spans(
                &compilation.typed,
                &compilation.facts,
                machine.symbol,
            )?
            .into_iter()
            .map(|source_span| ProjectedNestedSourceLocation {
                source_span,
                role: PackageReviewSourceLocationRole::BodyCall,
            }),
        );
        external_executable_supply.extend(executable_supply);
        callables.push(ProjectedReviewRow {
            row: callable,
            declaration: machine.symbol,
            nested_source_locations: contract_locations,
        });
        projected_build_machine |= role == PackageReviewCallableRole::Build;
    }

    project_private_external_supply(
        compilation,
        package,
        build_machine,
        &mut external_executable_supply,
    )?;

    if build_machine.is_some() && !projected_build_machine {
        return Err(vec![Diagnostic::error(
            "selected build machine is not owned by the reviewed root package",
        )]);
    }

    callables.sort_by(|left, right| {
        left.row
            .identity
            .cmp(&right.row.identity)
            .then(left.row.role.cmp(&right.row.role))
            .then(left.row.contracts.cmp(&right.row.contracts))
    });
    external_executable_supply.sort_by(|left, right| left.row.cmp(&right.row));
    if external_executable_supply
        .windows(2)
        .any(|rows| rows[0].row == rows[1].row)
    {
        return Err(vec![Diagnostic::error(
            "package review contains a duplicate exact external executable-supply row",
        )]);
    }

    Ok(ProjectedPackageCallables {
        callables,
        external_executable_supply,
    })
}

fn project_private_external_supply(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
    build_machine: Option<psi_symbols::SymbolHandle>,
    projected: &mut Vec<ProjectedReviewRow<PackageReviewExternalExecutableSupply>>,
) -> Result<(), Vec<Diagnostic>> {
    // External executable supply is trust-bearing even when the leaf is a
    // private implementation detail. Public/build leaves were projected with
    // their callable envelopes above; project every remaining package-owned
    // external leaf without manufacturing a public callable row.
    for machine in compilation.machines() {
        if !matches!(
            machine.supply_mode,
            MachineSupplyMode::ExternalRealization { .. }
        ) || machine.is_public
            || Some(machine.symbol) == build_machine
        {
            continue;
        }
        let owner = nominal_identity(compilation, machine.symbol)?;
        match owner.owner {
            PackageReviewNominalOwner::Package(owner_package) if owner_package == package => {}
            PackageReviewNominalOwner::Package(_)
            | PackageReviewNominalOwner::ToolchainSource(_) => continue,
            PackageReviewNominalOwner::Unresolved => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has no managed package owner",
                    owner.path
                ))]);
            }
        }
        projected.extend(project_private_external_executable_supply(
            compilation,
            machine,
            &owner,
        )?);
    }
    Ok(())
}
