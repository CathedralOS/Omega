//! Optimizer module role: executable entrance. Structural type, domain, function-local, and provider catalogs.
//!
//! Type indexing precedes domain indexing at this entrance. Projection,
//! declaration, graph, function-local, witness, provider-specialization, and
//! path mechanics descend into named leaves.

use super::*;

mod catalog;
mod content_projection;
mod function_catalog;
mod paths;
mod provider_specialization;
mod type_declarations;
mod witnesses;

pub(crate) use content_projection::*;
pub(crate) use function_catalog::*;
pub(crate) use paths::*;
pub(crate) use provider_specialization::*;
pub(crate) use type_declarations::*;
pub(crate) use witnesses::*;

pub(crate) fn index_structural_catalogs(
    unit: &PsiOptimizationUnit,
) -> Result<
    (
        BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
        BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
    ),
    OptimizationUnitValidationError,
> {
    let types = catalog::index_structural_types(unit)?;
    let domains = catalog::index_structural_domains(unit, &types)?;
    Ok((types, domains))
}
