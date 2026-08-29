use std::fmt;

/// A closed error from projection or strict canonical recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSourceClosureSubjectError {
    message: &'static str,
}

impl CanonicalSourceClosureSubjectError {
    pub(in super::super) fn new(message: &'static str) -> Self {
        Self { message }
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for CanonicalSourceClosureSubjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for CanonicalSourceClosureSubjectError {}
