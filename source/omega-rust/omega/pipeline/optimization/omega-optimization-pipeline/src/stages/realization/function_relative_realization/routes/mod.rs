//! Optimizer module role: stage group. Exact source-custody routes into one function-relative manifest boundary.

mod fixed_frame;
mod layout_optimization;
mod post_allocation_machine;
mod selected_lowering;

pub use fixed_frame::*;
pub use layout_optimization::*;
pub use post_allocation_machine::*;
pub use selected_lowering::*;
