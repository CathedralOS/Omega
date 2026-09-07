//! Allocation candidate policies and exact constraints on physical homes.

pub mod allocator_availability;
pub use allocator_availability::*;
pub mod allocation_legality;
pub use allocation_legality::*;
pub mod fixed_precolored_intervals;
pub use fixed_precolored_intervals::*;
pub mod fixed_precolored_split_requirements;
pub use fixed_precolored_split_requirements::*;
