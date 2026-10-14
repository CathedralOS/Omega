//! Independent catalog formation retains the complete element graph at every
//! fixed-array extent. Empty storage changes no type identity or reference
//! custody rule; unknown, cyclic and unsupported stored-reference shells reject.

use crate::OptimizationUnitValidationError;
use crate::unit_validation::structural_catalog::content_projection::validate_structural_content_projection;
use crate::unit_validation::structural_catalog::type_declarations::{
    attachment_storage_types, validate_structural_cases, validate_structural_fields,
    validate_structural_type_graph,
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
    terminal_semantics::quotients::validate_quotient_representations(
        &unit.structural_types,
        unit.structural_types.quotients(),
    )
    .map_err(OptimizationUnitValidationError::InvalidQuotientCatalog)?;
    terminal_semantics::quotients::validate_quotient_representation_identities(
        &unit.structural_types,
        unit.structural_types.quotients(),
        &unit.identities,
    )
    .map_err(OptimizationUnitValidationError::InvalidQuotientCatalog)?;
    let mut types = BTreeMap::new();
    let mut type_names = BTreeSet::new();
    for declaration in &unit.structural_types {
        if types.insert(declaration.id, declaration).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateStructuralType(
                declaration.id,
            ));
        }
        if declaration.identity == terminal_psi::TerminalIdentityIndex::ABSENT
            || !type_names.insert(declaration.identity)
        {
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
    let attachment_storage = attachment_storage_types(
        &types,
        unit.functions
            .iter()
            .filter_map(|function| function.attachment),
    );
    let mut referent_ids = BTreeSet::new();
    for referent in unit.structural_types.referents() {
        if types.contains_key(&referent.id) || !referent_ids.insert(referent.id) {
            return Err(OptimizationUnitValidationError::DuplicateStructuralType(
                referent.id,
            ));
        }
        if referent.identity == terminal_psi::TerminalIdentityIndex::ABSENT
            || !type_names.insert(referent.identity)
        {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralTypeIdentity(referent.id),
            );
        }
    }
    if unit
        .structural_types
        .referents()
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(OptimizationUnitValidationError::NonCanonicalStructuralTypeOrder);
    }
    for declaration in &unit.structural_types {
        // Attachment names a declaration, not a storage-establishment operation.
        // Incoming receivers retain the same field invariants as named inputs;
        // actual zero construction is checked by zeroed_establishment_matches.
        match &declaration.shape {
            // A reference carrier holds loan permission, not referent
            // storage; its exact contract is checked once `types` is complete.
            terminal_psi::StructuralTypeShape::Reference { .. } => {}
            // The marker declares no children; admission does not manufacture
            // its affine service-binding occurrence.
            terminal_psi::StructuralTypeShape::BindingCarrier => {}
            terminal_psi::StructuralTypeShape::PrimitiveScalar(_) => {}
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                | terminal_psi::ByteSequenceCarrier::BoundedView { .. },
            ) => {}
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
            ) => {}
            terminal_psi::StructuralTypeShape::FixedArray { .. } => {}
            terminal_psi::StructuralTypeShape::ElementView { .. } => {}
            terminal_psi::StructuralTypeShape::Record { fields, .. } => {
                validate_structural_fields(
                    declaration.id,
                    None,
                    fields,
                    attachment_storage.contains(&declaration.id),
                )?;
            }
            terminal_psi::StructuralTypeShape::Sum { cases } => {
                validate_structural_cases(declaration.id, cases)?;
            }
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                validate_structural_fields(declaration.id, None, fields, false)?;
                validate_structural_cases(declaration.id, cases)?;
            }
        }
    }
    for declaration in &unit.structural_types {
        let referenced = match &declaration.shape {
            terminal_psi::StructuralTypeShape::Reference { .. } => Vec::new(),
            terminal_psi::StructuralTypeShape::BindingCarrier
            | terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
            | terminal_psi::StructuralTypeShape::ByteSequence(_) => Vec::new(),
            terminal_psi::StructuralTypeShape::Record { fields, .. } => fields
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
        // A borrow carrier is one borrow permission over one referent: a
        // primitive scalar leaf, a whole plain aggregate, a view, or a byte
        // sequence, whose address transports the storage like the scalar
        // leaf. A service binding is also a referent: lending a binding field
        // (`&mut main.console`) to a state declares the carrier type, while
        // checking keeps every routed binding out of reference locals, so no
        // carrier holds its address. Only a nested reference is excluded — a
        // `ref` over a reference would carry a second custody tier, matching
        // the terminal module's storable-referent rule.
        if let terminal_psi::StructuralTypeShape::Reference {
            referent, access, ..
        } = &declaration.shape
        {
            let admissible = match access {
                terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::SharedBorrow => {
                    referent_ids.contains(referent)
                        || types.get(referent).is_some_and(|referent| {
                            !matches!(
                                referent.shape,
                                terminal_psi::StructuralTypeShape::Reference { .. }
                            )
                        })
                }
                terminal_psi::StructuralAccess::WriteOnlyBorrow => matches!(
                    types.get(referent).map(|referent| &referent.shape),
                    Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(_))
                ),
                _ => false,
            };
            if !admissible {
                return Err(
                    OptimizationUnitValidationError::InvalidStructuralTypeIdentity(declaration.id),
                );
            }
        }
    }
    validate_structural_type_graph(&types)?;
    // Owned constructors relocate complete declaration-derived leaf rosters,
    // including every element of a fixed array. A view instead carries a
    // runtime pointer and extent: its type cannot supply the concrete element
    // permission roster required by the receiving custody contract.
    for declaration in &unit.structural_types {
        if let terminal_psi::StructuralTypeShape::ElementView { element } = declaration.shape
            && crate::unit_validation::references::contains_reference(&types, element)
        {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralTypeIdentity(declaration.id),
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
    // Only primitive byte families may span carriers. Nominal/content domains
    // retain their unique semantic identity; capacity does not rename byte facts.
    let mut family_byte_predicates = BTreeMap::new();
    for declaration in unit.structural_domains.iter() {
        if domains.insert(declaration.id, declaration).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateStructuralDomain(
                declaration.id,
            ));
        }
        if declaration.identity == terminal_psi::TerminalIdentityIndex::ABSENT
            || !names.insert(declaration.identity)
            || !semantic_domains.insert((declaration.semantic_domain, declaration.carrier))
            || family_byte_predicates
                .insert(declaration.semantic_domain, declaration.byte_predicate)
                .is_some_and(|previous| {
                    previous.is_none() || previous != declaration.byte_predicate
                })
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
        if declaration.byte_predicate.is_some()
            && (declaration.content_projection.is_some()
                || !declaration.establishment_routes.is_empty()
                || !matches!(
                    types[&declaration.carrier].shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(_)
                ))
        {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralDomainIdentity(declaration.id),
            );
        }
        if declaration
            .content_projection
            .as_ref()
            .is_some_and(|projection| {
                !validate_structural_content_projection(
                    declaration.semantic_domain,
                    declaration.carrier,
                    projection,
                    &unit.identities,
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
