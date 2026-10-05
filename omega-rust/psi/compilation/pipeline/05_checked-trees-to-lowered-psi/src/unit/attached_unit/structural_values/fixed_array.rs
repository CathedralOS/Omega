//! Complete fixed-array operands in authored index order before assembly.
//!
//! Every child follows ordinary structural value production. The final array
//! operation transfers completed operands atomically; evaluation or suspension
//! before it keeps those operands under their existing temporary custody.
use super::super::{
    LoweringError, Multiplicity, Operation, OperationKind, OperationResult, PlaceId,
    StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeShape, allocate_dense, place_id, unsupported,
};
use super::emission;
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
        let element_multiplicity =
            match crate::reference_custody::result_multiplicity(self.checked, element_reference) {
                Multiplicity::Affine => StructuralMultiplicity::Affine,
                Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                Multiplicity::Linear => {
                    return unsupported("fixed array cannot create linear custody");
                }
            };
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
        let operation = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation,
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type: parent_type,
                multiplicity: parent_multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishStructuralArray { elements: operands },
        });
        self.temporary_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type: parent_type,
            },
        });
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
            result: OperationResult::Structural(StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type: self.structural_type,
                multiplicity: self.multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
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
