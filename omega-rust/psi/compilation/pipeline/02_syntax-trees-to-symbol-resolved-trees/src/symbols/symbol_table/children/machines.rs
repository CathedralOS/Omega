use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTableAppender, SymbolTableBuilder};

use super::insert_generic_parameter_children;
use crate::symbols::symbol_table::names::{parameter_seeds, symbol_seed, type_parameter_seeds};

pub(in crate::symbols::symbol_table) fn insert_machine_symbol_children(
    builder: &mut impl SymbolTableAppender,
    program: &SymbolResolvedTrees,
    machine_symbol: SymbolHandle,
    machine: &symbol_resolved_trees::machine::Machine,
    has_sources: bool,
    sources: Option<&source::SourceMap>,
) {
    let machine_children = builder.insert_children(
        machine_symbol,
        type_parameter_seeds(program.machine_type_parameters(machine), false, has_sources)
            .chain(machine.conformance_bounds.iter().map(|bound| {
                symbol_seed(
                    SymbolKind::ConformanceParameter,
                    &bound.binder_name,
                    has_sources,
                )
            }))
            .chain(
                program
                    .machine_owned_data(machine.owned_data)
                    .iter()
                    .map(|owned_data| {
                        symbol_seed(SymbolKind::Field, &owned_data.name, has_sources)
                    }),
            )
            .chain(
                program
                    .machine_state_handles(machine.states)
                    .iter()
                    .map(|state| program.machine_state(*state))
                    .map(|state| symbol_seed(SymbolKind::State, &state.name, has_sources)),
            ),
    );
    let mut machine_children = SymbolTableBuilder::child_handles(machine_children);

    insert_generic_parameter_children(
        builder,
        program,
        program.machine_type_parameters(machine),
        &machine.conformance_bounds,
        &mut machine_children,
        has_sources,
    );
    for bound in machine.conformance_bounds.iter() {
        let Some(binder_symbol) = machine_children.next() else {
            break;
        };
        super::evidence::insert_evidence_children(
            builder,
            program,
            binder_symbol,
            bound,
            has_sources,
            sources,
        );
    }
    for _ in program.machine_owned_data(machine.owned_data) {
        let _ = machine_children.next();
    }
    for state in program.machine_state_handles(machine.states) {
        if let Some(state_symbol) = machine_children.next() {
            let state = program.machine_state(*state);
            builder.insert_children(
                state_symbol,
                parameter_seeds(program.state_parameters(state.parameters), has_sources),
            );
        }
    }
}
