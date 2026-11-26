//! The observable leaves of a structural place for initialization and open-window checks.

use super::*;

impl Storage<'_> {
    /// Does any leaf under this place match? A whole-array proof can cover an
    /// array without visiting its elements. Keep one path while walking; even
    /// after a match or a query error, finish checking that the shape is valid.
    pub(super) fn any_leaf<S>(
        &self,
        place: &Lineage,
        concerns: Construct,
        state: &mut S,
        mut established_array: impl FnMut(&mut S, &Lineage) -> bool,
        mut matches: impl FnMut(&mut S, &Lineage) -> Checked<bool>,
    ) -> Checked<bool> {
        let mut found = Ok(false);
        let mut visit = |state: &mut S, leaf: &Lineage| {
            if matches!(found, Ok(false)) {
                found = matches(state, leaf);
            }
        };
        let mut at = place.clone();
        match self.shape(place, concerns)? {
            Shape::Atomic(_) | Shape::Scalar(_) | Shape::View => visit(state, &at),
            Shape::Typed(structural_type) => self.leaves_of(
                structural_type,
                &mut at,
                concerns,
                state,
                &mut established_array,
                &mut visit,
            )?,
            Shape::Payload(case) => self.field_leaves(
                &case.fields,
                &mut at,
                concerns,
                state,
                &mut established_array,
                &mut visit,
            )?,
        }
        found
    }

    fn field_leaves<S>(
        &self,
        fields: &[StructuralFieldDeclaration],
        at: &mut Lineage,
        concerns: Construct,
        state: &mut S,
        established_array: &mut impl FnMut(&mut S, &Lineage) -> bool,
        visit: &mut impl FnMut(&mut S, &Lineage),
    ) -> Checked<()> {
        for field in fields {
            // An erased field has no storage, so no leaf: it "contributes
            // no runtime field, address, read, or cleanup" (contracts.md).
            if field.relevance.is_erased() {
                continue;
            }
            at.segments.push(Segment::Field(field.id));
            match field.field_type {
                StructuralFieldType::Atomic(_)
                | StructuralFieldType::AtomicBoundedInteger(_)
                | StructuralFieldType::Scalar(_) => visit(state, at),
                StructuralFieldType::Structural(nested) => {
                    self.leaves_of(nested, at, concerns, state, established_array, visit)?;
                }
                ref stored if is_stored_view(stored) => visit(state, at),
                _ => return Err(self.unsupported(concerns, "a field type outside plain data")),
            }
            at.segments.pop();
        }
        Ok(())
    }

    /// Whether every leaf of an element shares the array's storage. A
    /// reference, view or independently managed value keeps its own checks.
    /// Permission to retire descriptors does not mean a value contains one.
    /// The field types determine that.
    fn inline_array_element(
        &self,
        element: StructuralTypeId,
        concerns: Construct,
        known: &mut Vec<Option<bool>>,
    ) -> Checked<bool> {
        let declaration = self.declaration(element, concerns)?;
        if declaration.owns_linear_claim
            || declaration.nominal_cleanup.is_some()
            || declaration.quotient_carrier.is_some()
        {
            return Ok(false);
        }
        if matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_)) {
            return Ok(true);
        }
        let slot = index(element.get());
        if slot >= known.len() {
            known.resize(slot + 1, None);
        }
        if let Some(inline) = known[slot] {
            return Ok(inline);
        }
        // An inline cycle cannot supply a finite element. Shared subtypes
        // reuse the completed answer when another field names them.
        known[slot] = Some(false);
        let inline = match &declaration.shape {
            StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                ..
            }
            | StructuralTypeShape::Sum { .. } => {
                for field in declaration.shape.fields() {
                    match field.field_type {
                        StructuralFieldType::Scalar(_) if !field.relevance.is_erased() => {}
                        StructuralFieldType::Structural(nested)
                            if !field.relevance.is_erased()
                                && self.inline_array_element(nested, concerns, known)? => {}
                        _ => return Ok(false),
                    }
                }
                true
            }
            StructuralTypeShape::FixedArray {
                element: nested,
                length: terminal_psi::ArrayExtent::Literal(_),
            } => self.inline_array_element(*nested, concerns, known)?,
            _ => false,
        };
        known[slot] = Some(inline);
        Ok(inline)
    }

    fn leaves_of<S>(
        &self,
        structural_type: StructuralTypeId,
        at: &mut Lineage,
        concerns: Construct,
        state: &mut S,
        established_array: &mut impl FnMut(&mut S, &Lineage) -> bool,
        visit: &mut impl FnMut(&mut S, &Lineage),
    ) -> Checked<()> {
        match &self.declaration(structural_type, concerns)?.shape {
            // References and opaque values each occupy one established
            // place. Neither grants owned access to hidden backing storage.
            StructuralTypeShape::AtomicCell(_)
            | StructuralTypeShape::Opaque
            | StructuralTypeShape::TypeParameter
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::Reference { .. } => {
                visit(state, at);
            }
            // Availability concerns the descriptor, not ownership of its
            // backing. An exclusive descriptor is readable too; copying it
            // still requires the separate unrestricted-copy judgment.
            shape if shape.is_view_descriptor() => visit(state, at),
            // A symbolic array can be established whole without enumerating
            // N elements. Every selected read still owes its own index bound.
            StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Parameter { .. },
                ..
            } => {
                visit(state, at);
            }
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } => {
                // A whole supply covers every inline field and tag. Reading
                // a selected payload still requires the case at that use.
                if *length > 1
                    && self.inline_array_element(*element, concerns, &mut Vec::new())?
                    && established_array(state, at)
                {
                    return Ok(());
                }
                for index in 0..*length {
                    at.segments.push(Segment::FixedIndex(index));
                    self.leaves_of(*element, at, concerns, state, established_array, visit)?;
                    at.segments.pop();
                }
            }
            StructuralTypeShape::Record { fields, .. } => {
                self.field_leaves(fields, at, concerns, state, established_array, visit)?;
            }
            // A sum is established when something was stored into it whole,
            // whatever its case: the sum's own place stands for its
            // discriminator, and the payload leaves of every case lie under
            // it. A sum with no payload has only that one leaf.
            StructuralTypeShape::Sum { cases } => {
                visit(state, at);
                for case in cases {
                    at.segments.push(Segment::Case(case.id));
                    self.field_leaves(&case.fields, at, concerns, state, established_array, visit)?;
                    at.segments.pop();
                }
            }
            _ => return Err(self.unsupported(concerns, "storage that is no plain data")),
        }
        Ok(())
    }
}
