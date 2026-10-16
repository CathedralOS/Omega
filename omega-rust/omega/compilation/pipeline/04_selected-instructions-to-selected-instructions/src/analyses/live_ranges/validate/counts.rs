//! Report counts for one admitted function, carried only with its unchanged rows.

use selected_instructions::FunctionLiveRanges;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FunctionCounts {
    pub(super) block_count: usize,
    pub(super) virtual_register_count: usize,
    pub(super) virtual_occurrence_count: usize,
    pub(super) fixed_constraint_count: usize,
    pub(super) virtual_fragment_count: usize,
    pub(super) architectural_unit_count: usize,
    pub(super) architectural_action_count: usize,
    pub(super) architectural_fragment_count: usize,
    pub(super) virtual_edge_connector_count: usize,
    pub(super) architectural_edge_connector_count: usize,
    pub(super) interference_count: usize,
    pub(super) tied_pair_count: usize,
    pub(super) copy_affinity_count: usize,
    pub(super) tied_component_count: usize,
    pub(super) early_clobber_count: usize,
    pub(super) early_clobber_use_count: usize,
}

impl FunctionCounts {
    pub(super) fn new(function: &FunctionLiveRanges) -> Self {
        Self {
            block_count: function.block_domains.len(),
            virtual_register_count: function.virtual_registers.len(),
            virtual_occurrence_count: function
                .virtual_registers
                .iter()
                .map(|row| row.occurrences.len())
                .sum(),
            fixed_constraint_count: function
                .virtual_registers
                .iter()
                .map(|row| row.fixed_constraints.len())
                .sum(),
            virtual_fragment_count: function
                .virtual_registers
                .iter()
                .map(|row| row.fragments.len())
                .sum(),
            architectural_unit_count: function.architectural_units.len(),
            architectural_action_count: function
                .architectural_units
                .iter()
                .map(|row| row.actions.len())
                .sum(),
            architectural_fragment_count: function
                .architectural_units
                .iter()
                .map(|row| row.fragments.len())
                .sum(),
            virtual_edge_connector_count: function
                .virtual_registers
                .iter()
                .map(|row| row.edge_connectors.len())
                .sum(),
            architectural_edge_connector_count: function
                .architectural_units
                .iter()
                .map(|row| row.edge_connectors.len())
                .sum(),
            interference_count: function.interference.len(),
            tied_pair_count: function.tied_pairs.len(),
            copy_affinity_count: function.copy_affinities.len(),
            tied_component_count: super::receipt::tied_component_count(&function.tied_pairs),
            early_clobber_count: function.early_clobbers.len(),
            early_clobber_use_count: function
                .early_clobbers
                .iter()
                .map(|row| row.uses.len())
                .sum(),
        }
    }
}
