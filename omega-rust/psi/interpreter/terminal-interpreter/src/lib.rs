//! Fuel-bounded reference execution of admitted Terminal Psi artifacts.
//!
//! Start at `terminal_interpreter.rs`: it owns the raw start entry — obtain
//! canonical program storage, construct the execution, bind its inputs — for a
//! caller that drives the loop itself. External bytes decode once; retained
//! artifacts are borrowed.
//! `execution.rs` is the engine that entry drives: the live state and the
//! interpreter loop. Execution retains immutable code and resumable live
//! custody; fuel exhaustion is not a semantic program outcome.
//! `runtime_rows` owns live-cell reservations for frame values/descriptors and
//! shared scalar record fields. Constructor and subtree staging pay for their
//! overlap, then transfer reservations with the rows. `result_custody` charges
//! completed publications from their retained storage graph, not declarations or
//! unrelated frame storage. These owners alone do not establish complete Build
//! accounting: remaining Build host operations and scheduler integration remain.
//!
//! The embedding surface sits beside it, by lifecycle: `program` (one owned or
//! borrowed immutable artifact — `TerminalProgram` — reusable across
//! independent instances), `instance` (`TerminalInstance`: one guest
//! activation whose demanded import slots bind checked host adapters before
//! `invoke` and release on `close`), and `adapters` (the typed contract a
//! host implementation declares so a slot can be checked before any effect
//! runs). Embedding a program goes through `TerminalProgram::load` or `borrow`,
//! `instantiate`, `bind_import`, `invoke`, `close`; the raw
//! `TerminalExecution`/`TerminalEffectHandler` surface remains the low-level
//! control the embedding layer is built over, not a binding API.
//! Shared `AdmittedProviderSelections` records only exact checked providers
//! and assumed links. `admit_runtime_supplies` combines those with this host's
//! import choices into `TerminalRuntimeSupplies` before instance creation;
//! native compilation never constructs that interpreter dispatch state.
//!
//! The domains beside them, by responsibility: `structural_inputs` (the
//! host-supplied entry inputs and their binding), `custody` (claims, the
//! affine frontier and argument binding), `structural_identities` (execution-wide
//! local identity allocation and host reservations), `reference`,
//! `primitive_storage`, `record`, `scalar_array` and `byte_sequences` (the storage the loop
//! mutates), `scalar_operations`, `structural_operations`,
//! `atomic_operations`, `calls` and `terminators` (what the loop dispatches),
//! `effects` (what it hands to the host and how host results bind), `values`,
//! `results` and `errors` (what crosses the boundary).

mod activation_storage;
mod adapters;
mod atomic_operations;
mod byte_sequences;
mod calls;
mod capabilities;
mod content_transformations;
mod custody;
mod effects;
mod memory_ordering;
pub use memory_ordering::TerminalMemoryOrderingEnvironment;
mod element_views;
mod errors;
mod execution;
mod instance;
mod primitive_storage;
mod program;
mod provider_storage;
mod record;
mod reference;
mod result_custody;
mod results;
mod runtime_indexes;
mod runtime_rows;
mod scalar_array;
mod scalar_operations;
mod scalar_storage;
mod storage_addresses;
mod structural_identities;
mod structural_inputs;
mod structural_operations;
mod structural_storage;
mod terminal_interpreter;
mod terminators;
mod values;

pub use adapters::{
    TerminalAdapterError, TerminalImportAdapter, TerminalImportSignature,
    TerminalImportStructuralParameter, TerminalScalarImport, TerminalUnitImport,
};
pub use byte_sequences::read::TerminalByteRead;
pub use capabilities::{
    TerminalExecutionRequirements, TerminalExternalCompletionDemand, TerminalUnsupportedCapability,
};
pub use effects::byte_buffers::TerminalBoundaryByteBuffer;
pub use effects::original_storage::{
    TerminalOriginalReferenceValue, TerminalOriginalScalarBackings, TerminalOriginalScalarValue,
    TerminalOriginalStorageLoan, TerminalOriginalStorageRequest, TerminalOriginalStorageRoot,
};
pub use effects::results::{TerminalByteSequenceValue, TerminalEffectResult};
pub use effects::scalar_loans::TerminalBoundaryScalarBuffer;
pub use effects::storage_inputs::{
    TerminalBoundaryReferenceSelection, TerminalBoundaryStorageBuffer,
    TerminalBoundaryStorageValue, TerminalBoundaryStructuralBuffer, TerminalStorageContents,
};
pub use effects::{
    AdmittedAssumedBinding, AdmittedTargetLayouts, AdmittedTargetMachineSelections,
    AssumedBindingSelection, ProviderInstallationSelection, TerminalEffect, TerminalEffectHandler,
    TerminalEffectRejection, TerminalRuntimeSupplies, TerminalRuntimeSupplyRequest,
    TerminalX86FlagsEnvironment, admit_target_layouts, admit_target_machine_selections,
};
pub use errors::{
    AssumedBindingRejection, ProviderInstallationError, TerminalArtifactInterpretError,
    TerminalInterpretError,
};
pub use execution::TerminalExecution;
pub use instance::{
    TerminalBindingRejection, TerminalComponentBinding, TerminalEntryStorageError,
    TerminalEntryStorageValue, TerminalInstance, TerminalInstanceCloseError, TerminalInstanceError,
    TerminalProcessExitAdmission,
};
pub use program::{TerminalInstantiation, TerminalProgram, TerminalProgramEntry};
pub use provider_storage::{
    TerminalProviderStorageInput, TerminalProviderStorageRejection, TerminalReturnedProviderStorage,
};
pub use results::{
    TerminalCrash, TerminalCrashSite, TerminalEffectWait, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalExternalCompletion, TerminalScalarCaseResult,
    TerminalStructuralResult,
};
pub use scalar_array::{TerminalScalarArrayResult, TerminalScalarArrayValue};
pub use scalar_operations::float_result_policy;
pub use scalar_storage::TerminalScalarBacking;
pub use structural_inputs::byte_arrays::TerminalStructuralByteArrayValue;
pub use structural_inputs::case_membership::TerminalStructuralCaseValue;
pub use structural_inputs::placed_views::TerminalPlacedViewEstablishment;
pub use structural_inputs::scalar_fields::TerminalStructuralScalarFieldValue;
pub use structural_inputs::{TerminalEstablishedErasedField, TerminalStructuralInputs};
pub use terminal_installation::{
    AdmittedProviderSelections, admit_provider_installation,
    admit_provider_installation_from_artifact,
    admit_provider_installation_with_assumed_bindings_from_artifact,
};
pub use values::{
    TerminalScalarAggregateValue, TerminalScalarValue, TerminalStructuralBooleanFieldValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
