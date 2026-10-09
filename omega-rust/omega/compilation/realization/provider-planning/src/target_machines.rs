//! Select exact provider members from typed declarations.
//!
//! Psi owns product-family formation and retains every authored target.
//! Only Build source selects an execution target before resolution. Neither
//! route carries a parallel inventory joined back by names or source spans.
//! Product provider selection remains immutable here; moving the remaining
//! provider settlement behind Terminal Psi is separate work.

use crate::SettledTargetMachineDeclarations;
use diagnostics::Diagnostic;
use target::TargetProfile;
use typed_trees::TypedTrees;
use typed_trees::typed_trees::inspection::callees::ExplicitStateCallees;

/// Harvest every Product default before choosing a realization. Build defaults
/// instead belong to its separately admitted execution, not foreign siblings.
pub fn retain_provider_defaults(
    context: crate::ProviderPlanningContext<'_>,
) -> Result<Vec<crate::TargetProviderDefaults>, Vec<Diagnostic>> {
    let mut rows: Vec<crate::TargetProviderDefaults> = Vec::new();
    for machine in context.typed.machines().iter().filter(|machine| {
        machine.purpose == typed_trees::machine::MachinePurpose::ProviderDefaults
            && context.contains(machine.symbol)
            && (context.scope == source::DependencyScope::Product || machine.target.is_none())
    }) {
        let target = machine
            .authored_target
            .as_ref()
            .map(|name| name.as_str().to_owned());
        let selections = crate::harvest_provider_selections(context, machine)?;
        if let Some(row) = rows.iter_mut().find(|row| row.target == target) {
            row.machines.push(machine.symbol);
            row.selections.extend(selections);
        } else {
            rows.push(crate::TargetProviderDefaults {
                target,
                machines: vec![machine.symbol],
                selections,
            });
        }
    }
    rows.sort_by(|left, right| left.target.cmp(&right.target));
    Ok(rows)
}

/// Project retained declarations onto one realization using their exact handles.
pub fn settle_provider_defaults(
    context: crate::ProviderPlanningContext<'_>,
    retained: &[crate::TargetProviderDefaults],
    target: TargetProfile,
) -> SettledTargetMachineDeclarations {
    let matches_target = |name: &str| {
        TargetProfile::from_canonical_target_name(name).is_ok_and(|resolved| resolved == target)
    };
    let provider_defaults = retained
        .iter()
        .filter(|row| row.target.as_ref().is_none_or(|name| matches_target(name)))
        .flat_map(|row| row.selections.iter().cloned())
        .collect();
    let origins = context
        .typed
        .machines()
        .iter()
        .filter(|machine| {
            context.contains(machine.symbol)
                && (context.scope == source::DependencyScope::Product || machine.target.is_none())
        })
        .filter_map(|machine| {
            let authored = machine.authored_target.as_ref()?;
            matches_target(authored.as_str()).then(|| crate::SelectedTargetMachineOrigin {
                machine: machine.symbol,
                target: authored.as_str().to_owned(),
            })
        })
        .collect();
    SettledTargetMachineDeclarations {
        provider_defaults,
        origins,
    }
}

/// Refuse the realized product's calls into bodies its target does not have.
///
/// A family implemented only by other targets keeps its declaration as an
/// inert sibling with its callers, so the call still resolves and checking
/// accepts it. Psi checks every body whether or not a realization reaches it,
/// so a missing body is a realization question, never a checking error for
/// the program (wiki/spec/language/machines.md, target-scoped families):
/// only a call the realized product reaches demands an implementation.
///
/// Reach starts at the selected program entry, the root every executable
/// product requires. It follows explicit calls and named transitions, and a
/// call to a requirement continues into the realization each selected
/// provider row binds for it, as the native execution scope does. It never
/// enters an inert body, so a caller scoped to the sibling's own target goes
/// inert with it and is never reported. A compilation without a selected
/// entry realizes nothing: a dependency package's candidate check and a
/// targetless check refuse no portable code, such as std's hosted console and
/// filesystem adapters checked for `uefi_x86_64`. Machine-valued arguments are
/// not explicit callees; a missing body reached only through one is refused
/// when the native execution scope reaches the absent member.
pub fn reject_reached_inert_sibling_calls(
    typed: &TypedTrees,
    target: TargetProfile,
    entry: Option<symbols::SymbolHandle>,
    selected_providers: &[crate::SelectedProviderReviewProvenance],
) -> Result<(), Vec<Diagnostic>> {
    let machines = typed.machines();
    let realized = target.target_name();
    // Selected rows pair each requirement with the realization this target
    // installs for it; unselected candidates are never reached.
    let provider_rows: Vec<(symbols::SymbolHandle, symbols::SymbolHandle)> = selected_providers
        .iter()
        .flat_map(|selected| {
            selected
                .provider
                .row_requirements
                .iter()
                .copied()
                .zip(selected.provider.row_realizations.iter().copied())
        })
        .collect();
    // Roster positions index the reach marks; the roster is fixed here.
    let mut reached = vec![false; machines.len()];
    let mut frontier: Vec<usize> = entry
        .and_then(|entry| typed.machine_position_by_symbol(entry))
        .filter(|position| inert_target(typed, &machines[*position], target).is_none())
        .into_iter()
        .collect();
    for position in &frontier {
        reached[*position] = true;
    }
    // One walk both extends reach and records refusals. A machine's states
    // are visited together, so ordering by caller position restores the
    // declaration order without a second walk.
    let mut refusals: Vec<(usize, Diagnostic)> = Vec::new();
    let mut callees = ExplicitStateCallees::default();
    while let Some(position) = frontier.pop() {
        let caller = &machines[position];
        for state in typed.machine_states(caller) {
            for called_symbol in callees.collect(typed, state) {
                // A trait requirement call names the member itself; a
                // top-level requirement call names its entry state.
                let direct = called_machine(typed, *called_symbol);
                let requirement_machine = direct.map(|callee| callee.symbol);
                let realizations = provider_rows
                    .iter()
                    .filter(|(requirement, _)| {
                        requirement == called_symbol || Some(*requirement) == requirement_machine
                    })
                    .filter_map(|(_, realization)| called_machine(typed, *realization));
                for callee in direct.into_iter().chain(realizations) {
                    if inert_target(typed, callee, target).is_some() {
                        refusals.push((
                            position,
                            Diagnostic::error(unrealized_call_message(
                                typed, caller, state, callee, realized,
                            )),
                        ));
                        continue;
                    }
                    let Some(callee_position) = typed.machine_position_by_symbol(callee.symbol)
                    else {
                        continue;
                    };
                    if !std::mem::replace(&mut reached[callee_position], true) {
                        frontier.push(callee_position);
                    }
                }
            }
        }
    }
    if refusals.is_empty() {
        return Ok(());
    }
    refusals.sort_by_key(|(position, _)| *position);
    Err(refusals
        .into_iter()
        .map(|(_, diagnostic)| diagnostic)
        .collect())
}

