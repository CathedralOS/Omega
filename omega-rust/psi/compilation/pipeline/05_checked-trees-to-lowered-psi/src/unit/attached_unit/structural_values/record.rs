//! Complete fields in authored order before committing a declaration-ordered record.
//! Byte members retain their complete owned value, including live length and
//! qualifications. A destination declaration never supplies missing source proof:
//! the result rebases memberships from its completed operands, not field types.
use super::super::super::StructuralFieldType;
use super::super::{
    CheckedScalarExpressionRole, LoweringError, OperationKind, PlaceId, ScalarType,
    StructuralTypeShape, ValueDeclaration, allocate_dense, place_id, unsupported,
};
use super::completed_aggregate::{self, CompletedAggregate};
use super::emission;
use crate::emission::operation_emission::expressions::emit_scalar_leaf;
use crate::unit::attached_unit::structural_values::ValueOutcome;
use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue};

impl emission::Emission<'_, '_, '_> {
    pub(super) fn record(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        value: checked_trees::CheckedStructuralValueHandle,
    ) -> Result<ValueOutcome<PlaceId>, LoweringError> {
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
            let ValueOutcome::Normal(value) = self.completed_member_value(
                identities,
                field,
                target,
                CheckedScalarExpressionRole::RecordField {
                    expression: node.expression,
                    field_ordinal: u32::try_from(ordinal)
                        .map_err(|_| LoweringError::Unsupported("record field ordinal overflow"))?,
                },
                None,
                &mut scalar_positions,
                initialized.len(),
                source_count,
                &mut claims,
            )?
            else {
                return Ok(ValueOutcome::NoNormal);
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
        let place = place_id(allocate_dense(self.environment.next_place)?);
        let declaration = completed_aggregate::emit_completed(
            place,
            self.structural_type,
            self.multiplicity,
            CompletedAggregate::Record {
                fields: initialized.into_iter().map(|(_, field)| field).collect(),
                declarations: &shape_fields,
            },
            self.evaluation.structural_root_parameters(),
            self.operations,
        )?;
        if let semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } =
            declaration.kind
        {
            self.evaluation
                .retire_structural_operation_operands(producer, self.operations)?;
        }
        self.operations
            .set_structural_result_claims(place, claims)?;
        self.environment.private_places.push(declaration);
        Ok(ValueOutcome::Normal(place))
    }

    /// Omitted scalar fields are checked zero operands, not structural children.
    /// Records and mixed-case common fields use the same carrier/range join.
    pub(super) fn zeroed_scalar_field(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        value: checked_trees::CheckedStructuralValueHandle,
        scalar_type: ScalarType,
        field: &terminal_psi::StructuralFieldDeclaration,
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
            identities,
            field,
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
