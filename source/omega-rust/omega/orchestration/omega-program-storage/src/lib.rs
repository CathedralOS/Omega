#![forbid(unsafe_code)]

//! Program-storage entry contracts shared by source checking, calling-policy
//! planning, and native bridge construction.

mod artifacts;
#[path = "local_storage_custody.rs"]
mod program_local_storage_custody;
#[path = "continuation_inbound.rs"]
mod program_storage_continuation_inbound;
#[path = "emitted_argument_binding.rs"]
mod program_storage_emitted_argument_binding;
#[path = "entry.rs"]
mod program_storage_entry;
#[path = "extent_operand.rs"]
mod program_storage_extent_operand;
#[path = "extent_value.rs"]
mod program_storage_extent_value;
#[path = "reserved_outgoing_frame.rs"]
mod program_storage_reserved_outgoing_frame;
#[path = "root_argument_binding.rs"]
mod program_storage_root_argument_binding;
#[path = "root_authority.rs"]
mod program_storage_root_authority;
#[path = "source_call.rs"]
mod program_storage_source_call;
#[path = "wrapper.rs"]
mod program_storage_wrapper;
#[path = "wrapper_arrival.rs"]
mod program_storage_wrapper_arrival;
#[path = "wrapper_body.rs"]
mod program_storage_wrapper_body;
#[path = "wrapper_evidence.rs"]
mod program_storage_wrapper_evidence;
#[path = "wrapper_frame.rs"]
mod program_storage_wrapper_frame;
mod selected_provider;

pub use artifacts::{
    PROGRAM_STORAGE_INSTALLATION_ARTIFACT, program_storage_installation_record_json,
};
pub use omega_program_entry_plan::*;
pub use program_local_storage_custody::*;
pub use program_storage_continuation_inbound::*;
pub use program_storage_emitted_argument_binding::*;
pub use program_storage_entry::*;
pub use program_storage_extent_operand::*;
pub use program_storage_extent_value::*;
pub use program_storage_reserved_outgoing_frame::*;
pub use program_storage_root_argument_binding::*;
pub use program_storage_root_authority::*;
pub use program_storage_source_call::*;
pub use program_storage_wrapper::*;
pub use program_storage_wrapper_arrival::*;
pub use program_storage_wrapper_body::*;
pub use program_storage_wrapper_evidence::*;
pub use program_storage_wrapper_frame::*;
pub use selected_provider::ProgramStorageSelectedProviderPlan;

mod provider_plans {
    pub type SelectedExternalRootProviderPlan = crate::ProgramStorageSelectedProviderPlan;
}
