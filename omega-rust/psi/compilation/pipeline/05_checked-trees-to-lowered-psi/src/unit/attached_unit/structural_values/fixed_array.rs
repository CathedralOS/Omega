//! Complete fixed-array operands in authored index order before assembly.
//!
//! Every child follows ordinary structural value production. The final array
//! operation transfers completed operands atomically; evaluation or suspension
//! before it keeps those operands under their existing temporary custody.
use super::super::{
    LoweringError, Operation, OperationKind, PlaceId, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeShape, allocate_dense, place_id, unsupported,
};
use super::emission;
use crate::emission::operation_emission::buffer::structural_result_declaration;
use checked_trees::CheckedStructuralValueHandle;

impl emission::Emission<'_, '_, '_> {
    pub(super) fn fixed_array(
        &mut self,
        elements: &[CheckedStructuralValueHandle],
        type_reference: checked_trees::types::TypeReferenceHandle,
    ) -> Result<PlaceId, LoweringError> {
        let declaration = self
            .structural_types
            .iter()
            .find(|declaration| declaration.id == self.structural_type)
            .ok_or(LoweringError::Unsupported("fixed array carrier is missing"))?;
        let StructuralTypeShape::FixedArray { element, .. } = declaration.shape else {
            return unsupported("fixed array literal has a nonarray carrier");
        };
        let reference = self
            .checked
            .typed
            .unwrapped_type_reference(type_reference)
            .ok_or(LoweringError::Unsupported(
                "fixed array source carrier is missing",
            ))?;
        let checked_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
            self.checked.type_reference_table.type_reference(reference)
        else {
            return unsupported("fixed array source has a nonarray carrier");
        };
        let element_reference = *element_type;
        let element_multiplicity = crate::terminal_identities::structural_multiplicity(
            crate::reference_custody::result_multiplicity(self.checked, element_reference),
        );
        let parent_type = self.structural_type;
        let parent_multiplicity = self.multiplicity;
        let source_count = self.values.len();
        let mut operands = Vec::with_capacity(elements.len());
        for value in elements {
            self.structural_type = element;
            self.multiplicity = element_multiplicity;
            let argument = self.constructor_operand(*value, element_reference);
            self.structural_type = parent_type;
            self.multiplicity = parent_multiplicity;
            operands.push(argument?);
        }
        let place = place_id(allocate_dense(self.next_place)?);
        self.temporary_places.push(emit_completed(
            place,
            parent_type,
            parent_multiplicity,
            operands,
            self.operations,
        ));
        self.values.truncate(source_count);
        Ok(place)
    }

    /// Establish the current carrier's whole zero as a fresh owned place.
    pub(super) fn establish_zeroed(&mut self) -> Result<PlaceId, LoweringError> {
        let place = place_id(allocate_dense(self.next_place)?);
        let operation = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation,
            result: structural_result_declaration(place, self.structural_type, self.multiplicity),
            kind: OperationKind::EstablishZeroedStructural {},
        });
        self.temporary_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type: self.structural_type,
            },
        });
        Ok(place)
    }
}

/// Assemble already completed element owners. Unit sequencing and scalar
/// continuations share this operation; each caller schedules evaluation before
/// transferring those owners into the final array atomically.
pub(crate) fn emit_completed(
    place: PlaceId,
    structural_type: semantic_vocabulary::StructuralTypeId,
    multiplicity: terminal_psi::StructuralMultiplicity,
    elements: Vec<terminal_psi::StructuralArgument>,
    operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
) -> StructuralPlaceDeclaration {
    let operation = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: structural_result_declaration(place, structural_type, multiplicity),
        kind: OperationKind::EstablishStructuralArray { elements },
    });
    StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation,
            structural_type,
        },
    }
}
