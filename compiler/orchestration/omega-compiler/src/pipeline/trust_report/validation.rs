use omega_artifacts::{TrustReport, UnresolvedTrustReference};
use omega_core::diagnostics::Diagnostic;

pub(super) fn validate_trust_report(report: &TrustReport) -> Result<(), Vec<Diagnostic>> {
    validate_operator_trusts(report)
}

fn validate_operator_trusts(report: &TrustReport) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = report
        .unresolved_trusts
        .iter()
        .filter_map(|(_, unresolved)| {
            operator_trust_diagnostic_context(unresolved).map(|context| {
                Diagnostic::error(format!(
                    "unresolved operator trust `{}` for {} `{}`",
                    unresolved.trust_level, context, unresolved.state
                ))
            })
        })
        .collect::<Vec<_>>();

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
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
