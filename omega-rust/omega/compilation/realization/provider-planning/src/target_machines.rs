//! Select exact provider members from Psi's retained declaration inventory.
//!
//! Build rows select execution targets; source declarations remain ordinary.
//! Exact retained handles preserve declaration and dependency-purpose custody.
//! Product provider selection remains immutable here; moving the remaining
//! provider settlement behind Terminal Psi is separate work.

use crate::SettledProviderDefaults;
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
    retained: &[crate::TargetProviderDefaults],
    target: TargetProfile,
) -> SettledProviderDefaults {
    let matches_target = |name: &str| {
        TargetProfile::from_canonical_target_name(name).is_ok_and(|resolved| resolved == target)
    };
    let provider_defaults = retained
        .iter()
        .filter(|row| row.target.as_ref().is_none_or(|name| matches_target(name)))
        .flat_map(|row| row.selections.iter().cloned())
        .collect();
    SettledProviderDefaults { provider_defaults }
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
