//! Direct writes and exclusive storage loans of the abstract operation vocabulary.
//!
//! Ownership replay and atomic coherence consume the same borrowed inventory.
//! A field remains attached to its carrier until the consumer resolves geometry;
//! this description does not assume that names on distinct edges are disjoint.

use crate::AbstractOperation;
use semantic_vocabulary::{PlaceId, StructuralFieldId};
use terminal_psi::StructuralPathSegment;

/// One mutation inventory serves whole-root join pins and projected child
/// freezes. A field id stays attached until its declaring record is resolved;
/// losing that last step would freeze unrelated sibling fields.
pub struct StorageMutation<'a> {
    pub place: PlaceId,
    pub path: &'a [StructuralPathSegment],
    pub field: Option<StructuralFieldId>,
}

impl AbstractOperation {
    /// Storage directly written or lent with exclusive authority by this
    /// operation. Callers resolve projected fields and possible aliases against
    /// their current graph; this inventory grants no authority by itself.
    pub fn storage_mutations(&self) -> Vec<StorageMutation<'_>> {
        fn exclusive(arguments: &[terminal_psi::StructuralArgument]) -> Vec<StorageMutation<'_>> {
            arguments
                .iter()
                .filter(|argument| {
                    matches!(
                        argument.access,
                        terminal_psi::exclusive_borrow_pair_pattern!()
                    )
                })
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
                .map(|location| region(location.root, &location.path, Some(location.field)))
                .unwrap_or_default(),
            Self::CallStructural {
                structural_arguments,
                ..
            }
            | Self::BoundaryCall {
                structural_arguments,
                ..
            }
            | Self::ExternalCompletion {
                structural_arguments,
                ..
            } => exclusive(structural_arguments),
            Self::EstablishReference { source, .. }
            | Self::EstablishPlacedReference { source, .. }
            | Self::EstablishElementView { source, .. } => exclusive(std::slice::from_ref(source)),
            Self::PrimitiveLocalStore { destination, .. }
            | Self::ByteSequenceWrite { destination, .. }
            | Self::ElementViewWrite { destination, .. }
            | Self::ByteSequenceStore { destination, .. } => region(*destination, &[], None),
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
            _ => Vec::new(),
        }
    }
}
