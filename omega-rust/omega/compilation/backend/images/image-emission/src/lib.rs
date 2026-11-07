#![forbid(unsafe_code)]

//! Standalone object and executable-image emission for the clean terminal-Psi
//! realization lane.
//!
//! This crate consumes emitted fragments. It does not reconstruct the legacy
//! `EncodedMachineCode` carrier or any source-shaped lowering state. Typed internal-call relocations may change only their exact
//! architecture-native immediate fields; every other compiler-authored bit and
//! every provenance-bearing function region remains final-byte validated.
//!
//! The crate has two lanes:
//!
//! - **The object artifact** lives in [`object_artifact`](object::object_artifact): the sealed
//!   [`ObjectArtifact`] carrier, built only by [`function_fragments`](object::function_fragments), which
//!   projects each admitted fragment once rather than replaying a second
//!   machine-code record model against final bytes. Executable-image output
//!   over a sealed object starts at
//!   [`image_output`](image::image_output)'s `emit_executable_image`, which selects the direct
//!   writers it owns or the dynamic ELF lane in [`dynamic_elf`](image::dynamic_elf), with
//!   [`final_image_validation`](image::final_image_validation) replaying either for retained-artifact
//!   readers; [`function_fragments`](object::function_fragments) places every addressed byte literal's
//!   octets in one read-only pool after the
//!   code, binding each literal address field to it. Ordinary callback bodies
//!   follow that same projection. [`private_object_composition`](object::private_object_composition) embeds their
//!   complete objects while keeping machine, call and replay identities local;
//!   helpers, imports and data are not discarded to manufacture a single thunk.
//! - **The canonical installation record wire format** lives in
//!   [`installation_record`]: manifest metadata over the resulting sealed
//!   image, its bytes under `installation_record/codec/` with one codec per
//!   record family and its canonical shape under `record_shape/`. It does not grant
//!   executable authority or replace the separate native admission, placement,
//!   and retirement ladder. [`installed_artifact`] joins that record to an
//!   installed code occurrence when the `installed-artifact` feature is on.

mod codec_bytes;
mod entries;
mod image;
mod object;
pub use entries::callable_entry_transport::{
    CallableEntryTransport, create_callable_entry_transport,
    create_installed_private_function_object, create_private_function_object,
};
pub use entries::foreign_adapter::{
    NativeForeignAdapterEmissionError, NativeForeignAdapterImage, emit_native_foreign_adapter,
};
pub use entries::physical_entry::{
    InstalledPhysicalCpuRequirement, PhysicalEntryBinding, PhysicalEntryPartitions,
    PhysicalEntryRequirements, PhysicalEntryStackRequirement, PhysicalIncomingStackRequirement,
    bind_physical_entry,
};
pub use object::component_object::{
    ComponentFunctionImage, component_export_symbol, prepare_component_function_image,
};
pub use object::component_shared_image::{
    ComponentSharedContainer, ComponentSharedImage, ComponentSharedImageEmissionError,
    emit_component_shared_image,
};
mod installation_record;
mod order;

pub use object::function_fragments::FunctionFragmentObjectArtifactError;
pub use object::object_artifact::{
    ObjectPrivateFunctionEntry, PrivateFunctionObject, PrivateObjectSymbolBinding,
};
pub use object::private_object_composition::compose_private_function_objects;

pub use object::function_fragments::replay::verify_retained_target_operations;

pub use object::object_artifact::replay::boundary::byte_sequence_custody::linux_write_line_custody_is_exact;
pub use object::object_artifact::replay::boundary::completion_receipts::completion_receipts_have_exact_custody;
pub(crate) use object::object_artifact::stack_demand::StackDemand;
pub use object::object_artifact::stack_demand::derive_stack_demand;
pub use object::object_artifact::{
    ObjectArtifact, ObjectBoundarySettlement, ObjectCodeAttribution, ObjectForeignCall,
    ObjectFunction, ObjectScalarCallStack, ObjectUnitCallStack,
    derive_normalized_foreign_call_custody, durable_home_offsets,
    foreign_invocation_provenance_matches, foreign_result_operand_count, fragment_instruction_span,
    producer_instruction, scalar_call_result_producer, scalar_immediate_span,
    structural_result_capture, transported_span_end, unique_block_instruction,
};
pub(crate) use object::object_artifact::{
    ObjectByteLiteralAddress, ObjectByteLiteralPool, ObjectCompilerPrivateFunction, ObjectError,
    ObjectPortEffect, ObjectScalarStack, ObjectUnitStack,
};

pub use object::function_fragments::{
    FunctionFragmentObjectArtifactRequest, build_function_fragment_object_artifact,
};

pub(crate) use entries::hosted_receiver::HostedReceiverBinding;
pub use entries::hosted_receiver::bind_hosted_receiver;
pub use image::dynamic_elf::{RequestedDynamicElfImage, validate_requested_dynamic_elf_image};
pub use image::final_image_validation::native_code_layouts::{
    LinkedNativeCodeLayout, LinkedNativeCodeRegion, inspect_native_code_layouts,
};
pub use image::image_output::{
    ExecutableImage, ExecutableImageEmissionRequest, ImageApplicationMetadataError,
    ObjectContainer, RequestedExecutableImage, can_emit_executable_image, emit_executable_image,
    validate_direct_executable_image, validate_direct_executable_image_output,
};

pub use installation_record::InstallationError;
pub use machine_code::BoundaryExecutionRecord;

pub use object::object_artifact::installed_stack::{
    compose_installed_arrival_demands, installed_interrupt_stack_requirement,
    require_installed_root_provision,
};
