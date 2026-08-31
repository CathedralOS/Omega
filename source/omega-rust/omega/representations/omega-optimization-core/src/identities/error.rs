use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityDecodeError {
    WrongLength { expected: usize, actual: usize },
}

impl fmt::Display for IdentityDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength { expected, actual } => {
                write!(formatter, "identity is {actual} bytes, expected {expected}")
            }
        }
    }
}

impl std::error::Error for IdentityDecodeError {}
