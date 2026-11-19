// We enumerate the direct writes and exclusive storage loans of an
// `AbstractOperation` so ownership replay and atomic coherence consume a single
// borrowed mutation inventory. A field ID remains attached to its carrier path
// until the consumer resolves record geometry so sibling fields are not
// falsely frozen.

use crate::AbstractOperation;
use semantic_vocabulary::{PlaceId, StructuralFieldId};
use terminal_psi::{StructuralPathSegment, exclusive_borrow_pair_pattern};

pub struct StorageMutation<'a> {
    pub place: PlaceId,
    pub path: &'a [StructuralPathSegment],
    pub field: Option<StructuralFieldId>,
}

impl AbstractOperation {
    pub fn storage_mutations(&self) -> Vec<StorageMutation<'_>> {
        fn exclusive(arguments: &[terminal_psi::StructuralArgument]) -> Vec<StorageMutation<'_>> {
            arguments
                .iter()
                .filter(|argument| matches!(argument.access, exclusive_borrow_pair_pattern!()))
                .map(|argument| StorageMutation {
                    place: argument.place,
                    path: argument.path.as_slice(),
                    field: None,
                })
                .collect::<Vec<_>>()
        }
        let region = |place, path, field| vec![StorageMutation { place, path, field }];
        match self {
            Self::AtomicEvent { event, .. } if event.joins_modification_order() => event
                .location()
                .map(|location| region(location.root, &location.path, None))
                .unwrap_or_default(),
            Self::EstablishReference { source, .. }
            | Self::EstablishAccessReference { source, .. }
            | Self::EstablishElementView { source, .. } => exclusive(std::slice::from_ref(source)),
            Self::PrimitiveLocalStore { destination, .. }
            | Self::ByteSequenceWrite { destination, .. }
            | Self::ByteSequenceStore { destination, .. } => region(*destination, &[], None),
            Self::ElementViewWrite { destination, .. } => {
                region(destination.place, &destination.path, None)
            }
            Self::StructuralByteSequenceFieldByteStore {
                destination,
                path,
                field,
                ..
            }
            | Self::StructuralByteSequenceFieldStore {
                destination,
                path,
                field,
                ..
            } => region(*destination, path, Some(*field)),
            Self::StructuralByteViewFieldStore {
                destination,
                path,
                field,
                ..
            } => region(*destination, path, *field),
            Self::WriteOnlyPrimitiveStore {
                destination, path, ..
            } => region(destination.place(), path, None),
            Self::StructuralScalarFieldStore {
                destination,
                path,
                field,
                ..
            } => region(destination.place(), path, Some(*field)),
            Self::StoreStructuralField {
                destination,
                path,
                field,
                ..
            } => region(destination.place(), path, *field),
            Self::MoveStructuralField {
                source,
                path,
                field,
                ..
            } => region(source.place(), path, *field),
            _ => exclusive(self.invocation_arguments()),
        }
    }
}
