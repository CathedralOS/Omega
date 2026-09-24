//! Optimizer module role: executable entrance. Structural type, domain, function-local, and provider catalogs.
//!
//! Type indexing precedes domain indexing at this entrance. Projection,
//! declaration, graph, function-local, witness, provider-specialization, and
//! path mechanics descend into named leaves.
use crate::OptimizationUnitValidationError;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use std::collections::BTreeMap;

mod catalog;
mod content_projection;
mod function_catalog;
mod paths;
mod provider_specialization;
mod type_declarations;
mod witnesses;

pub(crate) use function_catalog::{structural_root_key, validate_function_structural_catalog};
pub(crate) use paths::{resolve_structural_path, structural_qualifications_match};
pub(crate) use provider_specialization::validate_provider_attachment_specialization;
pub(crate) use witnesses::{
    validate_byte_sequence_literal_witnesses, validate_trivial_affine_local_witnesses,
};

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
