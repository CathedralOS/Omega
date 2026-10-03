//! Complete fields in authored order before committing a declaration-ordered record.
//! Byte members retain their complete owned value, including live length and
//! qualifications. A destination declaration never supplies missing source proof:
//! Terminal replay checks each completed operand before establishing the record.
use super::super::super::{StructuralAccess, StructuralArgument, StructuralFieldType};
use super::super::{
    CheckedScalarExpressionRole, LoweringError, Multiplicity, Operation, OperationKind,
    OperationResult, PlaceId, ScalarType, StructuralMultiplicity, StructuralOperationResult,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId, StructuralTypeShape,
    ValueDeclaration, allocate_dense, lookup_type_id, obligation_id, place_id, unsupported,
};
use super::emission;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::emit_scalar_leaf;
use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerValue};

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
        // A literal with every member omitted has no authored operand: each
        // minted child is a zeroed structural mint whose value lowers as one
        // whole-record EstablishZeroedStructural over the record's own type.
        if !fields.is_empty()
            && fields.iter().all(|field| match field.value {
                checked_trees::CheckedStructuralRecordFieldValue::Structural(node) => matches!(
                    self.checked
                        .facts
                        .values
                        .structural_values
                        .nodes
                        .get(node)
                        .kind,
                    checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
                        | checked_trees::CheckedStructuralValueKind::ZeroedScalarArray { .. }
                ),
                checked_trees::CheckedStructuralRecordFieldValue::Scalar(_) => false,
            })
        {
            return self.establish_zeroed();
        }
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
            let identity = declaration.path_identity();
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
                    if let Some(scalar_type) = target.field_type.scalar_type() {
                        // Omitted scalar members mint a zeroed structural
                        // child with no authored operand; establish the
                        // declared type's zero as the field's scalar value.
                        if !matches!(
                            self.checked
                                .facts
                                .values
                                .structural_values
                                .nodes
                                .get(value)
                                .kind,
                            checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
                        ) {
                            return unsupported("nested record operand has a scalar destination");
                        }
                        let zero = emit_scalar_leaf(
                            zero_constant(scalar_type),
                            scalar_type,
                            self.next_value,
                            self.operations,
                        );
                        scalar_positions.push((initialized.len(), self.values.len()));
                        self.values.push(ValueDeclaration {
                            qualifications: Default::default(),
                            id: zero,
                            scalar_type,
                        });
                        let range_obligation = if matches!(
                            target.field_type,
                            StructuralFieldType::BoundedInteger(_)
                        ) {
                            Some(obligation_id(allocate_dense(
                                &mut self.calls.next_obligation_identity,
                            )?))
                        } else {
                            None
                        };
                        terminal_psi::RecordFieldValue::Scalar {
                            value: zero,
                            range_obligation,
                        }
                    } else {
                        // Nominal children retain their identity. Byte members
                        // instead embed a carrier contract; resolve that carrier
                        // without requiring an unrelated source alias to survive.
                        let child_type = match &target.field_type {
                            StructuralFieldType::Structural(child_type) => *child_type,
                            StructuralFieldType::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                            )
                            | StructuralFieldType::PredicateByteSequence {
                                carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                                ..
                            } => {
                                let carrier = super::super::structural_carrier_type(
                                    self.checked,
                                    field.type_reference,
                                )?;
                                lookup_type_id(
                                    self.type_ids,
                                    self.checked.normalized_type_identity(carrier).as_str(),
                                )?
                            }
                            StructuralFieldType::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                            )
                            | StructuralFieldType::PredicateByteSequence {
                                carrier: terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                                ..
                            } => {
                                // Byte fields embed their carrier, not a nominal
                                // type identity. Choose that descriptor contract
                                // from the operand catalog; an unrelated parameter
                                // must not be needed to root a source-spelled alias.
                                self.structural_types.iter().find_map(|declaration|
                                    matches!(declaration.shape,
                                        terminal_psi::StructuralTypeShape::ByteSequence(
                                            terminal_psi::ByteSequenceCarrier::BorrowedView {
                                                access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                                            }
                                        )
                                    ).then_some(declaration.id)
                                ).ok_or(LoweringError::Unsupported(
                                    "shared byte member has no descriptor carrier",
                                ))?
                            }
                            _ => {
                                return unsupported(
                                    "nested record operand has a scalar destination",
                                );
                            }
                        };
                        // A reference member's declared carrier is its whole
                        // shell — `&mut T` and `&T` register as `Reference`
                        // shapes under their own identity — while view
                        // members register peeled, access binding on the
                        // member rather than the shape, exactly as the
                        // `&[u8]` arm above reads its unwrapped carrier.
                        let child_is_reference = self.structural_types.iter().any(|declaration| {
                            declaration.id == child_type
                                && matches!(
                                    declaration.shape,
                                    StructuralTypeShape::Reference { .. }
                                )
                        });
                        let member_carrier = if child_is_reference {
                            Some(field.type_reference)
                        } else {
                            validation::unwrapped_type_reference(
                                &self.checked.typed,
                                field.type_reference,
                            )
                        };
                        if !matches!(
                            target.field_type,
                            StructuralFieldType::ByteSequence(_)
                                | StructuralFieldType::PredicateByteSequence { .. }
                        ) && lookup_type_id(
                            self.type_ids,
                            self.checked
                                .normalized_type_identity(member_carrier.ok_or(
                                    LoweringError::Unsupported(
                                        "view record member lost its carrier",
                                    ),
                                )?)
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
                                    return unsupported(
                                        "nested record cannot create linear custody",
                                    );
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

/// The declared scalar type's zero as one constant operation: the value
/// omitted scalar members and unnamed common fields establish.
pub(crate) fn zero_constant(scalar_type: ScalarType) -> OperationKind {
    match scalar_type {
        ScalarType::Integer(integer) => OperationKind::IntegerConstant {
            value: match integer.sign() {
                IntegerSign::Signed => IntegerValue::Signed(0),
                IntegerSign::Unsigned => IntegerValue::Unsigned(0),
            },
        },
        ScalarType::Boolean => OperationKind::BooleanConstant { value: false },
        ScalarType::IeeeFloat(format) => OperationKind::IeeeFloatConstant {
            value: match format {
                IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0),
                IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(0),
            },
        },
    }
}
