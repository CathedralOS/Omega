//! Project diagnostic attribution while the lowering still owns source joins.

use crate::direct_calls::direct_call_parts;
use checked_trees::CheckedTrees;
use semantic_vocabulary::MachineId;
use std::collections::BTreeMap;
use terminal_psi::TerminalMachineProvenance;

pub fn lowered_module_provenance(
    checked: &CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
) -> TerminalMachineProvenance {
    let module = &lowered.semantic_module;
    let mut callee_by_operation = BTreeMap::new();
    for machine in &module.machines {
        for block in &machine.blocks {
            for operation in &block.operations {
                if let Some(call) = direct_call_parts(&operation.kind) {
                    callee_by_operation.insert(operation.id, call.callee);
                }
            }
        }
    }
    let mut provenance = TerminalMachineProvenance::default();
    for occurrence in &lowered.source_call_occurrences {
        if let Some(operation) = occurrence.terminal_site.operation()
            && let Some(callee) = callee_by_operation.get(&operation)
        {
            // Module custody uses its selected state. Diagnostic attribution
            // names that state's machine through the same exact occurrence join;
            // it is neither a source-label fallback nor an execution premise.
            if let Ok(target) = crate::call_sites::selected_module_target(checked, occurrence) {
                name(&mut provenance, checked, *callee, target.machine.symbol);
            }
        }
    }
    // Provider candidate bodies are reached through boundary dispatch,
    // not a source call; the catalog retains each candidate's canonical
    // checked identity, which rejoins its declaration.
    if !module.provider_candidates.is_empty() {
        let identities: BTreeMap<String, symbols::SymbolHandle> = checked
            .typed
            .machines()
            .iter()
            .filter_map(|machine| {
                checked
                    .typed
                    .machine_callable_identity_text(machine)
                    .map(|identity| (identity, machine.symbol))
            })
            .collect();
        for candidate in &module.provider_candidates {
            if let Some(symbol) = identities.get(
                module
                    .identities
                    .text(candidate.candidate_identity)
                    .unwrap_or_default(),
            ) {
                name(&mut provenance, checked, candidate.candidate, *symbol);
            }
        }
    }
    provenance
}

fn name(
    provenance: &mut TerminalMachineProvenance,
    checked: &CheckedTrees,
    machine: MachineId,
    symbol: symbols::SymbolHandle,
) {
    if let Some(declaration) = checked.typed.machine(symbol) {
        provenance.name(
            machine,
            declaration.name.as_str().to_owned(),
            checked.typed.symbols.symbol_source_span(symbol),
        );
    }
}
