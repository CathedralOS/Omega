//! Complete boundary-body candidates while checking owns declarations and
//! specialization judgments. Lowering publishes these facts without selecting
//! providers or re-running conformance discovery.

use arena::{Arena, HandleSpan};
use checked_trees::{
    CheckedBoundaryBodyCandidate, CheckedBoundaryOperatorApplicationDemand, CheckedTrees,
};

pub(crate) fn retain_boundary_body_candidates(
    checked: &mut CheckedTrees,
) -> Result<(), &'static str> {
    let demands = &checked.facts.operators.boundary_applications;
    let mut bodies = Arena::new();
    let mut spans: Vec<HandleSpan<CheckedBoundaryBodyCandidate>> =
        Vec::with_capacity(demands.len());
    let mut distinct = Vec::<usize>::new();
    for (ordinal, demand) in demands.iter().enumerate() {
        // Occurrences are distinct, but identical applications share the same
        // body set. Do not repeat specialization work or copy its stored rows.
        if let Some(previous) = distinct.iter().copied().find(|previous| {
            demands[*previous].requirement_symbol == demand.requirement_symbol
                && demands[*previous].arguments == demand.arguments
        }) {
            spans.push(spans[previous]);
            continue;
        }
        let candidates = candidates_for_application(checked, demand)?;
        spans.push(bodies.insert_many(candidates));
        distinct.push(ordinal);
    }
    for (demand, span) in checked
        .facts
        .operators
        .boundary_applications
        .iter_mut()
        .zip(spans)
    {
        demand.body_candidates = span;
    }
    checked.facts.operators.boundary_body_candidates = bodies;
    Ok(())
}

fn candidates_for_application(
    checked: &CheckedTrees,
    demand: &CheckedBoundaryOperatorApplicationDemand,
) -> Result<Vec<CheckedBoundaryBodyCandidate>, &'static str> {
    let typed = &checked.typed;
    let mut candidates = Vec::new();
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
            let satisfies = typed
                .machine_trait_conformances(machine)
                .iter()
                .any(|conformance| {
                    typed_trees::operator::resolve_satisfied_checked_operator_for_conformance(
                        typed,
                        machine,
                        conformance,
                    )
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
        let specialization = specialization
            .map(|row| {
                let identity = checked
                    .facts
                    .specialization_identities
                    .for_instance(row.instance)
                    .ok_or("boundary candidate has no checked specialization")?;
                Ok::<_, &'static str>((row.template, identity.commitment.as_bytes()))
            })
            .transpose()?;
        candidates.push(CheckedBoundaryBodyCandidate {
            selection: selection.symbol,
            selection_identity: typed.machine_callable_identity(selection),
            target: machine
                .target
                .as_ref()
                .map(|target| target.as_str().to_owned()),
            machine: machine.symbol,
            entry: entry.symbol,
            contract: contract.commitment.as_bytes(),
            specialization,
        });
    }
    Ok(candidates)
}
