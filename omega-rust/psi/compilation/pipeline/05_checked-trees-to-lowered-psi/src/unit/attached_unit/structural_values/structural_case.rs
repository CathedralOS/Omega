//! Complete fields in authored order before committing a declaration-ordered case.
use super::super::super::StructuralFieldType;
use super::super::{
    CheckedScalarExpressionRole, LoweringError, PlaceId, StructuralTypeShape, allocate_dense,
    place_id, unsupported,
};
use super::completed_aggregate::{self, CompletedAggregate};
use super::emission;
use crate::unit::attached_unit::structural_values::ValueOutcome;

impl emission::Emission<'_, '_, '_> {
    pub(super) fn structural_case(
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
            let ValueOutcome::Normal(value) = self.completed_member_value(
                identities,
                field,
                target,
                CheckedScalarExpressionRole::StructuralValueField {
                    expression: node.expression,
                    field_ordinal: u32::try_from(ordinal)
                        .map_err(|_| LoweringError::Unsupported("case field ordinal overflow"))?,
                },
                (roster.start == common.len()).then_some(shape_case.id),
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
        let place = place_id(allocate_dense(self.environment.next_place)?);
        let declaration = completed_aggregate::emit_completed(
            place,
            self.structural_type,
            self.multiplicity,
            CompletedAggregate::Case {
                result_case: shape_case.id,
                fields: initialized.into_iter().map(|(_, field)| field).collect(),
                declarations: &shape_fields,
                common_fields: common.len(),
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
        self.environment.private_places.push(declaration);
        claims.sort();
        self.operations
            .set_structural_result_claims(place, claims)?;
        Ok(ValueOutcome::Normal(place))
    }
}
