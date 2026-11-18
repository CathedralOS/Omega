/*
    Bind each target-authored body's calls to that target's declarations.
    The canonical member is a body too, not a portable caller. Ordinary
    resolution sees only canonical declarations: a sibling's symbol is spelled
    `<path>::<target>` and excluded from attachment lookups, so a
    `windows_x86_64` body that calls a
    `windows_x86_64`-only helper would otherwise stay unresolved, and one that
    calls a contract every target implements would bind to the selected
    target's body instead of its own target's.
*/

use crate::names::lookup::child_symbol_by_kinds;
use crate::selections::body_calls::{CallSite, retarget_machine_calls};
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

struct Sibling {
    machine: SymbolHandle,
    family: SymbolHandle,
    target: String,
    owner: Option<String>,
}

/*
    Runs after every other symbol assignment and selection pass: later passes
    re-derive call targets from names, which cannot see a sibling.
*/
pub(crate) fn bind_sibling_calls(
    program: &mut SymbolResolvedTrees,
    inputs: &[crate::selections::body_calls::CallSelectionInput],
    work: &[SymbolHandle],
) {
    let symbols = std::mem::take(&mut program.symbols);
    bind_sibling_calls_with(program, &symbols, inputs, work);
    program.symbols = symbols;
}

fn bind_sibling_calls_with(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    inputs: &[crate::selections::body_calls::CallSelectionInput],
    work: &[SymbolHandle],
) {
    let machine_families = program
        .machines
        .iter()
        .filter(|machine| machine.target_family.is_valid())
        .map(|machine| (machine.symbol, machine.target_family))
        .collect::<Vec<_>>();
    let siblings = program
        .machines
        .iter()
        .filter_map(|machine| {
            let target = machine
                .authored_target
                .as_ref()
                .or(machine.target.as_ref())?;
            Some(Sibling {
                machine: machine.symbol,
                family: machine.target_family,
                target: target.as_str().to_owned(),
                owner: machine
                    .attached_data
                    .as_ref()
                    .map(|owner| owner.as_str().to_owned()),
            })
        })
        .collect::<Vec<_>>();
    let bodies = program
        .machines
        .iter()
        .filter_map(|machine| {
            let body = machine.symbol;
            let mut owner = machine;
            loop {
                if let Some(sibling) = siblings
                    .iter()
                    .find(|sibling| sibling.machine == owner.symbol)
                {
                    return Some((body, sibling));
                }
                if !owner.lexical_parent.is_valid() {
                    return None;
                }
                owner = program.machines.get(owner.lexical_parent);
            }
        })
        .collect::<Vec<_>>();
    for (body, sibling) in bodies {
        let same_target = siblings
            .iter()
            .filter(|candidate| candidate.target == sibling.target)
            .collect::<Vec<_>>();
        let mut decide = |site: &CallSite<'_>| -> Option<SymbolHandle> {
            if site.target_symbol.is_valid() {
                let target = symbols.get(site.target_symbol);
                let callee = match target.kind {
                    SymbolKind::State => target.parent,
                    SymbolKind::Machine => site.target_symbol,
                    _ => return None,
                };
                if same_target
                    .iter()
                    .any(|candidate| candidate.machine == callee)
                {
                    return None;
                }
                let (_, family) = machine_families
                    .iter()
                    .find(|(symbol, _)| *symbol == callee)?;
                let replacement = same_target
                    .iter()
                    .find(|candidate| candidate.family == *family)?;
                if target.kind == SymbolKind::Machine {
                    return Some(replacement.machine);
                }
                let state = child_symbol_by_kinds(
                    symbols,
                    replacement.machine,
                    &[SymbolKind::State],
                    symbols.name(site.target_symbol),
                );
                return state.is_valid().then_some(state);
            }
            let spelled = site.target.as_str();
            let leaf = spelled.rsplit_once("::").map_or(spelled, |(_, leaf)| leaf);
            let lookup = |path: &str| {
                symbols.find_top_level_by_name_and_kinds_from_source(
                    &format!("{path}::{}", sibling.target),
                    &[SymbolKind::Machine],
                    site.target.source_span(),
                )
            };
            let candidate = lookup(spelled).or_else(|| {
                if !site.local_receiver {
                    return None;
                }
                let owner = sibling.owner.as_deref()?;
                lookup(&format!("{owner}::{leaf}"))
            })?;
            if !same_target
                .iter()
                .any(|sibling| sibling.machine == candidate)
            {
                return None;
            }
            crate::names::select_machine_entry(leaf, |name| {
                let state = child_symbol_by_kinds(symbols, candidate, &[SymbolKind::State], name);
                state.is_valid().then_some(state)
            })
        };
        retarget_machine_calls(program, body, inputs, work.contains(&body), &mut decide);
    }
}
