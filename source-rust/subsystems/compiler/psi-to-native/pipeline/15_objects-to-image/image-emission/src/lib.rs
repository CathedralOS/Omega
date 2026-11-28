//! Bind entries, compose body objects and emit executable and shared images.
//! Each input body already has its text and data sections, symbols and relocations.
//! `function_fragments` relates those sections to instruction and stack evidence.
//! `object` links private callback and component objects. `entries` binds physical,
//! hosted and foreign shims. `image` writes the container and checks its bytes.

#![forbid(unsafe_code)]

mod callable_calls;

mod codec_bytes {
    pub(crate) use ::wire::codec_bytes::u32_at;

    pub(crate) fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
        let end = offset.checked_add(2)?;
        Some(u16::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
    }

    pub(crate) fn u64_at(bytes: &[u8], offset: usize) -> Option<u64> {
        let end = offset.checked_add(8)?;
        Some(u64::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
    }
}

mod companion_images;
mod entries;
mod published_entry_bytes;
pub use companion_images::NativeCompanionImage;
pub use published_entry_bytes::verify_entry_bridge_bytes;

pub(crate) mod function_fragments;
mod image;
mod object;
pub(crate) mod object_artifact;
mod unwind_check;
pub use entries::callable_entry_transport::{
    CallableEntryTransport, EncodedPrivateEntry, NativeBodyCpuState, bind_private_function_entry,
    observe_native_body_cpu_state, observe_native_constant_operand,
    observe_native_function_cpu_state,
};
pub use entries::foreign_adapter::{
    NativeForeignAdapterEmissionError, NativeForeignAdapterImage, emit_native_foreign_adapter,
};
pub use entries::physical_entry::{
    EncodedPhysicalEntry, InstalledPhysicalCpuRequirement, PhysicalEntryBinding,
    PhysicalEntryPartitions, PhysicalEntryRequirements, PhysicalEntryStackRequirement,
    PhysicalIncomingStackRequirement, bind_encoded_physical_entry,
};
pub use object::component_object::{
    ComponentFunctionImage, component_export_symbol, prepare_component_function_image,
};
pub use object::component_shared_image::{
    ComponentSharedContainer, ComponentSharedImage, ComponentSharedImageEmissionError,
    emit_component_shared_image,
};
mod order {
    pub(crate) fn strictly_increasing<T, K: Ord>(rows: &[T], key: impl Fn(&T) -> K) -> bool {
        rows.windows(2).all(|pair| key(&pair[0]) < key(&pair[1]))
    }
}

pub use object::function_fragments::FunctionFragmentObjectArtifactError;
pub use object::object_artifact::{
    ObjectPrivateFunctionEntry, PrivateFunctionObject, PrivateObjectSymbolBinding,
};
pub use object::private_object_composition::compose_private_function_objects;

pub use object::function_fragments::replay::{
    FragmentReplay, PhysicalEncodingReadback, RetainedTargetOperations,
};

pub use object::object_artifact::replay::boundary::byte_sequence_custody::linux_write_line_custody_is_exact;
pub use object::object_artifact::replay::boundary::completion_receipts::completion_receipts_have_exact_custody;
pub(crate) use object::object_artifact::stack_demand::StackDemand;
pub use object::object_artifact::stack_demand::{ObjectStackError, derive_stack_demand};
pub use object::object_artifact::{
    NativeInstructionSourceRows, ObjectArtifact, ObjectBoundarySettlement, ObjectCodeAttribution,
    ObjectForeignCall, ObjectFunction, ObjectScalarCallStack, ObjectUnitCallStack,
    derive_normalized_foreign_call_custody, fragment_instruction_span, place_foreign_call,
    resolve_instruction_sequence,
};
pub(crate) use object::object_artifact::{
    ObjectCompilerPrivateFunction, ObjectPortEffect, ObjectScalarStack, ObjectUnitStack,
};

pub use object::function_fragments::prepare_object_for_image;

mod hosted_loader;
mod hosted_program;
pub use hosted_program::{EncodedHostedProgramEntry, bind_hosted_program_entry};
pub use image::dynamic_elf::{RequestedDynamicElfImage, validate_requested_dynamic_elf_image};
pub use image::final_image_validation::native_code_layouts::{
    LinkedNativeCodeLayout, LinkedNativeCodeRegion, inspect_native_code_layouts,
};
pub use image::image_output::{
    ExecutableImage, ExecutableImageEmissionRequest, ImageApplicationMetadataError,
    ObjectContainer, RequestedExecutableImage, can_emit_executable_image, emit_executable_image,
    validate_direct_executable_image, validate_direct_executable_image_output,
};

pub use machine_code::BoundaryExecutionRecord;

pub use object::object_artifact::installed_stack::{
    installed_interrupt_stack_requirement, require_installed_root_provision,
};

mod object_input;

/// Executable image bindings retain the exact origin of physical planning.
pub type NativeEntryBridge = object_file::NativeEntryBridge;
pub type NativeForeignBridge = object_file::NativeForeignBridge;
