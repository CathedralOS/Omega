use super::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use crate::lower_typed_trees_for_crash_fact_inspection;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;

mod crash_routes;
mod data_facts;
mod operational_contracts;
mod ranking;
mod write_frame_aggregate_arguments;
mod write_frame_array_literal_projection;
mod write_frame_assignment_values;
mod write_frame_boundary_arguments;
mod write_frame_call_arguments;
mod write_frame_computed_indexes;
mod write_frame_computed_receivers;
mod write_frame_cycles;
mod write_frame_indexed_calls;
mod write_frame_literal_moves;
mod write_frame_moved_aggregates;
mod write_frame_parameter_aggregates;
mod write_frame_returned_places;
mod write_frame_stored_aggregates;

fn symbol_of_checked(
    checked: &psi_checked_trees::CheckedTrees,
    name: &str,
) -> psi_symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("machine {name}"))
        .symbol
}
