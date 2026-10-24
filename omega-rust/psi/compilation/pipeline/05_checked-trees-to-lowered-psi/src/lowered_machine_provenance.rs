//! Project diagnostic attribution while the lowering still owns source joins.

use crate::direct_calls::direct_call_parts;
use checked_trees::CheckedTrees;
use semantic_vocabulary::MachineId;
use std::collections::BTreeMap;
use terminal_psi::TerminalMachineProvenance;

pub fn lowered_machine_provenance(
    checked: &CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
    entry_machine_symbol: symbols::SymbolHandle,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
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
    let entry_identity = checked
        .exact_owner_nominal_identity(entry_machine_symbol, exact_toolchain_sources)
        .and_then(|identity| checked.type_identity_text(identity));
    let entry = module
        .machines
        .iter()
        .find(|machine| {
            entry_identity.as_ref().is_some_and(|identity| {
                machine.declaration.as_ref().is_some_and(|declaration| {
                    module.identities.text(declaration.machine.identity())
                        == Some(identity.as_str())
                })
            })
        })
        .map(|machine| machine.id);
    if let Some(entry) = entry {
        name(&mut provenance, checked, entry, entry_machine_symbol);
    }
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
    // Static call parents from the entry, so an unnamed helper can be
    // attributed through the named machine that reaches it.
    let mut pending = entry.into_iter().collect::<std::collections::VecDeque<_>>();
    let mut visited = entry.into_iter().collect::<std::collections::BTreeSet<_>>();
    while let Some(current) = pending.pop_front() {
        let Some(machine) = module.machine(current) else {
            continue;
        };
        for block in &machine.blocks {
            for operation in &block.operations {
                if let Some(callee) = direct_call_parts(&operation.kind).map(|call| call.callee)
                    && visited.insert(callee)
                {
                    provenance.record_caller(callee, current);
                    pending.push_back(callee);
                }
                // A boundary call reaches its candidate bodies through
                // dispatch, the same edge the absence walk follows.
                if let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &operation.kind
                {
                    for candidate in module
                        .provider_candidates
                        .iter()
                        .filter(|candidate| candidate.boundary == *boundary)
                    {
                        if visited.insert(candidate.candidate) {
                            provenance.record_caller(candidate.candidate, current);
                            pending.push_back(candidate.candidate);
                        }
                    }
                }
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
