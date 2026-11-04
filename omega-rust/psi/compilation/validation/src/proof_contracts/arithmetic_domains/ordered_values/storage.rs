//! Keep arithmetic facts only while their storage dependencies survive a store.
//!
//! Reference reads first use the frame owner's suppliers and descriptor loads;
//! inline fields keep their declared homes. Complete destinations distinguish
//! sibling fields and a collection's extent from its elements. Selectors remain
//! separate reads: preserving element bytes does not preserve a changed index.

use super::Operand;
use crate::{AssignmentStorageOrigins, LocalWriteSource};
use facts::PlaceSegment;
use symbols::SymbolHandle;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_reference_read_cannot_use_inline_field_disjointness() {
        let root = SymbolHandle::from_parts(1, 1);
        let changed = SymbolHandle::from_parts(2, 1);
        let sibling = SymbolHandle::from_parts(3, 1);
        let observation = |field, name: &str| Operand::Field {
            receiver: Box::new(Operand::Place {
                root,
                path: "record".into(),
            }),
            field,
            name: name.into(),
            primitive: None,
        };
        let changed_value = observation(changed, "changed");
        let sibling_value = observation(sibling, "sibling");
        // The same declared fields can belong to inline storage or a borrowed
        // value. An unavailable reference read cannot use inline disjointness.
        let written = ["record".to_owned()];
        let storage = AssignmentStorageOrigins {
            destinations: vec![LocalWriteSource {
                invocation_output: None,
                invocation_result: None,
                is_image_storage: false,
                recasts: Vec::new(),
                representation: None,
                path: "record.changed".into(),
                collection_coarse: false,
                root,
                segments: vec![PlaceSegment::Field { symbol: changed }],
            }],
            references: None,
        };
        assert!(!changed_value.survives_assignment(&written, Some(&storage), &|_| Some(false)));
        assert!(!sibling_value.survives_assignment(&written, Some(&storage), &|_| Some(false)));
        assert!(!changed_value.survives_assignment(&written, Some(&storage), &|_| None));
        assert!(sibling_value.survives_assignment(&written, Some(&storage), &|_| None));
    }
}

#[derive(Clone)]
pub(super) enum ReadSegment {
    Field(SymbolHandle),
    Case(SymbolHandle),
    Element(Option<usize>),
    Length,
}

impl Operand {
    pub(super) fn has_stable_descriptor(
        &self,
        program: &dyn crate::CheckingInput,
        state: &typed_trees::state::State,
    ) -> bool {
        fn coordinate(value: &Operand, fields: &mut Vec<PlaceSegment>) -> Option<SymbolHandle> {
            match value {
                Operand::Place { root, .. } => Some(*root),
                Operand::Field {
                    receiver, field, ..
                } => {
                    let root = coordinate(receiver, fields)?;
                    fields.push(PlaceSegment::Field { symbol: *field });
                    Some(root)
                }
                _ => None,
            }
        }
        let mut fields = Vec::new();
        coordinate(self, &mut fields).is_some_and(|root| {
            crate::stable_slice_descriptor_place(program, state, root, &fields).is_some()
        })
    }

    pub(in crate::proof_contracts::arithmetic_domains) fn survives_assignment(
        &self,
        written: &[String],
        storage: Option<&AssignmentStorageOrigins>,
        reference_read: &impl Fn(&Operand) -> Option<bool>,
    ) -> bool {
        // A different alias name does not establish non-interference. Resolve
        // reference-dependent reads before the presentation-path shortcut.
        if let Some(preserved) = reference_read(self) {
            return preserved && self.selectors_survive(written, storage, reference_read);
        }
        match self {
            Self::Integer(..) => true,
            #[cfg(test)]
            Self::CompletedCall { .. } => false,
            Self::Call { arguments, .. }
            | Self::Binary {
                operands: arguments,
                ..
            } => arguments
                .iter()
                .all(|argument| argument.survives_assignment(written, storage, reference_read)),
            _ => {
                if self.survives(written) {
                    return true;
                }
                let Some(storage) = storage.filter(|storage| !storage.destinations.is_empty())
                else {
                    return false;
                };
                let mut segments = Vec::new();
                let Some(root) = self.read_coordinate(&mut segments) else {
                    return false;
                };
                if !self.selectors_survive(written, Some(storage), reference_read) {
                    return false;
                }
                // The read owner already classified this as inline storage.
                // Reference suppliers, including nested leaves, never reach
                // this declaration-only comparison.
                storage.destinations.iter().all(|destination| {
                    compatible_destination(root, destination)
                        && disjoint(&segments, &destination.segments)
                })
            }
        }
    }

