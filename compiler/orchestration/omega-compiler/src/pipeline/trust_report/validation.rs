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
        return None;
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