/// A call names the callee's entry state, a named transition one of its own
/// states, and a few call forms the machine itself.
fn called_machine(
    typed: &TypedTrees,
    called_symbol: symbols::SymbolHandle,
) -> Option<&typed_trees::machine::Machine> {
    typed
        .machine_by_symbol(called_symbol)
        .or_else(|| typed.machine_holding_state(called_symbol))
}

/// The authored target of a body this build does not realize. Canonical
/// members are not implicitly realized just because their suffix is absent.
fn inert_target<'program>(
    typed: &'program TypedTrees,
    machine: &'program typed_trees::machine::Machine,
    target: TargetProfile,
) -> Option<&'program str> {
    typed.machine_authored_target(machine).filter(|declared| {
        !TargetProfile::from_canonical_target_name(declared)
            .is_ok_and(|resolved| resolved == target)
    })
}

/// Select declaration candidates without changing the shared source program.
/// Build has its own admitted execution scope; its selected declaration marker
/// is resolved during Build preparation, not from the product target.
pub(crate) fn realizes_machine(
    context: crate::ProviderPlanningContext<'_>,
    machine: &typed_trees::machine::Machine,
    target: Option<&str>,
) -> bool {
    if !context.contains(machine.symbol) {
        return false;
    }
    if context.scope == source::DependencyScope::Build {
        return machine.target.is_none();
    }
    let target = target
        .filter(|target| !target.is_empty())
        .unwrap_or_else(|| TargetProfile::host().target_name());
    context
        .typed
        .machine_authored_target(machine)
        .is_none_or(|authored| authored == target)
}

/// Name every target the callee's family supplies: a family with bodies for
/// several other targets is not "declared only for" its canonical one.
fn unrealized_call_message(
    typed: &TypedTrees,
    caller: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    callee: &typed_trees::machine::Machine,
    realized: &str,
) -> String {
    let declared: Vec<&str> = typed
        .target_machine_families
        .iter()
        .find(|family| {
            family
                .members
                .iter()
                .any(|member| member.machine == callee.symbol)
        })
        .map(|family| {
            family
                .members
                .iter()
                .map(|member| member.target.as_str())
                .collect()
        })
        .unwrap_or_else(|| callee.target.iter().map(|target| target.as_str()).collect());
    let caller_name = caller.name.as_str();
    let state_name = state.name.as_str();
    let callee_name = callee.name.as_str();
    match declared.as_slice() {
        [only] => format!(
            "machine `{caller_name}` state `{state_name}` calls `{callee_name}`, which is \
             declared only for target `{only}`. This build realizes `{realized}`, so that body \
             is an inert sibling and the call has no implementation; scope the caller to \
             `{only}` as well, or declare `{callee_name}` for the target being built."
        ),
        several => format!(
            "machine `{caller_name}` state `{state_name}` calls `{callee_name}`, which is \
             declared only for targets {}. This build realizes `{realized}`, so those bodies \
             are inert siblings and the call has no implementation; scope the caller to one \
             of those targets, or declare `{callee_name}` for the target being built.",
            several
                .iter()
                .map(|target| format!("`{target}`"))
                .collect::<Vec<_>>()
                .join(", "),
        ),
    }
}
