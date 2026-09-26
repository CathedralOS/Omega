//! Canonical object/container/manifest/custody fixtures shared by mutation leaves.
//!
//! The owning stage replays the wrapper join with
//! `construct_object` → `encode_*` → `construct_manifest` → `custody`, then
//! compares each retained artifact against the recomputation. These fixtures
//! build the same artifact set so each leaf can substitute one field and rerun
//! the exact replay joins (`validate_object`, container decode/encode,
//! `validate_manifest`, `custody`) the staged validator performs.

use super::super::super::{
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectPlan,
};
use super::super::custody;
use super::super::{child, composed, template};
use crate::semantic_wrapper_object::compose_optimized_program_storage_semantic_wrapper_object;
use crate::semantic_wrapper_object::encode_optimized_program_storage_semantic_wrapper_object;
use crate::semantic_wrapper_object::manifest::construct_manifest;
use object_file::{
    ObjectLocalSymbolId, RelocationFreeFunctionSymbol, RelocationFreeObjectPlan,
    RelocationFreeObjectSymbolLinkage, RelocationFreeObjectSymbolRole,
    canonical_private_machine_symbol_name,
};
use optimization_core::{
    OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifestIdentity,
    RelocationFreeObjectContainerIdentity,
};
use semantic_vocabulary::MachineId;

/// Replays `compose_object` over the same upstream joins `composed()` binds,
/// exactly as the staged validator's `construct_object` does when it derives
/// the expected object for comparison.
pub(super) fn recompose(
    child: &RelocationFreeObjectPlan,
) -> OptimizedProgramStorageSemanticWrapperObjectPlan {
    compose_optimized_program_storage_semantic_wrapper_object(
        [5; 32],
        OptimizedObjectArtifactIdentity::from_canonical_bytes(b"artifact"),
        OptimizedObjectArtifactManifestIdentity::from_canonical_bytes(b"manifest"),
        RelocationFreeObjectContainerIdentity::from_canonical_bytes(b"container"),
        child,
        &template(),
    )
    .unwrap()
}

/// The complete retained artifact set for the canonical two-symbol wrapper
/// object: plan, canonical container, manifest, and custody receipt.
pub(super) fn staged_parts() -> (
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
) {
    parts(composed())
}

/// The same retained artifact set for a child that carries one private
/// terminal function symbol beside the semantic entry, so the
/// `PrivateTerminalFunctionV1` symbol row and roster mutations have a real row
/// to substitute.
pub(super) fn three_symbol_parts() -> (
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
) {
    parts(recompose(&child_with_terminal_function()))
}

fn parts(
    object: OptimizedProgramStorageSemanticWrapperObjectPlan,
) -> (
    OptimizedProgramStorageSemanticWrapperObjectPlan,
    OptimizedProgramStorageSemanticWrapperObjectContainer,
    OptimizedProgramStorageSemanticWrapperObjectManifest,
    OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
) {
    let container =
        encode_optimized_program_storage_semantic_wrapper_object(&object, &template()).unwrap();
    let manifest = construct_manifest(&object, &container).unwrap();
    let receipt = custody(&object, &container, &manifest);
    (object, container, manifest, receipt)
}

/// A `RelocationFreeObjectPlan` like `child()` plus one private terminal
/// function symbol (machine 8) occupying the byte after the semantic entry.
fn child_with_terminal_function() -> RelocationFreeObjectPlan {
    let mut child = child();
    let machine = MachineId::new(8).unwrap();
    child.text_section.bytes.push(0x90);
    child.text_section.byte_count = 2;
    child.symbols.push(RelocationFreeFunctionSymbol {
        symbol: ObjectLocalSymbolId::new(2).unwrap(),
        source_function_index: 1,
        machine,
        name: canonical_private_machine_symbol_name(machine),
        section_offset: 1,
        byte_count: 1,
        linkage: RelocationFreeObjectSymbolLinkage::ObjectLocalV1,
        role: RelocationFreeObjectSymbolRole::PrivateFunctionV1,
    });
    child.identity = child.recomputed_identity().unwrap();
    child
}
