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
/// custody is transferred by complete record and fixed-array construction.
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
    }
    let attachment_storage = attachment_storage_types(
        &types,
        module
            .machines
            .iter()
            .filter_map(|machine| machine.attachment),
    );
    for declaration in &module.structural_types {
        let fields: Vec<_> = match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.iter().collect(),
            StructuralTypeShape::Sum { cases } => {
                cases.iter().flat_map(|case| &case.fields).collect()
            }
            StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .collect(),
            _ => Vec::new(),
        };
        for field in fields {
            if let StructuralFieldType::PredicateByteSequence {
                carrier,
                predicates,
            } = field.field_type
                && (field.relevance.is_erased()
                    || predicates.is_empty()
                    || !carrier.supports_field_predicates())
            {
                return Err(ModuleError::InvalidStructuralFieldIdentity {
                    structural_type: declaration.id,
                    field: field.id,
                });
            }
        }
        // Attachment is nominal context metadata, not a storage establishment.
        // Incoming borrows may carry domains that reject zero. Actual zeroing
        // is checked at EstablishZeroedStructural and entry provisioning; doing
        // it here would reject the same established value only for methods.
        // A bounded sequence may be a completed owned concatenation result.
        // The establishing operation and every reader still validate custody;
        // admitting its type does not turn it into a borrowed view.
        if let StructuralTypeShape::Record { fields } = &declaration.shape {
            validate_structural_fields(
                declaration.id,
                fields,
                attachment_storage.contains(&declaration.id),
            )?;
        } else if let StructuralTypeShape::Sum { cases } = &declaration.shape {
            validate_structural_cases(declaration.id, cases)?;
        } else if let StructuralTypeShape::Mixed { fields, cases } = &declaration.shape {
            validate_structural_fields(declaration.id, fields, false)?;
            validate_structural_cases(declaration.id, cases)?;
        }
    }
    for declaration in &module.structural_types {
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::FusedServiceBinding => {}
            StructuralTypeShape::Reference { referent, access } => {
                if !types.contains_key(referent) {
                    return Err(ModuleError::UnknownStructuralType(*referent));
                }
                // A borrow names the borrowed storage directly: scalars,
                // records, sums, views and byte sequences are all storable
                // referents. Only a nested reference is not — a `ref` over
                // a reference would carry a second custody tier the
                // reference operations do not plan.
                if !(matches!(
                    *access,
                    StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
                ) && !matches!(types[referent].shape, StructuralTypeShape::Reference { .. }))
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
    // An empty extent has no element accesses, but still names a complete
    // element type. The same known-type and acyclic-graph checks apply at
    // every length; emptiness does not authorize a malformed element shell.
    validate_structural_type_graph(&types)?;
    // Complete record and fixed-array constructors relocate exact live reference
    // leaves. Other containers still lack that receiving contract: reject them
    // recursively, including a supported owned aggregate hidden beneath them.
    for declaration in &module.structural_types {
        if matches!(
            declaration.shape,
            StructuralTypeShape::Record { .. } | StructuralTypeShape::FixedArray { .. }
        ) {
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
            | StructuralTypeShape::FusedServiceBinding
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

/// Receiver storage retains nested provider declarations even when no method
/// on the nested record is demanded. This closure admits declarations only:
/// provider roots and calls must still join the calling machine's exact,
/// direct attachment. References and views do not contain receiver storage.
fn attachment_storage_types(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    attachments: impl Iterator<Item = StructuralTypeId>,
) -> BTreeSet<StructuralTypeId> {
    use terminal_psi::{StructuralFieldType, StructuralTypeShape};
    let mut pending: Vec<_> = attachments.collect();
    let mut storage = BTreeSet::new();
    while let Some(identity) = pending.pop() {
        if !storage.insert(identity) {
            continue;
        }
        let Some(declaration) = types.get(&identity) else {
            // Ordinary graph and attachment validation reject unknown types.
            continue;
        };
        let mut retain_fields = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
            pending.extend(fields.iter().filter_map(|field| match field.field_type {
                StructuralFieldType::Structural(child) => Some(child),
                _ => None,
            }));
        };
        match &declaration.shape {
            StructuralTypeShape::Record { fields } => retain_fields(fields),
            StructuralTypeShape::Mixed { fields, cases } => {
                retain_fields(fields);
                for case in cases {
                    retain_fields(&case.fields);
                }
            }
            StructuralTypeShape::Sum { cases } => {
                for case in cases {
                    retain_fields(&case.fields);
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
            StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::FusedServiceBinding
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ElementView { .. } => {}
        }
    }
    storage
}
