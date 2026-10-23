//! Complete fixed-array operands in authored index order before assembly.
//!
//! Every child follows ordinary structural value production. The final array
//! operation transfers completed operands atomically; evaluation or suspension
//! before it keeps those operands under their existing temporary custody.
//! Element memberships move with those completed values under FixedIndex paths,
//! never from the array's receiving annotation or a record-like field identity.
use super::super::{
    LoweringError, Operation, OperationKind, PlaceId, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeShape, allocate_dense, place_id, unsupported,
};
use super::completed_aggregate::{self, CompletedAggregate};
use super::emission;
use crate::emission::operation_emission::buffer::structural_result_declaration;
use checked_trees::CheckedStructuralValueHandle;

impl emission::Emission<'_, '_, '_> {
    pub(super) fn fixed_array(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        elements: &[CheckedStructuralValueHandle],
        type_reference: checked_trees::types::TypeReferenceHandle,
    ) -> Result<PlaceId, LoweringError> {
        let declaration =
            terminal_psi::find_structural_type(&self.structural_types[..], self.structural_type)
                .ok_or(LoweringError::Unsupported("fixed array carrier is missing"))?;
        let StructuralTypeShape::FixedArray { element, .. } = declaration.shape else {
            return unsupported("fixed array literal has a nonarray carrier");
        };
        let reference = self.checked.facts.resolved_types.carrier(type_reference);
        if !reference.is_valid() {
            return unsupported("fixed array source carrier is missing");
        }
        let checked_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
            self.checked.type_reference_table.type_reference(reference)
        else {
            return unsupported("fixed array source has a nonarray carrier");
        };
        let element_reference = *element_type;
        let parent_type = self.structural_type;
        let parent_multiplicity = self.multiplicity;
        let source_count = self.values.len();
        let mut operands = Vec::with_capacity(elements.len());
        let mut claims = Vec::new();
        for (element_position, value) in elements.iter().enumerate() {
            let element_index = u64::try_from(element_position)
                .map_err(|_| LoweringError::Unsupported("array element identity overflow"))?;
            let operand = self.child_constructor_operand(
                identities,
                element,
                *value,
                element_reference,
                &[terminal_psi::StructuralPathSegment::FixedIndex(
                    element_index,
                )],
            )?;
            for claim in operand.claims {
                let mut path = vec![terminal_psi::StructuralPathSegment::FixedIndex(
                    element_index,
                )];
                path.extend(claim.path);
                claims.push(terminal_psi::StructuralResultClaimBinding {
                    claim: claim.claim,
                    path,
                });
            }
            operands.push(operand.argument);
        }
        claims.sort();
        let place = place_id(allocate_dense(self.environment.next_place)?);
        self.environment
            .private_places
            .push(completed_aggregate::emit_completed(
                place,
                parent_type,
                parent_multiplicity,
                CompletedAggregate::Array { elements: operands },
                self.evaluation
                    .structural_parameters
                    .iter()
                    .map(|(_, parameter)| parameter),
                self.operations,
            )?);
        self.operations
            .set_structural_result_claims(place, claims)?;
        self.values.truncate(source_count);
        Ok(place)
    }

    /// Establish the current carrier's whole zero as a fresh owned place. The
    /// image owns storage only, so a linear carrier's zero is presented as
    /// affine (`zeroed_image`).
    pub(super) fn establish_zeroed(&mut self) -> Result<PlaceId, LoweringError> {
        let place = place_id(allocate_dense(self.environment.next_place)?);
        let operation = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: operation,
            result: structural_result_declaration(
                place,
                self.structural_type,
                crate::emission::borrowed_window::zeroed_image(self.multiplicity),
            ),
            kind: OperationKind::EstablishZeroedStructural {},
        });
        self.environment
            .private_places
            .push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type: self.structural_type,
                },
            });
        Ok(place)
    }
}
