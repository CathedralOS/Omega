//! Optimizer module role: executable entrance. Relocation-free object-container stage entrance.
//!
//! This file owns construction and independent replay. Contracts live in
//! `model`, object assembly in `reconstruction`, and canonical manifest
//! bytes in `codec`.

use omega_object_file::{
    ObjectLocalSymbolId, RelocationFreeFunctionSymbol, RelocationFreeObjectContainer,
    RelocationFreeObjectDecodeError, RelocationFreeObjectError, RelocationFreeObjectPlan,
    RelocationFreeObjectRelocationRequirements, RelocationFreeObjectSymbolLinkage,
    RelocationFreeObjectSymbolPolicy, RelocationFreeObjectSymbolRole,
    RelocationFreeObjectTextSection, SectionKind, canonical_private_machine_symbol_name,
    decode_relocation_free_object, encode_relocation_free_object, section_name,
    validate_relocation_free_object,
};
use omega_optimization_core::{
    FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    OptimizationSelectionIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use omega_selected_instructions::SelectedInstructionPlanIdentity;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

const MANIFEST_MAGIC: &[u8; 8] = b"OMGTOM\0\0";
const MANIFEST_VERSION: u32 = 1;

mod codec;
mod model;
mod reconstruction;
mod source;

#[cfg(test)]
mod tests;

pub use model::*;
pub use source::*;

use reconstruction::*;

pub fn stage_optimized_relocation_free_object_container(
    source: impl Into<StagedOptimizedObjectTextSectionSource>,
) -> Result<StagedOptimizedRelocationFreeObjectContainer, RelocationFreeObjectContainerError> {
    let source = source.into();
    source.validate()?;
    let object = construct_object(&source)?;
    let container = encode_relocation_free_object(&object)
        .map_err(RelocationFreeObjectContainerError::InvalidObject)?;
    let manifest = construct_manifest(&source, &object, &container)?;
    let custody = receipt(&manifest, &object, &container);
    let staged = StagedOptimizedRelocationFreeObjectContainer {
        source,
        object,
        container,
        manifest,
        custody,
    };
    validate_optimized_relocation_free_object_container(&staged)?;
    Ok(staged)
}

pub fn validate_optimized_relocation_free_object_container(
    staged: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<StagedRelocationFreeObjectContainerCustodyReceipt, RelocationFreeObjectContainerError> {
    staged.source.validate()?;
    let expected_object = replay_object(&staged.source)?;
    validate_relocation_free_object(&staged.object)
        .map_err(RelocationFreeObjectContainerError::InvalidObject)?;
    if staged.object != expected_object {
        return Err(RelocationFreeObjectContainerError::ArtifactMismatch);
    }
    if staged.container.object != staged.object.identity
        || staged.container.identity
            != RelocationFreeObjectContainerIdentity::from_canonical_bytes(&staged.container.bytes)
    {
        return Err(RelocationFreeObjectContainerError::ContainerMismatch);
    }
    let decoded = decode_relocation_free_object(&staged.container.bytes)
        .map_err(RelocationFreeObjectContainerError::InvalidContainer)?;
    let canonical = encode_relocation_free_object(&expected_object)
        .map_err(RelocationFreeObjectContainerError::InvalidObject)?;
    if decoded != expected_object || staged.container != canonical {
        return Err(RelocationFreeObjectContainerError::ContainerMismatch);
    }
    let expected_manifest = construct_manifest(&staged.source, &expected_object, &canonical)?;
    if staged.manifest != expected_manifest {
        return Err(RelocationFreeObjectContainerError::ManifestMismatch);
    }
    let expected_receipt = receipt(&expected_manifest, &expected_object, &canonical);
    if staged.custody != expected_receipt {
        return Err(RelocationFreeObjectContainerError::ReceiptMismatch);
    }
    Ok(expected_receipt)
}
