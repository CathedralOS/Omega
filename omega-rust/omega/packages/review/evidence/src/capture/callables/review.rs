use super::super::behavior::{
    project_capability_flow, project_crash, project_mutation, project_termination,
};
use super::super::contracts::facts::project_callable_contract_entailment_stand_down;
use super::super::semantics::signatures::parameters::project_type_parameters;
use super::conformances::project_callable_conformances;
use super::signatures::project_external_callable_signature;
use crate::capture::source::ProjectedReviewRow;
use crate::record::{
    CheckedPackageCallableReview, PackageReviewCallableRole, PackageReviewExternalExecutableSupply,
    PackageReviewNominalIdentity,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(in crate::capture) fn project_callable(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    role: PackageReviewCallableRole,
    identity: PackageReviewNominalIdentity,
) -> Result<
    (
        CheckedPackageCallableReview,
        Vec<ProjectedReviewRow<PackageReviewExternalExecutableSupply>>,
    ),
    Vec<Diagnostic>,
> {
    let projected = super::surface::project(compilation, machine, role, identity, false)?;
    let surface = projected.surface;
    let realized = projected.realized;
    let mut capability_flows = realized
        .capabilities
        .iter()
        .map(|flow| project_capability_flow(compilation, flow))
        .collect::<Result<Vec<_>, _>>()?;
    capability_flows.sort_by(|left, right| {
        left.capability
            .cmp(&right.capability)
            .then(left.kind.as_str().cmp(right.kind.as_str()))
            .then(left.state.cmp(&right.state))
            .then(left.statement_index.cmp(&right.statement_index))
            .then(left.call_ordinal.cmp(&right.call_ordinal))
            .then(left.via_state.cmp(&right.via_state))
    });
    Ok((
        CheckedPackageCallableReview {
            role,
            identity: surface.identity,
            supply: surface.supply,
            lifetime_parameter_count: surface.lifetime_parameter_count,
            type_parameters: surface.type_parameters,
            conformance_bounds: surface.conformance_bounds,
            parameters: surface.parameters,
            return_type: surface.return_type,
            conformances: surface.conformances,
            operator_realizations: surface.operator_realizations,
            contracts: surface.contracts,
            declared_service_reach: surface.declared_service_reach,
            checked_service_reach: surface.checked_service_reach,
            unresolved_installation_reaches: surface.unresolved_installation_reaches,
            declared_synchronous_invocations: surface.declared_synchronous_invocations,
            realized_synchronous_invocations: surface.realized_synchronous_invocations,
            checked_may_suspend: surface.checked_may_suspend,
            checked_may_block: surface.checked_may_block,
            capability_flows,
            checked_termination: project_termination(compilation, &realized.checked_termination)?,
            checked_crash: project_crash(compilation, &realized.checked_crash)?,
            mutation: project_mutation(compilation, &realized.mutation)?,
        },
        projected.external_executable_supply,
    ))
}

pub(in crate::capture) fn project_contract_entailment_open_contract(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    contract_index: usize,
    fact_index: usize,
) -> Result<crate::record::PackageReviewCallableContract, Vec<Diagnostic>> {
    let Some(entry) = compilation.machine_states(machine).first() else {
        return Err(vec![Diagnostic::error(
            "contract-entailment stand-down callable has no canonical entry signature",
        )]);
    };
    let (binders, _) = project_type_parameters(
        compilation,
        compilation.machine_type_parameters(machine),
        "callable",
        machine.name.as_str(),
        &machine.lifetime_parameters,
    )?;
    project_callable_contract_entailment_stand_down(
        compilation,
        machine,
        entry,
        &binders,
        contract_index,
        fact_index,
    )
}

pub(in crate::capture) fn project_private_external_executable_supply(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    identity: &PackageReviewNominalIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewExternalExecutableSupply>>, Vec<Diagnostic>> {
    let machine_type_parameters = compilation.machine_type_parameters(machine);
    let (binders, _) = project_type_parameters(
        compilation,
        machine_type_parameters,
        "external executable supply",
        identity.path.as_str(),
        &machine.lifetime_parameters,
    )?;
    let signature = project_external_callable_signature(compilation, machine, &binders)?;
    let (_, _, supply) = project_callable_conformances(
        compilation,
        machine,
        identity,
        &binders,
        Some(&signature),
        false,
        None,
    )?;
    Ok(supply)
}
