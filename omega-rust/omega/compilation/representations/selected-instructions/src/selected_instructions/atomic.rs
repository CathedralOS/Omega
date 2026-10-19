//! Indivisible native atomic accesses. The baseline forms provide sequentially
//! consistent machine ordering; weaker source requests may use that stronger
//! realization. Source ordering remains bound by legalization and selection
//! replay, not inferred from the register operands.
//!
//! Width and operation determine the machine form. Signedness is a scalar
//! transport concern: the instruction returns raw low-width observed bits,
//! followed by the usual signed or unsigned normalization. No form may replace
//! an observed prior with a separate load. A target using software retries must
//! additionally expose that control and work in its encoded effects; these
//! semantic forms do not themselves grant a fixed-work or termination claim.

use semantic_vocabulary::IntegerType;
use terminal_psi::{AtomicAccessEvent, AtomicReadModifyWrite};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicWidth {
    Bits8,
    Bits16,
    Bits32,
    Bits64,
}

impl AtomicWidth {
    pub const ALL: [Self; 4] = [Self::Bits8, Self::Bits16, Self::Bits32, Self::Bits64];

    pub const fn bytes(self) -> u8 {
        match self {
            Self::Bits8 => 1,
            Self::Bits16 => 2,
            Self::Bits32 => 4,
            Self::Bits64 => 8,
        }
    }

    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Bits8 => 0,
            Self::Bits16 => 1,
            Self::Bits32 => 2,
            Self::Bits64 => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomicOperation {
    Load,
    Store,
    FetchAdd,
    FetchSub,
    FetchAnd,
    FetchOr,
    FetchXor,
    Swap,
    CompareExchange,
}

impl AtomicOperation {
    pub const ALL: [Self; 9] = [
        Self::Load,
        Self::Store,
        Self::FetchAdd,
        Self::FetchSub,
        Self::FetchAnd,
        Self::FetchOr,
        Self::FetchXor,
        Self::Swap,
        Self::CompareExchange,
    ];

    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Load => 0,
            Self::Store => 1,
            Self::FetchAdd => 2,
            Self::FetchSub => 3,
            Self::FetchAnd => 4,
            Self::FetchOr => 5,
            Self::FetchXor => 6,
            Self::Swap => 7,
            Self::CompareExchange => 8,
        }
    }

    /// Number of incoming operands, including the address at position zero.
    pub const fn input_count(self) -> u16 {
        match self {
            Self::Load => 1,
            Self::CompareExchange => 3,
            _ => 2,
        }
    }

    pub const fn observes_resident(self) -> bool {
        !matches!(self, Self::Store)
    }
    pub const fn modifies_resident(self) -> bool {
        !matches!(self, Self::Load)
    }
}

/// A target must admit the matching constraint and checked encoding before
/// this form can be selected. Existence in the vocabulary is not availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AtomicForm {
    pub operation: AtomicOperation,
    pub width: AtomicWidth,
}

impl AtomicForm {
    pub const COUNT: usize = AtomicOperation::ALL.len() * AtomicWidth::ALL.len();
    pub const ALL: [Self; Self::COUNT] = {
        let mut forms = [Self {
            operation: AtomicOperation::Load,
            width: AtomicWidth::Bits8,
        }; Self::COUNT];
        let mut operation = 0;
        while operation < AtomicOperation::ALL.len() {
            let mut width = 0;
            while width < AtomicWidth::ALL.len() {
                forms[operation * AtomicWidth::ALL.len() + width] = Self {
                    operation: AtomicOperation::ALL[operation],
                    width: AtomicWidth::ALL[width],
                };
                width += 1;
            }
            operation += 1;
        }
        forms
    };

    pub const fn ordinal(self) -> u8 {
        self.operation.ordinal() * AtomicWidth::ALL.len() as u8 + self.width.ordinal()
    }

    pub const fn from_ordinal(ordinal: u8) -> Option<Self> {
        if (ordinal as usize) < Self::COUNT {
            Some(Self::ALL[ordinal as usize])
        } else {
            None
        }
    }

    /// Append-only machine identity tag, following Trapping's 122..193.
    pub const fn machine_tag(self) -> u8 {
        194 + self.ordinal()
    }

