//! Complete fields in authored order before committing a declaration-ordered case.
use super::super::super::{StructuralAccess, StructuralArgument, StructuralFieldType};
use super::super::{
    CheckedScalarExpressionRole, LoweringError, Multiplicity, Operation, OperationKind,
    OperationResult, PlaceId, StructuralMultiplicity, StructuralOperationResult,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId, StructuralTypeShape,
    allocate_dense, lookup_type_id, obligation_id, place_id, unsupported,
};
use super::emission;
use crate::emission::operation_emission::buffer::OperationBuffer;

impl emission::Emission<'_, '_, '_> {
    pub(super) fn structural_case(
        &mut self,
        value: checked_trees::CheckedStructuralValueHandle,
    ) -> Result<PlaceId, LoweringError> {
        let node = self
            .checked
            .facts
            .values
            .structural_values
            .nodes
            .get(value)
            .clone();
        let checked_trees::CheckedStructuralValueKind::StructuralCase {
            data_symbol,
            case: case_symbol,
            fields,
        } = node.kind
        else {
            return unsupported("structural case producer missing");
        };
        let fields = self
            .checked
            .facts
            .values
            .structural_values
            .record_fields
            .span(fields)
            .ok_or(LoweringError::Unsupported("case fields are stale"))?
            .to_vec();
        let owner = self
            .checked
            .data_definitions()
            .iter()
            .find(|data| data.symbol == data_symbol)
            .ok_or(LoweringError::Unsupported("case owner declaration missing"))?;
        let shape = self
            .structural_types
            .iter()
            .find(|item| item.id == self.structural_type)
            .ok_or(LoweringError::Unsupported("case structural type missing"))?;
        let StructuralTypeShape::Sum { cases } = &shape.shape else {
            return unsupported("case construction has a non-sum shape");
        };
        let cases = cases.clone();
        let mut selected = None;
        for member in self.checked.data_members(owner) {
            let checked_trees::data::DataMember::Variant(variant) = member else {
                continue;
            };
            let identity = variant
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| variant.name.as_str().to_owned());
            if variant.symbol == case_symbol {
                let case = cases
                    .iter()
                    .find(|case| case.identity == identity)
                    .ok_or(LoweringError::Unsupported("case declaration is absent"))?;
                selected = Some((variant, case.clone()));
            }
        }
        let (variant, shape_case) = selected.ok_or(LoweringError::Unsupported(
            "case construction selected a foreign case",
        ))?;
        let declarations = self.checked.data_payload_fields(variant);
        let shape_fields = shape_case.fields.clone();
        let source_count = self.values.len();
        let mut initialized = Vec::new();
        let mut scalar_positions = Vec::new();
        for (ordinal, field) in fields.iter().enumerate() {
            let declaration = declarations
                .iter()
                .find(|declaration| declaration.symbol == field.field)
                .ok_or(LoweringError::Unsupported("case field declaration missing"))?;
            let identity = declaration
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| declaration.name.as_str().to_owned());
            let position = shape_fields
                .iter()
                .position(|candidate| candidate.identity == identity)
                .ok_or(LoweringError::Unsupported("case field identity missing"))?;
            let target = &shape_fields[position];
            // Erased members carry no runtime field, so a case literal cannot
            // establish them; only transfer can deliver one.
            if matches!(target.field_type, StructuralFieldType::Erased { .. }) {
                return unsupported("erased case member has no runtime initializer");
            }
            let value = match field.value {
                checked_trees::CheckedStructuralRecordFieldValue::Scalar(value) => {
                    let evaluated = self.scalar(
                        CheckedScalarExpressionRole::StructuralValueField {
                            expression: node.expression,
                            field_ordinal: u32::try_from(ordinal).map_err(|_| {
                                LoweringError::Unsupported("case field ordinal overflow")
                            })?,
                        },
                        value,
                        source_count,
                    )?;
                    if !matches!(
                        target.field_type,
                        StructuralFieldType::Scalar(_)
                            | StructuralFieldType::IeeeFloat(_)
                            | StructuralFieldType::BoundedInteger(_)
                    ) || target.field_type.scalar_type() != Some(evaluated.scalar_type)
                    {
                        return unsupported(
                            "case operand requires its exact scalar carrier and range evidence",
                        );
                    }
                    scalar_positions.push((initialized.len(), self.values.len()));
                    self.values.push(evaluated);
                    // The verifier reconstructs the declared range against the
                    // final SSA initializer. This identity requests that proof;
                    // it is not an assertion that construction is valid.
                    let range_obligation =
                        if matches!(target.field_type, StructuralFieldType::BoundedInteger(_)) {
                            Some(obligation_id(allocate_dense(
                                &mut self.calls.next_obligation_identity,
                            )?))
                        } else {
                            None
                        };
                    terminal_psi::RecordFieldValue::Scalar {
                        value: evaluated.id,
                        range_obligation,
                    }
                }
                checked_trees::CheckedStructuralRecordFieldValue::Structural(value) => {
                    let StructuralFieldType::Structural(child_type) = target.field_type else {
                        return unsupported("nested case operand has a scalar destination");
                    };
                    if lookup_type_id(
                        self.type_ids,
                        self.checked
                            .normalized_type_identity(field.type_reference)
                            .as_str(),
                    )? != child_type
                    {
                        return unsupported("nested case operand changed its exact carrier");
                    }
                    let parent_type = self.structural_type;
                    let parent_multiplicity = self.multiplicity;
                    self.structural_type = child_type;
                    self.multiplicity =
                        match validation::reference_result_custody::result_multiplicity(
                            &self.checked.typed,
                            field.type_reference,
                        ) {
                            Multiplicity::Affine => StructuralMultiplicity::Affine,
                            Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                            Multiplicity::Linear => {
                                return unsupported("nested case cannot create linear custody");
                            }
                        };
                    let place = self.record_field_value(value)?;
                    self.structural_type = parent_type;
                    self.multiplicity = parent_multiplicity;
                    terminal_psi::RecordFieldValue::Structural(StructuralArgument {
                        place,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    })
                }
            };
            initialized.push((
                position,
                terminal_psi::RecordFieldInitializer {
                    field: target.id,
                    value,
                },
            ));
        }
        // Private control flow remaps saved scalar values. Read their final slots
        // only after every later field evaluation has completed.
        for (field, position) in scalar_positions {
            let terminal_psi::RecordFieldValue::Scalar { value, .. } =
                &mut initialized[field].1.value
            else {
                unreachable!()
            };
            *value = self.values[position].id;
        }
        self.values.truncate(source_count);
        initialized.sort_by_key(|(position, _)| *position);
        let place = place_id(allocate_dense(self.next_place)?);
        self.temporary_places.push(emit_completed(
            place,
            self.structural_type,
            self.multiplicity,
            shape_case.id,
            initialized.into_iter().map(|(_, field)| field).collect(),
            self.operations,
        ));
        Ok(place)
    }
}

/// Commit completed, declaration-ordered payload fields under the selected
/// case tag. Operand evaluation and source custody remain with their shared
/// scalar/structural semantic owners.
fn emit_completed(
    place: PlaceId,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    result_case: semantic_vocabulary::StructuralCaseId,
    fields: Vec<terminal_psi::RecordFieldInitializer>,
    operations: &mut OperationBuffer,
) -> StructuralPlaceDeclaration {
    let operation = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place,
            structural_type,
            multiplicity,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishStructuralCase {
            result_case,
            fields,
        },
    });
    StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation,
            structural_type,
        },
    }
}
