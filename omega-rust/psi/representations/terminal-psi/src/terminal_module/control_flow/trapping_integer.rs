//! Runtime-checked fixed-integer primitives under the `Trapping` policy.
//!
//! # Why the operation is its own crash site
//!
//! A `Trapping` operation evaluates its primitive's exact mathematical result
//! and either returns it (when every trap predicate of the settled
//! `numerics::integer_policy` row is false) or crashes with cause `Trap` at the
//! operation itself. The language contract (`numeric_values.md`,
//! `structural_predicates.md`) requires that crash to keep its exact cause,
//! its primitive denotation, and a path-conditioned site checked against the
//! machine's published same-cause ceiling.
//!
//! Terminal already had two crash coordinates: a `Crash` terminator edge and
//! the declared routes of a `BoundaryCall`. Neither fits: lowering `a + b` to
//! a comparison, a `Conditional`, and a `Crash` terminator would fabricate a
//! terminator edge and lose the primitive denotation (native realization and
//! independent replay would see an ordinary branch, not the Trapping add),
//! and there is no boundary identity to key a boundary row. The chosen
//! encoding therefore makes the operation the site:
//!
//! - one `OperationKind::TrappingInteger` variant carries the primitive and
//!   its operands. Its cause is fixed by the primitive (always `Trap`), so no
//!   producer-supplied cause or guard can drift from it;
//! - the observation profile appends a separate operation-crash-site group
//!   (machine, block, operation, cause, primitive denotation) that the
//!   verifier derives from every such operation, and moves to revision 2
//!   (`wiki/spec/terminal-psi/observations.md`). Re-keying group 3 by an
//!   edge-or-operation sum was rejected because every existing group-3
//!   consumer would re-read a changed key, and widening group 4 would strip
//!   the boundary identity its rows must keep. Groups 3 and 4 keep their
//!   exact shapes;
//! - the path context is the operation's own program point: a Terminal
//!   `OperationId` names one operation in one block, so the same source
//!   operation under a different guard lowers to a different operation and
//!   a different row, exactly as group 3 relies on edge identity.
//!
//! Coverage follows the crash-terminator rule: the machine contract must
//! publish a `Trap` bucket that covers the site. With no retained incoming
//! conjunction on the operation, only an unconditional (`Truth`) bucket
//! covers it; a guarded `Trap` ceiling over Trapping arithmetic rejects
//! rather than being silently widened. Retaining the incoming conjunction for
//! guarded ceilings is an additive extension of this operation, not of the
//! profile row.
//!
//! The live-claim frontier at the site is not carried: like a boundary-call
//! crash, it is exactly the frontier the verifier reconstructs before the
//! operation and the interpreter holds at run time.

use semantic_vocabulary::ValueId;

/// One Trapping primitive and its operands. Operand typing follows the exact
/// sibling Exact operation: binary arithmetic reads two values of the result
/// type, shifts read a value of the result type and an independently typed
/// integer count, and a conversion reads one integer of any fixed type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrappingIntegerOperation {
    /// Traps when `left + right` is outside the result carrier.
    Add { left: ValueId, right: ValueId },
    /// Traps when `left - right` is outside the result carrier.
    Subtract { left: ValueId, right: ValueId },
    /// Traps when `left * right` is outside the result carrier.
    Multiply { left: ValueId, right: ValueId },
    /// Traps on a zero divisor or a signed `MIN / -1`; otherwise returns the
    /// quotient truncated toward zero.
    Divide { left: ValueId, right: ValueId },
    /// Traps on a zero divisor or a signed `MIN % -1` (whose common quotient
    /// is unrepresentable); otherwise returns the dividend-signed remainder.
    Remainder { left: ValueId, right: ValueId },
    /// Traps when `count` is outside `0..width` or `value * 2^count` is
    /// outside the result carrier.
    ShiftLeft { value: ValueId, count: ValueId },
    /// Traps when `count` is outside `0..width`; otherwise an arithmetic
    /// (sign-preserving) right shift.
    ShiftRight { value: ValueId, count: ValueId },
    /// Traps when the operand's value is not representable in the result
    /// carrier; otherwise returns that same mathematical value.
    Convert { operand: ValueId },
}

/// The closed primitive family of [`TrappingIntegerOperation`], without
/// operands. Observation rows and denotation tables key on this tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrappingIntegerPrimitive {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
    Convert,
}

impl TrappingIntegerOperation {
    pub const fn primitive(self) -> TrappingIntegerPrimitive {
        match self {
            Self::Add { .. } => TrappingIntegerPrimitive::Add,
            Self::Subtract { .. } => TrappingIntegerPrimitive::Subtract,
            Self::Multiply { .. } => TrappingIntegerPrimitive::Multiply,
            Self::Divide { .. } => TrappingIntegerPrimitive::Divide,
            Self::Remainder { .. } => TrappingIntegerPrimitive::Remainder,
            Self::ShiftLeft { .. } => TrappingIntegerPrimitive::ShiftLeft,
            Self::ShiftRight { .. } => TrappingIntegerPrimitive::ShiftRight,
            Self::Convert { .. } => TrappingIntegerPrimitive::Convert,
        }
    }

    /// The operands in evaluation order: `[left, right]`, `[value, count]`,
    /// or `[operand]`.
    pub fn operands(self) -> Vec<ValueId> {
        match self {
            Self::Add { left, right }
            | Self::Subtract { left, right }
            | Self::Multiply { left, right }
            | Self::Divide { left, right }
            | Self::Remainder { left, right } => vec![left, right],
            Self::ShiftLeft { value, count } | Self::ShiftRight { value, count } => {
                vec![value, count]
            }
            Self::Convert { operand } => vec![operand],
        }
    }

    /// Rewrite every operand through `map`, preserving the primitive.
    pub fn map_operands(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
        match self {
            Self::Add { left, right }
            | Self::Subtract { left, right }
            | Self::Multiply { left, right }
            | Self::Divide { left, right }
            | Self::Remainder { left, right } => {
                *left = map(*left);
                *right = map(*right);
            }
            Self::ShiftLeft { value, count } | Self::ShiftRight { value, count } => {
                *value = map(*value);
                *count = map(*count);
            }
            Self::Convert { operand } => *operand = map(*operand),
        }
    }
}
