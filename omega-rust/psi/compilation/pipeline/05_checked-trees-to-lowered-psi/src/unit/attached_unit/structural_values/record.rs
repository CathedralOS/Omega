//! Complete fields in authored order before committing a declaration-ordered record.
//! Byte members retain their complete owned value, including live length and
//! qualifications. A destination declaration never supplies missing source proof:
//! the result rebases memberships from its completed operands, not field types.
use super::super::super::StructuralFieldType;
use super::super::{
    CheckedScalarExpressionRole, LoweringError, Operation, OperationKind, OperationResult, PlaceId,
    ScalarType, StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeId, StructuralTypeShape, ValueDeclaration, allocate_dense, place_id, unsupported,
};
use super::emission;
use crate::emission::operation_emission::buffer::{OperationBuffer, structural_result_declaration};
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
        let fields = self.structural_value_fields(fields, "record fields are stale")?;
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
        // Members whose type carries linear claims; only they bring custody.
        let mut claim_bearing_fields = Vec::new();
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
                        self.scalar_field_value(
                            &target.field_type,
                            ValueDeclaration {
                                qualifications: Default::default(),
                                id: zero,
                                scalar_type,
                            },
                            &mut scalar_positions,
                            initialized.len(),
                        )?
                    } else {
                        let child_type = super::field_carrier::resolve(
                            self.checked,
                            self.structural_types,
                            &target.field_type,
                            field.type_reference,
                        )?;
                        let argument = self.child_constructor_operand(
                            child_type,
                            value,
                            field.type_reference,
                            terminal_psi::StructuralPathSegment::Field(target.identity.clone()),
                        )?;
                        if !validation::linear_claim_frontier(
                            &self.checked.typed,
                            field.type_reference,
                        )
                        .is_empty()
                        {
                            claim_bearing_fields.push(target.id);
                        }
                        terminal_psi::RecordFieldValue::Structural(argument)
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
        let mut claims = Vec::new();
        if self.multiplicity == StructuralMultiplicity::Affine {
            for (_, initializer) in &initialized {
                let terminal_psi::RecordFieldValue::Structural(argument) = &initializer.value
                else {
                    continue;
                };
                if argument.access != terminal_psi::StructuralAccess::Owned
                    || !claim_bearing_fields.contains(&initializer.field)
                {
                    continue;
                }
                let field = shape_fields
                    .iter()
                    .find(|field| field.id == initializer.field)
                    .ok_or(LoweringError::Unsupported("record field identity missing"))?;
                for claim in self.claims_held_by(argument)? {
                    let mut path = vec![terminal_psi::StructuralPathSegment::Field(
                        field.identity.clone(),
                    )];
                    path.extend(claim.path);
                    claims.push(terminal_psi::StructuralResultClaimBinding {
                        claim: claim.claim,
                        path,
                    });
                }
            }
            claims.sort();
        }
        let place = place_id(allocate_dense(self.next_place)?);
        let declaration = emit_completed(
            place,
            self.structural_type,
            self.multiplicity,
            initialized.into_iter().map(|(_, field)| field).collect(),
            &shape_fields,
            self.operations,
        )?;
        if !claims.is_empty() {
            let produced = self
                .operations
                .operations
                .iter_mut()
                .rev()
                .find_map(|operation| match &mut operation.result {
                    OperationResult::Structural(result) if result.place == place => Some(result),
                    _ => None,
                })
                .ok_or(LoweringError::Unsupported("record producer is missing"))?;
            produced.claims = claims;
        }
        self.temporary_places.push(declaration);
        Ok(place)
    }

    /// The claims a whole owned operand holds beneath its argument path,
    /// relative to that path. A produced value publishes them on its
    /// operation result, completed binding or owned-move arrival; an entry
    /// parameter's claims are the identities the checker transfers out of it
    /// at this statement.
    fn claims_held_by(
        &self,
        argument: &terminal_psi::StructuralArgument,
    ) -> Result<Vec<terminal_psi::StructuralResultClaimBinding>, LoweringError> {
        let rebase = |claims: &[terminal_psi::StructuralResultClaimBinding]| {
            claims
                .iter()
                .filter_map(|claim| {
                    claim
                        .path
                        .strip_prefix(argument.path.as_slice())
                        .map(|suffix| terminal_psi::StructuralResultClaimBinding {
                            claim: claim.claim,
                            path: suffix.to_vec(),
                        })
                })
                .collect::<Vec<_>>()
        };
        if let Some((_, binding)) = self
            .operations
            .structural_values
            .iter()
            .find(|(_, binding)| binding.place() == argument.place)
        {
            return Ok(rebase(binding.claims()));
        }
        if let Some(result) = self
            .operations
            .operations
            .iter()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == argument.place)
        {
            return Ok(rebase(&result.claims));
        }
        if let Some((_, claims)) = self
            .arrival_claims
            .iter()
            .find(|(place, _)| *place == argument.place)
        {
            return Ok(rebase(claims));
        }
        let Some((position, parameter)) = self
            .evaluation
            .structural_parameters
            .iter()
            .find(|(_, parameter)| parameter.place == argument.place)
        else {
            return unsupported("record operand has no published claim custody");
        };
        let (_, authored) = crate::declarations::state_declaration(self.checked, self.state)?;
        let source = self
            .checked
            .state_parameters(authored)
            .get(*position as usize)
            .ok_or(LoweringError::Unsupported(
                "record operand parameter has no authored source",
            ))?;
        if parameter.multiplicity == StructuralMultiplicity::Unrestricted {
            return Ok(Vec::new());
        }
        let mut claims = Vec::new();
        for (_, event) in self.checked.facts.flow.ownership.permissions.iter() {
            if event.machine_symbol != self.machine
                || event.state_symbol != self.state
                || event.root != facts::PlaceRoot::Symbol(source.symbol)
                || event.kind != language_semantics::PermissionEventKind::Transfer
                || event.access != language_semantics::PermissionAccess::Owned
                || event.multiplicity != language_semantics::Multiplicity::Linear
                || !event.obligation_live
                || event.source
                    != (language_semantics::PermissionEventSource::Statement {
                        statement_index: self.statement as usize,
                    })
            {
                continue;
            }
            let path = validation::structural_claim_path(
                &self.checked.typed,
                source.type_reference,
                self.checked
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments),
            )
            .map_err(LoweringError::Unsupported)?;
            claims.push(terminal_psi::StructuralResultClaimBinding {
                claim: crate::terminal_identities::lookup_claim_id(
                    self.calls.claims.bindings(),
                    event.claim_identity,
                )?,
                path: crate::expression_preparation::bindings::structural_paths::lower_structural_path(
                    &path,
                )?,
            });
        }
        Ok(rebase(&claims))
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
    declarations: &[terminal_psi::StructuralFieldDeclaration],
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let mut projected_qualifications = Vec::new();
    for field in &fields {
        let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
            continue;
        };
        let Some(source) = operations
            .iter()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == argument.place)
        else {
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
        let (whole, projected) = terminal_psi::StructuralPathQualification::select(
            &source.qualifications,
            &source.projected_qualifications,
            &argument.path,
        );
        terminal_psi::StructuralPathQualification::append_under(
            &whole,
            &projected,
            &[terminal_psi::StructuralPathSegment::Field(identity.clone())],
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