    pub const fn from_machine_tag(tag: u8) -> Option<Self> {
        if tag >= 194 {
            Self::from_ordinal(tag - 194)
        } else {
            None
        }
    }

    /// Consume only a legal, fixed-width source access. In particular an
    /// illegal failure ordering does not become legal merely because the
    /// baseline hardware form would order more strongly.
    pub fn from_access(integer: IntegerType, event: AtomicAccessEvent) -> Option<Self> {
        if integer.is_address() || !event.ordering_is_legal() {
            return None;
        }
        let width = match integer.bits() {
            8 => AtomicWidth::Bits8,
            16 => AtomicWidth::Bits16,
            32 => AtomicWidth::Bits32,
            64 => AtomicWidth::Bits64,
            _ => return None,
        };
        let operation = match event {
            AtomicAccessEvent::Load { .. } => AtomicOperation::Load,
            AtomicAccessEvent::Store { .. } => AtomicOperation::Store,
            AtomicAccessEvent::Swap { .. } => AtomicOperation::Swap,
            AtomicAccessEvent::CompareExchange { .. } => AtomicOperation::CompareExchange,
            AtomicAccessEvent::CompareExchangeOnce { .. } => AtomicOperation::CompareExchange,
            AtomicAccessEvent::ReadModifyWrite { operation, .. } => match operation {
                AtomicReadModifyWrite::FetchAdd => AtomicOperation::FetchAdd,
                AtomicReadModifyWrite::FetchSub => AtomicOperation::FetchSub,
                AtomicReadModifyWrite::FetchAnd => AtomicOperation::FetchAnd,
                AtomicReadModifyWrite::FetchOr => AtomicOperation::FetchOr,
                AtomicReadModifyWrite::FetchXor => AtomicOperation::FetchXor,
            },
        };
        Some(Self { operation, width })
    }

    /// The observed-value output follows the preserved incoming operands.
    pub const fn observed_operand(self) -> Option<u16> {
        if self.operation.observes_resident() {
            Some(self.operation.input_count())
        } else {
            None
        }
    }

    pub const fn operand_count(self) -> u16 {
        self.operation.input_count() + self.operation.observes_resident() as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, ValueId};
    use terminal_psi::MemoryOrdering;

    #[test]
    fn atomic_forms_have_unique_dense_identities_and_exact_operand_roles() {
        for (ordinal, form) in AtomicForm::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(form.ordinal()), ordinal);
            assert_eq!(AtomicForm::from_ordinal(form.ordinal()), Some(form));
            assert_eq!(
                form.operand_count(),
                match form.operation {
                    AtomicOperation::Load | AtomicOperation::Store => 2,
                    AtomicOperation::CompareExchange => 4,
                    _ => 3,
                }
            );
            assert_eq!(
                form.observed_operand().is_some(),
                form.operation != AtomicOperation::Store
            );
        }
        assert_eq!(AtomicForm::from_ordinal(AtomicForm::COUNT as u8), None);
    }

    #[test]
    fn stronger_native_ordering_does_not_admit_illegal_source_ordering() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let value = ValueId::new(1).unwrap();
        for event in [
            AtomicAccessEvent::Load {
                ordering: MemoryOrdering::Publish,
            },
            AtomicAccessEvent::Store {
                value,
                ordering: MemoryOrdering::Receive,
            },
            AtomicAccessEvent::CompareExchange {
                expected: value,
                replacement: value,
                success: MemoryOrdering::NoOrdering,
                failure: MemoryOrdering::Receive,
            },
        ] {
            assert_eq!(AtomicForm::from_access(integer, event), None);
        }
        for ordering in [
            MemoryOrdering::NoOrdering,
            MemoryOrdering::Receive,
            MemoryOrdering::Publish,
            MemoryOrdering::ReceivePublish,
            MemoryOrdering::GlobalOrder,
        ] {
            let form = AtomicForm::from_access(
                integer,
                AtomicAccessEvent::ReadModifyWrite {
                    operation: AtomicReadModifyWrite::FetchAdd,
                    operand: value,
                    ordering,
                },
            )
            .unwrap();
            assert_eq!(form.operation, AtomicOperation::FetchAdd);
            assert_eq!(form.width, AtomicWidth::Bits32);
        }
    }
}
