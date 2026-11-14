/*
We project admitted machine-code function fragments into sealed object
artifacts (`ObjectArtifact`), compose private callback and component objects,
bind physical and hosted entry shims, and emit verified executable and
shared-component images across ELF, PE32+, and Mach-O. Every compiler-authored
instruction byte, read-only byte-literal pool entry, normalized foreign-call
custody record, and provenance-bearing function region is validated against the
emitted container bytes without reconstructing legacy machine-code carriers.

Our pipeline separates object custody from container emission and installation:
`function_fragments.rs` is the sole builder of `ObjectArtifact` (in
`object_artifact.rs`); `object.rs` composes private callback objects and
component shared images; `entries.rs` binds physical stack-switch, hosted
receiver, callable entry transport, and foreign adapter shims; `image.rs`
routes sealed objects through direct or dynamic ELF container writers and
replays terminal image custody. Receiver-side installation authority remains
separate from emitting these containers.
*/

/*
We project each admitted function fragment once into `ObjectArtifact` rather
than replaying a parallel machine-code record model, and we embed complete
private callback objects via `compose_private_function_objects` so helper
functions, imports, and read-only data survive intact instead of being
flattened into a synthetic single-block thunk. Keeping `RequestedDynamicElfImage`
distinct from `ExecutableImage` ensures that final-byte custody for a dynamic
ELF container can never be mistaken for direct execution or installation
authority at compile time.
*/

/*
`native-artifact`, `compiler`, `native-compiler`, `orchestration`, and
`native-provider-runtime` consume our object-artifact builders, entry bindings,
and executable/component image emitters. `image`, `image-elf`, `image-pe`, and
`image-macho` provide the underlying container writers and relocation envelope
verifiers that `image.rs` and `object.rs` orchestrate.
*/

#![forbid(unsafe_code)]

mod callable_calls;

mod codec_bytes {
    pub(crate) use optimization_core::codec_bytes::u32_at;

    pub(crate) fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
        let end = offset.checked_add(2)?;
        Some(u16::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
    }

    pub(crate) fn u64_at(bytes: &[u8], offset: usize) -> Option<u64> {
        let end = offset.checked_add(8)?;
        Some(u64::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
    }
}

mod entries;
pub(crate) mod function_fragments;
mod image;
mod object;
pub(crate) mod object_artifact;
pub use entries::callable_entry_transport::{
    CallableEntryTransport, NativeBodyCpuState, create_callable_entry_transport,
    create_installed_private_function_object, create_private_function_object,
    observe_native_body_cpu_state, observe_native_constant_operand,
    observe_native_function_cpu_state,
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

pub use object::function_fragments::replay::verify_retained_target_operations;

pub use object::object_artifact::replay::boundary::byte_sequence_custody::linux_write_line_custody_is_exact;
pub use object::object_artifact::replay::boundary::completion_receipts::completion_receipts_have_exact_custody;
pub(crate) use object::object_artifact::stack_demand::StackDemand;
pub use object::object_artifact::stack_demand::derive_stack_demand;
pub use object::object_artifact::{
    NativeInstructionSourceRows, ObjectArtifact, ObjectBoundarySettlement, ObjectCodeAttribution,
    ObjectForeignCall, ObjectFunction, ObjectScalarCallStack, ObjectSemanticSource,
    ObjectUnitCallStack, derive_normalized_foreign_call_custody, durable_home_offsets,
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

pub use machine_code::BoundaryExecutionRecord;

pub use object::object_artifact::installed_stack::{
    compose_installed_arrival_demands, correlate_native_instruction_sources,
    installed_interrupt_stack_requirement, require_installed_root_provision,
};
