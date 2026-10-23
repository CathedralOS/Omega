//! Complete fields in authored order before committing a declaration-ordered case.
use super::super::super::StructuralFieldType;
use super::super::{
    CheckedScalarExpressionRole, LoweringError, OperationKind, PlaceId, StructuralMultiplicity,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId, StructuralTypeShape,
    allocate_dense, place_id, unsupported,
};
use super::emission;
use crate::emission::operation_emission::buffer::{OperationBuffer, structural_result_declaration};

impl emission::Emission<'_, '_, '_> {
    pub(super) fn structural_case(
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
        let checked_trees::CheckedStructuralValueKind::StructuralCase {
            data_symbol,
            case: case_symbol,
            fields,
        } = node.kind
        else {
            return unsupported("structural case producer missing");
        };
        let fields = self.structural_value_fields(fields, "case fields are stale")?;
        let owner = self
            .checked
            .data_definition_by_symbol(data_symbol)
            .ok_or(LoweringError::Unsupported("case owner declaration missing"))?;
        let shape =
            terminal_psi::find_structural_type(&self.structural_types[..], self.structural_type)
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
            let identity = checked_trees::CheckedMemberIdentity::of_member(
                variant.identity,
                variant.name.clone(),
            );
            if variant.symbol == case_symbol {
                let case = terminal_psi::structural_case_by_identity(
                    &cases,
                    identities.intern(&identity.spelling()),
                )
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
        let mut claims = Vec::new();
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
            let identity = checked_trees::CheckedMemberIdentity::of_member(
                declaration.identity,
                declaration.name.clone(),
            );
            let position = shape_fields[roster.clone()]
                .iter()
                .position(|candidate| candidate.identity == identities.intern(&identity.spelling()))
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
                        identities,
                        CheckedScalarExpressionRole::StructuralValueField {
                            expression: node.expression,
                            field_ordinal: u32::try_from(ordinal).map_err(|_| {
                                LoweringError::Unsupported("case field ordinal overflow")
                            })?,
                        },
                        value,
                        source_count,
                    )?;
                    self.scalar_field_value(
                        identities,
                        target,
                        evaluated,
                        &mut scalar_positions,
                        initialized.len(),
                    )?
                }
                checked_trees::CheckedStructuralRecordFieldValue::Structural(value) => {
                    if let Some(scalar_type) = target.field_type.scalar_type() {
                        self.zeroed_scalar_field(
                            identities,
                            value,
                            scalar_type,
                            target,
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
                        let mut prefix = Vec::new();
                        if roster.start == common.len() {
                            prefix.push(terminal_psi::StructuralPathSegment::Case(shape_case.id));
                        }
                        prefix.push(terminal_psi::StructuralPathSegment::Field(
                            target.identity.clone(),
                        ));
                        let completed = self.child_constructor_operand(
                            identities,
                            child_type,
                            value,
                            field.type_reference,
                            &prefix,
                        )?;
                        if self.multiplicity == StructuralMultiplicity::Affine {
                            for claim in completed.claims {
                                let mut path = prefix.clone();
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
        let place = place_id(allocate_dense(self.environment.next_place)?);
        self.environment.private_places.push(emit_completed(
            place,
            self.structural_type,
            self.multiplicity,
            shape_case.id,
            initialized.into_iter().map(|(_, field)| field).collect(),
            &shape_fields,
            common.len(),
            self.evaluation
                .structural_parameters
                .iter()
                .map(|(_, parameter)| parameter),
            self.operations,
        )?);
        claims.sort();
        self.operations
            .set_structural_result_claims(place, claims)?;
        Ok(place)
    }
}

/// Commit completed, declaration-ordered payload fields under the selected
/// case tag. Operand evaluation and source custody remain with their shared
/// scalar/structural semantic owners.
pub(crate) fn emit_completed<'parameter>(
    place: PlaceId,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    result_case: semantic_vocabulary::StructuralCaseId,
    fields: Vec<terminal_psi::RecordFieldInitializer>,
    declarations: &[terminal_psi::StructuralFieldDeclaration],
    common_fields: usize,
    parameters: impl Iterator<Item = &'parameter terminal_psi::StructuralParameterDeclaration> + Clone,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let mut projected_qualifications = Vec::new();
    for (position, field) in fields.iter().enumerate() {
        let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
            continue;
        };
        // IDs may coincide between common and payload namespaces. The
        // declaration-ordered roster, not an ID search, selects the member.
        let declaration = declarations
            .get(position)
            .filter(|declaration| declaration.id == field.field)
            .ok_or(LoweringError::Unsupported(
                "completed case field has no declaration",
            ))?;
        // Inline byte fields carry their predicates in the field contract;
        // projected qualifications describe nominal structural children.
        if !matches!(declaration.field_type, StructuralFieldType::Structural(_)) {
            continue;
        }
        let mut prefix = Vec::new();
        if position >= common_fields {
            prefix.push(terminal_psi::StructuralPathSegment::Case(result_case));
        }
        prefix.push(terminal_psi::StructuralPathSegment::Field(
            declaration.identity.clone(),
        ));
        super::member_memberships::append(
            argument,
            &prefix,
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
    let operation = operations.push_leaf(
        result,
        OperationKind::EstablishStructuralCase {
            result_case,
            fields,
        },
    );
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation,
            structural_type,
        },
    })
}
