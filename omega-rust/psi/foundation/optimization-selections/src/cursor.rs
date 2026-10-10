//! Shared bounded-cursor error vocabulary for the workspace's wire decoders:
//! the two unit error shapes every crate-local cursor returns. Each decoder
//! maps these onto its own domain error through `From`.

/// The cursor exhausted before the requested read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Truncated;

/// The read failed either by exhaustion or by an oversized length; each
/// decoder maps it onto its own enum's Truncated/LengthOverflow variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakeError {
    Truncated,
    LengthOverflow,
}
