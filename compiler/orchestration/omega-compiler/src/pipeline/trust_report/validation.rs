use omega_artifacts::{TrustReport, UnresolvedTrustReference};
use omega_core::diagnostics::Diagnostic;

pub(super) fn validate_trust_report(report: &TrustReport) -> Result<(), Vec<Diagnostic>> {
    validate_unresolved_trusts(report)
}

fn validate_unresolved_trusts(report: &TrustReport) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = report
        .unresolved_trusts
        .iter()
        .filter_map(|(_, unresolved)| unresolved_trust_diagnostic(unresolved))
        .collect::<Vec<_>>();

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn unresolved_trust_diagnostic(unresolved: &UnresolvedTrustReference) -> Option<Diagnostic> {
    if unresolved.capability == "target" {
        return Some(Diagnostic::error(format!(
            "unresolved target trust `{}` for target `{}`",
            unresolved.trust_level, unresolved.state
        )));
    }
    if let Some(context) = operator_trust_diagnostic_context(unresolved) {
        return Some(Diagnostic::error(format!(
            "unresolved operator trust `{}` for {} `{}`",
            unresolved.trust_level, context, unresolved.state
        )));
    }
    Some(Diagnostic::error(format!(
        "unresolved trust `{}` for trusted contract `{}::{}`",
        unresolved.trust_level, unresolved.capability, unresolved.state
    )))
}

fn operator_trust_diagnostic_context(unresolved: &UnresolvedTrustReference) -> Option<&str> {
    if unresolved.capability == "operator" {
        return Some("operator");
    }
    unresolved
        .capability
        .strip_prefix("domain operator ")
        .map(|_| "domain operator")
}

#[cfg(test)]
mod tests {
    use super::validate_trust_report;
    use omega_artifacts::{TrustReport, UnresolvedTrustReference};

    #[test]
    fn validation_rejects_unresolved_trusted_contracts() {
        let mut report = TrustReport::default();
        report.unresolved_trusts.insert(UnresolvedTrustReference {
            capability: "TestHost".to_owned(),
            state: "host_write".to_owned(),
            trust_level: "missing_host_write_contract".to_owned(),
        });

        let diagnostics = validate_trust_report(&report).expect_err("unresolved library trust");
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(combined.contains("unresolved trust `missing_host_write_contract`"));
    }

    #[test]
    fn validation_rejects_unresolved_target_policy_references() {
        let mut report = TrustReport::default();
        report.unresolved_trusts.insert(UnresolvedTrustReference {
            capability: "target".to_owned(),
            state: "native".to_owned(),
            trust_level: "missing_target_root".to_owned(),
        });

        let diagnostics = validate_trust_report(&report).expect_err("unresolved target trust");
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(combined.contains("unresolved target trust `missing_target_root`"));
    }
}
