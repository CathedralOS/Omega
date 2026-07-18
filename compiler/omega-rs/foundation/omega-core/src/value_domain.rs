//! Built-in value domains.
//!
//! These domains describe values that may inhabit storage; they are distinct
//! from arithmetic policy domains (which choose operation behaviour) and from
//! user-declared carrier domains.

/// A compiler-known value-domain predicate from Omega's core roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueDomain {
    /// Excludes NaN and both infinities from a floating-point value.
    Finite,
}

impl ValueDomain {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Finite" => Some(Self::Finite),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Finite => "Finite",
        }
    }

    /// Internal proof-constraint spelling retained while the legacy
    /// bracketed named-constraint surface is retired.
    pub fn proof_name(self) -> &'static str {
        match self {
            Self::Finite => "finite",
        }
    }
}
