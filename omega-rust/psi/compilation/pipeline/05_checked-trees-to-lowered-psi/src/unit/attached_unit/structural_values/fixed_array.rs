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
        let place = place_id(allocate_dense(self.next_place)?);
        self.temporary_places.push(emit_completed(
            place,
            parent_type,
            parent_multiplicity,
            operands,
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
        let place = place_id(allocate_dense(self.next_place)?);
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
pub(crate) fn emit_completed<'parameter>(
    place: PlaceId,
    structural_type: semantic_vocabulary::StructuralTypeId,
    multiplicity: terminal_psi::StructuralMultiplicity,
    elements: Vec<terminal_psi::StructuralArgument>,
    parameters: impl Iterator<Item = &'parameter terminal_psi::StructuralParameterDeclaration> + Clone,
    operations: &mut crate::emission::operation_emission::buffer::OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let mut projected_qualifications = Vec::new();
    for (ordinal, argument) in elements.iter().enumerate() {
        super::member_memberships::append(
            argument,
            &[terminal_psi::StructuralPathSegment::FixedIndex(
                u64::try_from(ordinal)
                    .map_err(|_| LoweringError::Unsupported("array ordinal overflow"))?,
            )],
            parameters.clone(),
            operations,
            &mut projected_qualifications,
        );
    }
    projected_qualifications.sort();
    projected_qualifications.dedup();
    let mut result = structural_result_declaration(place, structural_type, multiplicity);
    if let terminal_psi::OperationResult::Structural(result) = &mut result {
        result.projected_qualifications = projected_qualifications;
    }
    let operation = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result,
        kind: OperationKind::EstablishStructuralArray { elements },
    });
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation,
            structural_type,
        },
    })
}
