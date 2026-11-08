//! Do not invent evaluated transfers for values a private continuation never reads.
//!
//! The complete graph and its retained evidence decide parameter demand. This
//! only finishes generated block signatures: operations, source entry telescopes,
//! cleanup order and selected call operands remain exactly as produced.

use lowered_psi::LoweredPsi;
use terminal_semantics::scalar_demand::{ScalarDemandMode, complete_scalar_demand};

pub(super) fn complete(lowered: &mut LoweredPsi) {
    let module = &lowered.semantic_module;
    let mut retained = terminal_semantics::scalar_demand::record_named_rows(module).values;
    for operation in module.operations() {
        if module
            .boundary_application_demands
            .binary_search_by_key(&operation.id, |demand| demand.terminal_operation())
            .is_ok()
            && let Some(result) = operation.result.scalar()
        {
            retained.push(result.id);
        }
    }
    for occurrence in &lowered.source_call_occurrences {
        occurrence
            .frontier_before_call
            .visit_value_ids(|value| retained.push(value));
    }
    let mut protected = lowered
        .state_blocks
        .iter()
        .map(|row| (row.machine, row.block))
        .collect::<Vec<_>>();
    for row in &module.scalar_block_invariants {
        protected.push((row.machine, row.header));
        row.predicate.visit_value_ids(|value| retained.push(value));
    }
    let Some(qualified) = terminal_semantics::scalar_demand::qualification_parameter_blocks(module)
    else {
        return;
    };
    protected.extend(qualified);
    for row in &module.scalar_qualifications.coercions {
        retained.extend([row.source, row.destination]);
    }
    protected.sort_unstable();
    let Some(retained) =
        terminal_semantics::scalar_demand::retained_values_by_machine(module, retained)
    else {
        return;
    };
    for machine in &mut lowered.semantic_module.machines {
        let first = protected.partition_point(|(owner, _)| *owner < machine.id);
        let last = protected.partition_point(|(owner, _)| *owner <= machine.id);
        let blocks = protected[first..last]
            .iter()
            .map(|(_, block)| *block)
            .chain([machine.entry]);
        let values = retained
            .binary_search_by_key(&machine.id, |(owner, _)| *owner)
            .ok()
            .map_or(&[][..], |position| retained[position].1.as_slice());
        complete_scalar_demand(
            machine,
            values.iter().copied(),
            blocks,
            ScalarDemandMode::KeepAllOperations,
        );
    }
}
