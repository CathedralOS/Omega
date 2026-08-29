use super::*;

pub(crate) fn validate_structural_fields(
    unit: &PsiOptimizationUnit,
    structural_type: StructuralTypeId,
    case: Option<psi_core::StructuralCaseId>,
    fields: &[psi_terminal::StructuralFieldDeclaration],
    permit_provider_attachment: bool,
) -> Result<(), OptimizationUnitValidationError> {
    if fields.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return Err(
            OptimizationUnitValidationError::NonCanonicalStructuralFieldOrder {
                structural_type,
                case,
            },
        );
    }
    let mut identities = BTreeSet::new();
    for field in fields {
        if field.identity.is_empty() || !identities.insert(field.identity.as_str()) {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralFieldIdentity {
                    structural_type,
                    field: field.id,
                },
            );
        }
        let invalid_erased = || OptimizationUnitValidationError::InvalidErasedStructuralField {
            structural_type,
            field: field.id,
        };
        match (&field.field_type, field.relevance) {
            (psi_terminal::StructuralFieldType::Erased { type_identity }, _)
                if type_identity.is_empty() =>
            {
                return Err(invalid_erased());
            }
            (
                psi_terminal::StructuralFieldType::Erased { .. },
                psi_terminal::BindingRelevance::Erased,
            ) => {}
            (
                psi_terminal::StructuralFieldType::Erased { .. },
                psi_terminal::BindingRelevance::Relevant,
            ) if permit_provider_attachment
                && has_provider_attachment_witness(unit, structural_type, field.id) => {}
            (
                psi_terminal::StructuralFieldType::Erased { .. },
                psi_terminal::BindingRelevance::Relevant,
            ) => return Err(invalid_erased()),
            (
                psi_terminal::StructuralFieldType::Scalar(_)
                | psi_terminal::StructuralFieldType::IeeeFloat(_)
                | psi_terminal::StructuralFieldType::Structural(_),
                psi_terminal::BindingRelevance::Erased,
            ) => return Err(invalid_erased()),
            (
                psi_terminal::StructuralFieldType::Scalar(_)
                | psi_terminal::StructuralFieldType::IeeeFloat(_)
                | psi_terminal::StructuralFieldType::ByteSequence(_)
                | psi_terminal::StructuralFieldType::Structural(_),
                psi_terminal::BindingRelevance::Relevant,
            )
            | (
                psi_terminal::StructuralFieldType::ByteSequence(_),
                psi_terminal::BindingRelevance::Erased,
            ) => {}
        }
    }
    Ok(())
}

pub(crate) fn validate_structural_cases(
    unit: &PsiOptimizationUnit,
    structural_type: StructuralTypeId,
    cases: &[psi_terminal::StructuralCaseDeclaration],
) -> Result<(), OptimizationUnitValidationError> {
    if cases.is_empty() {
        return Err(OptimizationUnitValidationError::EmptyStructuralSum(
            structural_type,
        ));
    }
    if cases.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return Err(
            OptimizationUnitValidationError::NonCanonicalStructuralCaseOrder(structural_type),
        );
    }
    let mut identities = BTreeSet::new();
    for case in cases {
        if case.identity.is_empty() || !identities.insert(case.identity.as_str()) {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralCaseIdentity {
                    structural_type,
                    case: case.id,
                },
            );
        }
    }
    for case in cases {
        validate_structural_fields(unit, structural_type, Some(case.id), &case.fields, false)?;
    }
    Ok(())
}

pub(crate) fn has_provider_attachment_witness(
    unit: &PsiOptimizationUnit,
    structural_type: StructuralTypeId,
    field: psi_core::StructuralFieldId,
) -> bool {
    unit.functions.iter().any(|function| {
        function.attachment == Some(structural_type)
            && function.structural_places.iter().any(|place| {
                matches!(
                    place.kind,
                    StructuralPlaceKind::ProviderAttachment {
                        attachment,
                        field: provider_field,
                        ..
                    } if attachment == structural_type && provider_field == field
                )
            })
    })
}

pub(crate) fn validate_structural_type_graph(
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    fn visit(
        id: StructuralTypeId,
        types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
        active: &mut BTreeSet<StructuralTypeId>,
        complete: &mut BTreeSet<StructuralTypeId>,
    ) -> Result<(), OptimizationUnitValidationError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return Err(OptimizationUnitValidationError::RecursiveStructuralType(id));
        }
        let declaration = types[&id];
        match &declaration.shape {
            psi_terminal::StructuralTypeShape::ByteSequence(_) => {}
            psi_terminal::StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let psi_terminal::StructuralFieldType::Structural(target) = field.field_type
                    {
                        visit(target, types, active, complete)?;
                    }
                }
            }
            psi_terminal::StructuralTypeShape::FixedArray { element, .. } => {
                visit(*element, types, active, complete)?;
            }
            psi_terminal::StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let psi_terminal::StructuralFieldType::Structural(target) = field.field_type
                    {
                        visit(target, types, active, complete)?;
                    }
                }
            }
            psi_terminal::StructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    if let psi_terminal::StructuralFieldType::Structural(target) = field.field_type
                    {
                        visit(target, types, active, complete)?;
                    }
                }
            }
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for id in types.keys().copied() {
        visit(id, types, &mut active, &mut complete)?;
    }
    Ok(())
}
