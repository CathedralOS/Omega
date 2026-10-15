//! Whether a borrowed parameter's referent pointer must stay live.
//!
//! Entry capture keeps an incoming referent pointer only for a parameter the
//! body actually reaches through: a store (including a view field's
//! descriptor replacement), read, reference establishment, view, case test, leaf copy
//! (including a borrowed window's move), window reseat, call argument, or a
//! view it transfers. Construction and its validation
//! replay both ask this one question, so a newly lowered instruction family
//! cannot be counted as a use in one and missed in the other.
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarFunction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::PlaceId;

pub(super) fn referent_used(source: &LegalizedScalarFunction, place: PlaceId) -> bool {
    super::established_view_input::transferred(source, place)
        || source.blocks.iter().any(|block| block.terminator.cleanup_calls().any(|(_, call)|
            call.arguments.iter().any(|argument|
                matches!(argument, LegalizedScalarArgument::Structural { semantic, .. } if semantic.place == place))))
        || source.blocks.iter().any(|block| {
            match &block.terminator {
                legalized_operations::LegalizedScalarTerminator::StructuralCase { source, .. } => source.place() == place,
                // Returning the referent's value reads its incoming storage,
                // even when no body instruction otherwise observes it.
                legalized_operations::LegalizedScalarTerminator::Return(returned)
                | legalized_operations::LegalizedScalarTerminator::ReturnCleanup { returned, .. } => matches!(
                    returned.value,
                    legalized_operations::LegalizedScalarReturnValue::StructuralParameter { place: returned }
                    | legalized_operations::LegalizedScalarReturnValue::SharedDescriptor { place: returned } if returned == place
                ),
                _ => false,
            }
        })
        || source.instructions()
            .any(|row| match &row.kind {
                // Forming a reference needs the incoming address even when
                // this activation never reads the referent's contents.
                LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
                | LegalizedScalarInstructionKind::ByteSequenceSubslice { source, .. }
                | LegalizedScalarInstructionKind::ElementViewSubslice { source, .. }
                | LegalizedScalarInstructionKind::EstablishReference { source, .. }
                | LegalizedScalarInstructionKind::EstablishPlacedReference { source, .. } => source.place == place,
                LegalizedScalarInstructionKind::AtomicAccess { location, .. }
                | LegalizedScalarInstructionKind::PlacedScalarAccess { location, .. } => location.place == place,
                LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation { left, right, .. } => left.source() == place || right.source() == place,
                LegalizedScalarInstructionKind::ByteSequenceEqual { left, right } => left.source() == place || right.source() == place,
                LegalizedScalarInstructionKind::ByteSequencePredicate { operand, .. } => operand.source() == place,
                LegalizedScalarInstructionKind::ByteSequenceStore { destination, source, .. } => *destination == place || *source == place,
                LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore {
                    destination,
                    source,
                    ..
                }
                | LegalizedScalarInstructionKind::StructuralByteViewFieldStore {
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
                }
                | LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead {
                    source: argument,
                    ..
                } => argument.place == place,
                LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore {
                    destination,
                    ..
                } => destination.place == place,
                LegalizedScalarInstructionKind::StoreStructuralField { destination, .. } => {
                    destination.place() == place
                }
                LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                    destination, ..
                } => destination.place() == place,
                LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { destination, .. } => {
                    destination.place() == place
                }
                LegalizedScalarInstructionKind::PrimitiveScalarRead { source, .. }
                | LegalizedScalarInstructionKind::StructuralCaseMembership { source, .. }
                | LegalizedScalarInstructionKind::StructuralLeafCopy { source, .. }
                | LegalizedScalarInstructionKind::ByteSequenceRead { source, .. }
                | LegalizedScalarInstructionKind::ElementViewRead { source, .. } => {
                    *source == place
                }
                // `&self.arr[..]` roots a view in the parameter's fixed-array
                // field, so the referent pointer must stay live.
                LegalizedScalarInstructionKind::EstablishElementView { source, .. }
                | LegalizedScalarInstructionKind::ElementViewLength { source, .. } => {
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
