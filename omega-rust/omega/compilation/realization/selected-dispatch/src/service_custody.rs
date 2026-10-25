//! Entry services are demanded by the checked call closure. Entry selection
//! supplies no fields, values or service-carrier storage.

use checked_trees::CheckedTrees;

pub fn program_entry_service_requirements(
    checked: &CheckedTrees,
    machine_symbol: symbols::SymbolHandle,
) -> Vec<symbols::SymbolHandle> {
    let reaches = &checked.facts.service_reaches;
    let Some(reach) = reaches.for_machine(machine_symbol) else {
        return Vec::new();
    };
    let published = match reach.interface {
        language_semantics::ServiceReachInterface::PublishedCeiling(row) => Some(row),
        language_semantics::ServiceReachInterface::InternalInferred => None,
    };
    published
        .into_iter()
        .chain([reach.inferred_transitive, reach.concrete_transitive])
        .flat_map(|row| reaches.rows.services(row))
        .filter_map(|&service| reaches.services.definition(service))
        .map(|definition| definition.symbol)
        .collect()
}
