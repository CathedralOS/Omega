//! The module's structural domains and their content projections.

use super::super::{
    BTreeMap, BTreeSet, ModuleError, StructuralDomainId, StructuralTypeId, TerminalModule,
};
use super::validate_structural_content_projection;
use terminal_psi::{StructuralDomainDeclaration, StructuralTypeDeclaration};

/// Registers the module's structural domains: ids, names and semantic
/// family/carrier pairs are unique, each carrier is a known type, and each content
/// projection is well formed over its carrier.
pub(super) fn register_structural_domains<'m>(
    module: &'m TerminalModule,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<BTreeMap<StructuralDomainId, &'m StructuralDomainDeclaration>, ModuleError> {
    let mut domains = BTreeMap::new();
    let mut domain_names = BTreeSet::new();
    let mut semantic_domains = BTreeSet::new();
    // Only primitive byte families may span carriers. Nominal/content domains
    // retain their unique semantic identity; capacity does not rename byte facts.
    let mut family_byte_predicates = BTreeMap::new();
    for declaration in &module.structural_domains {
        if domains.insert(declaration.id, declaration).is_some() {
            return Err(ModuleError::DuplicateStructuralDomain(declaration.id));
        }
        if declaration.identity.is_empty()
            || !domain_names.insert(declaration.identity.as_str())
            || !semantic_domains.insert((declaration.semantic_domain, declaration.carrier))
            || family_byte_predicates
                .insert(declaration.semantic_domain, declaration.byte_predicate)
                .is_some_and(|previous| {
                    previous.is_none() || previous != declaration.byte_predicate
                })
        {
            return Err(ModuleError::InvalidStructuralDomainIdentity(declaration.id));
        }
        if !types.contains_key(&declaration.carrier) {
            return Err(ModuleError::UnknownStructuralType(declaration.carrier));
        }
        if declaration.byte_predicate.is_some()
            && (declaration.content_projection.is_some()
                || !declaration.establishment_routes.is_empty()
                || !matches!(
                    types[&declaration.carrier].shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(_)
                ))
        {
            return Err(ModuleError::InvalidStructuralDomainIdentity(declaration.id));
        }
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
            return Err(ModuleError::InvalidStructuralDomainContentProjection(
                declaration.id,
            ));
        }
        if declaration
            .establishment_routes
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ModuleError::NonCanonicalStructuralEstablishmentRoutes(
                declaration.id,
            ));
        }
    }
    Ok(domains)
}
