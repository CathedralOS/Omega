//! The module's structural domains and their content projections.

use super::super::{
    BTreeMap, BTreeSet, ModuleError, StructuralDomainId, StructuralTypeId, TerminalModule,
};
use super::validate_structural_content_projection;
use terminal_psi::{StructuralDomainDeclaration, StructuralTypeDeclaration};

/// Registers the module's structural domains: ids, names and semantic
/// domains are unique, each carrier is a known type, and each content
/// projection is well formed over its carrier.
pub(super) fn register_structural_domains<'m>(
    module: &'m TerminalModule,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<BTreeMap<StructuralDomainId, &'m StructuralDomainDeclaration>, ModuleError> {
    let mut domains = BTreeMap::new();
    let mut domain_names = BTreeSet::new();
    let mut semantic_domains = BTreeSet::new();
    for declaration in &module.structural_domains {
        if domains.insert(declaration.id, declaration).is_some() {
            return Err(ModuleError::DuplicateStructuralDomain(declaration.id));
        }
        if declaration.identity.is_empty()
            || !domain_names.insert(declaration.identity.as_str())
            || !semantic_domains.insert(declaration.semantic_domain)
        {
            return Err(ModuleError::InvalidStructuralDomainIdentity(declaration.id));
        }
        if !types.contains_key(&declaration.carrier) {
            return Err(ModuleError::UnknownStructuralType(declaration.carrier));
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
