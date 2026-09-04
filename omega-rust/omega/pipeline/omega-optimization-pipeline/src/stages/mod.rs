//! Optimizer module role: stage group. Ordered custody boundaries from target selection through native artifacts.

pub(crate) mod allocation;
pub(crate) mod artifacts;
pub(crate) mod encoding;
pub(crate) mod layout;
pub(crate) mod machine;
pub(crate) mod realization;
pub(crate) mod selection;

pub use allocation::*;
pub use artifacts::*;
pub use encoding::*;
pub use layout::*;
pub use machine::*;
pub use omega_allocation_legality_to_fixed_view_copies::*;
pub use omega_fixed_view_copies_to_reanalyzed_legality::*;
pub use omega_live_ranges_to_allocation_legality::*;
pub use omega_liveness_to_live_ranges::*;
pub use omega_selected_instructions_to_liveness::*;
pub use realization::*;
pub use selection::*;
