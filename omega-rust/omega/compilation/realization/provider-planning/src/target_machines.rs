//! Select exact provider members from Psi's retained declaration inventory.
//!
//! Build rows select execution targets; source declarations remain ordinary.
//! Exact retained handles preserve declaration and dependency-purpose custody.
//! Product provider selection remains immutable here; moving the remaining
//! provider settlement behind Terminal Psi is separate work.

use crate::SettledTargetMachineDeclarations;
use checked_trees::typed_trees::TypedTrees;
use target::TargetProfile;

mod authored_providers;
mod direct_declaration_absences;
pub use authored_providers::{AuthoredTargetProviders, TargetProviderInventoryRequest};
mod provider_publication;
pub(crate) use direct_declaration_absences::DirectDeclarationAbsences;
pub use provider_publication::{
    PreparedTargetProviderPublication, TargetProviderPreparationRequest, provider_selection_roots,
};

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

/// Resolve exactly the authored ordinary declaration in this planning scope.
/// Target applicability belongs to the evaluated Build row, not a source body.
pub(crate) fn scoped_row_machine<'program>(
    context: crate::ProviderPlanningContext<'program>,
    named: symbols::SymbolHandle,
) -> Option<&'program checked_trees::typed_trees::machine::Machine> {
    context
        .contains(named)
        .then(|| context.typed.machine_by_symbol(named))?
}
