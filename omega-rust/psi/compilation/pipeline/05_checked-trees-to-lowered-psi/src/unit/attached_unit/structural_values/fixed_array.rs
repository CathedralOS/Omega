//! Complete fixed-array operands in authored index order before assembly.
//!
//! Every child follows ordinary structural value production. The final array
//! operation transfers completed operands atomically; evaluation or suspension
//! before it keeps those operands under their existing temporary custody.
//! Element memberships move with those completed values under FixedIndex paths,
//! never from the array's receiving annotation or a record-like field identity.
use super::super::{
    LoweringError, Operation, OperationKind, PlaceId, StructuralTypeShape, allocate_dense,
    place_id, unsupported,
};
use super::completed_aggregate::{self, CompletedAggregate};
use super::emission;
use crate::emission::operation_emission::buffer::structural_result_declaration;
use crate::emission::operation_emission::place_declarations::operation_result_place;
use crate::unit::attached_unit::structural_values::ValueOutcome;
use checked_trees::CheckedStructuralValueHandle;

impl emission::Emission<'_, '_, '_> {
    pub(super) fn fixed_array(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        elements: &[CheckedStructuralValueHandle],
        type_reference: checked_trees::types::TypeReferenceHandle,
    ) -> Result<ValueOutcome<PlaceId>, LoweringError> {
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
            let ValueOutcome::Normal(operand) =
                self.child_constructor_operand(identities, element, *value, element_reference)?
            else {
                return Ok(ValueOutcome::NoNormal);
            };
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
        let declaration = completed_aggregate::emit_completed(
            place,
            parent_type,
            parent_multiplicity,
            CompletedAggregate::Array { elements: operands },
            self.evaluation.structural_root_parameters(),
            self.operations,
        )?;
        if let semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } =
            declaration.kind
        {
            self.evaluation
                .retire_structural_operation_operands(producer, self.operations)?;
        }
        self.environment.private_places.push(declaration);
        self.operations
            .set_structural_result_claims(place, claims)?;
        self.values.truncate(source_count);
        Ok(ValueOutcome::Normal(place))
    }

    /// Copy a quoted evaluator value into ordinary exact-width array storage.
    /// Its cooked source bytes and complete scalar declaration are checked
    /// before any operation is emitted; immutable image storage never becomes
    /// an owned argument or a fabricated borrowed descriptor.
    pub(super) fn fixed_byte_array_literal(
        &mut self,
        expression: checked_trees::expression::ExpressionHandle,
        bytes: &[u8],
        type_reference: checked_trees::types::TypeReferenceHandle,
    ) -> Result<PlaceId, LoweringError> {
        if validation::fixed_byte_array_literal(&self.checked.typed, expression, type_reference)
            != Some(bytes)
        {
            return unsupported("fixed byte literal lost its exact source and declared width");
        }
        let declaration =
            terminal_psi::find_structural_type(&self.structural_types[..], self.structural_type)
                .ok_or(LoweringError::Unsupported(
                    "fixed byte literal carrier is missing",
                ))?;
        let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
            return unsupported("fixed byte literal has a nonarray carrier");
        };
        if usize::try_from(length).ok() != Some(bytes.len()) {
            return unsupported("fixed byte literal width differs from its array carrier");
        }
        let scalar_type = crate::emission::scalar_types::integer_scalar_type(
            checked_trees::types::PrimitiveType::U8,
        )?;
        let element = terminal_psi::find_structural_type(&self.structural_types[..], element)
            .ok_or(LoweringError::Unsupported(
                "fixed byte literal element is missing",
            ))?;
        if element.shape != StructuralTypeShape::PrimitiveScalar(scalar_type)
            || element.quotient_carrier.is_some()
        {
            return unsupported("fixed byte literal element is not an ordinary byte scalar");
        }
        let integer_type = scalar_type.as_integer().ok_or(LoweringError::Unsupported(
            "fixed byte literal element is not an integer",
        ))?;
        if let Some(restriction) = &element.scalar_restriction {
            for byte in bytes {
                let literal = semantic_vocabulary::ScalarTerm::Integer {
                    scalar_type: integer_type,
                    value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*byte)),
                };
                if !terminal_semantics::scalar_field_accepts_literal(restriction, &literal) {
                    return unsupported("fixed byte literal violates its element declaration");
                }
            }
        }
        let mut elements = Vec::with_capacity(bytes.len());
        for byte in bytes {
            elements.push(
                crate::emission::operation_emission::expressions::emit_scalar_leaf(
                    OperationKind::IntegerConstant {
                        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*byte)),
                    },
                    scalar_type,
                    self.next_value,
                    self.operations,
                ),
            );
        }
        let (place, operation) = super::super::operation_frame::allocate_structural_result(
            self.operations,
            self.environment.next_place,
            self.structural_type,
            self.multiplicity,
            Vec::new(),
            OperationKind::EstablishScalarArray { elements },
        )?;
        self.environment.private_places.push(operation_result_place(
            place,
            operation,
            self.structural_type,
        ));
        Ok(place)
    }

    /// Establish the current carrier's whole zero as a fresh owned place. The
    /// image owns storage only, so a linear carrier's zero is presented as
    /// affine (`zeroed_image`).
    pub(super) fn establish_zeroed(&mut self) -> Result<PlaceId, LoweringError> {
        let place = place_id(allocate_dense(self.environment.next_place)?);
        let operation = self.operations.allocate();
        self.operations.push(Operation::unsuspended(
            operation,
            structural_result_declaration(
                place,
                self.structural_type,
                crate::emission::borrowed_window::zeroed_image(self.multiplicity),
            ),
            OperationKind::EstablishZeroedStructural {},
        ));
        self.environment.private_places.push(operation_result_place(
            place,
            operation,
            self.structural_type,
        ));
        Ok(place)
    }
}
