//! Every write must preserve the defaults of the actual lent storage, even
//! when its current descriptor displays fewer domains. Plan these questions
//! after reference origins and lifetime ends have been published.

use crate::{Unserved, refuse};
use arena::IdTable;
use semantic_vocabulary::MachineId;
use terminal_check_tables::obligations::{ObligationKind, ObligationRow};
use terminal_check_tables::program::MachineSources;
use terminal_check_tables::{CheckTables, Checked, Construct, ProgramPoint};
use terminal_psi::TerminalModule;
use terminal_semantics::descriptor_results::terminal_borrowed_storage_graph_from;
use terminal_semantics::structural_qualifications::DefaultRequirements;

fn build_defaults<'a>(
    module: &'a TerminalModule,
    machine: &'a terminal_psi::TerminalMachine,
) -> Checked<DefaultRequirements<'a>> {
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
}

pub(crate) fn install(
    module: &TerminalModule,
    tables: &mut CheckTables,
    sources: &mut IdTable<MachineId, MachineSources>,
    parallel: bool,
) -> Checked<()> {
    let defaults = if parallel && module.machines.len() >= 2 {
        use rayon::prelude::*;
        module
            .machines
            .par_iter()
            .with_max_len(1)
            .map(|machine| build_defaults(module, machine))
            .collect::<Checked<Vec<_>>>()?
    } else {
        module
            .machines
            .iter()
            .map(|machine| build_defaults(module, machine))
            .collect::<Checked<Vec<_>>>()?
    };
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
    // Each machine replays from its own analysis, and every analysis takes
    // the callee summaries another one already completed.
    let summaries = crate::references::results::module_summaries(module);
    let install_machine =
        |machine: &terminal_psi::TerminalMachine,
         required: DefaultRequirements,
         machine_tables: &mut terminal_check_tables::MachineTables,
         source: &mut MachineSources,
         references: &mut terminal_semantics::descriptor_results::DescriptorAnalysis<'_>|
         -> Checked<()> {
            let unavailable =
                |detail| refuse(machine.id, Construct::Machine, Unserved::Machine(detail));
            for block in &machine.blocks {
                let mut at = ProgramPoint::BlockEntry(block.id);
                for (position, operation) in block.operations.iter().enumerate() {
                    let position = u32::try_from(position).map_err(|_| {
                        unavailable("default restoration position exceeds the descriptor graph")
                    })?;
                    let origin = *source.operations.get(operation.id).ok_or_else(|| {
                        unavailable("default restoration lost its operation source")
                    })?;
                    let clauses = required
                        .operation(module, operation, (block.id, position), references)
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
                        let obligation = machine_tables.obligations.push(ObligationRow {
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
            Ok(())
        };
    let aligned = module.machines.len() == tables.machines.len()
        && module.machines.len() == sources.len()
        && module
            .machines
            .iter()
            .zip(tables.machines.keys().zip(sources.keys()))
            .all(|(machine, (table_id, source_id))| {
                machine.id == table_id && machine.id == source_id
            });
    if parallel && aligned && module.machines.len() >= 2 {
        use rayon::prelude::*;
        let work: Vec<_> = module
            .machines
            .iter()
            .zip(defaults)
            .zip(
                tables
                    .machines
                    .iter_mut()
                    .map(|(_, tables)| tables)
                    .zip(sources.iter_mut().map(|(_, source)| source)),
            )
            .collect();
        let results: Vec<Checked<()>> = work
            .into_par_iter()
            .with_max_len(1)
            .map(|((machine, required), (machine_tables, source))| {
                let mut references =
                    crate::references::results::shared_analysis(module, &graph, &summaries);
                install_machine(machine, required, machine_tables, source, &mut references)
            })
            .collect();
        for result in results {
            result?;
        }
    } else {
        let mut references =
            crate::references::results::shared_analysis(module, &graph, &summaries);
        for (machine, required) in module.machines.iter().zip(defaults) {
            let unavailable =
                |detail| refuse(machine.id, Construct::Machine, Unserved::Machine(detail));
            let machine_tables = tables
                .machines
                .get_mut(machine.id)
                .ok_or_else(|| unavailable("default restoration lost its check tables"))?;
            let source = sources
                .get_mut(machine.id)
                .ok_or_else(|| unavailable("default restoration lost its source owner"))?;
            install_machine(machine, required, machine_tables, source, &mut references)?;
        }
    }
    Ok(())
}
