//! Executable IEEE relations, distinct from structural proof equality.
//!
//! NaNs make every ordered relation and equality false, but inequality true.
//! Signed zeros compare equal. These are operation meanings, not permission to
//! replace an authored selected operator with a builtin comparison.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IeeeFloatComparisonOperation {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}
