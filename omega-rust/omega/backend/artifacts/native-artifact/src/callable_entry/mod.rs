//! Optimizer module role: executable entrance. Optimized ordinary-callable-entry stage entrance.
//!
//! This file owns the build-and-replay join. The data contract lives in
//! `model`, semantic reconstruction in `reconstruction`, and canonical
//! wire encoding in `codec`.

use calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, ValueLocation, ValueShape, evaluate_call_plan,
};
use object_file::{
    ObjectLocalSymbolId, RelocationFreeObjectSymbolLinkage, RelocationFreeObjectSymbolRole,
};
use optimization_core::{
    OptimizationSelectionIdentity, OptimizedObjectArtifactIdentity,
    OptimizedObjectArtifactManifestIdentity, OptimizedOrdinaryCallableEntryManifestIdentity,
    OptimizedTerminalOrdinaryCallableEntryIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity,
};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
};
use selected_instructions::{
    SelectedInstructionId, SelectedInstructionPlanIdentity, SelectedTerminator, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use selected_instructions_to_register_homes::{RegisterHomeIdentity, register_home_identity};
use semantic_vocabulary::{
    EdgeId, IntegerCarrier, IntegerSign, IntegerType, MachineId, ScalarType, ValueId,
};
use target::{Architecture, NativeTarget, ObjectFormat};
use terminal_psi::{TerminalMachineResult, TerminalPsiIdentity, Terminator, ValueDeclaration};

use machine_code::{
    WholeFunctionEntryAssumption, WholeFunctionExitContractIdentity, WholeFunctionExitPolicy,
    WholeFunctionHardeningPolicy,
};
use object_file::{
    OptimizedObjectArtifactError, StagedValidatedOptimizedObjectArtifact,
    validate_optimized_object_artifact,
};

const RECORD_MAGIC: &[u8; 8] = b"OMGOER\0\0";
const MANIFEST_MAGIC: &[u8; 8] = b"OMGOEM\0\0";
const VERSION: u32 = 4;

mod codec;
mod model;
mod reconstruction;

pub use model::*;

use reconstruction::{manifest, receipt, reconstruct};

pub fn stage_validated_optimized_ordinary_callable_entry(
    source: StagedValidatedOptimizedObjectArtifact,
) -> Result<StagedValidatedOptimizedOrdinaryCallableEntry, OptimizedOrdinaryCallableEntryError> {
    validate_optimized_object_artifact(&source)
        .map_err(OptimizedOrdinaryCallableEntryError::Source)?;
    let entry = reconstruct(&source)?;
    let manifest = manifest(&entry)?;
    let custody = receipt(&entry, &manifest);
    Ok(StagedValidatedOptimizedOrdinaryCallableEntry {
        source,
        entry,
        manifest: ValidatedOptimizedOrdinaryCallableEntryManifest { record: manifest },
        custody,
    })
}

pub fn validate_optimized_ordinary_callable_entry(
    staged: &StagedValidatedOptimizedOrdinaryCallableEntry,
) -> Result<OptimizedOrdinaryCallableEntryCustodyReceipt, OptimizedOrdinaryCallableEntryError> {
    validate_optimized_object_artifact(&staged.source)
        .map_err(OptimizedOrdinaryCallableEntryError::Source)?;
    let entry = reconstruct(&staged.source)?;
    if entry != staged.entry {
        return Err(OptimizedOrdinaryCallableEntryError::RecordMismatch);
    }
    let manifest = manifest(&entry)?;
    if manifest != staged.manifest.record {
        return Err(OptimizedOrdinaryCallableEntryError::ManifestMismatch);
    }
    let receipt = receipt(&entry, &manifest);
    if receipt != staged.custody {
        return Err(OptimizedOrdinaryCallableEntryError::ReceiptMismatch);
    }
    Ok(receipt)
}
