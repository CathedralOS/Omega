//! Project checked product custody into source-free native compilation inputs.

use crate::checked::CheckedCompilation;
use diagnostics::Diagnostic;

/// Borrow one complete production input per explicitly Independent selected
/// plan. A description with no current native product is a refusal here, never
/// a reason to realize the same body inside the receiver.
pub(super) fn independent_components(
    selected: &Result<Vec<compilation_products::SelectedIndependentComponent>, String>,
) -> Result<Vec<native_production::ComponentCompilation<'_>>, Vec<Diagnostic>> {
    let selected = selected
        .as_ref()
        .map_err(|reason| vec![Diagnostic::error(reason.clone())])?;
    selected
        .iter()
        .map(|component| {
            Ok(native_production::ComponentCompilation {
                input: component
                    .input()
                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
                plan: component.plan(),
                receiving_identities: component.receiving_identities(),
                accepted_package_permissions: component.accepted_package_permissions(),
            })
        })
        .collect()
}

/// Project and validate the checked package's accepted Terminal permission
/// rows before native proposal production. These are package evidence, not
/// the independently supplied receiver-admission policy.
pub(crate) fn accepted_terminal_authority_permission_policy(
    checked: &CheckedCompilation,
) -> Result<native_production::TerminalAuthorityPermissionPolicy, Vec<Diagnostic>> {
    let (permissions, permission_identities) = checked.execution().terminal_authority_permissions();
    native_production::admit_package_permissions(permissions, permission_identities)
}
