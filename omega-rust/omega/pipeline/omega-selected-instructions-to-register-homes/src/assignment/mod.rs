//! Optimizer module role: stage group. Register homes for admitted selected-program facts.

mod baseline;
mod current;
mod recovery;
mod transformed;

pub use baseline::*;
pub use current::{RegisterAllocationError, stage_register_allocation};
pub use recovery::{
    stage_active_resident_register_allocation, stage_fixed_view_register_allocation,
};
pub use transformed::*;
