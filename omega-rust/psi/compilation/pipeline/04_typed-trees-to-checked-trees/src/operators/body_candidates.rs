//! Complete boundary-body candidates while checking owns declarations and
//! specialization judgments. Lowering publishes these facts without selecting
//! providers or re-running conformance discovery.

use arena::{Arena, HandleSpan};
use checked_trees::{
    CheckedBoundaryBodyCandidate, CheckedBoundaryOperatorApplicationDemand,
    CheckedBoundaryOperatorDeclaration, CheckedTrees,
};

pub(crate) fn retain_boundary_body_candidates(
    checked: &mut CheckedTrees,
) -> Result<(), &'static str> {
    checked.facts.operators.boundary_declarations = declarations(checked)?;
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

fn declarations(
    checked: &CheckedTrees,
) -> Result<Arena<CheckedBoundaryOperatorDeclaration>, &'static str> {
    let typed = &checked.typed;
    let mut declarations = Arena::new();
    let in_product = |symbol| {
        typed
            .symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| typed.symbols.source_file(span))
            .is_none_or(|file| file.dependency_scope == source::DependencyScope::Product)
    };
    for machine in typed.machines().iter().filter(|machine| {
        machine.supply_mode.is_checked_body()
            && !machine.target_family_interface
            && !typed
                .machine_specializations
                .iter()
                .any(|row| row.instance == machine.symbol)
    }) {
        let conformances = typed.machine_trait_conformances(machine);
        let is_product = in_product(machine.symbol)
            && (!machine.attached_data_symbol.is_valid()
                || in_product(machine.attached_data_symbol))
            && conformances
                .iter()
                .all(|edge| in_product(edge.symbol) && in_product(edge.requirement_symbol));
        for conformance in conformances.iter()
        {
            let Some(operator) =
                typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                    typed,
                    machine,
                    conformance,
                )
            else {
                continue;
            };
            // Token-bearing requirements have a signature view, not an operator
            // provider declaration. Their ordinary requirement catalog owns them.
            if typed.machine(operator.symbol).is_some() {
                continue;
            }
            let callable = typed
                .machine_callable_identity_text(machine)
                .ok_or("checked operator declaration lost its callable identity")?;
            let provider = typed.attached_data_path(machine).unwrap_or_default();
            let provider_package = typed
                .data_definition_by_symbol(machine.attached_data_symbol)
                .filter(|owner| machine.attached_data.as_ref() == Some(&owner.name))
                .and_then(|owner| typed.symbols.symbol_package_identity(owner.symbol));
            declarations.append(CheckedBoundaryOperatorDeclaration {
                requirement: operator.symbol,
                machine: machine.symbol,
                selection: language_semantics::BoundaryBodySelection {
                    requirement: typed_trees::operator::boundary_operator_requirement_identity(
                        typed, operator,
                    ),
                    requirement_package: typed.symbols.symbol_package_identity(operator.symbol),
                    callable,
                    package: typed.symbols.symbol_package_identity(machine.symbol),
                    provider,
                    provider_package,
                    // Operator declarations have no selecting trait-map capture
                    // or trait-region telescope. Their source plan producer emits
                    // the same empty coordinates; a different selected row cannot
                    // silently acquire them from this declaration.
                    provider_application: None,
                    requirement_lifetime_partition: Vec::new(),
                    target: typed.source_target_context(machine).map(str::to_owned),
                    is_product,
                },
            });
        }
    }
    Ok(declarations)
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
            machine
        };
        let mut declarations =
            checked
                .facts
                .operators
                .boundary_declarations
                .iter()
                .filter(|(_, declaration)| {
                    declaration.machine == selection.symbol
                        && declaration.requirement == demand.requirement_symbol
                });
        let Some((declaration, _)) = declarations.next() else {
            continue;
        };
        if declarations.next().is_some() {
            return Err("checked operator body has competing declaration relationships");
        }
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
            declaration,
            is_product: typed
                .symbols
                .symbol_provenance_source_span(machine.symbol)
                .and_then(|span| typed.symbols.source_file(span))
                .is_none_or(|file| file.dependency_scope == source::DependencyScope::Product)
                && typed.symbols.symbol_package_identity(machine.symbol)
                    == typed.symbols.symbol_package_identity(selection.symbol),
            selection: selection.symbol,
            selection_identity: typed.machine_callable_identity(selection),
            target: typed.source_target_context(machine).map(str::to_owned),
            machine: machine.symbol,
            entry: entry.symbol,
            contract: contract.commitment.as_bytes(),
            specialization,
        });
    }
    Ok(candidates)
}
