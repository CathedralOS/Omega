/*
Control-flow, arithmetic carrier, and case-dispatch vocabulary for legalized scalar functions.

We define the fixed-width integer carrier and policy forms shared by legalized and selected
instructions (`SaturatingCarrier`, `SaturatingOperation`, `TrappingOperation`, `TrappingForm`),
the structural case-dispatch successor edges (`LegalizedStructuralCaseSuccessor`,
`LegalizedStructuralCasePayload`), and re-export the ordered basic-block and instruction graph
types from `scalar_graph`.
*/

/*
Why saturating and trapping forms carry their result carrier explicitly:
Scalar transport keeps narrow 8/16/32-bit carriers sign- or zero-normalized in 64-bit general
registers. Realization therefore computes the exact 64-bit result of normalized narrow operands
(or reads the 64-bit overflow/carry condition or high product half on 64-bit carriers) and clamps
or traps against the bounds named by `SaturatingCarrier`. Naming the carrier inside the operation
form prevents a narrow operation from silently clamping or trapping against a different width's
bounds while register-level checks still pass.

Why a `TrappingForm` is a single instruction with an inline architectural trap:
A Terminal `TrappingInteger` operation is its own `Trap` crash site: it produces the exact result
or crashes when a `numerics::integer_policy` predicate holds (overflow, zero divisor, signed
`MIN / -1` or `MIN % -1`, shift count outside `0..width`, or unrepresentable conversion). Because
Terminal target control nodes are basic blocks and edges, expanding a trapping arithmetic op into
a compare, conditional branch, and separate `Crash` block would fabricate a control edge with no
Terminal identity. Keeping each `TrappingForm` as one instruction that branches over an inline
architectural trap (`brk #0` / `ud2`) preserves a 1-to-1 control-flow correspondence.
*/

mod scalar_graph;
pub use scalar_graph::{
    LegalizedCasePointerSlot, LegalizedDescriptorOrigin, LegalizedNormalizedForeignCall,
    LegalizedRuntimeIndexOperand, LegalizedScalarArgument, LegalizedScalarBlock,
    LegalizedScalarCall, LegalizedScalarComparison, LegalizedScalarFunction,
    LegalizedScalarInstruction, LegalizedScalarInstructionKind, LegalizedScalarParameter,
    LegalizedScalarReturn, LegalizedScalarReturnValue, LegalizedScalarSuccessor,
    LegalizedScalarTerminator, LegalizedStructuralBinding, LegalizedStructuralCaseSource,
    LegalizedValueDefinition,
};

use optimization_unit::WorkSettlement;
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerCarrier, IntegerSign, IntegerType, StructuralCaseId, StructuralFieldId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SaturatingCarrier {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SaturatingOperation {
    Add,
    Subtract,
    Divide,
    Remainder,
    Multiply,
}

impl SaturatingCarrier {
    pub const ALL: [Self; 8] = [
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
    ];

    pub fn from_integer(integer: IntegerType) -> Option<Self> {
        if integer.carrier() != IntegerCarrier::Fixed {
            return None;
        }
        Some(match (integer.sign(), integer.bits()) {
            (IntegerSign::Signed, 8) => Self::I8,
            (IntegerSign::Signed, 16) => Self::I16,
            (IntegerSign::Signed, 32) => Self::I32,
            (IntegerSign::Signed, 64) => Self::I64,
            (IntegerSign::Unsigned, 8) => Self::U8,
            (IntegerSign::Unsigned, 16) => Self::U16,
            (IntegerSign::Unsigned, 32) => Self::U32,
            (IntegerSign::Unsigned, 64) => Self::U64,
            _ => return None,
        })
    }

    pub fn integer_type(self) -> IntegerType {
        IntegerType::new(self.sign(), self.bits()).expect("saturating carriers are fixed widths")
    }

    pub const fn sign(self) -> IntegerSign {
        if self.is_signed() {
            IntegerSign::Signed
        } else {
            IntegerSign::Unsigned
        }
    }

    pub const fn is_signed(self) -> bool {
        matches!(self, Self::I8 | Self::I16 | Self::I32 | Self::I64)
    }

    pub const fn bits(self) -> u16 {
        match self {
            Self::I8 | Self::U8 => 8,
            Self::I16 | Self::U16 => 16,
            Self::I32 | Self::U32 => 32,
            Self::I64 | Self::U64 => 64,
        }
    }

    pub const fn is_narrow(self) -> bool {
        self.bits() < 64
    }

    pub const fn minimum_bits(self) -> u64 {
        match self {
            Self::I8 => i8::MIN as i64 as u64,
            Self::I16 => i16::MIN as i64 as u64,
            Self::I32 => i32::MIN as i64 as u64,
            Self::I64 => i64::MIN as u64,
            Self::U8 | Self::U16 | Self::U32 | Self::U64 => 0,
        }
    }

    pub const fn maximum_bits(self) -> u64 {
        match self {
            Self::I8 => i8::MAX as u64,
            Self::I16 => i16::MAX as u64,
            Self::I32 => i32::MAX as u64,
            Self::I64 => i64::MAX as u64,
            Self::U8 => u8::MAX as u64,
            Self::U16 => u16::MAX as u64,
            Self::U32 => u32::MAX as u64,
            Self::U64 => u64::MAX,
        }
    }

    pub const fn ordinal(self) -> u8 {
        match self {
            Self::I8 => 0,
            Self::I16 => 1,
            Self::I32 => 2,
            Self::I64 => 3,
            Self::U8 => 4,
            Self::U16 => 5,
            Self::U32 => 6,
            Self::U64 => 7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrappingOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
    Convert { source: IntegerSign },
}

impl TrappingOperation {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrappingForm {
    pub operation: TrappingOperation,
    pub carrier: SaturatingCarrier, // Result carrier whose bounds decide range traps and shift-count width traps.
}

impl TrappingForm {
    pub const COUNT: usize = TrappingOperation::ALL.len() * SaturatingCarrier::ALL.len();

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

    pub const fn ordinal(self) -> u8 {
        self.operation.ordinal() * SaturatingCarrier::ALL.len() as u8 + self.carrier.ordinal()
    }

    pub const fn from_ordinal(ordinal: u8) -> Option<Self> {
        if (ordinal as usize) < Self::COUNT {
            Some(Self::ALL[ordinal as usize])
        } else {
            None
        }
    }

    pub const fn source_count(self) -> usize {
        match self.operation {
            TrappingOperation::Convert { .. } => 1,
            _ => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalizedStructuralCaseSuccessor {
    pub edge: EdgeId,
    pub target: BlockId,
    pub case: StructuralCaseId,
    pub case_tag: i32,
    pub payloads: Vec<LegalizedStructuralCasePayload>,
    pub structural_bindings: Vec<LegalizedStructuralBinding>,
    pub cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
    pub cleanup_calls: Vec<(target_operations::CallSiteOwner, LegalizedScalarCall)>,
    pub work: Vec<WorkSettlement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegalizedStructuralCasePayload {
    pub field: StructuralFieldId,
    pub field_byte_offset: u32,
    pub parameter: crate::LegalizedValueDefinition, // Target-block scalar arrival defined after observing the selected case payload.
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
