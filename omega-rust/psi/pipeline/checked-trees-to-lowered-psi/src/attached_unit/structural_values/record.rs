//! Complete fields in authored order before committing a declaration-ordered record.
use super::*;

impl emission::Emission<'_, '_, '_> {
    pub(super) fn record(
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
        let checked_trees::CheckedStructuralValueKind::Record {
            data_symbol,
            fields,
        } = node.kind
        else {
            return unsupported("record producer missing");
        };
        let fields = self
            .checked
            .facts
            .values
            .structural_values
            .record_fields
            .span(fields)
            .ok_or(LoweringError::Unsupported("record fields are stale"))?
            .to_vec();
        let record = self
            .checked
            .data_definitions()
            .iter()
            .find(|data| data.symbol == data_symbol)
            .ok_or(LoweringError::Unsupported("record declaration missing"))?;
        let declarations = self.checked.data_members(record);
        let shape = self
            .structural_types
            .iter()
            .find(|item| item.id == self.structural_type)
            .ok_or(LoweringError::Unsupported("record structural type missing"))?;
        let StructuralTypeShape::Record {
            fields: shape_fields,
        } = &shape.shape
        else {
            return unsupported("record construction has a nonrecord shape");
        };
        let shape_fields = shape_fields.clone();
        let source_count = self.values.len();
        let mut initialized = Vec::new();
        let mut scalar_positions = Vec::new();
        for (ordinal, field) in fields.iter().enumerate() {
            let declaration = declarations
                .iter()
                .find_map(|member| match member {
                    checked_trees::data::DataMember::Field(declaration)
                        if declaration.symbol == field.field =>
                    {
                        Some(declaration)
                    }
                    _ => None,
                })
                .ok_or(LoweringError::Unsupported(
                    "record field declaration missing",
                ))?;
            let identity = declaration
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| declaration.name.as_str().to_owned());
            let position = shape_fields
                .iter()
                .position(|candidate| candidate.identity == identity)
                .ok_or(LoweringError::Unsupported("record field identity missing"))?;
            let target = &shape_fields[position];
            let value = match field.value {
                checked_trees::CheckedStructuralRecordFieldValue::Scalar(value) => {
                    let evaluated = self.scalar(
                        CheckedScalarExpressionRole::RecordField {
                            expression: node.expression,
                            field_ordinal: u32::try_from(ordinal).map_err(|_| {
                                LoweringError::Unsupported("record field ordinal overflow")
                            })?,
                        },
                        value,
                        source_count,
                    )?;
                    if !matches!(
                        target.field_type,
                        StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_)
                    ) || target.field_type.scalar_type() != Some(evaluated.scalar_type)
                    {
                        return unsupported(
                            "record operand requires its exact scalar carrier and range evidence",
                        );
                    }
                    scalar_positions.push((initialized.len(), self.values.len()));
                    self.values.push(evaluated);
                    terminal_psi::RecordFieldValue::Scalar {
                        value: evaluated.id,
                        range_obligation: None,
                    }
                }
                checked_trees::CheckedStructuralRecordFieldValue::Structural(value) => {
                    let StructuralFieldType::Structural(child_type) = target.field_type else {
                        return unsupported("nested record operand has a scalar destination");
                    };
                    if lookup_type_id(
                        self.type_ids,
                        self.checked
                            .normalized_type_identity(field.type_reference)
                            .as_str(),
                    )? != child_type
                    {
                        return unsupported("nested record operand changed its exact carrier");
                    }
                    let parent_type = self.structural_type;
                    let parent_multiplicity = self.multiplicity;
                    self.structural_type = child_type;
                    self.multiplicity = match self.checked.type_multiplicity(field.type_reference) {
                        Multiplicity::Affine => StructuralMultiplicity::Affine,
                        Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                        Multiplicity::Linear => {
                            return unsupported("nested record cannot create linear custody");
                        }
                    };
                    let place = self.value(value, None)?;
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
        let operation = self.operations.allocate();
        let place = place_id(allocate_dense(self.next_place)?);
        self.temporary_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type: self.structural_type,
            },
        });
        self.operations.push(Operation {
            static_reach_binding: None,
            id: operation,
            result: OperationResult::Structural(StructuralOperationResult {
                place,
                structural_type: self.structural_type,
                multiplicity: self.multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishRecord {
                fields: initialized.into_iter().map(|(_, field)| field).collect(),
            },
        });
        Ok(place)
    }
}
