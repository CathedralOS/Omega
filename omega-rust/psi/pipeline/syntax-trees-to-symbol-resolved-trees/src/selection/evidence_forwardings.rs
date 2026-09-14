//! Evidence forwardings bound to the exact machine, state, and source
//! conformance they name.
//!
//! Forwardings are recorded by root index and state name while machines
//! lower; once every state has a symbol they bind to it, and a source that
//! is not an incoming `requires` binding resolves to the subjectless
//! conformance alias it names.

use symbol_resolved_trees::SymbolResolvedTrees;

pub(crate) fn bind_evidence_forwarding_owners(program: &mut SymbolResolvedTrees) {
    let mut owners = Vec::new();
    let mut incoming_evidence_names = Vec::new();
    for (machine_root_index, machine) in program.machines.iter().enumerate() {
        incoming_evidence_names.extend(program.machine_contracts(machine).iter().filter_map(
            |contract| {
                (contract.kind == symbol_resolved_trees::signature::SignatureContractKind::Requires)
                    .then_some(contract.binding.as_ref())
                    .flatten()
                    .map(|binding| (machine.symbol, binding.as_str().to_owned()))
            },
        ));
        for state_handle in program.machine_state_handles(machine.states) {
            let state = program.machine_state(*state_handle);
            owners.push((
                machine_root_index,
                state.name.as_str().to_owned(),
                machine.symbol,
                state.symbol,
            ));
        }
    }
    let subjectless_conformances = program
        .conformances
        .iter()
        .filter_map(|conformance| {
            (matches!(
                conformance.subject,
                symbol_resolved_trees::trait_definition::ConformanceSubject::Subjectless
            ))
            .then(|| {
                conformance
                    .alias
                    .as_ref()
                    .map(|alias| (alias.as_str().to_owned(), conformance.symbol))
            })
            .flatten()
        })
        .collect::<Vec<_>>();
    for forwarding in &mut program.evidence_forwardings {
        if let Some((_, _, machine_symbol, state_symbol)) =
            owners
                .iter()
                .find(|(machine_root_index, state_name, _, _)| {
                    *machine_root_index == forwarding.machine_root_index
                        && state_name == forwarding.state_name.as_str()
                })
        {
            forwarding.machine_symbol = *machine_symbol;
            forwarding.state_symbol = *state_symbol;
        }
        if !incoming_evidence_names.iter().any(|(machine, name)| {
            *machine == forwarding.machine_symbol && name == forwarding.source.as_str()
        }) {
            forwarding.source_conformance =
                subjectless_conformances.iter().find_map(|(alias, symbol)| {
                    (alias == forwarding.source.as_str()).then_some(*symbol)
                });
        }
    }
}
