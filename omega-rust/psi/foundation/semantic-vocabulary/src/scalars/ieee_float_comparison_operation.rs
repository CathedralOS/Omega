//! Executable IEEE relations, distinct from structural proof equality.
//!
//! NaNs make every ordered relation and equality false, but inequality true.
//! Signed zeros compare equal. These are operation meanings, not permission to
//! replace an authored selected operator with a builtin comparison.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum IeeeFloatComparisonOperation {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl IeeeFloatComparisonOperation {
    /// In ordinal order; operations are appended, never renumbered.
    pub const ALL: [Self; 6] = [
        Self::Equal,
        Self::NotEqual,
        Self::Less,
        Self::LessOrEqual,
        Self::Greater,
        Self::GreaterOrEqual,
    ];

    /// The operation's position in [`Self::ALL`]. Selected-form ordinals
    /// derive from this one table.
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Equal => 0,
            Self::NotEqual => 1,
            Self::Less => 2,
            Self::LessOrEqual => 3,
            Self::Greater => 4,
            Self::GreaterOrEqual => 5,
        }
    }
}
