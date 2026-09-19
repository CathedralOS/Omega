#![forbid(unsafe_code)]

//! Standalone object and executable-image emission for the clean terminal-Psi
//! realization lane.
//!
//! This crate consumes only owned terminal machine-code functions. It does not
//! reconstruct the legacy `EncodedMachineCode` carrier or any source-shaped
//! lowering state. Typed internal-call relocations may change only their exact
//! architecture-native immediate fields; every other compiler-authored bit and
//! every provenance-bearing function region remains final-byte validated.
//!
//! The crate has two lanes:
//!
//! - **Object-artifact construction and replay validation** lives in
//!   [`object_artifact`]: the sealed [`ObjectArtifact`] carrier, its builder
//!   entry points, and the evidence replay passes beneath
//!   `object_artifact/replay/` that re-decode every retained record from final
//!   bytes. Executable-image output over a sealed object is dispatched by
//!   [`image_output`] (direct writers), [`dynamic_elf`] (admitted dynamic ELF
//!   custody), and [`final_image_validation`]; [`function_fragments`] projects
//!   admitted current fragments into that same object.
//! - **The canonical installation record wire format** lives in
//!   [`installation_record`]: manifest metadata over the resulting sealed
//!   image, its bytes under `installation_record/codec/` with one codec per
//!   record family and its canonical shape under `record_shape/`. It does not grant
//!   executable authority or replace the separate native admission, placement,
//!   and retirement ladder. [`installed_artifact`] joins that record to an
//!   installed code occurrence when the `installed-artifact` feature is on.

mod dynamic_elf;
mod final_image_validation;
mod function_fragments;
mod hosted_receiver;
mod hosted_unit_entry;
mod image_output;
mod installation_record;
#[cfg(feature = "installed-artifact")]
mod installed_artifact;
mod object_artifact;

pub use object_artifact::replay::scalar::control_flow::reconstruct_scalar_control_flow;
pub use object_artifact::stack_demand::{
    StackDemand, UnitStackDemand, derive_stack_demand, derive_unit_stack_demand,
};
pub use object_artifact::{
    ObjectArtifact, ObjectBoundarySettlement, ObjectCodeAttribution, ObjectCompilerPrivateFunction,
    ObjectDynamicConformanceSlot, ObjectDynamicConformanceTable, ObjectError, ObjectForeignCall,
    ObjectForwardedDynamicDescriptorAdapter, ObjectForwardedDynamicDescriptorSlot,
    ObjectForwardedDynamicDescriptorTable, ObjectFunction, ObjectPortEffect, ObjectScalarCallStack,
    ObjectScalarStack, ObjectUnitCallStack, ObjectUnitStack,
    build_admitted_x86_fma_object_artifact, build_feature_required_x86_fma_object_artifact,
    build_object_artifact, build_object_artifact_with_private_functions,
    derive_normalized_foreign_call_custody,
};

pub use function_fragments::{
    FunctionFragmentObjectArtifactError, build_function_fragment_object_artifact,
    validate_function_fragment_object_artifact,
};

pub use dynamic_elf::{
    DynamicElfImageEmission, DynamicElfImageEmissionError, DynamicElfOrchestrationError,
    ExecutableImageEmissionRequest, RequestedDynamicElfImage, RequestedExecutableImage,
    RequestedExecutableImageError, emit_admitted_dynamic_elf_image, emit_dynamic_elf_image,
    emit_requested_executable_image, validate_dynamic_elf_image_emission,
    validate_requested_dynamic_elf_image, validate_requested_executable_image,
};
pub use hosted_receiver::{HostedReceiverBinding, HostedReceiverPartitions, bind_hosted_receiver};
pub use hosted_unit_entry::LinuxX86ScalarExitShim;
pub use image_output::{
    ExecutableImage, ObjectContainer, ScalarCallReferenceImage, can_emit_executable_image,
    emit_executable_image, emit_object_container, emit_scalar_call_reference_linux_x86_64_image,
    validate_executable_image,
};

pub use installation_record::{
    INSTALLATION_FORMAT_MARKER, ImageFingerprint, InitializedDataFingerprint, InstallationError,
    InstallationFingerprint, InstallationRecord, InstallationStackError,
    InstalledCompilerPrivateFunction, InstalledComponentProgress, InstalledDynamicCall,
    InstalledDynamicConformanceSlot, InstalledDynamicConformanceTable,
    InstalledDynamicParameterCall, InstalledForeignCallStack,
    InstalledForwardedDynamicDescriptorAdapter, InstalledForwardedDynamicDescriptorCall,
    InstalledForwardedDynamicDescriptorSlot, InstalledForwardedDynamicDescriptorTable,
    InstalledForwardedDynamicParameterCall, InstalledFunction, InstalledImageSections,
    InstalledInternalUnitCall, InstalledInternalUnitScalarCall, InstalledStoredDynamicCall,
    InstalledStructuralReturn, SelectedProviderPlanReportIdentity, build_installation_record,
    build_installation_record_with_evidence, build_installation_record_with_provider_executions,
    build_installation_record_with_selected_provider_plans_and_evidence,
    decode_installation_record, derive_installation_stack_demand, encode_installation_record,
    installation_fingerprint, validate_installation_record,
};
#[cfg(feature = "installed-artifact")]
pub use installed_artifact::{
    InstalledArtifact, InstalledArtifactBindingError, InstalledArtifactMemoryImages,
    InstalledArtifactMemoryProjectionError, InstalledCompilerPrivateFunctionEntry,
    InstalledCompilerPrivateFunctionEntryBindingError, bind_installed_artifact,
    bind_installed_compiler_private_function_entry, project_installed_artifact_memory_images,
};

pub use machine_code::BoundaryExecutionRecord;
