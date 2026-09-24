//! The module's structural type declarations and the shapes they may take.

use super::super::{
    BTreeMap, BTreeSet, ModuleError, StructuralAccess, StructuralFieldType, StructuralTypeId,
    StructuralTypeShape, TerminalModule,
};
use super::{
    validate_structural_cases, validate_structural_fields, validate_structural_type_graph,
};
use terminal_psi::StructuralTypeDeclaration;

/// Registers the module's structural types: ids and names are unique, each
/// shape's fields, elements and cases name known types with the layout the
/// shape allows, the type graph is well founded, and stored reference
/// custody is transferred only by record construction.
pub(super) fn register_structural_types(
    module: &TerminalModule,
) -> Result<BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>, ModuleError> {
    let mut types = BTreeMap::new();
    let mut type_names = BTreeSet::new();
    for declaration in &module.structural_types {
        if types.insert(declaration.id, declaration).is_some() {
            return Err(ModuleError::DuplicateStructuralType(declaration.id));
        }
        if declaration.identity.is_empty() || !type_names.insert(declaration.identity.as_str()) {
            return Err(ModuleError::InvalidStructuralTypeIdentity(declaration.id));
        }
        if matches!(
            declaration.shape,
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
            )
        ) {
            return Err(ModuleError::InvalidStructuralTypeIdentity(declaration.id));
        } else if let StructuralTypeShape::Record { fields } = &declaration.shape {
            validate_structural_fields(module, declaration.id, fields, true)?;
        } else if let StructuralTypeShape::Sum { cases } = &declaration.shape {
            validate_structural_cases(module, declaration.id, cases)?;
        } else if let StructuralTypeShape::Mixed { fields, cases } = &declaration.shape {
            validate_structural_fields(module, declaration.id, fields, false)?;
            validate_structural_cases(module, declaration.id, cases)?;
        } else if matches!(
            declaration.shape,
            StructuralTypeShape::FixedArray { length: 0, .. }
        ) && terminal_semantics::scalar_array_leaf_shape(
            module.structural_types.iter(),
            declaration.id,
        )
        .is_none()
        {
            return Err(ModuleError::InvalidStructuralArrayLength(declaration.id));
        }
    }
    for declaration in &module.structural_types {
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {}
            StructuralTypeShape::Reference { referent, access } => {
                if !types.contains_key(referent) {
                    return Err(ModuleError::UnknownStructuralType(*referent));
                }
                let named_referent = matches!(
                    types[referent].shape,
                    StructuralTypeShape::Record { .. }
                        | StructuralTypeShape::Sum { .. }
                        | StructuralTypeShape::Mixed { .. }
                );
                if !(*access == StructuralAccess::MutableBorrow
                    && matches!(
                        types[referent].shape,
                        StructuralTypeShape::PrimitiveScalar(_)
                    ))
                    && !(*access == StructuralAccess::SharedBorrow && named_referent)
                {
                    return Err(ModuleError::InvalidStructuralTypeIdentity(declaration.id));
                }
            }
            StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let StructuralFieldType::Structural(target) = &field.field_type
                        && !types.contains_key(target)
                    {
                        return Err(ModuleError::UnknownStructuralType(*target));
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, .. }
            | StructuralTypeShape::ElementView { element } => {
                if !types.contains_key(element) {
                    return Err(ModuleError::UnknownStructuralType(*element));
                }
            }
            StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let StructuralFieldType::Structural(target) = &field.field_type
                        && !types.contains_key(target)
                    {
                        return Err(ModuleError::UnknownStructuralType(*target));
                    }
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    if let StructuralFieldType::Structural(target) = &field.field_type
                        && !types.contains_key(target)
                    {
                        return Err(ModuleError::UnknownStructuralType(*target));
                    }
                }
            }
        }
    }
    validate_structural_type_graph(&types)?;
    // Only record construction currently transfers stored reference custody.
    // Reject other containers recursively, including a record hidden in them.
    for declaration in &module.structural_types {
        if matches!(declaration.shape, StructuralTypeShape::Record { .. }) {
            continue;
        }
        let children: Vec<StructuralTypeId> = match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields
                .iter()
                .filter_map(|field| {
                    if let StructuralFieldType::Structural(child) = field.field_type {
                        Some(child)
                    } else {
                        None
                    }
                })
                .collect(),
            StructuralTypeShape::Sum { cases } => cases
                .iter()
                .flat_map(|case| &case.fields)
                .filter_map(|field| {
                    if let StructuralFieldType::Structural(child) = field.field_type {
                        Some(child)
                    } else {
                        None
                    }
                })
                .collect(),
            StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .filter_map(|field| {
                    if let StructuralFieldType::Structural(child) = field.field_type {
                        Some(child)
                    } else {
                        None
                    }
                })
                .collect(),
            StructuralTypeShape::FixedArray { element, .. }
            | StructuralTypeShape::ElementView { element } => vec![*element],
            StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::Reference { .. } => Vec::new(),
        };
        if children
            .iter()
            .any(|child| super::super::references::contains_reference(module, *child))
        {
            return Err(ModuleError::InvalidStructuralTypeIdentity(declaration.id));
        }
    }
    Ok(types)
}
