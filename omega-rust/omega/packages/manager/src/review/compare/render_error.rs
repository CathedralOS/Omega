//! Bounded rendering failures for the human review form.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOnlyCapabilityConflictRenderError {
    LimitExceeded {
        maximum_bytes: usize,
        required_bytes: usize,
    },
    AllocationFailed,
}

impl ReviewOnlyCapabilityConflictRenderError {
    pub const fn maximum_bytes(self) -> Option<usize> {
        match self {
            Self::LimitExceeded { maximum_bytes, .. } => Some(maximum_bytes),
            Self::AllocationFailed => None,
        }
    }

    pub const fn required_bytes(self) -> Option<usize> {
        match self {
            Self::LimitExceeded { required_bytes, .. } => Some(required_bytes),
            Self::AllocationFailed => None,
        }
    }
}

impl fmt::Display for ReviewOnlyCapabilityConflictRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded {
                maximum_bytes,
                required_bytes,
            } => write!(
                formatter,
                "capability conflict view requires {required_bytes} bytes, exceeding the {maximum_bytes}-byte ceiling"
            ),
            Self::AllocationFailed => {
                formatter.write_str("capability conflict view allocation failed")
            }
        }
    }
}

impl std::error::Error for ReviewOnlyCapabilityConflictRenderError {}
