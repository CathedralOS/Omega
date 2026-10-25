//! Native storage for an admitted indivisible atomic event.
//!
//! Boolean values remain Boolean in Terminal Psi, operands and result homes.
//! Their hosted storage lane is one byte, independently of the logical type.
//! Integer cells keep their own fixed-width lane. Production and independent
//! reconstruction consume this same relation only after authenticating the
//! exact dedicated cell. It supplies no authority, alignment, resident validity,
//! or target instruction support, and never admits Boolean RMW operations.

use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use terminal_psi::AtomicAccessEvent;

pub fn atomic_storage_integer_type(
    event: AtomicAccessEvent,
    logical_type: ScalarType,
) -> Option<IntegerType> {
    if !event.ordering_is_legal() {
        return None;
    }
    match logical_type {
        ScalarType::Boolean
            if matches!(
                event,
                AtomicAccessEvent::Load { .. } | AtomicAccessEvent::Store { .. }
            ) =>
        {
            IntegerType::new(IntegerSign::Unsigned, 8).ok()
        }
        ScalarType::Integer(integer)
            if !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            Some(integer)
        }
        _ => None,
    }
}
