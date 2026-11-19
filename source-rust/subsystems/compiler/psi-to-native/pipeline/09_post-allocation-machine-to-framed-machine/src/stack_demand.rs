//! Select physical stack extents and call edges from current instructions.

pub use machine_code::physical_stack::{
    ForeignStackExtent, LocalStackExtent, PhysicalForeignStackCall, PhysicalStackCall,
    PhysicalStackFunction, PhysicalStackProgram, StackDemand, StackDemandError,
    derive_physical_stack_demand,
};

mod physical;
pub use physical::plan_physical_stack_program;
