/*
    Work-bounded reference interpreter and host embedding surface for admitted
    Terminal Psi (`PSITERM\0`) artifacts.

    We decode external semantic bytes once or borrow retained compiler artifacts
    into an immutable `TerminalProgram`, instantiate isolated `TerminalInstance`
    or `TerminalExecution` state machines, bind checked host import adapters and
    structural inputs, and step through blocks, operations, and terminators
    under explicit work and storage-cell accounting.
*/

/*
    Canonical portable execution requires a target-neutral engine in `psi/` that
    depends only on verified Terminal Psi representations and semantic rules,
    never on Omega backend ABIs or source-tree evaluators. Keeping code
    ownership, affine claim custody, paid runtime-row reservations, and host
    boundary validation in this crate ensures both build-time evaluation and
    differential testing execute the exact portable IR contract.
*/

/*
    Start at `embedding` (`TerminalProgram`, `TerminalInstance`, and
    `TerminalImportAdapter`) for the host embedding lifecycle, or
    `TerminalExecution::start_artifact` below for raw resumable execution.
    `engine_execution`, `engine_calls`, `engine_custody`, `engine_structural`,
    and `engine_terminators` implement the dispatch loop and ownership rules;
    `storage_primitives`, `storage_bytes_and_views`, `storage_providers`,
    `storage_records_and_arrays`, and `storage_references` own runtime storage;
    and `boundary_values`, `boundary_inputs`, `boundary_effects`, and
    `boundary_storage_inputs` govern values and effects crossing the host
    boundary.
*/

mod boundary_effects;
mod boundary_inputs;
mod boundary_storage_inputs;
mod boundary_values;
mod embedding;
mod engine_calls;
mod engine_custody;
mod engine_execution;
mod engine_structural;
mod engine_terminators;
mod storage_bytes_and_views;
mod storage_primitives;
mod storage_providers;
mod storage_records_and_arrays;
mod storage_references;

pub(crate) mod boundary {
    pub(crate) use crate::boundary_effects as effects;
    pub(crate) use crate::boundary_inputs as structural_inputs;
    pub(crate) use crate::boundary_values::{
        entry_results, errors, memory_ordering, native_code_layouts, result_custody, results,
        values,
    };
}

pub(crate) mod engine {
    pub(crate) use crate::engine_calls as calls;
    pub(crate) use crate::engine_custody as custody;
    pub(crate) use crate::engine_execution::{capabilities, execution, scalar_operations};
    pub(crate) use crate::engine_terminators as terminators;
}

pub(crate) mod storage {
    pub(crate) use crate::storage_bytes_and_views::{byte_sequences, element_views};
    pub(crate) use crate::storage_primitives::{
        activation_storage, primitive_storage, runtime_rows, scalar_storage, storage_addresses,
        structural_identities,
    };
    pub(crate) use crate::storage_providers as provider_storage;
    pub(crate) use crate::storage_records_and_arrays::{record, scalar_array, structural_storage};
    pub(crate) use crate::storage_references as reference;
}

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

impl TerminalExecution<'_> {
    pub fn start_artifact(
        semantic_bytes: &[u8],
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
    ) -> Result<Self, TerminalArtifactInterpretError> {
        crate::embedding::program::TerminalProgram::load(semantic_bytes)?.start_execution(
            scalar_arguments,
            structural_inputs,
            None,
        )
    }
}
