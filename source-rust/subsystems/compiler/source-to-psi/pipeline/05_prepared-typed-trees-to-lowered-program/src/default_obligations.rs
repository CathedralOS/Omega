//! Every write must preserve the defaults of the actual lent storage, even
//! when its current descriptor displays fewer domains. Plan these questions
//! after reference origins and lifetime ends have been published.

use crate::lowering::{Unserved, refuse};
use arena::IdTable;
use semantic_vocabulary::MachineId;
use terminal_check_tables::obligations::{ObligationKind, ObligationRow};
use terminal_check_tables::program::MachineSources;
use terminal_check_tables::{CheckTables, Checked, Construct, ProgramPoint};
use terminal_psi::TerminalModule;
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, terminal_borrowed_storage_graph_from,
};
use terminal_semantics::structural_qualifications::DefaultRequirements;

pub(crate) fn install(
    module: &TerminalModule,
    tables: &mut CheckTables,
    sources: &mut IdTable<MachineId, MachineSources>,
) -> Checked<()> {
    let defaults = module
        .machines
        .iter()
        .map(|machine| {
            DefaultRequirements::new(
                &module.types.structural_types,
                &module.types.structural_domains,
                machine,
            )
            .map_err(|error| {
                refuse(
                    machine.id,
                    Construct::Machine,
                    Unserved::Machine(error.detail),
                )
            })
        })
        .collect::<Checked<Vec<_>>>()?;
    let roots = module
        .machines
        .iter()
        .zip(&defaults)
        .filter(|(_, required)| required.needs_borrowed_storage())
        .map(|(machine, _)| machine.id);
    let graph = terminal_borrowed_storage_graph_from(module, roots, &[]).map_err(|error| {
        let mut refused = refuse(
            error.machine(),
            Construct::Machine,
            Unserved::Machine("default restoration has no complete borrowed-storage graph"),
        );
        refused.detail = format!("{}: {}", refused.detail, error.in_module(module));
        refused
    })?;
    for (machine, required) in module.machines.iter().zip(defaults) {
        let mut references =
            DescriptorAnalysis::with_borrowed_storage_slots(&module.types.structural_types, &graph);
        let unavailable =
            |detail| refuse(machine.id, Construct::Machine, Unserved::Machine(detail));
        let tables = tables
            .machines
            .get_mut(machine.id)
            .ok_or_else(|| unavailable("default restoration lost its check tables"))?;
        let source = sources
            .get_mut(machine.id)
            .ok_or_else(|| unavailable("default restoration lost its source owner"))?;
        for block in &machine.blocks {
            let mut at = ProgramPoint::BlockEntry(block.id);
            for (position, operation) in block.operations.iter().enumerate() {
                let position = u32::try_from(position).map_err(|_| {
                    unavailable("default restoration position exceeds the descriptor graph")
                })?;
                let origin = *source
                    .operations
                    .get(operation.id)
                    .ok_or_else(|| unavailable("default restoration lost its operation source"))?;
                let clauses = required
                    .operation(operation, (block.id, position), &mut references)
                    .map_err(|error| {
                        refuse(
                            machine.id,
                            Construct::Operation(operation.id),
                            Unserved::Machine(error.detail),
                        )
                        .with_source_span(origin.authored_span())
                    })?;
                for clause in 0..clauses.len() {
                    let clause = u32::try_from(clause)
                        .map_err(|_| unavailable("default restoration has too many clauses"))?;
                    let obligation = tables.obligations.push(ObligationRow {
                        kind: ObligationKind::DataDomain {
                            operation: operation.id,
                            clause,
                        },
                        created_at: at,
                    });
                    if source.obligations.push(origin) != obligation {
                        return Err(unavailable(
                            "default restoration question/source identities differ",
                        ));
                    }
                }
                at = ProgramPoint::AfterOperation(operation.id);
            }
        }
    }
    Ok(())
}
