//! Complete fields in authored order before committing a declaration-ordered record.
//! Byte members retain their complete owned value, including live length and
//! qualifications. A destination declaration never supplies missing source proof:
//! the result rebases memberships from its completed operands, not field types.
use super::super::super::StructuralFieldType;
use super::super::{
    CheckedScalarExpressionRole, LoweringError, Operation, OperationKind, PlaceId, ScalarType,
    StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId,
    StructuralTypeShape, ValueDeclaration, allocate_dense, place_id, unsupported,
};
use super::emission;
use crate::emission::operation_emission::buffer::{OperationBuffer, structural_result_declaration};
use crate::emission::operation_emission::expressions::emit_scalar_leaf;
use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue};

impl emission::Emission<'_, '_, '_> {
    pub(super) fn record(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
        let fields = self.structural_value_fields(fields, "record fields are stale")?;
        let record = self
            .checked
            .data_definition_by_symbol(data_symbol)
            .ok_or(LoweringError::Unsupported("record declaration missing"))?;
        let declarations = self.checked.data_members(record);
        let shape =
            terminal_psi::find_structural_type(&self.structural_types[..], self.structural_type)
                .ok_or(LoweringError::Unsupported("record structural type missing"))?;
        let StructuralTypeShape::Record {
            fields: shape_fields,
            ..
        } = &shape.shape
        else {
            return unsupported("record construction has a nonrecord shape");
        };
        let shape_fields = shape_fields.clone();
        let source_count = self.values.len();
        let mut initialized = Vec::new();
        let mut scalar_positions = Vec::new();
        let mut claims = Vec::new();
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
            let identity = checked_trees::CheckedMemberIdentity::of_member(
                declaration.identity,
                declaration.name.clone(),
            );
            let position = shape_fields
                .iter()
                .position(|candidate| candidate.identity == identities.intern(&identity.spelling()))
                .ok_or(LoweringError::Unsupported("record field identity missing"))?;
            let target = &shape_fields[position];
            // Source custody replays every semantic initializer. Erased fields
            // keep their declaration identity but produce no executable operand,
            // storage or cleanup, including for nested initializer values.
            if target.relevance.is_erased() {
                if !matches!(target.field_type, StructuralFieldType::Erased { .. }) {
                    return unsupported("erased record field has a runtime carrier");
                }
                continue;
            }
            let value = match field.value {
                checked_trees::CheckedStructuralRecordFieldValue::Scalar(value) => {
                    let evaluated = self.scalar(
                        identities,
                        CheckedScalarExpressionRole::RecordField {
                            expression: node.expression,
                            field_ordinal: u32::try_from(ordinal).map_err(|_| {
                                LoweringError::Unsupported("record field ordinal overflow")
                            })?,
                        },
                        value,
                        source_count,
                    )?;
                    self.scalar_field_value(
                        &target.field_type,
                        evaluated,
                        &mut scalar_positions,
                        initialized.len(),
                    )?
                }
                checked_trees::CheckedStructuralRecordFieldValue::Structural(value) => {
                    if let Some(scalar_type) = target.field_type.scalar_type() {
                        self.zeroed_scalar_field(
                            value,
                            scalar_type,
                            &target.field_type,
                            &mut scalar_positions,
                            initialized.len(),
                        )?
                    } else {
                        let child_type = super::field_carrier::resolve(
                            identities,
                            self.checked,
                            self.structural_types,
                            &target.field_type,
                            field.type_reference,
                        )?;
                        let completed = self.child_constructor_operand(
                            identities,
                            child_type,
                            value,
                            field.type_reference,
                            &[terminal_psi::StructuralPathSegment::Field(
                                target.identity.clone(),
                            )],
                        )?;
                        if self.multiplicity == StructuralMultiplicity::Affine {
                            for claim in completed.claims {
                                let mut path = vec![terminal_psi::StructuralPathSegment::Field(
                                    target.identity.clone(),
                                )];
                                path.extend(claim.path);
                                claims.push(terminal_psi::StructuralResultClaimBinding {
                                    claim: claim.claim,
                                    path,
                                });
                            }
                        }
                        terminal_psi::RecordFieldValue::Structural(completed.argument)
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
        self.remap_saved_scalar_fields(scalar_positions, &mut initialized, source_count);
        initialized.sort_by_key(|(position, _)| *position);
        // An affine record contributes no claim of its own; each owned member
        // brings the claims it already holds, rebased under its field. A
        // linear record's whole-root claim is minted at its binding instead.
        claims.sort();
        let place = place_id(allocate_dense(self.next_place)?);
        let declaration = emit_completed(
            place,
            self.structural_type,
            self.multiplicity,
            initialized.into_iter().map(|(_, field)| field).collect(),
            &shape_fields,
            self.evaluation
                .structural_parameters
                .iter()
                .map(|(_, parameter)| parameter),
            self.operations,
        )?;
        if !claims.is_empty() {
            self.operations
                .set_structural_result_claims(place, claims)?;
        }
        self.temporary_places.push(declaration);
        Ok(place)
    }

    /// Omitted scalar fields are checked zero operands, not structural children.
    /// Records and mixed-case common fields use the same carrier/range join.
    pub(super) fn zeroed_scalar_field(
        &mut self,
        value: checked_trees::CheckedStructuralValueHandle,
        scalar_type: ScalarType,
        field_type: &StructuralFieldType,
        positions: &mut Vec<(usize, usize)>,
        field_index: usize,
    ) -> Result<terminal_psi::RecordFieldValue, LoweringError> {
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
            return unsupported("nested constructor operand has a scalar destination");
        }
        let zero = emit_scalar_leaf(
            zero_constant(scalar_type),
            scalar_type,
            self.next_value,
            self.operations,
        );
        self.scalar_field_value(
            field_type,
            ValueDeclaration {
                qualifications: Default::default(),
                id: zero,
                scalar_type,
            },
            positions,
            field_index,
        )
    }
}

/// Commit completed, declaration-ordered fields. Both structural statement
/// evaluation and scalar graphs use this operation; operand evaluation and
/// source custody remain with their shared scalar/structural semantic owners.
pub(crate) fn emit_completed<'parameter>(
    place: PlaceId,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    fields: Vec<terminal_psi::RecordFieldInitializer>,
    declarations: &[terminal_psi::StructuralFieldDeclaration],
    parameters: impl Iterator<Item = &'parameter terminal_psi::StructuralParameterDeclaration> + Clone,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let mut projected_qualifications = Vec::new();
    for field in &fields {
        let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
            continue;
        };
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.id == field.field)
            .ok_or(LoweringError::Unsupported(
                "completed record field has no declaration",
            ))?;
        // Inline byte fields already retain their represented predicates in
        // the carrier contract; projected rows describe nominal children.
        if !matches!(declaration.field_type, StructuralFieldType::Structural(_)) {
            continue;
        }
        let identity = &declaration.identity;
        super::member_memberships::append(
            argument,
            &[terminal_psi::StructuralPathSegment::Field(identity.clone())],
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
        kind: OperationKind::EstablishRecord { fields },
    });
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation,
            structural_type,
        },
    })
}

/// The declared scalar type's zero as one constant operation: the value
/// omitted scalar members and unnamed common fields establish.
pub(crate) fn zero_constant(scalar_type: ScalarType) -> OperationKind {
    match scalar_type {
        ScalarType::Integer(integer) => OperationKind::IntegerConstant {
            value: integer.zero(),
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
