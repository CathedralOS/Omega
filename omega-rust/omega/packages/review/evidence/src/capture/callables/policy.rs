//! Complete checked callable policy for one root activation and target.

use super::surface;
use crate::capture::behavior::policy as behavior;
use crate::capture::semantics::conformances::policy_callable_identity;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::{
    PackagePolicyCallable, PackagePolicyCallableRole, PackagePolicyCallables,
    PackageReviewCallableRole, PackageReviewNominalOwner,
};
use omega_compiler::CheckedCompilation;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::MachineSupplyMode;

/// Capture semantics only; this neither accepts assumptions nor reconstitutes
/// compiler certificates, build replay, or native authority.
pub fn project_checked_callable_policy(
    compilation: &CheckedCompilation,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<PackagePolicyCallables, Vec<Diagnostic>> {
    if compilation.package_identity() != Some(package)
        || compilation.selected_target_profile() != Some(target)
        || compilation.selected_native_target() != Some(target.native_target())
    {
        return Err(rejected(
            "package or target differs from the checked root activation",
        ));
    }
    let build = compilation.selected_build_machine_symbol();
    let mut projected_build = false;
    let mut callables = Vec::new();
    let inferred_crash_causes = psi_typed_trees_to_checked_trees::infer_checked_crash_causes(
        &compilation.typed,
        &compilation.facts,
    );
    for machine in compilation.machines() {
        let role = if Some(machine.symbol) == build {
            PackagePolicyCallableRole::Build
        } else if !machine.is_public && machine.supply_mode == MachineSupplyMode::AdmissionClaim {
            PackagePolicyCallableRole::PrivateAssumption
        } else if !machine.is_public
            && matches!(
                machine.supply_mode,
                MachineSupplyMode::ExternalRealization { .. }
            )
        {
            PackagePolicyCallableRole::PrivateExternal
        } else if machine.supply_mode.is_boundary_declaration() {
            PackagePolicyCallableRole::Boundary
        } else if machine.is_public {
            PackagePolicyCallableRole::Public
        } else {
            continue;
        };
        let identity = nominal_identity(compilation, machine.symbol)?;
        match identity.owner {
            PackageReviewNominalOwner::Package(owner) if owner == package => {}
            PackageReviewNominalOwner::Package(_)
            | PackageReviewNominalOwner::ToolchainSource(_) => continue,
            PackageReviewNominalOwner::Unresolved => {
                return Err(rejected("selected callable has no exact source owner"));
            }
        }
        let review_role = match role {
            PackagePolicyCallableRole::Build => PackageReviewCallableRole::Build,
            PackagePolicyCallableRole::Boundary | PackagePolicyCallableRole::PrivateAssumption => {
                PackageReviewCallableRole::Boundary
            }
            PackagePolicyCallableRole::Public | PackagePolicyCallableRole::PrivateExternal => {
                PackageReviewCallableRole::Public
            }
        };
        let identity = policy_callable_identity(compilation, machine.symbol)?;
        let projected = surface::project(compilation, machine, review_role, identity, true)?;
        let capability_flows = behavior::capability_flows(compilation, projected.realized)?;
        let reachable_capability_flows =
            behavior::reachable_capability_flows(compilation, projected.realized)?;
        let mutation =
            behavior::mutation(compilation, machine, projected.entry, projected.realized)?;
        let checked_termination =
            behavior::termination(compilation, machine, projected.entry, projected.realized)?;
        let declared_termination = behavior::declared_termination(
            compilation,
            machine,
            projected.entry,
            projected.realized,
        )?;
        let checked_crash = behavior::crash(
            compilation,
            machine,
            projected.entry,
            &projected.binders,
            projected.realized,
            &inferred_crash_causes,
        )?;
        let surface = projected.surface;
        let return_type = projected
            .entry
            .return_type
            .is_valid()
            .then_some(surface.return_type);
        callables.push(PackagePolicyCallable {
            role,
            identity: surface.identity,
            supply: surface.supply,
            lifetime_parameter_count: surface.lifetime_parameter_count,
            type_parameters: surface.policy_type_parameters,
            conformance_bounds: surface.conformance_bounds,
            parameters: surface.parameters,
            conformances: surface.policy_conformances,
            operator_realizations: surface.operator_realizations,
            contracts: surface.contracts,
            declared_service_reach: surface.declared_service_reach,
            checked_service_reach: surface.checked_service_reach,
            unresolved_installation_reaches: surface.unresolved_installation_reaches,
            declared_synchronous_invocations: surface.declared_synchronous_invocations,
            realized_synchronous_invocations: surface.realized_synchronous_invocations,
            checked_may_suspend: surface.checked_may_suspend,
            checked_may_block: surface.checked_may_block,
            return_type,
            capability_flows,
            reachable_capability_flows,
            mutation,
            checked_termination,
            declared_termination,
            declared_may_suspend: surface.declared_may_suspend,
            declared_may_block: surface.declared_may_block,
            checked_crash,
        });
        projected_build |= role == PackagePolicyCallableRole::Build;
    }
    if build.is_some() && !projected_build {
        return Err(rejected(
            "selected build machine is not owned by the reviewed root package",
        ));
    }
    callables.sort_by(|left, right| {
        left.identity
            .cmp(&right.identity)
            .then(left.role.cmp(&right.role))
    });
    let policy = PackagePolicyCallables {
        package,
        target,
        callables,
    };
    policy.validate_canonical_structure().map_err(rejected)?;
    Ok(policy)
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "callable policy rejects {reason}"
    ))]
}
