//! Optimizer module role: executable entrance. Structural type, domain, function-local, and provider catalogs.
//!
//! Type indexing precedes domain indexing at this entrance. Projection,
//! declaration, graph, function-local, witness, provider-specialization, and
//! path mechanics descend into named leaves.
//! `paths` types standalone endpoints; `storage_projection` retains regions
//! such as borrowed byte windows without minting a standalone owner type.
use crate::OptimizationUnitValidationError;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use std::collections::BTreeMap;

mod catalog;
mod content_projection;
mod function_catalog;
mod paths;
mod provider_specialization;
mod storage_projection;
mod type_declarations;
mod witnesses;

pub(crate) use paths::structural_projection_type;
pub use storage_projection::{
    StructuralStorageProjection, structural_borrowed_storage_projection,
    structural_storage_projection,
};

pub(crate) use function_catalog::{structural_root_key, validate_function_structural_catalog};
pub(crate) use paths::{
    byte_carrier_leaf_window, resolve_leaf_copy_path, resolve_runtime_projection,
    resolve_structural_path, structural_projected_qualifications_match,
    structural_qualifications_match,
};
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
