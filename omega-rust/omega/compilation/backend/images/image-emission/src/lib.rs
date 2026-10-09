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
//!   bytes. Executable-image output over a sealed object starts at
//!   [`image_output`]'s `emit_executable_image`, which selects the direct
//!   writers it owns or the dynamic ELF lane in [`dynamic_elf`], with
//!   [`final_image_validation`] replaying either; [`function_fragments`]
//!   projects admitted current fragments into that same object and places
//!   every addressed byte literal's octets in one read-only pool after the
//!   code, binding each literal address field to it.
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

pub use object_artifact::replay::boundary::byte_sequence_custody::linux_write_line_custody_is_exact;
pub use object_artifact::replay::boundary::completion_receipts::completion_receipts_have_exact_custody;
pub use object_artifact::stack_demand::{ derive_stack_demand};
pub(crate) use object_artifact::stack_demand::{StackDemand};
pub use object_artifact::{
    ObjectArtifact, ObjectBoundarySettlement,
    ObjectCodeAttribution, ObjectForeignCall,
    ObjectFunction, ObjectScalarCallStack,
    ObjectUnitCallStack, build_object_artifact, derive_normalized_foreign_call_custody,
    foreign_invocation_provenance_matches, foreign_result_operand_count, fragment_instruction_span,
    durable_home_offsets, producer_instruction, scalar_immediate_span, transported_span_end, scalar_call_result_producer,
    unique_block_instruction,
    structural_result_capture,
};
pub(crate) use object_artifact::{ ObjectByteLiteralAddress, ObjectByteLiteralPool, ObjectCompilerPrivateFunction, ObjectError, ObjectPortEffect, ObjectScalarStack, ObjectUnitStack};

pub use function_fragments::{ FunctionFragmentObjectArtifactRequest,
    build_function_fragment_object_artifact,
};

pub use dynamic_elf::{
    RequestedDynamicElfImage, validate_requested_dynamic_elf_image,
};
pub use hosted_receiver::{ bind_hosted_receiver};
pub(crate) use hosted_receiver::{HostedReceiverBinding};
pub use image_output::{
    ExecutableImage, ExecutableImageEmissionRequest, ImageApplicationMetadataError,
    ObjectContainer, RequestedExecutableImage,
    can_emit_executable_image, emit_executable_image, validate_direct_executable_image,
};

pub use installation_record::{ InstallationError,
};
pub(crate) use installation_record::{ InstallationRecord,
    InstalledCompilerPrivateFunction, validate_installation_record};
pub use machine_code::BoundaryExecutionRecord;
