mod artifact;
mod builder;
mod data_address_records;
mod data_addresses;
mod input;
mod instruction_records;
mod lookups;
mod materialization;
mod offsets;

pub use artifact::append_validated_artifact_relocations;
pub use builder::build_relocation_plan;
pub use input::RelocationPlanningInput;
pub use materialization::append_native_materialization_relocations;
