//! Select each function against the bound plan and target register constraints.

macro_rules! failed_display {
    ($type:ty, $message:literal) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!($message, ": {:?}"), self)
            }
        }
    };
}

pub use legalized_operations::installed_interrupt_geometry::InstalledInterruptOperandLayout;
pub(crate) use legalized_operations::{native_geometry, structural_inputs};
mod register_constraints;
mod selection;
#[path = "selection/access_lifecycles.rs"]
mod selection_access_lifecycles;
#[path = "selection/call_abi.rs"]
mod selection_call_abi;
#[path = "selection/edge_transfers.rs"]
mod selection_edge_transfers;
mod selection_inlining;
#[path = "selection/inputs.rs"]
mod selection_inputs;
#[path = "selection/interrupts.rs"]
mod selection_interrupts;
#[path = "selection/scalar_graph.rs"]
mod selection_scalar_graph;
#[path = "selection/shared_parameters.rs"]
mod selection_shared_parameters;
#[path = "selection/structural.rs"]
mod selection_structural;
mod selection_vector_promotion;
#[path = "selection/views_and_bytes.rs"]
mod selection_views_and_bytes;

#[cfg(test)]
use legalized_operations::structural_inputs::structural_reference_input::reference_formation_geometry;
pub use selection::ValidatedSelectedInstructions;
pub(crate) use selection::{SelectedInstructionError, select_bound_instructions};
