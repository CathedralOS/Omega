#![forbid(unsafe_code)]

//! Standalone object and executable-image emission for the clean terminal-Psi
//! realization lane.
//!
//! This crate consumes emitted fragments and owned terminal machine-code functions. It does not
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
//!   bytes. Executable-image output over a sealed object starts at
//!   [`image_output`]'s `emit_executable_image`, which selects the direct
//!   writers it owns or the dynamic ELF lane in [`dynamic_elf`], with
//!   [`final_image_validation`] replaying either; [`function_fragments`]
//!   projects admitted current fragments into that same object and places
//!   every addressed byte literal's octets in one read-only pool after the
//!   code, binding each literal address field to it. Ordinary callback bodies
//!   follow that same projection. [`private_object_composition`] embeds their
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
mod private_entry_transport;
pub use private_entry_transport::create_private_function_object;
mod physical_entry;
pub use physical_entry::{PhysicalEntryBinding, PhysicalEntryPartitions, bind_physical_entry};
mod dynamic_elf;
mod final_image_validation;
mod function_fragments;
mod hosted_receiver;
mod hosted_unit_entry;
mod image_output;
mod installation_record;
mod object_artifact;
mod private_object_composition;

pub use function_fragments::FunctionFragmentObjectArtifactError;
pub use object_artifact::{
    ObjectPrivateFunctionEntry, PrivateFunctionObject, PrivateObjectSymbolBinding,
};
pub use private_object_composition::compose_private_function_objects;

pub use function_fragments::replay::verify_retained_target_operations;

pub use object_artifact::replay::boundary::byte_sequence_custody::linux_write_line_custody_is_exact;
pub use object_artifact::replay::boundary::completion_receipts::completion_receipts_have_exact_custody;
pub(crate) use object_artifact::stack_demand::StackDemand;
pub use object_artifact::stack_demand::derive_stack_demand;
pub use object_artifact::{
    ObjectArtifact, ObjectBoundarySettlement, ObjectCodeAttribution, ObjectForeignCall,
    ObjectFunction, ObjectScalarCallStack, ObjectUnitCallStack, build_object_artifact,
    derive_normalized_foreign_call_custody, durable_home_offsets,
    foreign_invocation_provenance_matches, foreign_result_operand_count, fragment_instruction_span,
    producer_instruction, scalar_call_result_producer, scalar_immediate_span,
    structural_result_capture, transported_span_end, unique_block_instruction,
};
pub(crate) use object_artifact::{
    ObjectByteLiteralAddress, ObjectByteLiteralPool, ObjectCompilerPrivateFunction, ObjectError,
    ObjectPortEffect, ObjectScalarStack, ObjectUnitStack,
};

pub use function_fragments::{
    FunctionFragmentObjectArtifactRequest, build_function_fragment_object_artifact,
};

pub use dynamic_elf::{RequestedDynamicElfImage, validate_requested_dynamic_elf_image};
pub(crate) use hosted_receiver::HostedReceiverBinding;
pub use hosted_receiver::bind_hosted_receiver;
pub use image_output::{
    ExecutableImage, ExecutableImageEmissionRequest, ImageApplicationMetadataError,
    ObjectContainer, RequestedExecutableImage, can_emit_executable_image, emit_executable_image,
    validate_direct_executable_image,
};

pub use installation_record::InstallationError;
pub use machine_code::BoundaryExecutionRecord;
