use super::*;

pub(super) fn index_structural_types(
    unit: &PsiOptimizationUnit,
) -> Result<
    BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    OptimizationUnitValidationError,
> {
    let mut types = BTreeMap::new();
    let mut type_names = BTreeSet::new();
    for declaration in &unit.structural_types {
        if types.insert(declaration.id, declaration).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateStructuralType(
                declaration.id,
            ));
        }
        if declaration.identity.is_empty() || !type_names.insert(declaration.identity.as_str()) {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralTypeIdentity(declaration.id),
            );
        }
    }
    if unit
        .structural_types
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(OptimizationUnitValidationError::NonCanonicalStructuralTypeOrder);
    }
    for declaration in &unit.structural_types {
        match &declaration.shape {
            terminal_psi::StructuralTypeShape::PrimitiveScalar(_) => {}
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            ) => {}
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
            ) => {
                return Err(
                    OptimizationUnitValidationError::InvalidStructuralTypeIdentity(declaration.id),
                );
            }
            terminal_psi::StructuralTypeShape::FixedArray { length: 0, .. } => {
                return Err(
                    OptimizationUnitValidationError::InvalidStructuralArrayLength(declaration.id),
                );
            }
            terminal_psi::StructuralTypeShape::FixedArray { .. } => {}
            terminal_psi::StructuralTypeShape::Record { fields } => {
                validate_structural_fields(unit, declaration.id, None, fields, true)?;
            }
            terminal_psi::StructuralTypeShape::Sum { cases } => {
                validate_structural_cases(unit, declaration.id, cases)?;
            }
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                validate_structural_fields(unit, declaration.id, None, fields, false)?;
                validate_structural_cases(unit, declaration.id, cases)?;
            }
        }
    }
    for declaration in &unit.structural_types {
        let referenced = match &declaration.shape {
            terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
            | terminal_psi::StructuralTypeShape::ByteSequence(_) => Vec::new(),
            terminal_psi::StructuralTypeShape::Record { fields } => fields
                .iter()
                .filter_map(|field| match field.field_type {
                    terminal_psi::StructuralFieldType::Structural(target) => Some(target),
                    _ => None,
                })
                .collect(),
            terminal_psi::StructuralTypeShape::FixedArray { element, .. } => vec![*element],
            terminal_psi::StructuralTypeShape::Sum { cases } => cases
                .iter()
                .flat_map(|case| &case.fields)
                .filter_map(|field| match field.field_type {
                    terminal_psi::StructuralFieldType::Structural(target) => Some(target),
                    _ => None,
                })
                .collect(),
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .filter_map(|field| match field.field_type {
                    terminal_psi::StructuralFieldType::Structural(target) => Some(target),
                    _ => None,
                })
                .collect(),
        };
        if let Some(target) = referenced.iter().find(|target| !types.contains_key(target)) {
            return Err(OptimizationUnitValidationError::UnknownStructuralType(
                *target,
            ));
        }
    }
    validate_structural_type_graph(&types)?;
    Ok(types)
}

pub(super) fn index_structural_domains<'unit>(
    unit: &'unit PsiOptimizationUnit,
    types: &BTreeMap<StructuralTypeId, &'unit terminal_psi::StructuralTypeDeclaration>,
) -> Result<
    BTreeMap<StructuralDomainId, &'unit terminal_psi::StructuralDomainDeclaration>,
    OptimizationUnitValidationError,
> {
    let mut domains = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut semantic_domains = BTreeSet::new();
    for declaration in unit.structural_domains.iter() {
        if domains.insert(declaration.id, declaration).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateStructuralDomain(
                declaration.id,
            ));
        }
        if declaration.identity.is_empty()
            || !names.insert(declaration.identity.as_str())
            || !semantic_domains.insert(declaration.semantic_domain)
        {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralDomainIdentity(declaration.id),
            );
        }
    }
    if unit
        .structural_domains
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(OptimizationUnitValidationError::NonCanonicalStructuralDomainOrder);
    }
    if let Some(carrier) = unit
        .structural_domains
        .iter()
        .map(|declaration| declaration.carrier)
        .find(|carrier| !types.contains_key(carrier))
    {
        return Err(OptimizationUnitValidationError::UnknownStructuralType(
            carrier,
        ));
    }
    for declaration in unit.structural_domains.iter() {
        if declaration
            .content_projection
            .as_ref()
            .is_some_and(|projection| {
                !validate_structural_content_projection(
                    declaration.semantic_domain,
                    declaration.carrier,
                    projection,
                    types,
                )
            })
        {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralDomainContentProjection(
                    declaration.id,
                ),
            );
        }
    }
    Ok(domains)
}
