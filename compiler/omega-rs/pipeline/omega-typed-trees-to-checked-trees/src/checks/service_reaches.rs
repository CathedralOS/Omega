use omega_core::diagnostics::Diagnostic;

pub(super) fn check_service_reach_ceilings(
    program: &omega_typed_trees::TypedTrees,
    facts: &omega_checked_trees::CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        let publishes = machine.supply_mode
            != omega_core::semantics::MachineSupplyMode::CheckedBody
            || !program
                .service_reach_rows
                .services(machine.service_reach_row)
                .is_empty();
        if !publishes {
            continue;
        }
        let reaches = &facts.effect_rows.service_reaches;
        let Some(reach) = reaches.for_machine(machine.symbol) else {
            continue;
        };
        let published = reaches.rows.services(reach.published_ceiling);
        let inferred = reaches.rows.services(reach.inferred_transitive);
        let missing = inferred
            .iter()
            .copied()
            .filter(|service| !published.contains(service))
            .collect::<Vec<_>>();
        if missing.is_empty() {
            continue;
        }
        let names = missing
            .iter()
            .filter_map(|service| reaches.services.definition(*service))
            .map(|definition| definition.name.as_str())
            .collect::<Vec<_>>()
            .join(" + ");
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` publishes service reach `{}` but its checked body reaches undeclared service{} `{}`",
            machine.name,
            format_row(facts, published),
            if missing.len() == 1 { "" } else { "s" },
            names,
        )));
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn format_row(
    facts: &omega_checked_trees::CheckFacts,
    row: &[omega_core::semantics::ServiceReachId],
) -> String {
    let names = row
        .iter()
        .filter_map(|service| {
            facts
                .effect_rows
                .service_reaches
                .services
                .definition(*service)
        })
        .map(|definition| definition.name.as_str())
        .collect::<Vec<_>>();
    if names.is_empty() {
        "<none>".to_owned()
    } else {
        names.join(" + ")
    }
}
