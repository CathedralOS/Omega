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
use crate::emission::operation_emission::expressions::emit_scalar_leaf;

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
        // A mixed shape's establishment roster is its runtime common fields in
        // declaration order, then the selected case's payload. Erased common
        // members carry no runtime field and take no roster position, so a
        // union whose common members are all erased writes the tag and the
        // payload alone, exactly as a closed sum does.
        let (common, cases) = match &shape.shape {
            StructuralTypeShape::Sum { cases } => (Vec::new(), cases.clone()),
            StructuralTypeShape::Mixed { fields, cases } => (
                fields
                    .iter()
                    .filter(|field| !matches!(field.field_type, StructuralFieldType::Erased { .. }))
                    .cloned()
                    .collect::<Vec<_>>(),
                cases.clone(),
            ),
            _ => return unsupported("case construction has a non-sum shape"),
        };
        let common_declarations = self
            .checked
            .data_members(owner)
            .iter()
            .filter_map(|member| match member {
                checked_trees::data::DataMember::Field(field) => Some(field),
                checked_trees::data::DataMember::Variant(_) => None,
            })
            .collect::<Vec<_>>();
        let mut selected = None;
        for member in self.checked.data_members(owner) {
            let checked_trees::data::DataMember::Variant(variant) = member else {
                continue;
            };
            let identity = variant.path_identity();
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
        // Erased payload members hold no runtime slot either; checking consumed
        // any authored one as proof material, so none reaches this roster.
        let shape_fields =
            common
                .iter()
                .chain(shape_case.fields.iter().filter(|field| {
                    !matches!(field.field_type, StructuralFieldType::Erased { .. })
                }))
                .cloned()
                .collect::<Vec<_>>();
        let source_count = self.values.len();
        let mut initialized = Vec::new();
        let mut scalar_positions = Vec::new();
        for (ordinal, field) in fields.iter().enumerate() {
            // Common and payload identities live in separate namespaces; the
            // declaring member decides which half of the roster it indexes.
            let (declaration, roster) = match declarations
                .iter()
                .find(|declaration| declaration.symbol == field.field)
            {
                Some(declaration) => (declaration, common.len()..shape_fields.len()),
                None => (
                    *common_declarations
                        .iter()
                        .find(|declaration| declaration.symbol == field.field)
                        .ok_or(LoweringError::Unsupported("case field declaration missing"))?,
                    0..common.len(),
                ),
            };
            let identity = declaration.path_identity();
            let position = shape_fields[roster.clone()]
                .iter()
                .position(|candidate| candidate.identity == identity)
                .map(|offset| roster.start + offset)
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
                            | StructuralFieldType::RefinedScalar(_)
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
                        if terminal_semantics::scalar_field_requires_evidence(&target.field_type) {
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
                    let place = self.record_field_value(value, field.type_reference)?;
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
        // Case construction zero-initializes every common field the literal
        // leaves unnamed. The zero is an ordinary constant after every
        // authored operand, so no later evaluation can move it; a bounded
        // common field still requests its range proof against that zero.
        for (position, declaration) in common.iter().enumerate() {
            if initialized
                .iter()
                .any(|(initialized, _)| *initialized == position)
            {
                continue;
            }
            let scalar_type =
                declaration
                    .field_type
                    .scalar_type()
                    .ok_or(LoweringError::Unsupported(
                        "an unnamed common field has no scalar zero",
                    ))?;
            let zero = emit_scalar_leaf(
                super::record::zero_constant(scalar_type),
                scalar_type,
                self.next_value,
                self.operations,
            );
            let range_obligation =
                if terminal_semantics::scalar_field_requires_evidence(&declaration.field_type) {
                    Some(obligation_id(allocate_dense(
                        &mut self.calls.next_obligation_identity,
                    )?))
                } else {
                    None
                };
            initialized.push((
                position,
                terminal_psi::RecordFieldInitializer {
                    field: declaration.id,
                    value: terminal_psi::RecordFieldValue::Scalar {
                        value: zero,
                        range_obligation,
                    },
                },
            ));
        }
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
