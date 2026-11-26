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
mod selection_access_lifecycles;
mod selection_call_abi;
mod selection_edge_transfers;
mod selection_inlining;
mod selection_inputs;
mod selection_interrupts;
mod selection_scalar_graph;
mod selection_structural;
mod selection_vector_promotion;
mod selection_views_and_bytes;

#[cfg(test)]
use legalized_operations::structural_inputs::structural_reference_input::reference_formation_geometry;
pub use selection::ValidatedSelectedInstructions;
pub(crate) use selection::{SelectedInstructionError, select_bound_instructions};