    pub(super) fn read_coordinate(&self, segments: &mut Vec<ReadSegment>) -> Option<SymbolHandle> {
        match self {
            Self::Place { root, .. } => Some(*root),
            Self::Field {
                receiver, field, ..
            } => {
                let root = receiver.read_coordinate(segments)?;
                segments.push(ReadSegment::Field(*field));
                Some(root)
            }
            Self::Indexed {
                collection, index, ..
            } => {
                let root = collection.read_coordinate(segments)?;
                // Different live selectors may choose the same element.
                // Their identities prove equality, never disjointness.
                let selected = match &**index {
                    Self::Integer(value, _) => value
                        .value_u64()
                        .and_then(|value| usize::try_from(value).ok()),
                    _ => None,
                };
                segments.push(ReadSegment::Element(selected));
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
        storage: Option<&AssignmentStorageOrigins>,
        reference_read: &impl Fn(&Operand) -> Option<bool>,
    ) -> bool {
        match self {
            Self::Place { .. } => true,
            Self::Field { receiver, .. } => {
                receiver.selectors_survive(written, storage, reference_read)
            }
            Self::CollectionLength(collection) => {
                collection.selectors_survive(written, storage, reference_read)
            }
            Self::Indexed {
                collection, index, ..
            } => {
                collection.selectors_survive(written, storage, reference_read)
                    && index.survives_assignment(written, storage, reference_read)
            }
            _ => false,
        }
    }
}

pub(super) fn compatible_destination(root: SymbolHandle, destination: &LocalWriteSource) -> bool {
    destination.root == root
        && root.is_valid()
        && destination.recasts.is_empty()
        && destination.representation.is_none()
}

pub(super) fn disjoint(read: &[ReadSegment], written: &[PlaceSegment]) -> bool {
    for (read, written) in read.iter().zip(written) {
        match (read, written) {
            (ReadSegment::Case(read), PlaceSegment::Case { variant }) if read == variant => {}
            (ReadSegment::Field(read), PlaceSegment::Field { symbol: written })
                if read.is_valid() && written.is_valid() =>
            {
                if read != written {
                    return true;
                }
            }
            (ReadSegment::Element(Some(read)), PlaceSegment::FixedIndex { index: written })
                if read != written =>
            {
                return true;
            }
            (
                ReadSegment::Element(_),
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

/// Capturing the completed result does not capture mutable actual operands.
/// A store which replaces one of those operands must not reinstall a promise
/// about its old value under the destination's new storage identity.
pub(in crate::proof_contracts::arithmetic_domains) fn survives_delivery(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    operand: &Operand,
    result: &Operand,
    written: Option<&[String]>,
) -> bool {
    let preserved = written.map_or_else(
        || operand.is_immutable_snapshot(program, state),
        |written| operand.survives_storage_writes(program, Some(state), written),
    );
    if operand == result || preserved {
        return true;
    }
    let survives = |operand: &Operand| survives_delivery(program, state, operand, result, written);
    match operand {
        Operand::Field { receiver, .. } => survives(receiver),
        Operand::CollectionLength(collection) => survives(collection),
        Operand::Indexed {
            collection, index, ..
        } => survives(collection) && survives(index),
        Operand::Binary { operands, .. } => operands.iter().all(survives),
        _ => false,
    }
}
