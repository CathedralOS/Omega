//! Readable byte geometry shared by expression formation and checked lowering.
//!
//! A carrier establishes only which observations have meaning. Actual custody,
//! backing, qualification and evaluation order remain with the consuming owner.

use typed_trees::types::PrimitiveType;

/// The carrier of `type_reference` when it has a live byte length and
/// readable bytes: a bounded owned byte buffer, or a shared or mutable
/// borrowed byte view.
/// Observation geometry is independent of the stored collection kind. A
/// fixed array contributes its declared extent; an explicit byte carrier
/// contributes the extent its own representation exposes.
#[derive(Clone)]
pub enum ByteObservationCarrier {
    FixedArray { length: u64 },
    Sequence(checked_trees::CheckedByteSequenceCarrier),
}

pub fn comparable_byte_carrier_type(
    program: &dyn crate::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<ByteObservationCarrier> {
    // Observation follows a readable loan to its actual array. Custody and
    // qualification remain on the original operand; write-only access never
    // authorizes a byte observation.
    let observed_type = match crate::reference_shell(program, type_reference) {
        Some(shell) if shell.access == language_semantics::ReferenceAccess::WriteOnly => {
            return None;
        }
        Some(shell) => shell.referee,
        None => type_reference,
    };
    let storage = program
        .type_reference_table
        .unconstrained_type_reference(observed_type);
    if let typed_trees::types::TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(storage)
        && program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
    {
        return Some(ByteObservationCarrier::FixedArray {
            length: u64::try_from(*length).ok()?,
        });
    }
    crate::byte_sequence_carrier(program, type_reference, &[])
        .filter(|carrier| {
            matches!(
                carrier,
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                    | checked_trees::CheckedByteSequenceCarrier::BoundedView {
                        access: Some(
                            checked_trees::CheckedStructuralAccess::SharedBorrow
                                | checked_trees::CheckedStructuralAccess::MutableBorrow
                        ),
                        ..
                    }
                    | checked_trees::CheckedByteSequenceCarrier::BorrowedView {
                        access: Some(
                            checked_trees::CheckedStructuralAccess::SharedBorrow
                                | checked_trees::CheckedStructuralAccess::MutableBorrow
                        ),
                    }
            )
        })
        .map(ByteObservationCarrier::Sequence)
}
