pub mod encoding;
mod entry;
mod generated_writer;
pub mod operands;
mod selection;
pub mod widths;

pub use encoding::*;
pub use entry::{
    DerivedBoundaryEntryParameterStorage, DerivedBoundaryEntryStorage, DerivedBoundaryExit,
    derive_boundary_call_return_mechanics_footprint, derive_boundary_checked_assembly_footprint,
    derive_boundary_compiler_body_atomic_footprint,
    derive_boundary_compiler_body_constant_host_result_footprint,
    derive_boundary_compiler_body_outbound_authored_aggregate_import_footprint,
    derive_boundary_compiler_body_outbound_authored_aggregate_import_result_footprint,
    derive_boundary_compiler_body_outbound_authored_aggregate_result_footprint,
    derive_boundary_compiler_body_outbound_authored_float_import_footprint,
    derive_boundary_compiler_body_outbound_authored_float_import_result_footprint,
    derive_boundary_compiler_body_outbound_authored_import_footprint,
    derive_boundary_compiler_body_outbound_authored_import_result_footprint,
    derive_boundary_compiler_body_outbound_data_import_footprint,
    derive_boundary_compiler_body_outbound_data_import_result_footprint,
    derive_boundary_compiler_body_outbound_dereferenced_import_result_footprint,
    derive_boundary_compiler_body_outbound_float_import_result_footprint,
    derive_boundary_compiler_body_outbound_immediate_import_footprint,
    derive_boundary_compiler_body_outbound_immediate_import_result_footprint,
    derive_boundary_compiler_body_outbound_indirect_call_footprint,
    derive_boundary_compiler_body_outbound_open_create_import_footprint,
    derive_boundary_compiler_body_outbound_storage_import_footprint,
    derive_boundary_compiler_body_outbound_storage_import_result_footprint,
    derive_boundary_compiler_body_outbound_syscall_data_arguments_footprint,
    derive_boundary_compiler_body_outbound_syscall_footprint,
    derive_boundary_compiler_body_outbound_syscall_result_data_arguments_footprint,
    derive_boundary_compiler_body_outbound_syscall_result_footprint,
    derive_boundary_compiler_body_outbound_syscall_result_storage_arguments_footprint,
    derive_boundary_compiler_body_outbound_syscall_storage_arguments_footprint,
    derive_boundary_compiler_body_outbound_syscall_timespec_argument_footprint,
    derive_boundary_compiler_body_outbound_syscall_timespec_result_footprint,
    derive_boundary_compiler_body_place_address_write_footprint,
    derive_boundary_compiler_body_place_binary_write_footprint,
    derive_boundary_compiler_body_place_bounded_buffer_write_footprint,
    derive_boundary_compiler_body_place_copy_footprint,
    derive_boundary_compiler_body_place_integer_write_footprint,
    derive_boundary_compiler_body_place_string_write_footprint,
    derive_boundary_compiler_body_runtime_byte_read_footprint,
    derive_boundary_compiler_body_runtime_byte_write_footprint,
    derive_boundary_compiler_body_runtime_line_read_footprint,
    derive_boundary_compiler_body_storage_bit_field_write_footprint,
    derive_boundary_compiler_body_storage_convert_write_footprint,
    derive_boundary_compiler_body_text_assembly_write_footprint,
    derive_boundary_compiler_body_wire_byte_slice_read_footprint,
    derive_boundary_compiler_body_wire_expected_byte_read_footprint,
    derive_boundary_compiler_body_wire_literal_byte_append_footprint,
    derive_boundary_compiler_body_wire_nested_close_footprint,
    derive_boundary_compiler_body_wire_nested_open_footprint,
    derive_boundary_compiler_body_wire_repeated_scalar_varint_append_footprint,
    derive_boundary_compiler_body_wire_repeated_scalar_varint_read_footprint,
    derive_boundary_compiler_body_wire_scalar_slice_append_footprint,
    derive_boundary_compiler_body_wire_scalar_varint_append_footprint,
    derive_boundary_compiler_body_wire_scalar_varint_read_footprint,
    derive_boundary_compiler_body_wire_text_bytes_append_footprint,
    derive_boundary_dispatch_scaffold_footprint, derive_boundary_entry_slice_descriptor_footprint,
    derive_boundary_entry_storage, derive_boundary_entry_storage_writes, derive_boundary_exit,
    derive_boundary_exit_indirect_result_copy_footprint,
    derive_boundary_exit_result_register_footprint, derive_boundary_place_guard_footprint,
    derive_boundary_runtime_text_guard_footprint, derive_boundary_runtime_value_guard_footprint,
    derive_boundary_static_guard_footprint, derive_internal_call_entry_storage,
};
pub use generated_writer::{
    LoweredPostHandoffWriter, LoweredPostHandoffWriterFragment, PostHandoffEntryWriterBindingError,
    PreparedPostHandoffEntryWriterInvocation, bind_post_handoff_entry_writer_invocation,
    lower_post_handoff_writer_fragment, validate_lowered_post_handoff_writer,
};
pub use operands::*;
pub use selection::build_instruction_plan;
pub use widths::*;

