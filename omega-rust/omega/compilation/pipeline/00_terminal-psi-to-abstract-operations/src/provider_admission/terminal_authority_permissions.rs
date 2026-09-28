//! Exact package-permission custody shared by direct and retained native routes.

use diagnostics::Diagnostic;
use effects::ServiceTerminalAuthorityPermission;
use std::collections::BTreeSet;

/// Rejoin the rows retained before checked-state destruction to the exact
/// package-acceptance policy supplied independently at retained re-entry.
/// Unlike the receiving policy, this set admits no unrelated extra rows:
/// every retained rule must rejoin one exact accepted row and every accepted
/// rule must be retained, with each divergence named on its own row.
pub fn validate_retained_package_terminal_authority_permissions(
    retained: &[ServiceTerminalAuthorityPermission],
    accepted: &crate::provider_admission::TerminalAuthorityPermissionPolicy,
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for permission in retained {
        let coordinate = (
            permission.service_schema(),
            permission.requirement_identity().to_owned(),
        );
        if !seen.insert(coordinate) {
            diagnostics.push(Diagnostic::error(format!(
                "retained Terminal package permission custody repeats terminal-authority permission `{}` for one exact service schema",
                permission.requirement_identity(),
            )));
            continue;
        }
        match accepted.rows().iter().find(|row| {
            row.service_schema() == permission.service_schema()
                && row.requirement_identity() == permission.requirement_identity()
        }) {
            Some(row) if row.permitted() == permission.permitted() => {}
            Some(_) => diagnostics.push(Diagnostic::error(format!(
                "retained Terminal package permission for `{}` alters the accepted disposition",
                permission.requirement_identity(),
            ))),
            None => diagnostics.push(Diagnostic::error(format!(
                "retained Terminal package permission for `{}` has no accepted package row",
                permission.requirement_identity(),
            ))),
        }
    }
    for row in accepted.rows() {
        if !seen.contains(&(row.service_schema(), row.requirement_identity().to_owned())) {
            diagnostics.push(Diagnostic::error(format!(
                "accepted package terminal-authority permission for `{}` was not retained",
                row.requirement_identity(),
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
) -> Result<(), Vec<Diagnostic>> {
    let mut seen = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for permission in permissions {
        let coordinate = (
            permission.service_schema(),
            permission.requirement_identity().to_owned(),
        );
        if !seen.insert(coordinate) {
            diagnostics.push(Diagnostic::error(format!(
                "package permission custody repeats terminal-authority permission `{}` for one exact service schema",
                permission.requirement_identity(),
            )));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_admission::terminal_authority_permission_policy_with_rows;
    use effects::{TerminalAuthorityClass, TerminalAuthorityDisposition};

    #[test]
    fn retained_package_permissions_still_require_exact_acceptance() {
        let permission = ServiceTerminalAuthorityPermission::new(
            effects::provider_plan::ServiceSchemaDigest::from_digest([1; 32]),
            "Console::exit_process",
            TerminalAuthorityDisposition::from_classes([
                TerminalAuthorityClass::ProcessTermination,
            ]),
        );
        let accepted = terminal_authority_permission_policy_with_rows(vec![permission.clone()])
            .expect("one accepted permission");
        let exact = std::slice::from_ref(&permission);
        validate_retained_package_terminal_authority_permissions(exact, &accepted).unwrap();
        validate_package_terminal_authority_permission_custody(exact.iter()).unwrap();
        assert!(validate_retained_package_terminal_authority_permissions(&[], &accepted).is_err());

        let empty = terminal_authority_permission_policy_with_rows(Vec::new()).unwrap();
        assert!(validate_retained_package_terminal_authority_permissions(exact, &empty).is_err());
        let altered = ServiceTerminalAuthorityPermission::new(
            permission.service_schema(),
            permission.requirement_identity(),
            TerminalAuthorityDisposition::from_classes([]),
        );
        assert!(
            validate_retained_package_terminal_authority_permissions(&[altered], &accepted)
                .is_err()
        );

        let repeated = [permission.clone(), permission];
        assert!(
            validate_retained_package_terminal_authority_permissions(&repeated, &accepted).is_err()
        );
        assert!(validate_package_terminal_authority_permission_custody(repeated.iter()).is_err());
    }
}
