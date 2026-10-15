//! Stores rejoin the exact vacated region to an owned home's custody.
//! Declared access distinguishes owned storage from borrowed backing; opaque
//! storage identities cannot grant a callee ownership. Complete child partitions
//! coalesce back into their ancestor, the inverse of projected transfer splitting.
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::StructuralRuntimePlace;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
use std::collections::BTreeSet;
use terminal_psi::{
    StructuralAccess, StructuralAffineDiscard, StructuralFieldType, StructuralMultiplicity,
    StructuralPathSegment, StructuralTypeShape,
};

impl TerminalExecution<'_> {
    /// Restore the exact stored region in an owned home's reserved geometry.
    /// A store through a loan changes the caller's backing, never the callee's
    /// owned frontier. The typed root declaration supplies that distinction;
    /// physical identities and the presence of other debts cannot grant it.
    pub(crate) fn restore_store_frontier(
        &self,
        frontier: &mut BTreeSet<StructuralAffineDiscard>,
        destination: PlaceId,
        path: &[StructuralPathSegment],
        hole: &StructuralRuntimePlace,
        structural_type: StructuralTypeId,
    ) -> Result<(), TerminalInterpretError> {
        if path.contains(&StructuralPathSegment::Referent) {
            return Ok(());
        }
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        let parameter = machine.structural_parameter(destination);
        let owned_affine = if let Some(parameter) = parameter {
            parameter.access == StructuralAccess::Owned
                && parameter.multiplicity == StructuralMultiplicity::Affine
        } else {
            let declaration = machine
                .structural_place(destination)
                .ok_or_else(malformed)?;
            let StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } = declaration.kind
            else {
                return Err(malformed());
            };
            let result = machine
                .operation(producer)
                .and_then(|operation| operation.result.structural())
                .ok_or_else(malformed)?;
            if result.place != destination || result.structural_type != structural_type {
                return Err(malformed());
            }
            result.multiplicity == StructuralMultiplicity::Affine
        };
        if !owned_affine {
            return Ok(());
        }
        // An affine view owns its exclusive descriptor, not the selected
        // element. Move, claim repair and frontier repair share the same loan
        // crossing judgment; result multiplicity cannot confer backing custody.
        let root = self
            .structural_values
            .get(&destination)
            .ok_or_else(malformed)?;
        let Some(owned) = self.owned_window_projection(destination, root, path, hole)? else {
            return Ok(());
        };
        let selected_path = owned.path;
        if frontier
            .iter()
            .any(|entry| entry.place == destination && selected_path.starts_with(&entry.path))
        {
            // Initial establishment writes into reserved geometry already
            // covered by the owner; replacement reseats a vacated region.
            return Ok(());
        }
        if frontier
            .iter()
            .any(|entry| entry.place == destination && entry.path.starts_with(&selected_path))
        {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        frontier.insert(StructuralAffineDiscard {
            place: destination,
            path: selected_path.clone(),
            structural_type,
        });
        // Reseating the last missing child restores its complete ancestor's
        // ordinary whole custody. This inverse of projection splitting follows
        // declaration geometry, not initializer syntax or storage contents.
        for depth in (0..selected_path.len()).rev() {
            let parent_path = &selected_path[..depth];
            let parent_type = crate::custody::resolve_structural_path_type(
                &self.program.structural_types(),
                root.structural_type,
                parent_path,
            )?;
            let declaration = self
                .program
                .structural_types()
                .get(&parent_type)
                .ok_or_else(malformed)?;
            let children = match &declaration.shape {
                StructuralTypeShape::Record { fields, .. } => fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .filter_map(|field| match field.field_type {
                        StructuralFieldType::Structural(structural_type) => {
                            let mut path = parent_path.to_vec();
                            path.push(StructuralPathSegment::Field(field.identity.clone()));
                            Some(StructuralAffineDiscard {
                                place: destination,
                                path,
                                structural_type,
                            })
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                StructuralTypeShape::FixedArray { element, length }
                    if usize::try_from(*length).is_ok_and(|length| length <= frontier.len()) =>
                {
                    (0..*length)
                        .map(|index| {
                            let mut path = parent_path.to_vec();
                            path.push(StructuralPathSegment::FixedIndex(index));
                            StructuralAffineDiscard {
                                place: destination,
                                path,
                                structural_type: *element,
                            }
                        })
                        .collect::<Vec<_>>()
                }
                _ => break,
            };
            if children.is_empty() || !children.iter().all(|child| frontier.contains(child)) {
                break;
            }
            for child in children {
                frontier.remove(&child);
            }
            frontier.insert(StructuralAffineDiscard {
                place: destination,
                path: parent_path.to_vec(),
                structural_type: parent_type,
            });
        }
        Ok(())
    }
}
