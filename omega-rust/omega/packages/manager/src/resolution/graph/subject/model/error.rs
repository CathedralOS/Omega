use std::fmt;

/// A closed error from projection or strict canonical recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSourceClosureSubjectError {
    message: &'static str,
    allocation_limit_exceeded: bool,
}

impl CanonicalSourceClosureSubjectError {
    pub(in super::super) fn new(message: &'static str) -> Self {
        Self {
            message,
            allocation_limit_exceeded: false,
        }
    }

    pub(in super::super) fn allocation_limit(message: &'static str) -> Self {
        Self {
            message,
            allocation_limit_exceeded: true,
        }
    }

    pub(crate) const fn is_allocation_limit_exceeded(&self) -> bool {
        self.allocation_limit_exceeded
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
