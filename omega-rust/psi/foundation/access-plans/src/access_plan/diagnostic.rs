//! The diagnostic every access-plan operation reports.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPlanDiagnostic(pub String);

impl std::fmt::Display for AccessPlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AccessPlanDiagnostic {}

/// Routes one access-contract transition: `validate` inspects the carrier,
/// then `accept` builds the validated carrier from it and its evidence, or
/// `reject` returns the intact carrier with the diagnostic.
pub(crate) fn into_validated_access<Input, Evidence, Accepted, Rejected>(
    input: Input,
    validate: impl FnOnce(&Input) -> Result<Evidence, AccessPlanDiagnostic>,
    accept: impl FnOnce(Input, Evidence) -> Accepted,
    reject: impl FnOnce(Input, AccessPlanDiagnostic) -> Rejected,
) -> Result<Accepted, Rejected> {
    match validate(&input) {
        Ok(evidence) => Ok(accept(input, evidence)),
        Err(diagnostic) => Err(reject(input, diagnostic)),
    }
}
