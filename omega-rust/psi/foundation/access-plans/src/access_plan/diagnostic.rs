//! The diagnostic every access-plan operation reports.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPlanDiagnostic(pub String);

impl std::fmt::Display for AccessPlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AccessPlanDiagnostic {}

/// Reject the enclosing operation when any named comparison fails, reporting
/// the message bound to it. Every admission and custody check in this crate
/// compares one retained fact and reports one sentence naming that fact: the
/// comparison and its wording stay at the call site, only the return shape is
/// emitted here.
macro_rules! reject_when {
    ($($condition:expr => $message:expr),+ $(,)?) => {
        $(
            if $condition {
                return Err(AccessPlanDiagnostic($message.into()));
            }
        )+
    };
}

pub(crate) use reject_when;
