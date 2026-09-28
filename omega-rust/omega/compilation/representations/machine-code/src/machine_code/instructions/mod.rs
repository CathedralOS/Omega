//! Instruction-specific mechanics and exact semantic occurrences.

pub mod x86_fma;
pub use x86_fma::{
    X86FloatingControlRecord, X86ForeignCallFloatingControlRecord, X86ScalarFmaFormat,
    X86ScalarFmaFragment, X86ScalarFmaOccurrenceRecord, X86ScalarFmaOperandRecord,
    x86_scalar_fma_fragment_identity,
};
