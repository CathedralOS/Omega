//! Project checked product custody into source-free native compilation inputs.

use crate::checked::CheckedCompilation;
use diagnostics::Diagnostic;

/// Project and validate the checked package's accepted Terminal permission
/// rows before native proposal production. These are package evidence, not
/// the independently supplied receiver-admission policy.
pub(crate) fn accepted_terminal_authority_permission_policy(
    checked: &CheckedCompilation,
) -> Result<native_production::TerminalAuthorityPermissionPolicy, Vec<Diagnostic>> {
    let (permissions, permission_identities) = checked.execution().terminal_authority_permissions();
    native_production::admit_package_permissions(permissions, permission_identities)
}
