//! Nearest-even IEEE arithmetic forms: one operation at one interchange format.
//!
//! Scalar graphs hold IEEE values as raw payloads in general registers, the
//! same carrier IEEE comparison and field storage use. A form therefore reads
//! two general registers and defines one: its encoding moves both payloads
//! into fixed scratch floating registers, performs one scalar operation under
//! Omega's canonical floating controls (round to nearest, ties to even; no
//! trap unmasking; no flush to zero), and moves the result payload back. The
//! scratch registers and the sticky status register are declared clobbers.

pub use semantic_vocabulary::IeeeFloatFormat;
pub use target_operations::IeeeFloatArithmeticOperation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IeeeFloatArithmeticForm {
    pub operation: IeeeFloatArithmeticOperation,
    pub format: IeeeFloatFormat,
}

impl IeeeFloatArithmeticForm {
    pub const COUNT: usize = 8;
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
        let operation = match self.operation {
            IeeeFloatArithmeticOperation::Add => 0,
            IeeeFloatArithmeticOperation::Subtract => 1,
            IeeeFloatArithmeticOperation::Multiply => 2,
            IeeeFloatArithmeticOperation::Divide => 3,
        };
        let format = match self.format {
            IeeeFloatFormat::Binary32 => 0,
            IeeeFloatFormat::Binary64 => 1,
        };
        operation * 2 + format
    }

    /// The one machine-semantic and alternative-family tag of each form:
    /// 230 plus its dense ordinal (230 through 237), above every fixed,
    /// saturating, Trapping and atomic tag. Forms are appended, never
    /// renumbered.
    pub const fn machine_tag(self) -> u8 {
        230 + self.ordinal()
    }

    pub const fn from_machine_tag(tag: u8) -> Option<Self> {
        if tag >= 230 && ((tag - 230) as usize) < Self::COUNT {
            Some(Self::ALL[(tag - 230) as usize])
        } else {
            None
        }
    }

    /// Whether the operation is binary32 (single-precision) arithmetic.
    pub const fn is_binary32(self) -> bool {
        matches!(self.format, IeeeFloatFormat::Binary32)
    }
}

#[cfg(test)]
mod tests {
    use super::IeeeFloatArithmeticForm;

    #[test]
    fn forms_are_dense_distinct_and_round_trip_their_tags() {
        for (ordinal, form) in IeeeFloatArithmeticForm::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(form.ordinal()), ordinal);
            assert_eq!(
                IeeeFloatArithmeticForm::from_machine_tag(form.machine_tag()),
                Some(form)
            );
        }
        assert_eq!(IeeeFloatArithmeticForm::from_machine_tag(229), None);
        assert_eq!(IeeeFloatArithmeticForm::from_machine_tag(238), None);
    }
}
