use crate::OptimizationUnitValidationError;
use crate::unit_validation::structural_catalog::content_projection::validate_structural_content_projection;
use crate::unit_validation::structural_catalog::type_declarations::{
    validate_structural_cases, validate_structural_fields, validate_structural_type_graph,
};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};

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
            // A reference carrier holds loan permission, not referent
            // storage; its exact contract is checked once `types` is complete.
            terminal_psi::StructuralTypeShape::Reference { .. } => {}
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
            terminal_psi::StructuralTypeShape::FixedArray { .. } => {}
            terminal_psi::StructuralTypeShape::ElementView { .. } => {}
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
            terminal_psi::StructuralTypeShape::Reference { referent, .. } => vec![*referent],
            terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
            | terminal_psi::StructuralTypeShape::ByteSequence(_) => Vec::new(),
            terminal_psi::StructuralTypeShape::Record { fields } => fields
                .iter()
                .filter_map(|field| match field.field_type {
                    terminal_psi::StructuralFieldType::Structural(target) => Some(target),
                    _ => None,
                })
                .collect(),
            terminal_psi::StructuralTypeShape::FixedArray { element, .. }
            | terminal_psi::StructuralTypeShape::ElementView { element } => vec![*element],
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
        // The supported carrier is one exclusive permission over one primitive
        // scalar referent; write-only carriers share the mutable loan shape.
        if let terminal_psi::StructuralTypeShape::Reference {
            referent, access, ..
        } = &declaration.shape
            && (!matches!(
                access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) || !matches!(
                types.get(referent).map(|referent| &referent.shape),
                Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(_))
            ))
        {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralTypeIdentity(declaration.id),
            );
        }
    }
    validate_structural_type_graph(&types)?;
    // Only record and case-payload carriers currently transfer stored
    // reference custody. Reject other containers recursively, including a
    // carrier hidden in them.
    for declaration in &unit.structural_types {
        if matches!(
            declaration.shape,
            terminal_psi::StructuralTypeShape::Record { .. }
                | terminal_psi::StructuralTypeShape::Sum { .. }
                | terminal_psi::StructuralTypeShape::Mixed { .. }
        ) {
            continue;
        }
        let children: Vec<StructuralTypeId> = match &declaration.shape {
            terminal_psi::StructuralTypeShape::Sum { cases } => cases
                .iter()
                .flat_map(|case| &case.fields)
                .filter_map(|field| {
                    if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
                        Some(child)
                    } else {
                        None
                    }
                })
                .collect(),
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .filter_map(|field| {
                    if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
                        Some(child)
                    } else {
                        None
                    }
                })
                .collect(),
            terminal_psi::StructuralTypeShape::FixedArray { element, .. }
            | terminal_psi::StructuralTypeShape::ElementView { element } => vec![*element],
            terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
            | terminal_psi::StructuralTypeShape::ByteSequence(_)
            | terminal_psi::StructuralTypeShape::Reference { .. }
            | terminal_psi::StructuralTypeShape::Record { .. } => Vec::new(),
        };
        if children
            .iter()
            .any(|child| crate::unit_validation::references::contains_reference(&types, *child))
        {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralTypeIdentity(declaration.id),
            );
        }
    }
    // Empty primitive arrays retain their complete type below the zero extent.
    // Resolve references and cycles first; zero bytes cannot hide invalid types.
    for declaration in &unit.structural_types {
        if matches!(
            declaration.shape,
            terminal_psi::StructuralTypeShape::FixedArray { length: 0, .. }
        ) && terminal_semantics::scalar_array_leaf_shape(types.values().copied(), declaration.id)
            .is_none()
        {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralArrayLength(declaration.id),
            );
        }
    }
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
