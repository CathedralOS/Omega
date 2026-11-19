/*
We select machine instructions for a prepared execution plan (`Omega 03`).

`Omega 02` publishes a `PreparedExecutionPlan` (`legalized-operations`): what
must execute, with concrete types, placements, proof identities and ownership
evidence already attached. `stage_optimized_instruction_selection` in
`optimized.rs` binds that plan to the installations the driver resolved,
selects instructions for it against the target register environment, and
records the plan's and the selected program's identities in one
`StagedOptimizedSelectionCustodyReceipt`.
*/

/*
This stage owns the choices that depend on instructions and registers:
instruction sequences, virtual registers, fixed-register constraints, and how
prepared transports are realized. It owns no decision about what the program
means. It is handed no source module, abstract program or optimization-unit
table, and `check-boundary` rejects a dependency that could reach one. A
question it must ask of the plan's structural declarations is asked through
`legalized_operations::structural_inputs`, which preparation uses too.
*/

/*
- Input: `SelectionInput` (`optimized.rs`): the prepared plan, the bound installations, and the optimization identities the receipt records.
- Output: `StagedOptimizedSelectedInstructions` and `ValidatedSelectedInstructions` consumed by `Omega 04` (`selected-instructions-to-selected-instructions`).
- Flat module layout:
  - `optimized.rs`: stage entry, installation binding, and `StagedOptimizedSelectedInstructions`.
  - `selection.rs`, `selection_inputs.rs`, `selection_edge_transfers.rs`, `selection_call_abi.rs`: instruction-selection driver, input projections, block-edge transfers, and scalar/foreign call ABI placement.
  - `selection_scalar_graph.rs`, `selection_structural.rs`, `selection_views_and_bytes.rs`, `selection_access_lifecycles.rs`, `selection_interrupts.rs`: selected-CFG instruction emission across scalar, structural, byte/element-view, access-lifecycle, and interrupt operations.
*/

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
mod optimized;
mod selection;
mod selection_access_lifecycles;
mod selection_call_abi;
mod selection_edge_transfers;
mod selection_inputs;
mod selection_interrupts;
mod selection_scalar_graph;
mod selection_structural;
mod selection_views_and_bytes;

#[cfg(test)]
use legalized_operations::structural_inputs::structural_reference_input::reference_formation_geometry;
pub(crate) use optimized::BoundExecutionPlan;
pub use optimized::{
    OptimizedSelectionPipelineError, SelectionInput, StagedOptimizedSelectedInstructions,
    stage_optimized_instruction_selection,
};
pub(crate) use selection::SelectedInstructionError;
pub use selection::ValidatedSelectedInstructions;

#[cfg(feature = "test-support")]
pub use optimized::{BoundInstallations, InstallationBindingError, select_instructions_for_plan};
