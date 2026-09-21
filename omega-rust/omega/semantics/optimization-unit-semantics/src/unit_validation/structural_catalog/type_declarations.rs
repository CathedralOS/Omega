use crate::BTreeMap;
use crate::BTreeSet;
use crate::OptimizationUnitValidationError;
use crate::PsiOptimizationUnit;
use crate::StructuralTypeId;

pub(crate) fn validate_structural_fields(
    unit: &PsiOptimizationUnit,
    structural_type: StructuralTypeId,
    case: Option<semantic_vocabulary::StructuralCaseId>,
    fields: &[terminal_psi::StructuralFieldDeclaration],
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
    // An unused provider field retains its declaration and attachment, but
    // contributes no provider root. Function validation separately rejoins
    // each used field's specialization roots to its exact boundary calls.
    let has_provider_attachment = permit_provider_attachment
        && unit
            .functions
            .iter()
            .any(|function| function.attachment == Some(structural_type));
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
            (terminal_psi::StructuralFieldType::Erased { type_identity }, _)
                if type_identity.is_empty() =>
            {
                return Err(invalid_erased());
            }
            (
                terminal_psi::StructuralFieldType::Erased { .. },
                terminal_psi::BindingRelevance::Erased,
            ) => {}
            (
                terminal_psi::StructuralFieldType::Erased { .. },
                terminal_psi::BindingRelevance::Relevant,
            ) if has_provider_attachment => {}
            (
                terminal_psi::StructuralFieldType::Erased { .. },
                terminal_psi::BindingRelevance::Relevant,
            ) => return Err(invalid_erased()),
            (
                terminal_psi::StructuralFieldType::Scalar(_)
                | terminal_psi::StructuralFieldType::BoundedInteger(_)
                | terminal_psi::StructuralFieldType::IeeeFloat(_)
                | terminal_psi::StructuralFieldType::Structural(_),
                terminal_psi::BindingRelevance::Erased,
            ) => return Err(invalid_erased()),
            (
                terminal_psi::StructuralFieldType::Scalar(_)
                | terminal_psi::StructuralFieldType::BoundedInteger(_)
                | terminal_psi::StructuralFieldType::IeeeFloat(_)
                | terminal_psi::StructuralFieldType::ByteSequence(_)
                | terminal_psi::StructuralFieldType::Structural(_),
                terminal_psi::BindingRelevance::Relevant,
            )
            | (
                terminal_psi::StructuralFieldType::ByteSequence(_),
                terminal_psi::BindingRelevance::Erased,
            ) => {}
        }
    }
    Ok(())
}

pub(crate) fn validate_structural_cases(
    unit: &PsiOptimizationUnit,
    structural_type: StructuralTypeId,
    cases: &[terminal_psi::StructuralCaseDeclaration],
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

pub(crate) fn validate_structural_type_graph(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    fn visit(
        id: StructuralTypeId,
        types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
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
            terminal_psi::StructuralTypeShape::Reference { referent, .. } => {
                visit(*referent, types, active, complete)?;
            }
            terminal_psi::StructuralTypeShape::PrimitiveScalar(_) => {}
            terminal_psi::StructuralTypeShape::ByteSequence(_) => {}
            terminal_psi::StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let terminal_psi::StructuralFieldType::Structural(target) = field.field_type
                    {
                        visit(target, types, active, complete)?;
                    }
                }
            }
            terminal_psi::StructuralTypeShape::FixedArray { element, .. }
            | terminal_psi::StructuralTypeShape::ElementView { element } => {
                visit(*element, types, active, complete)?;
            }
            terminal_psi::StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let terminal_psi::StructuralFieldType::Structural(target) = field.field_type
                    {
                        visit(target, types, active, complete)?;
                    }
                }
            }
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    if let terminal_psi::StructuralFieldType::Structural(target) = field.field_type
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
