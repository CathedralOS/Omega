//! Select exact provider members from Psi's retained declaration inventory.
//!
//! Psi owns product-family formation and retains every authored target.
//! Only Build source selects an execution target before resolution. Neither
//! route carries a parallel inventory joined back by names or source spans.
//! Product provider selection remains immutable here; moving the remaining
//! provider settlement behind Terminal Psi is separate work.

use crate::SettledTargetMachineDeclarations;
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::typed_trees::inspection::callees::ExplicitStateCallees;
use diagnostics::Diagnostic;
use target::TargetProfile;

mod proof_applications;
mod provider_publication;
pub use proof_applications::{proof_application_machines, target_has_proof_demand};
pub use provider_publication::{TargetProviderPublicationRequest, provider_selection_roots};

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
/// provider row binds for it, as the native execution scope does. It does not
/// treat a portable family head as an executable body: exact target membership
/// selects the body whose calls must be visited. A missing member keeps the
/// authored family at the diagnostic boundary rather than importing a sibling.
/// It never enters an inert body, so a caller scoped to the sibling's own target
/// goes inert with it and is never reported. A compilation without a selected
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
        .and_then(|entry| called_machine(typed, entry))
        .and_then(|entry| realized_machine(typed, entry, target))
        .and_then(|entry| typed.machine_position_by_symbol(entry.symbol))
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
                    let Some(callee) = realized_machine(typed, callee, target) else {
                        refusals.push((
                            position,
                            Diagnostic::error(unrealized_call_message(
                                typed, caller, state, callee, realized,
                            )),
                        ));
                        continue;
                    };
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
) -> Option<&checked_trees::typed_trees::machine::Machine> {
    typed
        .machine_by_symbol(called_symbol)
        .or_else(|| typed.machine_holding_state(called_symbol))
}

/// Resolve an explicit execution edge against the complete retained family.
/// A head has no authored target or states; visiting it would hide both a
/// missing member and calls inside the selected member. Exact member references
/// instead keep their own target and never redirect into another sibling.
fn realized_machine<'program>(
    typed: &'program TypedTrees,
    machine: &'program checked_trees::typed_trees::machine::Machine,
    target: TargetProfile,
) -> Option<&'program checked_trees::typed_trees::machine::Machine> {
    if machine.target_family_interface {
        return family_member_for_target(typed, machine.symbol, target.target_name());
    }
    typed
        .machine_authored_target(machine)
        .is_none_or(|declared| {
            TargetProfile::from_canonical_target_name(declared)
                .is_ok_and(|resolved| resolved == target)
        })
        .then_some(machine)
}

/// The retained member body a family head stands for on one target, from
/// Psi's explicit family catalog. `None` when the family has no body for
/// that target: the head itself is never a realization.
pub(crate) fn family_member_for_target<'program>(
    typed: &'program TypedTrees,
    head: symbols::SymbolHandle,
    target: &str,
) -> Option<&'program checked_trees::typed_trees::machine::Machine> {
    let family = typed
        .target_machine_families
        .iter()
        .find(|family| family.canonical == head)?;
    let member = family
        .members
        .iter()
        .find(|member| member.target.as_str() == target)?;
    typed.machine_by_symbol(member.machine)
}

/// The machine a provider row realizes on the selected target. A row names
/// the machine resolution bound its authored path to: a portable body, an
/// exact target member, or a target family's head when the authored name is
/// a family (`pub macos_arm64 machine sleep` in a library). The head is the
/// family's declaration, not a body, so the row realizes the member the
/// catalog retains for the selected target; a family without a body for that
/// target leaves the row, and its map, inert on that target exactly like a
/// member authored for another target. A Build-scope context realizes only
/// portable bodies, as `realizes_machine` states.
pub(crate) fn realized_row_machine<'program>(
    context: crate::ProviderPlanningContext<'program>,
    named: symbols::SymbolHandle,
    selected_target: Option<&str>,
) -> Option<&'program checked_trees::typed_trees::machine::Machine> {
    let typed = context.typed;
    let machine = typed.machine_by_symbol(named)?;
    let realization = if machine.target_family_interface {
        let target = selected_target
            .filter(|target| !target.is_empty())
            .unwrap_or_else(|| TargetProfile::host().target_name());
        family_member_for_target(typed, named, target)?
    } else {
        machine
    };
    realizes_machine(context, realization, selected_target).then_some(realization)
}

/// Select declaration candidates without changing the shared source program.
/// Build has its own admitted execution scope; its selected declaration marker
/// is resolved during Build preparation, not from the product target.
pub(crate) fn realizes_machine(
    context: crate::ProviderPlanningContext<'_>,
    machine: &checked_trees::typed_trees::machine::Machine,
    target: Option<&str>,
) -> bool {
    // A portable family interface has no implementation or satisfies edges.
    // Sharing the members' callable signature does not make it a second
    // checked adapter. Entry/family dispatch selects a retained member through
    // the explicit family catalog instead of this provider-candidate query.
    if machine.target_family_interface || !context.contains(machine.symbol) {
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
    caller: &checked_trees::typed_trees::machine::Machine,
    state: &checked_trees::typed_trees::state::State,
    callee: &checked_trees::typed_trees::machine::Machine,
    realized: &str,
) -> String {
    let declared: Vec<&str> = typed
        .target_machine_families
        .iter()
        .find(|family| {
            family.canonical == callee.symbol
                || family
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
