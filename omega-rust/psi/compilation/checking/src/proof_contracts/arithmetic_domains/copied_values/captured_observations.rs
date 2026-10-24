//! The unpublished value is detached; storage reached through its references is not.
//!
//! A later constructor operand cannot replace an earlier owned field or its
//! private descriptor. It can mutate the original backing of a returned view.
//! Until that backing has an exact preserved origin, only a complete empty
//! suffix frame preserves its contents. Unknown writes never imply separation.
//! This judgment runs before each operand, so that operand's own completed
//! result guarantees are captured afterward rather than expired retroactively.

use super::super::ordered_values::{self, Operand};
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

/// Numeric payloads use the same receiving-type distinction as relational
/// observations. The path names the captured value, never its source home.
/// A reference at any crossing (including the final primitive slot) observes
/// live backing; only a complete empty later frame preserves that observation.
pub(super) fn scalar_path_survives(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    path: Option<&[facts::PlaceSegment]>,
    writes: Option<&[String]>,
) -> bool {
    if writes.is_some_and(<[String]>::is_empty) {
        return true;
    }
    if !reference.is_valid() {
        return false;
    }
    let Some(path) = path else {
        return false;
    };
    let mut reference = reference;
    for segment in path {
        if crate::reference_shell(program, reference).is_some() {
            return false;
        }
        let Some(projected) = crate::project_type_reference_from_segments(
            program,
            reference,
            std::slice::from_ref(segment),
        ) else {
            return false;
        };
        reference = projected;
    }
    crate::reference_shell(program, reference).is_none()
}

pub(super) struct CapturedValue<'value> {
    pub(super) operand: &'value Operand,
    pub(super) type_reference: TypeReferenceHandle,
}

impl CapturedValue<'_> {
    pub(super) fn observation_survives(
        &self,
        program: &dyn crate::CheckingInput,
        state: &State,
        observed: &Operand,
        writes: Option<&[String]>,
    ) -> bool {
        if !super::depends_on(observed, self.operand) {
            return ordered_values::survives_delivery(
                program,
                state,
                observed,
                self.operand,
                writes,
            );
        }
        if writes.is_some_and(<[String]>::is_empty) {
            return true;
        }
        if observed == self.operand {
            // Integer use of a primitive reference implicitly reads its
            // referent. Capturing that carrier is not a scalar snapshot.
            return self.type_reference.is_valid()
                && crate::reference_shell(program, self.type_reference).is_none();
        }
        let survives = |operand| self.observation_survives(program, state, operand, writes);
        match observed {
            Operand::Field { .. } | Operand::Indexed { .. } => self
                .owned_projection_type(program, state, observed, writes)
                .is_some_and(|reference| crate::reference_shell(program, reference).is_none()),
            Operand::CollectionLength(collection) => self
                .owned_projection_type(program, state, collection, writes)
                .is_some_and(|reference| {
                    crate::slice_view_shell(program, reference, &[]).is_some()
                }),
            Operand::Binary { operands, .. } => operands.iter().all(survives),
            Operand::Call { arguments, .. } => arguments.iter().all(survives),
            Operand::CompletedCall { .. } => false,
            Operand::Place { .. } | Operand::Integer(..) => false,
        }
    }

    /// The last slot may itself be a copied reference carrier. Crossing it to
    /// inspect a field or element would read borrowed backing, not this value.
    fn owned_projection_type(
        &self,
        program: &dyn crate::CheckingInput,
        state: &State,
        observed: &Operand,
        writes: Option<&[String]>,
    ) -> Option<TypeReferenceHandle> {
        if observed == self.operand {
            return self
                .type_reference
                .is_valid()
                .then_some(self.type_reference);
        }
        let (parent, field) = match observed {
            Operand::Field {
                receiver, field, ..
            } => (receiver.as_ref(), Some(*field)),
            Operand::Indexed {
                collection, index, ..
            } => {
                // Check selectors wherever they occur, including beneath a
                // field or a descriptor-length observation.
                if !self.observation_survives(program, state, index, writes) {
                    return None;
                }
                (collection.as_ref(), None)
            }
            _ => return None,
        };
        let reference = self.owned_projection_type(program, state, parent, writes)?;
        if crate::reference_shell(program, reference).is_some() {
            return None;
        }
        if let Some(symbol) = field {
            crate::project_type_reference_from_segments(
                program,
                reference,
                &[facts::PlaceSegment::Field { symbol }],
            )
        } else {
            crate::collection_element_type_reference(program, reference)
        }
    }
}
