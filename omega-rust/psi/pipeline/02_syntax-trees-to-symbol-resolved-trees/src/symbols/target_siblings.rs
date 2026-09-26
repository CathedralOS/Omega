//! Bind the calls inside a target sibling's body to the same target's sibling
//! declarations. Ordinary resolution sees only the selected target's
//! declarations: a sibling's symbol is spelled `<path>::<target>` and it is
//! excluded from attachment lookups, so a `windows_x86_64` body that calls a
//! `windows_x86_64`-only helper would otherwise stay unresolved, and one that
//! calls a contract every target implements would bind to the selected
//! target's body instead of its own target's.

use crate::selection::body_calls::{CallSite, retarget_machine_calls};
use crate::symbol_resolved_trees::SymbolResolvedTrees;
use crate::symbols::lookup::child_symbol_by_kinds;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

struct Sibling {
    machine: SymbolHandle,
    target: String,
    /// The authored path, shared with the selected declaration of the same
    /// family.
    path: String,
    owner: Option<String>,
}

/// Runs after every other symbol assignment and selection pass: later passes
/// re-derive call targets from names, which cannot see a sibling.
pub(crate) fn bind_sibling_calls(program: &mut SymbolResolvedTrees) {
    let symbols = std::mem::take(&mut program.symbols);
    bind_sibling_calls_with(program, &symbols);
    program.symbols = symbols;
}

fn bind_sibling_calls_with(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let machine_paths = program
        .machines
        .iter()
        .map(|machine| (machine.symbol, machine.name.as_str().to_owned()))
        .collect::<Vec<_>>();
    let siblings = program
        .machines
        .iter()
        .filter_map(|machine| {
            let target = machine.target.as_ref()?;
            Some(Sibling {
                machine: machine.symbol,
                target: target.as_str().to_owned(),
                path: machine.name.as_str().to_owned(),
                owner: machine
                    .attached_data
                    .as_ref()
                    .map(|owner| owner.as_str().to_owned()),
            })
        })
        .collect::<Vec<_>>();
    for sibling in &siblings {
        let same_target = siblings
            .iter()
            .filter(|candidate| candidate.target == sibling.target)
            .collect::<Vec<_>>();
        let mut decide = |site: &CallSite<'_>| -> Option<SymbolHandle> {
            if site.target_symbol.is_valid() {
                // Resolved to the selected target's declaration: prefer this
                // target's own body of the same path when it has one.
                let state = symbols.get(site.target_symbol);
                if state.kind != SymbolKind::State {
                    return None;
                }
                let callee = state.parent;
                if same_target
                    .iter()
                    .any(|candidate| candidate.machine == callee)
                {
                    return None;
                }
                let (_, callee_path) =
                    machine_paths.iter().find(|(symbol, _)| *symbol == callee)?;
                let replacement = same_target
                    .iter()
                    .find(|candidate| candidate.path == *callee_path)?;
                let state = child_symbol_by_kinds(
                    symbols,
                    replacement.machine,
                    &[SymbolKind::State],
                    symbols.name(site.target_symbol),
                );
                return state.is_valid().then_some(state);
            }
            // Unresolved: only this target declares the callee.
            let spelled = site.target.as_str();
            let leaf = spelled.rsplit_once("::").map_or(spelled, |(_, leaf)| leaf);
            let candidate = same_target
                .iter()
                .find(|candidate| candidate.path == spelled)
                .or_else(|| {
                    if !site.local_receiver {
                        return None;
                    }
                    let owner = sibling.owner.as_deref()?;
                    same_target.iter().find(|candidate| {
                        candidate.owner.as_deref() == Some(owner)
                            && candidate.path == format!("{owner}::{leaf}")
                    })
                })?;
            let named =
                child_symbol_by_kinds(symbols, candidate.machine, &[SymbolKind::State], leaf);
            let state = if named.is_valid() {
                named
            } else {
                child_symbol_by_kinds(symbols, candidate.machine, &[SymbolKind::State], "entry")
            };
            state.is_valid().then_some(state)
        };
        retarget_machine_calls(program, sibling.machine, &[], &mut decide);
    }
}

#[cfg(test)]
mod tests;
