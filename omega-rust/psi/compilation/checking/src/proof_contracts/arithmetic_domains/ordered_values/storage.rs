//! Preserve observations using the performed store's captured coordinates.
//!
//! String frames remain conservative and include aliases. When they overlap,
//! the frame owner's complete destination roster can distinguish a sibling
//! field or a collection's extent from its elements. Unknown origins, recasts
//! and differing storage namespaces withhold that precision. Selectors are
//! independent value reads: keeping an element's bytes does not keep its index.

use super::Operand;
use crate::LocalWriteSource;
use facts::PlaceSegment;
use symbols::SymbolHandle;

enum ReadSegment {
    Field(SymbolHandle),
    Element,
    Length,
}

impl Operand {
    pub(in crate::proof_contracts::arithmetic_domains) fn survives_assignment(
        &self,
        written: &[String],
        destinations: Option<&[LocalWriteSource]>,
    ) -> bool {
        if self.survives(written) {
            return true;
        }
        match self {
            Self::Integer(..) => true,
            Self::Call { arguments, .. }
            | Self::Binary {
                operands: arguments,
                ..
            } => arguments
                .iter()
                .all(|argument| argument.survives_assignment(written, destinations)),
            _ => {
                let Some(destinations) = destinations.filter(|sources| !sources.is_empty()) else {
                    return false;
                };
                let mut segments = Vec::new();
                let Some(root) = self.read_coordinate(&mut segments) else {
                    return false;
                };
                self.selectors_survive(written, Some(destinations))
                    && destinations.iter().all(|destination| {
                        destination.root == root
                            && root.is_valid()
                            && destination.recasts.is_empty()
                            && destination.representation.is_none()
                            && disjoint(&segments, &destination.segments)
                    })
            }
        }
    }

    fn read_coordinate(&self, segments: &mut Vec<ReadSegment>) -> Option<SymbolHandle> {
        match self {
            Self::Place { root, .. } => Some(*root),
            Self::Field {
                receiver, field, ..
            } => {
                let root = receiver.read_coordinate(segments)?;
                segments.push(ReadSegment::Field(*field));
                Some(root)
            }
            Self::Indexed { collection, .. } => {
                let root = collection.read_coordinate(segments)?;
                // Different live selectors may choose the same element.
                // Their identities prove equality, never disjointness.
                segments.push(ReadSegment::Element);
                Some(root)
            }
            Self::CollectionLength(collection) => {
                let root = collection.read_coordinate(segments)?;
                segments.push(ReadSegment::Length);
                Some(root)
            }
            _ => None,
        }
    }

    fn selectors_survive(
        &self,
        written: &[String],
        destinations: Option<&[LocalWriteSource]>,
    ) -> bool {
        match self {
            Self::Place { .. } => true,
            Self::Field { receiver, .. } => receiver.selectors_survive(written, destinations),
            Self::CollectionLength(collection) => {
                collection.selectors_survive(written, destinations)
            }
            Self::Indexed {
                collection, index, ..
            } => {
                collection.selectors_survive(written, destinations)
                    && index.survives_assignment(written, destinations)
            }
            _ => false,
        }
    }
}

fn disjoint(read: &[ReadSegment], written: &[PlaceSegment]) -> bool {
    for (read, written) in read.iter().zip(written) {
        match (read, written) {
            (ReadSegment::Field(read), PlaceSegment::Field { symbol: written })
                if read.is_valid() && written.is_valid() =>
            {
                if read != written {
                    return true;
                }
            }
            (
                ReadSegment::Element,
                PlaceSegment::Index { .. } | PlaceSegment::FixedIndex { .. },
            ) => {}
            (ReadSegment::Length, PlaceSegment::Index { .. } | PlaceSegment::FixedIndex { .. }) => {
                return true;
            }
            _ => return false,
        }
    }
    // Replacing a containing value, or writing within a whole-value read,
    // overlaps. A missing suffix is never evidence of independence.
    false
}
