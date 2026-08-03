//! Arithmetic DOMAINS (frozen decision 17). A primitive integer can be tagged
//! with an explicit overflow behaviour via `T in <Domain>`:
//!
//! - default (no domain): EXACT -- overflow is a proof obligation (compile error
//!   if not provably in range; enforcement is a later stage).
//! - `Wrapping`: wraps modulo the fixed-width representation.
//! - `Saturating`: clamps to the representable minimum/maximum.
//! - `Trapping`: checks at runtime and traps on overflow.
//!
//! Carried as a `TypeConstraintNode::ArithmeticDomain` across the type layers; it
//! is a BEHAVIOUR tag (not a value-range predicate like a `Range` constraint), so
//! layout/range consumers ignore it and only the arithmetic emission branches on
//! it.

use crate::float_semantics::FloatFormat;

/// The overflow behaviour of a primitive's arithmetic. `Exact` is the implicit
/// default (no `in <Domain>` suffix); the others are opt-in via the suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArithmeticDomain {
    /// Overflow is a proof obligation (the default; not represented as a
    /// constraint -- the ABSENCE of a domain constraint means exact).
    Exact,
    /// Wraps modulo 2^width.
    Wrapping,
    /// Clamps to the representable minimum/maximum.
    Saturating,
    /// Runtime-checked; traps on overflow.
    Trapping,
}

impl ArithmeticDomain {
    /// Parse a domain name (`Wrapping`/`Saturating`/`Trapping`). `Exact` has no
    /// surface spelling (it is the default), so it is never produced here.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Wrapping" => Some(Self::Wrapping),
            "Saturating" => Some(Self::Saturating),
            "Trapping" => Some(Self::Trapping),
            _ => None,
        }
    }

    /// The domain's surface name (for diagnostics / round-tripping).
    pub fn name(self) -> &'static str {
        match self {
            Self::Exact => "Exact",
            Self::Wrapping => "Wrapping",
            Self::Saturating => "Saturating",
            Self::Trapping => "Trapping",
        }
    }

    /// Combine two operand domains into the binary operation's domain (decision
    /// 17 is OPERAND-driven: the domain lives on each value's type, not on the
    /// assignment target). `Exact` is NEUTRAL -- a literal or exact value adopts
    /// the other operand's domain (`count + 1` where `count` is Saturating is
    /// Saturating). A genuine conflict (two DIFFERENT non-exact domains) is a
    /// type error rejected upstream by the semantics; this picks the left operand
    /// as a deterministic fallback so backend codegen stays total.
    pub fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Exact, domain) | (domain, Self::Exact) => domain,
            (left, _) => left,
        }
    }
}

/// The normalized result adapter selected for one checked arithmetic
/// operation. Unlike [`ArithmeticDomain`], this is already-resolved semantic
/// evidence: downstream lowering must preserve and realize this exact adapter
/// instead of rediscovering policy from operand storage types.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ArithmeticPolicyAdapter {
    /// The checked operation has no result adapter.
    #[default]
    None,
    /// Clamp only magnitude overflow produced from finite operands. Invalid
    /// operations and division by signed zero keep their IEEE non-finite result.
    FloatSaturatingOverflowOnly { format: FloatFormat },
    /// Reject every non-finite semantic result, including a propagated NaN or
    /// infinity.
    FloatTrappingNonFinite { format: FloatFormat },
}

impl ArithmeticPolicyAdapter {
    pub const fn float_format(self) -> Option<FloatFormat> {
        match self {
            Self::None => None,
            Self::FloatSaturatingOverflowOnly { format }
            | Self::FloatTrappingNonFinite { format } => Some(format),
        }
    }
}
