//! Record the closed boundary requirement each concrete provider refines
//! (`checked_trees::CheckedProviderClosedRequirement`).
//!
//! Checking already admitted every provider conformance against the closed
//! application its specialization receipt selects
//! (`validate_machine_top_level_requirement_conformance`). The same join runs
//! once here over the finalized program so lowering's provider catalog reads
//! the selected requirement instead of re-entering the resolution.

use checked_trees::CheckedProviderClosedRequirement;
use language_semantics::MachineSupplyMode;

pub(crate) fn record_provider_closed_requirements(
    program: &dyn validation::CheckingInput,
) -> Vec<CheckedProviderClosedRequirement> {
    // Built on the first generic template: most programs name none. An
    // invalid declaration identity was reported when checking resolved the
    // same symbols; the rows below only join what checking admitted.
    let mut symbols = None;
    let mut rows = Vec::new();
    for provider in program.machines().iter().filter(|machine| {
        machine.supply_mode == MachineSupplyMode::CheckedBody
            && program.machine_type_parameters(machine).is_empty()
    }) {
        for conformance in program.machine_trait_conformances(provider) {
            if conformance.external_binding.is_some()
                || conformance.symbol != conformance.requirement_symbol
            {
                continue;
            }
            let Some(template) = program.machines().iter().find(|candidate| {
                candidate.symbol == conformance.symbol
                    && candidate.supply_mode == MachineSupplyMode::TopLevelRequirement
                    && !program.machine_type_parameters(candidate).is_empty()
            }) else {
                continue;
            };
            let symbols = symbols.get_or_insert_with(|| {
                validation::TopLevelSymbols::build(program, &mut Vec::new())
            });
            if let Ok(closed) = validation::resolve_closed_requirement_application(
                program, provider, template, symbols,
            ) {
                rows.push(CheckedProviderClosedRequirement {
                    provider: provider.symbol,
                    template: template.symbol,
                    closed_requirement: closed.symbol,
                });
            }
        }
    }
    rows
}
