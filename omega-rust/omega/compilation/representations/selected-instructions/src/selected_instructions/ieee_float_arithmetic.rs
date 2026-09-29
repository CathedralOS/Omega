//! IEEE forms: one operation on one interchange format, rounding to nearest
//! (ties to even) or, for conversion to an integer, toward zero.
//!
//! Scalar graphs hold IEEE values as raw payloads in general registers, the
//! same carrier IEEE comparison and field storage use, and integers in
//! general registers too. A form therefore reads one to three general registers
//! (its operation's operand count) and defines one: its encoding moves or
//! converts the operands into fixed scratch floating registers, performs the
//! operation under Omega's canonical floating controls (round to nearest,
//! ties to even; no trap unmasking; no flush to zero), and moves the result
//! payload back. The scratch registers are declared clobbers.
//!
//! Every form shares machine tag 230, followed in each identity and codec by
//! one payload byte holding its dense ordinal.

pub use semantic_vocabulary::IeeeFloatFormat;
pub use target_operations::{
    IeeeFloatArithmeticOperation, IeeeFloatIntegerCarrier, IeeeFloatToIntegerDomain,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IeeeFloatArithmeticForm {
    pub operation: IeeeFloatArithmeticOperation,
    pub format: IeeeFloatFormat,
}

impl IeeeFloatArithmeticForm {
    pub const COUNT: usize = IeeeFloatArithmeticOperation::ALL.len() * 2;
    pub const ALL: [Self; Self::COUNT] = {
        let formats = [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64];
        let mut forms = [Self {
            operation: IeeeFloatArithmeticOperation::Add,
            format: IeeeFloatFormat::Binary32,
        }; Self::COUNT];
        let mut operation = 0;
        while operation < IeeeFloatArithmeticOperation::ALL.len() {
            let mut format = 0;
            while format < formats.len() {
                forms[operation * formats.len() + format] = Self {
                    operation: IeeeFloatArithmeticOperation::ALL[operation],
                    format: formats[format],
                };
                format += 1;
            }
            operation += 1;
        }
        forms
    };

    /// Dense ordinal: operation-major, binary32 before binary64.
    pub const fn ordinal(self) -> u8 {
        let operation = self.operation.ordinal();
        let format = match self.format {
            IeeeFloatFormat::Binary32 => 0,
            IeeeFloatFormat::Binary64 => 1,
        };
        operation * 2 + format
    }

    /// The one machine-semantic and alternative-family tag every form
    /// shares. The one-byte tag space has no room for a dense block per form,
    /// so each identity, program encoding and recovery codec writes the
    /// form's [`Self::machine_payload`] byte directly after this tag.
    pub const FAMILY_TAG: u8 = 230;

    pub const fn machine_tag(self) -> u8 {
        Self::FAMILY_TAG
    }

    /// The payload byte after [`Self::FAMILY_TAG`]: the dense ordinal.
    /// Forms are appended, never renumbered.
    pub const fn machine_payload(self) -> u8 {
        self.ordinal()
    }

    pub const fn from_machine_payload(payload: u8) -> Option<Self> {
        if (payload as usize) < Self::COUNT {
            Some(Self::ALL[payload as usize])
        } else {
            None
        }
    }

    /// Whether the result is binary32 (single precision).
    pub const fn is_binary32(self) -> bool {
        matches!(self.format, IeeeFloatFormat::Binary32)
    }

    /// The general registers the form reads before its one result.
    pub const fn operand_count(self) -> usize {
        self.operation.operand_count()
    }
}

#[cfg(test)]
mod tests {
    use super::IeeeFloatArithmeticForm;

    #[test]
    fn forms_are_dense_distinct_and_round_trip_their_tags() {
        for (ordinal, form) in IeeeFloatArithmeticForm::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(form.ordinal()), ordinal);
            assert_eq!(form.machine_tag(), IeeeFloatArithmeticForm::FAMILY_TAG);
            assert_eq!(
                IeeeFloatArithmeticForm::from_machine_payload(form.machine_payload()),
                Some(form)
            );
        }
        assert_eq!(
            IeeeFloatArithmeticForm::from_machine_payload(IeeeFloatArithmeticForm::COUNT as u8),
            None
        );
        assert_eq!(IeeeFloatArithmeticForm::ALL.len(), 62);
        // Catalog declarations follow this order, so it must be sorted.
        assert!(IeeeFloatArithmeticForm::ALL.is_sorted());
    }
}
