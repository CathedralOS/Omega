//! The fixed-integer `Trapping` primitives the physical route realizes,
//! shared by the legalized and selected instruction vocabularies.
//!
//! # Why one form is one instruction with an in-function trap
//!
//! A Terminal `TrappingInteger` operation is its own `Trap` crash site
//! (`terminal-psi/.../control_flow/trapping_integer.rs`): it returns the
//! primitive's exact result, or crashes when a settled
//! `numerics::integer_policy` Trapping predicate holds (result outside the
//! carrier, zero divisor, signed `MIN / -1` or `MIN % -1`, shift count outside
//! `0..width`, an unrepresentable conversion). Terminal target control nodes
//! are blocks and edges, so realizing the check as a compare, a conditional
//! branch and a separate `Crash` block would fabricate a control edge with no
//! Terminal identity. Each form is therefore one selected instruction whose
//! encoding computes the result and branches over an inline architectural
//! trap (`brk #0` / `ud2`, the same bytes the `Crash` leaf emits) when the
//! predicate holds, the way the hosted `*OrTrapV1` forms trap in place.
//!
//! # Why the carrier is part of the form
//!
//! Scalar transport keeps narrow carriers sign- or zero-normalized in 64-bit
//! registers, so the realization computes the exact 64-bit result of
//! normalized narrow operands and checks it against the bounds this carrier
//! names (the 64-bit carriers read the overflow or carry condition, or the
//! product's high half, instead). A form naming another width would check the
//! wrong bounds while every register-level check still passed. A conversion
//! names its destination carrier and only its source sign: normalization makes
//! the source register the exact mathematical value, so representability in
//! the destination depends on the source sign and never on the source width.
//! Carriers reuse the fixed-carrier vocabulary saturating arithmetic
//! introduced; address carriers and other widths have no Trapping form.
use super::SaturatingCarrier;
use semantic_vocabulary::IntegerSign;

/// The primitive a [`TrappingForm`] realizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrappingOperation {
    /// Traps when the exact sum is outside the carrier.
    Add,
    /// Traps when the exact difference is outside the carrier.
    Subtract,
    /// Traps when the exact product is outside the carrier.
    Multiply,
    /// Traps on a zero divisor or a signed `MIN / -1`; otherwise the quotient
    /// truncated toward zero.
    Divide,
    /// Traps on a zero divisor or a signed `MIN % -1`; otherwise the
    /// dividend-signed remainder.
    Remainder,
    /// Traps when the count is outside `0..width` or `value * 2^count` is
    /// outside the carrier. The count may have any fixed integer type: its
    /// normalized register value, read unsigned, is below the width exactly
    /// when the count is inside `0..width`.
    ShiftLeft,
    /// Traps when the count is outside `0..width`; otherwise the
    /// sign-preserving (signed) or zero-filling (unsigned) right shift.
    ShiftRight,
    /// Traps when the normalized source value of the named sign is not
    /// representable in the carrier; otherwise returns that same value.
    Convert { source: IntegerSign },
}

impl TrappingOperation {
    /// Every operation, in ordinal order.
    pub const ALL: [Self; 9] = [
        Self::Add,
        Self::Subtract,
        Self::Multiply,
        Self::Divide,
        Self::Remainder,
        Self::ShiftLeft,
        Self::ShiftRight,
        Self::Convert {
            source: IntegerSign::Signed,
        },
        Self::Convert {
            source: IntegerSign::Unsigned,
        },
    ];

    /// The dense position of this operation in [`Self::ALL`].
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Add => 0,
            Self::Subtract => 1,
            Self::Multiply => 2,
            Self::Divide => 3,
            Self::Remainder => 4,
            Self::ShiftLeft => 5,
            Self::ShiftRight => 6,
            Self::Convert {
                source: IntegerSign::Signed,
            } => 7,
            Self::Convert {
                source: IntegerSign::Unsigned,
            } => 8,
        }
    }
}

/// One realized Trapping primitive at one result carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrappingForm {
    pub operation: TrappingOperation,
    /// The result carrier. Its bounds decide the range traps and, for a
    /// shift, its width decides the count trap.
    pub carrier: SaturatingCarrier,
}

impl TrappingForm {
    /// The number of distinct forms: every operation at every carrier.
    pub const COUNT: usize = TrappingOperation::ALL.len() * SaturatingCarrier::ALL.len();

    /// Every form, operation-major in [`Self::ordinal`] order.
    pub const ALL: [Self; Self::COUNT] = {
        let mut forms = [Self {
            operation: TrappingOperation::Add,
            carrier: SaturatingCarrier::I8,
        }; Self::COUNT];
        let mut operation = 0;
        while operation < TrappingOperation::ALL.len() {
            let mut carrier = 0;
            while carrier < SaturatingCarrier::ALL.len() {
                forms[operation * SaturatingCarrier::ALL.len() + carrier] = Self {
                    operation: TrappingOperation::ALL[operation],
                    carrier: SaturatingCarrier::ALL[carrier],
                };
                carrier += 1;
            }
            operation += 1;
        }
        forms
    };

    /// The dense position of this form in [`Self::ALL`]. Identity tables add
    /// it to a per-table base; forms are appended, never renumbered.
    pub const fn ordinal(self) -> u8 {
        self.operation.ordinal() * SaturatingCarrier::ALL.len() as u8 + self.carrier.ordinal()
    }

    /// The form at `ordinal`, the inverse of [`Self::ordinal`].
    pub const fn from_ordinal(ordinal: u8) -> Option<Self> {
        if (ordinal as usize) < Self::COUNT {
            Some(Self::ALL[ordinal as usize])
        } else {
            None
        }
    }

    /// The number of source operands: a conversion reads one, every other
    /// primitive reads two (`[left, right]` or `[value, count]`).
    pub const fn source_count(self) -> usize {
        match self.operation {
            TrappingOperation::Convert { .. } => 1,
            _ => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SaturatingCarrier, TrappingForm, TrappingOperation};

    #[test]
    fn every_form_has_a_distinct_dense_ordinal() {
        for (index, form) in TrappingForm::ALL.iter().enumerate() {
            assert_eq!(usize::from(form.ordinal()), index);
            assert_eq!(TrappingForm::from_ordinal(form.ordinal()), Some(*form));
        }
        assert_eq!(TrappingForm::COUNT, 72);
        assert_eq!(TrappingForm::from_ordinal(72), None);
        for operation in TrappingOperation::ALL {
            for carrier in SaturatingCarrier::ALL {
                assert!(TrappingForm::ALL.contains(&TrappingForm { operation, carrier }));
            }
        }
    }
}
