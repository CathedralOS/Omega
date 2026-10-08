//! Retain checked-body choices before discarding source identities. This pass
//! records conformance and specialization facts, never a target's selection.

use checked_trees::{CheckedBoundaryOperatorApplicationDemand, CheckedTrees};
use terminal_psi::{
    BoundaryApplication, BoundaryApplicationCandidate, BoundaryBodySpecialization,
    BoundaryCheckedBody, BoundaryNominalIdentity, BoundaryOperatorRequirement,
};

pub(super) fn retain(
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    demand: &CheckedBoundaryOperatorApplicationDemand,
    requirement: &BoundaryOperatorRequirement,
    application: &BoundaryApplication,
    output: &mut Vec<BoundaryApplicationCandidate>,
) -> Result<(), &'static str> {
    let typed = &checked.typed;
    for machine in typed.machines().iter().filter(|machine| {
        machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
            && typed.machine_type_parameters(machine).is_empty()
            && machine.lifetime_parameters.is_empty()
    }) {
        let specialization = typed
            .machine_specializations
            .iter()
            .find(|row| row.instance == machine.symbol);
        let selection = if let Some(specialization) = specialization {
            if !specialization.operator_realizations.iter().any(|row| {
                row.requirement_symbol == demand.requirement_symbol
                    && validation::checked_operator_application_matches_realization(
                        typed, demand, row,
                    )
            }) {
                continue;
            }
            typed
                .machine_by_symbol(specialization.template)
                .ok_or("boundary candidate specialization lost its template")?
        } else {
            if !demand.arguments.is_empty() {
                continue;
            }
            let satisfies = typed.machine_trait_conformances(machine).iter().any(|conformance| {
                conformance.external_binding.is_none()
                    && typed_trees::operator::resolve_satisfied_checked_operator_for_conformance(typed, machine, conformance)
                        .is_some_and(|operator| operator.symbol == demand.requirement_symbol)
            });
            if !satisfies {
                continue;
            }
            machine
        };
        let entry = typed
            .machine_states(machine)
            .first()
            .ok_or("boundary candidate has no entry")?;
        let contract = checked
            .facts
            .contract_plans
            .machines
            .iter()
            .find(|row| row.machine == machine.symbol)
            .ok_or("boundary candidate has no checked contract")?;
        if contract.commitment.is_zero() {
            return Err("boundary candidate has an empty checked contract");
        }
        let nominal = |symbol| {
            checked
                .exact_owner_nominal_identity(symbol, sources)
                .ok_or("boundary candidate has no exact declaration owner")
                .and_then(|identity| BoundaryNominalIdentity::new(identity.into_string()))
        };
        let specialization = specialization
            .map(|row| {
                let checked_identity = checked
                    .facts
                    .specialization_identities
                    .for_instance(row.instance)
                    .ok_or("boundary candidate has no checked specialization")?;
                Ok::<_, &'static str>(BoundaryBodySpecialization {
                    template: nominal(row.template)?,
                    commitment: checked_identity.commitment.as_bytes(),
                })
            })
            .transpose()?;
        output.push(BoundaryApplicationCandidate {
            requirement: requirement.clone(),
            application: application.clone(),
            selection_identity: typed
                .machine_callable_identity_text(selection)
                .ok_or("boundary candidate has no callable identity")?,
            selection_package: checked.symbols.symbol_package_identity(selection.symbol),
            target: machine
                .target
                .as_ref()
                .map(|target| target.as_str().to_owned()),
            body: BoundaryCheckedBody {
                machine: nominal(machine.symbol)?,
                state: nominal(entry.symbol)?,
                contract: contract.commitment.as_bytes(),
                specialization,
            },
        });
    }
    Ok(())
}
