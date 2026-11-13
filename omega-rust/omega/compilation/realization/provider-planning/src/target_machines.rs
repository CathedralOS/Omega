// Target publication consumes retained canonical choices. The old checked
// source preparation/derivation path has no replacement producer here.
mod prepared_providers;
pub use prepared_providers::ProviderPublicationCatalog;
mod provider_publication;
pub use provider_publication::{PreparedTargetProviderPublication, provider_selection_roots};

use target::TargetProfile;

pub(crate) fn realized_row_machine<'program>(
    context: crate::ProviderPlanningContext<'program>,
    named: symbols::SymbolHandle,
    selected_target: Option<&str>,
) -> Option<&'program typed_trees::machine::Machine> {
    let typed = context.typed;
    let machine = typed.machine_by_symbol(named)?;
    let realization = if machine.target_family_interface {
        let target = selected_target
            .filter(|target| !target.is_empty())
            .unwrap_or_else(|| TargetProfile::host().target_name());
        typed.target_family_member(named, target)?
    } else {
        machine
    };
    realizes_machine(context, realization, selected_target).then_some(realization)
}

/// Whether `machine` is a body this scope and target actually realize: in
/// scope, not a target-family interface, and authored for `target` (or for no
/// target in a Build program).
pub fn realizes_machine(
    context: crate::ProviderPlanningContext<'_>,
    machine: &typed_trees::machine::Machine,
    target: Option<&str>,
) -> bool {
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
        .source_target_context(machine)
        .is_none_or(|authored| authored == target)
}
