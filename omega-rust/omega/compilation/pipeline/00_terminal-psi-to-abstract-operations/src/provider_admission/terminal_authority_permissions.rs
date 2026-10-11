//! Exact package-permission custody shared by direct and retained native routes.

use diagnostics::Diagnostic;
use effects::ServiceTerminalAuthorityPermission;
use std::collections::BTreeSet;

/// Rejoin the rows retained before checked-state destruction to the exact
/// package-acceptance policy supplied independently at retained re-entry.
/// Unlike the receiving policy, this set admits no unrelated extra rows:
/// every retained rule must rejoin one exact accepted row and every accepted
/// rule must be retained, with each divergence named on its own row.
pub(crate) fn validate_retained_package_terminal_authority_permissions(
    retained: &[ServiceTerminalAuthorityPermission],
    retained_identities: &terminal_psi::TerminalIdentityTable,
    accepted: &crate::provider_admission::TerminalAuthorityPermissionPolicy,
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for permission in retained {
        let requirement = retained_identities
            .text(permission.requirement_identity())
            .unwrap_or_default()
            .to_owned();
        let coordinate = (permission.service_schema(), requirement.clone());
        if !seen.insert(coordinate) {
            diagnostics.push(Diagnostic::error(format!(
                "retained Terminal package permission custody repeats terminal-authority permission `{requirement}` for one exact service schema",
            )));
            continue;
        }
        match accepted.rows().iter().find(|row| {
            row.service_schema() == permission.service_schema()
                && accepted.identities().text(row.requirement_identity())
                    == Some(requirement.as_str())
        }) {
            Some(row) if row.permitted() == permission.permitted() => {}
            Some(_) => diagnostics.push(Diagnostic::error(format!(
                "retained Terminal package permission for `{requirement}` alters the accepted disposition",
            ))),
            None => diagnostics.push(Diagnostic::error(format!(
                "retained Terminal package permission for `{requirement}` has no accepted package row",
            ))),
        }
    }
    for row in accepted.rows() {
        let requirement = accepted
            .identities()
            .text(row.requirement_identity())
            .unwrap_or_default();
        if !seen.contains(&(row.service_schema(), requirement.to_owned())) {
            diagnostics.push(Diagnostic::error(format!(
                "accepted package terminal-authority permission for `{requirement}` was not retained",
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        let mut all = vec![Diagnostic::error(
            "retained Terminal package permissions differ from the independently accepted package policy",
        )];
        all.append(&mut diagnostics);
        Err(all)
    }
}

/// Package permission custody independent of any receiving policy: reject a
/// repeated `(service schema, requirement)` coordinate across the checked
/// compilation's resolved bindings. This runs even when no receiving policy
/// was supplied; absence of a receiving policy is not a substitute for the
/// package axis.
pub fn validate_package_terminal_authority_permission_custody<'a>(
    permissions: impl Iterator<Item = &'a ServiceTerminalAuthorityPermission>,
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for permission in permissions {
        let requirement = identities
            .text(permission.requirement_identity())
            .unwrap_or_default()
            .to_owned();
        let coordinate = (permission.service_schema(), requirement.clone());
        if !seen.insert(coordinate) {
            diagnostics.push(Diagnostic::error(format!(
                "package permission custody repeats terminal-authority permission `{requirement}` for one exact service schema",
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
