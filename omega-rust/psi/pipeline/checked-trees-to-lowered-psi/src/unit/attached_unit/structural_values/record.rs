//! Complete fields in authored order before committing a declaration-ordered record.
use super::super::super::{StructuralAccess, StructuralArgument, StructuralFieldType};
use super::super::{
    CheckedScalarExpressionRole, LoweringError, Multiplicity, Operation, OperationKind,
    OperationResult, PlaceId, StructuralMultiplicity, StructuralOperationResult,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId, StructuralTypeShape,
    allocate_dense, lookup_type_id, obligation_id, place_id, unsupported,
};
use super::emission;
use crate::emission::operation_emission::buffer::OperationBuffer;
use checked_trees::expression::ExpressionNode;

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
        let ExpressionNode::StructLiteral(literal) =
            self.checked.expression_table.expression(node.expression)
        else {
            return unsupported("record construction lost its authored literal");
        };
        let source_count = self.values.len();
        let mut initialized = Vec::new();
        let mut scalar_positions = Vec::new();
        for field in fields.iter() {
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
            // Erased members carry no runtime field, so a record literal
            // cannot establish them; only transfer can deliver one.
            if matches!(target.field_type, StructuralFieldType::Erased { .. }) {
                return unsupported("erased record member has no runtime initializer");
            }
            let value = match field.value {
                checked_trees::CheckedStructuralRecordFieldValue::Scalar(value) => {
                    let initializer_ordinal = self
                        .checked
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .position(|initializer| {
                            initializer.field_symbol == field.field
                                && initializer.value == field.expression
                        })
                        .ok_or(LoweringError::Unsupported(
                            "record field escaped its authored literal",
                        ))?;
                    let evaluated = self.scalar(
                        CheckedScalarExpressionRole::RecordField {
                            expression: node.expression,
                            field_ordinal: u32::try_from(initializer_ordinal).map_err(|_| {
                                LoweringError::Unsupported("record field ordinal overflow")
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
                            "record operand requires its exact scalar carrier and range evidence",
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
                    self.multiplicity =
                        match validation::reference_result_custody::result_multiplicity(
                            &self.checked.typed,
                            field.type_reference,
                        ) {
                            Multiplicity::Affine => StructuralMultiplicity::Affine,
                            Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                            Multiplicity::Linear => {
                                return unsupported("nested record cannot create linear custody");
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
                checked_trees::CheckedStructuralRecordFieldValue::Zero => {
                    // An authored literal omits this declared member: it reads
                    // the zero-initialized value of the declared type, emitted
                    // from the field's shape rather than from an operand.
                    match &target.field_type {
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::IeeeFloat(_)
                        | StructuralFieldType::BoundedInteger(_) => {
                            let Some(scalar_type) = target.field_type.scalar_type() else {
                                return unsupported("zero record member lost its carrier");
                            };
                            terminal_psi::RecordFieldValue::Scalar {
                                value: zero_scalar_leaf(
                                    scalar_type,
                                    self.next_value,
                                    self.operations,
                                ),
                                range_obligation: if matches!(
                                    target.field_type,
                                    StructuralFieldType::BoundedInteger(_)
                                ) {
                                    Some(obligation_id(allocate_dense(
                                        &mut self.calls.next_obligation_identity,
                                    )?))
                                } else {
                                    None
                                },
                            }
                        }
                        StructuralFieldType::Structural(child_type) => {
                            if lookup_type_id(
                                self.type_ids,
                                self.checked
                                    .normalized_type_identity(field.type_reference)
                                    .as_str(),
                            )? != *child_type
                            {
                                return unsupported(
                                    "omitted record member changed its exact carrier",
                                );
                            }
                            let multiplicity =
                                match validation::reference_result_custody::result_multiplicity(
                                    &self.checked.typed,
                                    field.type_reference,
                                ) {
                                    Multiplicity::Affine => StructuralMultiplicity::Affine,
                                    Multiplicity::Unrestricted => {
                                        StructuralMultiplicity::Unrestricted
                                    }
                                    Multiplicity::Linear => {
                                        return unsupported(
                                            "omitted record member cannot establish linear custody",
                                        );
                                    }
                                };
                            let child_place = place_id(allocate_dense(self.next_place)?);
                            self.temporary_places.extend(emit_zero_place(
                                child_place,
                                *child_type,
                                multiplicity,
                                self.structural_types,
                                self.next_place,
                                self.next_value,
                                &mut self.calls.next_obligation_identity,
                                self.operations,
                            )?);
                            terminal_psi::RecordFieldValue::Structural(StructuralArgument {
                                place: child_place,
                                path: Vec::new(),
                                access: StructuralAccess::Owned,
                            })
                        }
                        _ => {
                            return unsupported(
                                "omitted record member has no composed zero spelling",
                            );
                        }
                    }
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
            initialized.into_iter().map(|(_, field)| field).collect(),
            self.operations,
        ));
        Ok(place)
    }
}

/// One zero-initialized leaf of a composed field zero.
pub(crate) fn zero_scalar_leaf(
    scalar_type: semantic_vocabulary::ScalarType,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> semantic_vocabulary::ValueId {
    let kind = match scalar_type {
        semantic_vocabulary::ScalarType::Boolean => OperationKind::BooleanConstant { value: false },
        semantic_vocabulary::ScalarType::Integer(integer) => OperationKind::IntegerConstant {
            value: match integer.sign() {
                semantic_vocabulary::IntegerSign::Signed => {
                    semantic_vocabulary::IntegerValue::Signed(0)
                }
                semantic_vocabulary::IntegerSign::Unsigned => {
                    semantic_vocabulary::IntegerValue::Unsigned(0)
                }
            },
        },
        semantic_vocabulary::ScalarType::IeeeFloat(format) => OperationKind::IeeeFloatConstant {
            value: match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => {
                    semantic_vocabulary::IeeeFloatValue::Binary32(0)
                }
                semantic_vocabulary::IeeeFloatFormat::Binary64 => {
                    semantic_vocabulary::IeeeFloatValue::Binary64(0)
                }
            },
        },
    };
    crate::emission::operation_emission::expressions::emit_scalar_leaf(
        kind,
        scalar_type,
        next_value,
        operations,
    )
}

/// The scalar carrier roster of a closed array's zero, outer index first.
/// Scalar graphs resolve the carriers before their leaf mint is scheduled;
/// record emission mints each leaf in place.
pub(crate) fn zero_scalar_leaf_types(
    element_type: StructuralTypeId,
    length: u64,
    types: &[terminal_psi::StructuralTypeDeclaration],
    leaves: &mut Vec<semantic_vocabulary::ScalarType>,
) -> Result<(), LoweringError> {
    let element = types
        .iter()
        .find(|declaration| declaration.id == element_type)
        .ok_or(LoweringError::Unsupported(
            "zero array lost its element declaration",
        ))?;
    match &element.shape {
        StructuralTypeShape::PrimitiveScalar(scalar_type) => {
            for _ in 0..length {
                leaves.push(*scalar_type);
            }
            Ok(())
        }
        StructuralTypeShape::FixedArray {
            element: inner,
            length: inner_length,
        } => {
            for _ in 0..length {
                zero_scalar_leaf_types(*inner, *inner_length, types, leaves)?;
            }
            Ok(())
        }
        _ => unsupported("zero array elements have no scalar leaf spelling"),
    }
}

/// The scalar-leaf roster of a closed array's zero, outer index first.
fn zero_scalar_leaves(
    element_type: StructuralTypeId,
    length: u64,
    types: &[terminal_psi::StructuralTypeDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    leaves: &mut Vec<semantic_vocabulary::ValueId>,
) -> Result<(), LoweringError> {
    let mut carriers = Vec::new();
    zero_scalar_leaf_types(element_type, length, types, &mut carriers)?;
    for carrier in carriers {
        leaves.push(zero_scalar_leaf(carrier, next_value, operations));
    }
    Ok(())
}

/// Establish the composed zero of one structural type: records emit a nested
/// `EstablishRecord` of recursively zero fields, closed scalar-leaf arrays an
/// `EstablishScalarArray` of zero leaves. Returns every place declaration the
/// emission created, the root place last, mirroring `emit_completed`.
pub(crate) fn emit_zero_place(
    place: PlaceId,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    types: &[terminal_psi::StructuralTypeDeclaration],
    next_place: &mut u64,
    next_value: &mut u64,
    next_obligation: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Vec<StructuralPlaceDeclaration>, LoweringError> {
    let declaration = types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .ok_or(LoweringError::Unsupported(
            "zero place lost its structural declaration",
        ))?;
    let mut declarations = Vec::new();
    match &declaration.shape {
        StructuralTypeShape::Record { fields } => {
            let mut initializers = Vec::with_capacity(fields.len());
            for field in fields {
                let value = match &field.field_type {
                    StructuralFieldType::Scalar(_)
                    | StructuralFieldType::IeeeFloat(_)
                    | StructuralFieldType::BoundedInteger(_) => {
                        let Some(scalar_type) = field.field_type.scalar_type() else {
                            return unsupported("zero scalar field lost its carrier");
                        };
                        terminal_psi::RecordFieldValue::Scalar {
                            value: zero_scalar_leaf(scalar_type, next_value, operations),
                            range_obligation: if matches!(
                                field.field_type,
                                StructuralFieldType::BoundedInteger(_)
                            ) {
                                Some(obligation_id(allocate_dense(next_obligation)?))
                            } else {
                                None
                            },
                        }
                    }
                    StructuralFieldType::Structural(child_type) => {
                        let child_place = place_id(allocate_dense(next_place)?);
                        declarations.extend(emit_zero_place(
                            child_place,
                            *child_type,
                            StructuralMultiplicity::Unrestricted,
                            types,
                            next_place,
                            next_value,
                            next_obligation,
                            operations,
                        )?);
                        terminal_psi::RecordFieldValue::Structural(StructuralArgument {
                            place: child_place,
                            path: Vec::new(),
                            access: StructuralAccess::Owned,
                        })
                    }
                    _ => return unsupported("zero record member has no composed field spelling"),
                };
                initializers.push(terminal_psi::RecordFieldInitializer {
                    field: field.id,
                    value,
                });
            }
            declarations.push(emit_completed(
                place,
                structural_type,
                multiplicity,
                initializers,
                operations,
            ));
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let mut elements = Vec::new();
            zero_scalar_leaves(
                *element,
                *length,
                types,
                next_value,
                operations,
                &mut elements,
            )?;
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
                kind: OperationKind::EstablishScalarArray { elements },
            });
            declarations.push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type,
                },
            });
        }
        _ => return unsupported("zero place has no composed structural spelling"),
    }
    Ok(declarations)
}

/// Commit completed, declaration-ordered fields. Both structural statement
/// evaluation and scalar graphs use this operation; operand evaluation and
/// source custody remain with their shared scalar/structural semantic owners.
pub(crate) fn emit_completed(
    place: PlaceId,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
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
        kind: OperationKind::EstablishRecord { fields },
    });
    StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation,
            structural_type,
        },
    }
}
