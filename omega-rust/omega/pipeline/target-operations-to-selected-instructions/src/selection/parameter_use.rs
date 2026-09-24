//! Whether a borrowed parameter's referent pointer must stay live.
//!
//! Entry capture keeps an incoming referent pointer only for a parameter the
//! body actually reaches through: a store, read, view, case test, leaf copy,
//! call argument, or a view it transfers. Construction and its validation
//! replay both ask this one question, so a newly lowered instruction family
//! cannot be counted as a use in one and missed in the other.
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarFunction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::PlaceId;

pub(super) fn referent_used(source: &LegalizedScalarFunction, place: PlaceId) -> bool {
    super::established_view_input::transferred(source, place)
        || source
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|row| match &row.kind {
                LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore {
                    destination,
                    source,
                    ..
                } => destination.place == place || *source == place,
                LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                    source: argument,
                    ..
                }
                | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                    source: argument,
                    ..
                } => argument.place == place,
                LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore {
                    destination,
                    ..
                } => destination.place == place,
                LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                    destination, ..
                }
                | LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { destination, .. }
                | LegalizedScalarInstructionKind::WriteOnlyIndexedPrimitiveStore {
                    destination,
                    ..
                } => destination.place == place,
                LegalizedScalarInstructionKind::PrimitiveScalarRead { source, .. }
                | LegalizedScalarInstructionKind::StructuralCaseMembership { source, .. }
                | LegalizedScalarInstructionKind::StructuralLeafCopy { source, .. }
                | LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
                | LegalizedScalarInstructionKind::ByteSequenceRead { source, .. }
                | LegalizedScalarInstructionKind::ByteSequenceSubslice { source, .. }
                | LegalizedScalarInstructionKind::ElementViewLength { source, .. }
                | LegalizedScalarInstructionKind::ElementViewRead { source, .. }
                | LegalizedScalarInstructionKind::ElementViewSubslice { source, .. } => {
                    *source == place
                }
                // `self.arr.as_slice()` roots a view in the parameter's
                // fixed-array field, so the referent pointer must stay live.
                LegalizedScalarInstructionKind::EstablishElementView { source, .. } => {
                    source.place == place
                }
                LegalizedScalarInstructionKind::Call(call) => call.arguments.iter().any(
                    |argument| matches!(argument, LegalizedScalarArgument::Structural { semantic, .. } if semantic.place == place),
                ),
                LegalizedScalarInstructionKind::NormalizedForeignCall(call) => call
                    .structural_arguments
                    .iter()
                    .any(|argument| argument.place == place),
                _ => false,
            })
}
