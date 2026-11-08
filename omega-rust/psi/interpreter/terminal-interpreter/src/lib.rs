//! Fuel-bounded reference execution of admitted Terminal Psi artifacts.
//!
//! Start at `terminal_interpreter.rs`: it owns the raw start entry — obtain
//! canonical program storage, construct the execution, bind its inputs — for a
//! caller that drives the loop itself. External bytes decode once; retained
//! artifacts are borrowed. The folders are the four roles around that entry:
//! `embedding` (the surface a host builds on), `engine` (the loop and what it
//! dispatches), `storage` (what the loop mutates) and `boundary` (what crosses
//! between an execution and its host).
//!
//! `engine::execution` is the engine that entry drives: the live state and the
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
//! `instantiate`, `bind_import`, `invoke`, `close`. Entries returning checked
//! loans additionally require `retain_returned_loans` before execution;
//! `returned_loan` then borrows actual completed storage until the instance
//! closes. Its read-only observations grant no new guest value or authority. The raw
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

mod boundary;
mod embedding;
mod engine;
mod storage;
mod terminal_interpreter;

pub use boundary::entry_results::TerminalEntryResultRequest;
pub use boundary::memory_ordering::TerminalMemoryOrderingEnvironment;

pub use boundary::effects::byte_buffers::TerminalBoundaryByteBuffer;
pub use boundary::effects::original_storage::{
    TerminalOriginalReferenceValue, TerminalOriginalScalarBackings, TerminalOriginalScalarValue,
    TerminalOriginalStorageLoan, TerminalOriginalStorageRequest, TerminalOriginalStorageRoot,
};
pub use boundary::effects::results::{
    TerminalByteSequenceValue, TerminalEffectResult, TerminalReferencePayload,
    TerminalReferenceResult,
};
pub use boundary::effects::scalar_loans::TerminalBoundaryScalarBuffer;
pub use boundary::effects::storage_inputs::{
    TerminalBoundaryByteViewBuffer, TerminalBoundaryReferenceSelection,
    TerminalBoundaryStorageBuffer, TerminalBoundaryStructuralBuffer,
};
pub use boundary::effects::{
    AdmittedAssumedBinding, AdmittedTargetLayouts, AdmittedTargetMachineSelections,
    AssumedBindingSelection, ProviderInstallationSelection, TerminalCallStorageRequirements,
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalRuntimeSupplies,
    TerminalRuntimeSupplyRequest, TerminalX86FlagsEnvironment, admit_target_layouts,
    admit_target_machine_selections,
};
pub use boundary::errors::{
    AssumedBindingRejection, ProviderInstallationError, TerminalArtifactInterpretError,
    TerminalInterpretError,
};
pub use boundary::native_code_layouts::{
    TerminalNativeCodeLayout, TerminalNativeCodeLayoutEnvironment, TerminalNativeCodeRegion,
};
pub use boundary::results::{
    TerminalCrash, TerminalCrashSite, TerminalEffectWait, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalExternalCompletion, TerminalScalarCaseResult,
    TerminalStructuralResult,
};
pub use boundary::structural_inputs::access_views::TerminalAccessViewEstablishment;
pub use boundary::structural_inputs::byte_arrays::TerminalStructuralByteArrayValue;
pub use boundary::structural_inputs::contents::TerminalStructuralContents;
pub use boundary::structural_inputs::scalar_fields::TerminalStructuralScalarFieldValue;
pub use boundary::structural_inputs::{TerminalEstablishedErasedField, TerminalStructuralInputs};
pub use boundary::values::{
    TerminalScalarAggregateValue, TerminalScalarValue, TerminalStructuralBooleanFieldValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
pub use embedding::adapters::{
    TerminalAdapterError, TerminalImportAdapter, TerminalImportSignature,
    TerminalImportStructuralParameter, TerminalScalarImport, TerminalUnitImport,
};
pub use embedding::instance::{
    TerminalBindingRejection, TerminalComponentBinding, TerminalInstance,
    TerminalInstanceCloseError, TerminalInstanceError, TerminalProcessExitAdmission,
    TerminalReturnedLoan, TerminalReturnedLoanError, TerminalStorageObservationError,
    TerminalStorageObservationValue,
};
pub use embedding::program::{
    TerminalInstantiation, TerminalProgram, TerminalProgramEntry, TerminalScalarApplicationRequest,
};
pub use engine::capabilities::{
    TerminalExecutionRequirements, TerminalExternalCompletionDemand, TerminalUnsupportedCapability,
};
pub use engine::execution::TerminalExecution;
pub use engine::scalar_operations::float_result_policy;
pub use storage::byte_sequences::read::TerminalByteRead;
pub use storage::provider_storage::{
    TerminalProviderBacking, TerminalProviderStorageInput, TerminalProviderStorageRejection,
    TerminalProviderStorageRequest, TerminalProviderStorageResolver, TerminalProviderStorageSupply,
    TerminalReturnedProviderStorage,
};
pub use storage::scalar_array::{TerminalScalarArrayResult, TerminalScalarArrayValue};
pub use storage::scalar_storage::TerminalScalarBacking;
pub use storage::structural_storage::contents::{
    TerminalBoundaryStorageValue, TerminalStorageContents,
};
pub use terminal_installation::{
    AdmittedProviderSelections, admit_provider_installation,
    admit_provider_installation_from_artifact,
    admit_provider_installation_with_assumed_bindings_from_artifact,
};
