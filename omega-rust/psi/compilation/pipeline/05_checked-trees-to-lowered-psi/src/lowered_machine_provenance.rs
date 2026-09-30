//! Source attribution for one unoptimized lowering, built only when a
//! consumer needs diagnostics. These names and spans are not admission
//! evidence and do not decide whether a module is valid.

use crate::direct_calls::direct_call_parts;
use checked_trees::CheckedTrees;
use semantic_vocabulary::MachineId;
use source::SourceSpan;
use std::collections::BTreeMap;

/// Diagnostic provenance for the Terminal machines of one lowered closure:
/// the authored machine name and declaration span, so a rejection can name
/// the machine that retains the prohibited site instead of a Terminal
/// coordinate alone. Names come from the entry symbol, the lowering's
/// source-call joins, and the provider candidate catalog's checked
/// identities; a machine none of those cover is attributed through the
/// nearest named caller on the walk from the entry, and reported by its
/// Terminal identity when no caller is named either.
#[derive(Debug, Default)]
pub struct LoweredMachineProvenance {
    machines: BTreeMap<MachineId, (String, Option<SourceSpan>)>,
    /// First caller reached from the entry on the static call graph.
    callers: BTreeMap<MachineId, MachineId>,
}

impl LoweredMachineProvenance {
    /// Join the replayed module's call operations to the checked machines
    /// they target. Each ephemeral source-call occurrence names the Terminal
    /// call operation and the checked callee symbol; the operation's callee
    /// is the Terminal machine that symbol lowered to. The entry is named by
    /// its selected symbol.
    pub fn from_lowering(
        checked: &CheckedTrees,
        lowered: &lowered_psi::LoweredPsi,
        entry_machine_symbol: symbols::SymbolHandle,
    ) -> Self {
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
        let mut provenance = Self::default();
        provenance.name(checked, module.entry, entry_machine_symbol);
        for occurrence in &lowered.source_call_occurrences {
            if let Some(callee) = callee_by_operation.get(&occurrence.terminal_operation) {
                provenance.name(checked, *callee, occurrence.source_target);
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
                        .normalized_machine_overload_identity(machine)
                        .map(|identity| (identity.identity(), machine.symbol))
                })
                .collect();
            for candidate in &module.provider_candidates {
                if let Some(symbol) = identities.get(&candidate.candidate_identity) {
                    provenance.name(checked, candidate.candidate, *symbol);
                }
            }
        }
        // Static call parents from the entry, so an unnamed helper can be
        // attributed through the named machine that reaches it.
        let mut pending = std::collections::VecDeque::from([module.entry]);
        let mut visited = std::collections::BTreeSet::from([module.entry]);
        while let Some(current) = pending.pop_front() {
            let Some(machine) = module.machines.iter().find(|machine| machine.id == current) else {
                continue;
            };
            for block in &machine.blocks {
                for operation in &block.operations {
                    if let Some(callee) = direct_call_parts(&operation.kind).map(|call| call.callee)
                        && visited.insert(callee)
                    {
                        provenance.callers.insert(callee, current);
                        pending.push_back(callee);
                    }
                    // A boundary call reaches its candidate bodies through
                    // dispatch, the same edge the absence walk follows.
                    if let terminal_psi::OperationKind::BoundaryCall { boundary, .. } =
                        &operation.kind
                    {
                        for candidate in module
                            .provider_candidates
                            .iter()
                            .filter(|candidate| candidate.boundary == *boundary)
                        {
                            if visited.insert(candidate.candidate) {
                                provenance.callers.insert(candidate.candidate, current);
                                pending.push_back(candidate.candidate);
                            }
                        }
                    }
                }
            }
        }
        provenance
    }

    /// The nearest machine on the walk from the entry that has a checked
    /// name: the machine itself when named, otherwise its first named caller.
    fn named_ancestor(
        &self,
        machine: MachineId,
    ) -> Option<(MachineId, &(String, Option<SourceSpan>))> {
        let mut current = machine;
        let mut steps = 0_usize;
        loop {
            if let Some(named) = self.machines.get(&current) {
                return Some((current, named));
            }
            current = *self.callers.get(&current)?;
            steps += 1;
            if steps > self.callers.len() {
                return None;
            }
        }
    }

    fn name(&mut self, checked: &CheckedTrees, machine: MachineId, symbol: symbols::SymbolHandle) {
        if self.machines.contains_key(&machine) {
            return;
        }
        let Some(declaration) = checked
            .typed
            .machines()
            .iter()
            .find(|candidate| candidate.symbol == symbol)
        else {
            return;
        };
        // Checked machine names are already owner-qualified (`Type::name`).
        let name = declaration.name.as_str().to_owned();
        self.machines.insert(
            machine,
            (name, checked.typed.symbols.symbol_source_span(symbol)),
        );
    }

    /// `machine \`name\` (Terminal machine N)` when the machine is joined to
    /// checked provenance; `Terminal machine N, reached through machine
    /// \`caller\` (Terminal machine M)` when only a caller is; otherwise
    /// `Terminal machine N`.
    pub fn label(&self, machine: MachineId) -> String {
        match self.named_ancestor(machine) {
            Some((named, (name, _))) if named == machine => {
                format!("machine `{name}` (Terminal machine {machine})")
            }
            Some((named, (name, _))) => format!(
                "Terminal machine {machine}, reached through machine `{name}` (Terminal machine {named})"
            ),
            None => format!("Terminal machine {machine}"),
        }
    }

    /// The nearest named machine's declaration span, when authored source
    /// custody is available. An unnamed helper inherits its caller's span.
    pub fn source_span(&self, machine: MachineId) -> Option<SourceSpan> {
        let (_, (_, span)) = self.named_ancestor(machine)?;
        *span
    }
}
