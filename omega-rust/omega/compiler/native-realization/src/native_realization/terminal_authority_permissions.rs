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
    accepted: &crate::TerminalAuthorityPermissionPolicy,
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
    validate_permission_custody(permissions, None)
}

/// Rejoin every package-approved permission to the independently supplied
/// receiving policy. The receiving policy may contain rows for other
/// artifacts, but it may neither omit nor alter an approved row.
pub fn validate_package_terminal_authority_permissions<'a>(
    permissions: impl Iterator<Item = &'a ServiceTerminalAuthorityPermission>,
    policy: &crate::TerminalAuthorityPermissionPolicy,
) -> Result<(), Vec<Diagnostic>> {
    validate_permission_custody(permissions, Some(policy))
}

fn validate_permission_custody<'a>(
    permissions: impl Iterator<Item = &'a ServiceTerminalAuthorityPermission>,
    policy: Option<&crate::TerminalAuthorityPermissionPolicy>,
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
            continue;
        }
        let Some(policy) = policy else {
            continue;
        };
        match policy.permission_for(
            permission.service_schema(),
            permission.requirement_identity(),
        ) {
            Ok(permitted) if &permitted == permission.permitted() => {}
            Ok(_) => diagnostics.push(Diagnostic::error(format!(
                "receiving terminal-authority policy substitutes the accepted permission for `{}`",
                permission.requirement_identity(),
            ))),
            Err(_) => diagnostics.push(Diagnostic::error(format!(
                "receiving terminal-authority policy omits the accepted permission for `{}`",
                permission.requirement_identity(),
            ))),
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
    use super::{
        ServiceTerminalAuthorityPermission,
        validate_retained_package_terminal_authority_permissions,
    };
    use crate::terminal_authority_permission_policy_with_rows;
    use effects::provider_plan::ServiceSchemaDigest;
    use effects::{TerminalAuthorityClass, TerminalAuthorityDisposition};

    fn schema(marker: u8) -> ServiceSchemaDigest {
        ServiceSchemaDigest::from_digest([marker; 32])
    }

    fn permission(
        marker: u8,
        requirement: &str,
        classes: &[TerminalAuthorityClass],
    ) -> ServiceTerminalAuthorityPermission {
        ServiceTerminalAuthorityPermission::new(
            schema(marker),
            requirement,
            TerminalAuthorityDisposition::from_classes(classes.iter().copied()),
        )
    }

    fn policy(
        rows: Vec<ServiceTerminalAuthorityPermission>,
    ) -> crate::TerminalAuthorityPermissionPolicy {
        terminal_authority_permission_policy_with_rows(rows)
            .expect("test policy rows must be canonical and unique")
    }

    #[test]
    fn retained_route_enforces_every_rule_on_the_package_axis() {
        let accepted = policy(vec![
            permission(
                1,
                "Sink::emit#exact",
                &[TerminalAuthorityClass::ProcessOutput],
            ),
            permission(
                2,
                "Store::write#exact",
                &[TerminalAuthorityClass::FilesystemContentWrite],
            ),
        ]);
        assert!(
            validate_retained_package_terminal_authority_permissions(
                &[
                    permission(
                        2,
                        "Store::write#exact",
                        &[TerminalAuthorityClass::FilesystemContentWrite]
                    ),
                    permission(
                        1,
                        "Sink::emit#exact",
                        &[TerminalAuthorityClass::ProcessOutput]
                    ),
                ],
                &accepted,
            )
            .is_ok(),
            "the same rules in a different retained order still rejoin",
        );

        let diagnostics = validate_retained_package_terminal_authority_permissions(
            &[permission(
                1,
                "Sink::emit#exact",
                &[TerminalAuthorityClass::ProcessOutput],
            )],
            &accepted,
        )
        .expect_err("omitting one accepted rule must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("was not retained")),
            "unexpected diagnostics: {diagnostics:#?}",
        );
    }

    #[test]
    fn retained_route_names_each_divergent_rule() {
        let accepted = policy(vec![permission(
            1,
            "Sink::emit#exact",
            &[TerminalAuthorityClass::ProcessOutput],
        )]);

        let diagnostics = validate_retained_package_terminal_authority_permissions(
            &[permission(
                1,
                "Sink::emit#exact",
                &[TerminalAuthorityClass::ProcessTermination],
            )],
            &accepted,
        )
        .expect_err("an altered disposition must reject");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("alters the accepted disposition")
        }));

        let diagnostics = validate_retained_package_terminal_authority_permissions(
            &[
                permission(
                    1,
                    "Sink::emit#exact",
                    &[TerminalAuthorityClass::ProcessOutput],
                ),
                permission(
                    3,
                    "Store::read#exact",
                    &[TerminalAuthorityClass::FilesystemContentRead],
                ),
            ],
            &accepted,
        )
        .expect_err("an unrelated extra row must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("has no accepted package row"))
        );

        let diagnostics = validate_retained_package_terminal_authority_permissions(
            &[
                permission(
                    1,
                    "Sink::emit#exact",
                    &[TerminalAuthorityClass::ProcessOutput],
                ),
                permission(
                    1,
                    "Sink::emit#exact",
                    &[TerminalAuthorityClass::ProcessTermination],
                ),
            ],
            &accepted,
        )
        .expect_err("a repeated coordinate must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("repeats"))
        );
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("differ from the independently accepted package policy")),
            "the acceptance-axis headline diagnostic must be retained",
        );
    }
}