pub use omega_isa_aarch64::{BoundedBufferPlaceSide, BoundedBufferPlaceSites};
/// Re-exported for the relocation walker: the `CopyPlaces` site list is the
/// x86_64 materializer's own record of where its base movs sit.
pub use omega_isa_x86_64::{PlaceCopySide, PlaceCopySites};

use omega_abstract_operations::AbstractDataPlan;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_layout::LayoutPlan;
use omega_platform_interface::HostCallPlan;
use omega_runtime_abi::RuntimeAbiPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_runtime_branching::RuntimeBranchingCallPlan;
use omega_runtime_dispatch_loop::RuntimeDispatchLoopPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_runtime_text::RuntimeTextPlan;
use omega_state_calls::{AliasFlowPlan, StateCallPlan};
use omega_state_graph::RuntimeFlowPlan;
use omega_state_guards::StateGuardPlan;
use omega_state_storage::StateStoragePlan;
use psi_checked_trees::CheckedTrees;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionSelectionInput<'plan> {
    pub target: omega_target::NativeTarget,
    pub freestanding: bool,
    pub runtime_abi: &'plan RuntimeAbiPlan,
    pub entry_key: StateKey,
    pub entry_boundary_plan: Option<&'plan omega_calling_conventions::BoundaryEntryPlan>,
    pub entry_symbol: Arc<str>,
    pub callback_placements: &'plan [omega_backend_plan::BoundNominalCallbackPlacement],
    pub callback_thunks: &'plan [omega_backend_plan::CallbackThunkPlan],
    pub program: &'plan CheckedTrees,
    pub selected_provider_plans: &'plan omega_effects::SelectedProviderPlanFacts,
    pub control_flow: &'plan ControlFlowPlan,
    pub host_abi: &'plan omega_calling_conventions::HostAbiPlan,
    pub host_calls: &'plan HostCallPlan,
    pub state_calls: &'plan StateCallPlan,
    /// See BackendPlan::receiver_bases (per-instance receiver dispatch).
    pub receiver_bases: &'plan [Option<usize>],
    /// See BackendPlan::state_contexts (same-context slot resolution).
    pub state_contexts: &'plan [u32],
    pub alias_flow: &'plan AliasFlowPlan,
    pub state_storage: &'plan StateStoragePlan,
    pub runtime_flow: &'plan RuntimeFlowPlan,
    pub runtime_bodies: &'plan RuntimeDispatchBodyPlan,
    pub runtime_branching_calls: &'plan RuntimeBranchingCallPlan,
    pub runtime_dispatch_loop: &'plan RuntimeDispatchLoopPlan,
    pub runtime_storage: &'plan RuntimeStoragePlan,
    pub runtime_text: &'plan RuntimeTextPlan,
    pub state_guards: &'plan StateGuardPlan,
    pub layouts: &'plan LayoutPlan,
    pub data: &'plan AbstractDataPlan,
}
