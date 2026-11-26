//! A returned permission keeps the actual input storage its body returned.
//! This publishes those existing canonical origin mappings from the shared
//! descriptor analysis. It derives a contract, never its loan or authority;
//! the checker reconstructs formation and completion itself.

use crate::{Unserved, refuse};
use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct, Refusal};
use terminal_psi::TerminalModule;
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorGraphError, ModuleSummaries, terminal_borrowed_storage_graph_from,
};

/// The nominal calls of every graph this stage builds with
/// `terminal_borrowed_storage_graph_from`: none. `ModuleSummaries` checks a
/// graph's nominal calls by address, so a table and the analyses sharing it
/// name this one slice.
static NO_NOMINAL_CALLS: [terminal_psi::generic_calls::NominalCallArgument; 0] = [];

/// One module's callee summaries for the per-machine analyses of graphs this
/// stage builds without nominal calls. Each analysis takes a summary only
/// where its own replay would have produced the same one, so each callee is
/// replayed once per module rather than once per analysis.
pub(crate) fn module_summaries(module: &TerminalModule) -> ModuleSummaries<'_> {
    ModuleSummaries::borrowed_storage(module, &NO_NOMINAL_CALLS)
}

/// A fresh analysis over `graph`, a closure built from `module` without
/// nominal calls, sharing `summaries` (`module_summaries` of that module).
pub(crate) fn shared_analysis<'a>(
    module: &'a TerminalModule,
    graph: &'a [terminal_semantics::descriptor_results::DescriptorMachine],
    summaries: &'a ModuleSummaries<'a>,
) -> DescriptorAnalysis<'a> {
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.types.structural_types, graph);
    analysis.share_summaries(summaries, &NO_NOMINAL_CALLS);
    analysis
}

fn graph_unavailable(module: &TerminalModule, error: DescriptorGraphError) -> Refusal {
    let mut refusal = refuse(
        error.machine(),
        error
            .operation(module)
            .map(Construct::Operation)
            .unwrap_or(Construct::Machine),
        Unserved::Call("executable borrowed-storage closure cannot be reconstructed"),
    );
    refusal.detail = format!("{}: {}", refusal.detail, error.in_module(module));
    refusal
}

pub(crate) fn publish(
    elaboration: &mut crate::Elaboration,
    selections: &[crate::Selected],
    parallel: bool,
) -> Checked<()> {
    let published = (|| {
        publish_results(
            &mut elaboration.module,
            selections,
            &elaboration.nominal_calls,
        )?;
        let module = &elaboration.module;
        let summaries = module_summaries(module);
        publish_retained_call_loans(
            module,
            &mut elaboration.tables,
            &mut elaboration.sources,
            &summaries,
            parallel,
        )?;
        let successor_loans =
            publish_successor_loans(module, &mut elaboration.tables, &mut elaboration.sources)?;
        publish_parents(
            module,
            &mut elaboration.tables,
            &elaboration.sources,
            &elaboration.names,
            &summaries,
            &successor_loans,
            parallel,
        )
    })();
    published.map_err(|mut refusal| {
        if refusal.source_span.is_none()
            && let terminal_check_tables::RefusalSite::Machine(
                machine,
                Construct::Operation(operation),
            ) = refusal.site
        {
            refusal.source_span = elaboration
                .sources
                .get(machine)
                .and_then(|source| source.operations.get(operation))
                .and_then(|origin| origin.span());
        }
        refusal
    })
}

/// A generic check form's requirement has no body whose returns could
/// supply its origins. Its declared lifetime groups stand for them, every
/// input of a leaf's lifetime a possible source (lifetimes.md, "Returned
/// views"), so a rigid call keeps each such input region borrowed. They go
/// on the requirement's contract, the same account a selected call reads
/// from its public header. Run before the bodies' own results are
/// reconstructed, which read this header.
pub(crate) fn publish_requirements(
    module: &mut TerminalModule,
    program: &crate::read::Program<'_>,
    requirements: &[MachineId],
) -> Checked<()> {
    let unavailable =
        |machine: MachineId, reason| refuse(machine, Construct::Machine, Unserved::Call(reason));
    for requirement in requirements {
        let requirement = *requirement;
        let machine = module
            .machine(requirement)
            .ok_or_else(|| unavailable(requirement, "requirement result lost its declaration"))?;
        if !machine.result.structural().is_some_and(|result| {
            terminal_psi::structural_type_contains_reference_by(
                |id| module.structural_type(id),
                result.structural_type,
            )
        }) {
            continue;
        }
        if !machine.blocks.is_empty() || machine.contract.result_lifetimes.is_some() {
            return Err(unavailable(
                requirement,
                "requirement result already has a lifetime account",
            ));
        }
        let signature = terminal_check_tables::row(
            &program.signatures,
            requirement,
            requirement,
            Construct::Machine,
        )?;
        let groups = program
            .declared_result_lifetimes(requirement, signature, &module.types.structural_types)
            .map_err(|reason| unavailable(requirement, reason))?;
        module
            .machines
            .iter_mut()
            .find(|machine| machine.id == requirement)
            .ok_or_else(|| unavailable(requirement, "requirement result declaration disappeared"))?
            .contract
            .result_lifetimes = groups;
    }
    Ok(())
}

/// A returned reference keeps only the storage its published origin names
/// borrowed: `pick(@mutable grid, i)` returning `@mutable grid.cells[i].x` retains
/// `grid.cells`, not `grid`. The call's own argument loan covers the whole
/// argument for the call and ends when it returns; a second `Loan` row,
/// formed at the same point, captures the narrower region and carries the
/// result's lifetime (lifetimes.md, "Returned views"). Mint that row here,
/// once results are published, so the checker can reconstruct the same
/// region from `reference_call_lifetimes` and claim it.
fn publish_retained_call_loans(
    module: &TerminalModule,
    tables: &mut terminal_check_tables::CheckTables,
    sources: &mut arena::IdTable<MachineId, terminal_check_tables::program::MachineSources>,
    summaries: &ModuleSummaries<'_>,
    parallel: bool,
) -> Checked<()> {
    use terminal_check_tables::ProgramPoint;
    use terminal_check_tables::obligations::{Lender, ObligationKind, ObligationRow};
    let contains_borrowed = |ty| {
        terminal_psi::structural_type_has_borrowed_storage_by(|id| module.structural_type(id), ty)
    };
    let returns_borrowed = |operation: &terminal_psi::Operation| {
        (operation.declared_invocation().is_some()
            || operation.kind.callable_invocation().is_some())
            && operation
                .kind
                .structural_arguments()
                .iter()
                .any(|argument| reference_access(argument.access).is_some())
            && operation
                .result
                .as_structural()
                .is_some_and(|result| contains_borrowed(result.structural_type))
    };
    let mut callers: Vec<_> = module
        .machines
        .iter()
        .filter(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(returns_borrowed)
        })
        .map(|machine| machine.id)
        .collect();
    callers.sort_unstable();
    if callers.is_empty() {
        return Ok(());
    }
    // A caller's retained regions come from its callees' actual returns, so
    // the one closure covers every body that returns a reference.
    let bodies = module.machines.iter().filter(|machine| {
        !machine.blocks.is_empty()
            && (callers.binary_search(&machine.id).is_ok()
                || machine
                    .result
                    .structural()
                    .is_some_and(|result| contains_borrowed(result.structural_type)))
    });
    let graph = terminal_borrowed_storage_graph_from(module, bodies.map(|machine| machine.id), &[])
        .map_err(|error| graph_unavailable(module, error))?;
    struct RetainedLoanInsert {
        captured_arg: terminal_psi::StructuralArgument,
        access: language_core::ReferenceAccess,
        lender: Lender,
        origin: terminal_check_tables::program::SourceOrigin,
        point: ProgramPoint,
    }
    let tables_ref = &tables.machines;
    let sources_ref = &*sources;
    let solve_caller =
        |machine: &terminal_psi::TerminalMachine| -> Checked<(MachineId, Vec<RetainedLoanInsert>)> {
            let unavailable =
                |reason| refuse(machine.id, Construct::Machine, Unserved::Call(reason));
            let mut analysis = shared_analysis(module, &graph, summaries);
            let flow = analysis
                .flow_arc(machine.id)
                .map_err(|error| graph_unavailable(module, error))?;
            let predecessors =
                terminal_semantics::descriptor_lifetimes::FlowPredecessors::new(&flow);
            let cutoff = terminal_semantics::reference_ends::dead_tail_cutoff(
                module,
                machine,
                &flow,
                &predecessors,
            );
            let tables = tables_ref
                .get(machine.id)
                .ok_or_else(|| unavailable("retained call loan lost its machine tables"))?;
            let loan_index = MachineLoanIndex::build(tables).map_err(unavailable)?;
            let sources = sources_ref
                .get(machine.id)
                .ok_or_else(|| unavailable("retained call loan lost its source coordinates"))?;
            let mut inserts = Vec::new();
            for block in &machine.blocks {
                let mut before = ProgramPoint::BlockEntry(block.id);
                for (position, operation) in block.operations.iter().enumerate() {
                    let point = before;
                    before = ProgramPoint::AfterOperation(operation.id);
                    if !returns_borrowed(operation) {
                        continue;
                    }
                    let pos_u32 = u32::try_from(position)
                        .map_err(|_| unavailable("retained call loan position overflows"))?;
                    let retained = terminal_semantics::reference_call_lifetimes_indexed(
                        module,
                        machine,
                        operation,
                        &mut analysis,
                        terminal_semantics::CallLifetimeIndex {
                            predecessors: &predecessors,
                            cutoff: Some(&cutoff),
                            at: (block.id, pos_u32),
                            include_surviving: false,
                        },
                    )
                    .map_err(|error| graph_unavailable(module, error))?;
                    let arguments = operation.kind.structural_arguments();
                    for retained in retained {
                        let argument = arguments.get(retained.position).ok_or_else(|| {
                            unavailable("retained call loan has no actual argument")
                        })?;
                        if retained.path == argument.path {
                            continue;
                        }
                        let access = reference_access(argument.access).ok_or_else(|| {
                            unavailable("retained call loan has no borrowed input")
                        })?;
                        // The argument's own row supplies the lender and source
                        // coordinates; the narrower row is formed by the same call.
                        let argument_loan = retained_loan_row_indexed(
                            module,
                            machine,
                            &loan_index,
                            point,
                            argument,
                            access,
                            &[],
                        )
                        .map_err(unavailable)?
                        .ok_or_else(|| {
                            unavailable("retained call loan has no argument occurrence row")
                        })?;
                        let Some(ObligationKind::Loan { lender, .. }) =
                            tables.obligations.get(argument_loan).map(|row| row.kind)
                        else {
                            return Err(unavailable("retained call loan lost its argument row"));
                        };
                        let origin =
                            sources
                                .obligations
                                .get(argument_loan)
                                .copied()
                                .ok_or_else(|| {
                                    unavailable("retained call loan has no source origin")
                                })?;
                        inserts.push(RetainedLoanInsert {
                            captured_arg: terminal_psi::StructuralArgument {
                                place: argument.place,
                                path: retained.path,
                                access: argument.access,
                            },
                            access,
                            lender,
                            origin,
                            point,
                        });
                    }
                }
            }
            Ok((machine.id, inserts))
        };
    let caller_machines: Vec<_> = module
        .machines
        .iter()
        .filter(|machine| callers.binary_search(&machine.id).is_ok())
        .collect();
    let per_caller: Vec<Checked<(MachineId, Vec<RetainedLoanInsert>)>> =
        if parallel && caller_machines.len() >= 2 {
            use rayon::prelude::*;
            caller_machines
                .into_par_iter()
                .with_max_len(1)
                .map(solve_caller)
                .collect()
        } else {
            caller_machines.into_iter().map(solve_caller).collect()
        };
    for result in per_caller {
        let (machine_id, inserts) = result?;
        if inserts.is_empty() {
            continue;
        }
        let unavailable = |reason| refuse(machine_id, Construct::Machine, Unserved::Call(reason));
        let machine = module
            .machine(machine_id)
            .ok_or_else(|| unavailable("retained call loan lost its machine"))?;
        let machine_tables = tables
            .machines
            .get_mut(machine_id)
            .ok_or_else(|| unavailable("retained call loan lost its machine tables"))?;
        let machine_sources = sources
            .get_mut(machine_id)
            .ok_or_else(|| unavailable("retained call loan lost its source coordinates"))?;
        for insert in inserts {
            let captured = capture_argument(
                machine,
                &module.types.structural_types,
                machine_tables,
                &insert.captured_arg,
            )?;
            let obligation = machine_tables.obligations.push(ObligationRow {
                kind: ObligationKind::Loan {
                    captured,
                    access: insert.access,
                    lender: insert.lender,
                },
                created_at: insert.point,
            });
            if machine_sources.obligations.push(insert.origin) != obligation {
                return Err(unavailable(
                    "retained call loan and source coordinates differ",
                ));
            }
        }
    }
    Ok(())
}

/// A real reborrow on a block edge keeps its exact formation occurrence.
/// Passing an unchanged permission continues it; borrowing owned storage or
/// reducing access creates a child loan whose lifetime follows the new name.
struct SuccessorLoan {
    machine: MachineId,
    obligation: semantic_vocabulary::ObligationId,
    birth: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
    target: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
    parameter: semantic_vocabulary::PlaceId,
}

fn publish_successor_loans(
    module: &TerminalModule,
    check_tables: &mut terminal_check_tables::CheckTables,
    all_sources: &mut arena::IdTable<MachineId, terminal_check_tables::program::MachineSources>,
) -> Checked<Vec<SuccessorLoan>> {
    use terminal_check_tables::ProgramPoint;
    use terminal_check_tables::obligations::{Lender, ObligationKind, ObligationRow};
    use terminal_psi::StructuralAccess;
    let mut published = Vec::new();
    for machine in &module.machines {
        let unavailable = |reason| refuse(machine.id, Construct::Machine, Unserved::Call(reason));
        let tables = check_tables
            .machines
            .get_mut(machine.id)
            .ok_or_else(|| unavailable("successor loan publication lost its machine tables"))?;
        let sources = all_sources
            .get_mut(machine.id)
            .ok_or_else(|| unavailable("successor loan publication lost its source coordinates"))?;
        for block in &machine.blocks {
            let successors = block.terminator.successor_edges();
            let arguments = block.terminator.successor_block_arguments();
            if successors.len() != arguments.len() {
                return Err(unavailable(
                    "successor loan publication has no complete edge roster",
                ));
            }
            for ((edge, target), arguments) in successors.into_iter().zip(arguments) {
                let head = machine.block(&target).ok_or_else(|| {
                    unavailable("successor loan publication lost its target header")
                })?;
                if arguments.len() != head.structural_parameters.len() {
                    return Err(unavailable(
                        "successor loan arguments differ from their target header",
                    ));
                }
                for (argument, parameter) in arguments.iter().zip(&head.structural_parameters) {
                    let Some(access) = reference_access(parameter.access) else {
                        continue;
                    };
                    let authority = terminal_semantics::structural_projection_authority(
                        machine,
                        &module.types.structural_types,
                        argument,
                    )
                    .ok_or_else(|| unavailable("successor loan has no actual source authority"))?;
                    // Equal access only continues the same permission while
                    // the projection remains in its storage. Selecting an
                    // element behind a descriptor creates a child loan even
                    // when both parent and child are shared (or mutable).
                    let root = machine
                        .structural_place_type(argument.place)
                        .ok_or_else(|| {
                            unavailable("successor loan has no original storage declaration")
                        })?;
                    if authority == parameter.access
                        && terminal_semantics::projection_stays_in_inline_storage(
                            module.types.structural_types.iter(),
                            root,
                            &argument.path,
                        )
                    {
                        continue;
                    }
                    let lender = if authority == StructuralAccess::Owned {
                        Lender::Owner
                    } else {
                        Lender::Incoming
                    };
                    let captured = capture_argument(
                        machine,
                        &module.types.structural_types,
                        tables,
                        argument,
                    )?;
                    let obligation = tables.obligations.push(ObligationRow {
                        kind: ObligationKind::Loan {
                            captured,
                            access,
                            lender,
                        },
                        created_at: ProgramPoint::Edge(edge),
                    });
                    let origin =
                        sources.edges.get(edge).copied().ok_or_else(|| {
                            unavailable("successor loan has no selected source edge")
                        })?;
                    let emitted = sources.obligations.push(origin);
                    if emitted != obligation {
                        return Err(unavailable("successor loan and source coordinates differ"));
                    }
                    let end = u32::try_from(block.operations.len()).map_err(|_| {
                        unavailable("successor loan formation position exceeds u32")
                    })?;
                    published.push(SuccessorLoan {
                        machine: machine.id,
                        obligation,
                        birth: (block.id, end),
                        target: (target, 0),
                        parameter: parameter.place,
                    });
                }
            }
        }
    }
    Ok(published)
}

/// Reuse the write log's place interner, preserving evaluated index values.
/// Resolving a field uses the shared structural path walker, never its text.
fn capture_argument(
    machine: &terminal_psi::TerminalMachine,
    types: &[terminal_psi::StructuralTypeDeclaration],
    tables: &mut terminal_check_tables::MachineTables,
    argument: &terminal_psi::StructuralArgument,
) -> Checked<terminal_check_tables::ExactPlaceId> {
    use terminal_check_tables::bindings::{Segment, intern_projection};
    use terminal_psi::StructuralPathSegment;
    let unavailable = |reason| refuse(machine.id, Construct::Machine, Unserved::Call(reason));
    let mut captured = tables
        .roots
        .get(argument.place)
        .copied()
        .ok_or_else(|| unavailable("successor loan has no exact storage root"))?;
    let root_type = machine
        .structural_place_type(argument.place)
        .ok_or_else(|| unavailable("successor loan has no source storage type"))?;
    for (position, step) in argument.path.iter().enumerate() {
        let segment = match step {
            StructuralPathSegment::Field(_) => Segment::Field(
                terminal_semantics::runtime_structural_field(
                    types.iter(),
                    root_type,
                    &argument.path[..=position],
                )
                .ok_or_else(|| unavailable("successor loan field has no exact declaration"))?
                .id,
            ),
            StructuralPathSegment::Case(case) => Segment::Case(*case),
            StructuralPathSegment::FixedIndex(index) => Segment::FixedIndex(*index),
            StructuralPathSegment::RuntimeIndex { index, .. } => {
                Segment::RuntimeIndex { selector: *index }
            }
            StructuralPathSegment::Referent => Segment::Referent,
            _ => {
                return Err(unavailable(
                    "successor loan does not name one captured storage place",
                ));
            }
        };
        captured = intern_projection(machine.id, &mut tables.places, captured, segment)?;
    }
    Ok(captured)
}

fn publish_results(
    module: &mut TerminalModule,
    selections: &[crate::Selected],
    nominal_calls: &[terminal_psi::generic_calls::NominalCallArgument],
) -> Checked<()> {
    let selected: Vec<_> = module
        .machines
        .iter()
        .filter_map(|machine| {
            // A bodiless requirement's header is its declared relation
            // (`publish_requirements`); there are no returns to reconstruct.
            if machine.blocks.is_empty() {
                return None;
            }
            machine
                .result
                .structural()
                .filter(|result| {
                    terminal_psi::structural_type_contains_reference_by(
                        |id| module.structural_type(id),
                        result.structural_type,
                    )
                })
                .map(|_| machine.id)
        })
        .collect();
    if selected.is_empty() {
        return Ok(());
    }
    let unavailable =
        |machine: MachineId, reason| refuse(machine, Construct::Machine, Unserved::Call(reason));
    // A selected body can return a narrower permission than its public
    // requirement promises. Calls retain the full declared lifetime union;
    // the body's actual origins remain independently reconstructed below.
    let mut selected_calls: Vec<_> = selections.iter().collect();
    selected_calls.sort_unstable_by_key(|selection| selection.implementation);
    for machine in &selected {
        let start = selected_calls.partition_point(|selection| selection.implementation < *machine);
        let end = selected_calls.partition_point(|selection| selection.implementation <= *machine);
        for selection in &selected_calls[start..end] {
            let required = selection.required_contract.ok_or_else(|| {
                unavailable(
                    selection.caller,
                    "selected reference result requires its public requirement contract",
                )
            })?;
            terminal_semantics::selected_reference_result(
                module,
                selection.caller,
                *machine,
                required,
            )
            .map_err(|reason| unavailable(selection.caller, reason))?;
        }
    }
    let graph =
        terminal_semantics::descriptor_results::terminal_borrowed_storage_graph_with_nominal_calls(
            module,
            selected.iter().copied(),
            &[],
            nominal_calls,
        )
        .map_err(|error| graph_unavailable(module, error))?;
    // A returned generic value can carry a loan from a view inside an input
    // record. Keep those views during replay, even though the return header
    // names only the references found by `reference_result_contents`.
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.types.structural_types, &graph);
    let mut published = Vec::with_capacity(selected.len());
    for id in selected {
        let machine = module
            .machine(id)
            .ok_or_else(|| unavailable(id, "reference result lost its actual machine"))?;
        let result = machine
            .result
            .structural()
            .ok_or_else(|| unavailable(id, "reference result lost its structural declaration"))?;
        let contents = analysis.result_contents(id).ok_or_else(|| {
            analysis
                .preparation_error(id)
                .map(|error| graph_unavailable(module, error))
                .unwrap_or_else(|| {
                    unavailable(
                        id,
                        "reference result has no complete actual return-source correspondence",
                    )
                })
        })?;
        let mappings = terminal_semantics::reference_result_contents(
            &module.types.structural_types,
            &machine.structural_parameters,
            result,
            &contents,
        )
        .map_err(|reason| unavailable(id, reason))?;
        let start = selected_calls.partition_point(|selection| selection.implementation < id);
        let end = selected_calls.partition_point(|selection| selection.implementation <= id);
        for selection in &selected_calls[start..end] {
            let required = selection.required_contract.ok_or_else(|| {
                unavailable(
                    selection.caller,
                    "selected reference result lost its public requirement contract",
                )
            })?;
            terminal_semantics::selected_reference_result(module, selection.caller, id, required)
                .and_then(|public| {
                    public.validate_actual_sources(&machine.structural_parameters, &mappings)
                })
                .map_err(|reason| unavailable(selection.caller, reason))?;
        }
        published.push((id, mappings));
    }
    // Commit only after every actual reference result is complete.
    for (id, mappings) in published {
        let machine = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == id)
            .ok_or_else(|| unavailable(id, "reference result producer disappeared"))?;
        let terminal_psi::TerminalMachineResult::Structural(result) = &mut machine.contract.result
        else {
            return Err(unavailable(
                id,
                "reference result changed before publication",
            ));
        };
        result.reference_sources = mappings;
    }
    Ok(())
}

struct IndexedLoan {
    id: semantic_vocabulary::ObligationId,
    access: language_core::ReferenceAccess,
    root: semantic_vocabulary::PlaceId,
    path: Vec<terminal_check_tables::bindings::Segment>,
}

/// Loans of one machine grouped by creation point so parent publication finds
/// a point's loans in O(log N) instead of rescanning the full obligation table.
struct MachineLoanIndex {
    rows: Vec<(terminal_check_tables::ProgramPoint, IndexedLoan)>,
}

impl MachineLoanIndex {
    fn build(tables: &terminal_check_tables::MachineTables) -> Result<Self, &'static str> {
        use terminal_check_tables::obligations::ObligationKind;
        let mut rows = Vec::new();
        for (id, row) in tables.obligations.iter() {
            let ObligationKind::Loan {
                captured, access, ..
            } = row.kind
            else {
                continue;
            };
            let (root, path) = captured_path(tables, captured)?;
            rows.push((
                row.created_at,
                IndexedLoan {
                    id,
                    access,
                    root,
                    path,
                },
            ));
        }
        rows.sort_by_key(|(point, _)| *point);
        Ok(Self { rows })
    }

    fn at(
        &self,
        point: terminal_check_tables::ProgramPoint,
    ) -> &[(terminal_check_tables::ProgramPoint, IndexedLoan)] {
        let start = self.rows.partition_point(|(at, _)| *at < point);
        let end = self.rows.partition_point(|(at, _)| *at <= point);
        &self.rows[start..end]
    }
}

/// Complete the existing lender column after all descriptor transports are
/// available. These rows describe actual source occurrences; 06 reconstructs
/// them before deciding any loan and grants no authority from this publication.
fn publish_parents(
    module: &TerminalModule,
    check_tables: &mut terminal_check_tables::CheckTables,
    sources_ref: &arena::IdTable<MachineId, terminal_check_tables::program::MachineSources>,
    names_ref: &arena::IdTable<MachineId, crate::MachineNames>,
    summaries: &ModuleSummaries<'_>,
    successor_loans: &[SuccessorLoan],
    parallel: bool,
) -> Checked<()> {
    use terminal_check_tables::ProgramPoint;
    use terminal_check_tables::obligations::{Lender, ObligationKind};
    use terminal_semantics::descriptor_lifetimes::{
        DescriptorLocation, FlowPredecessors, descriptor_location_lifetime_indexed,
    };
    let unavailable = |machine, reason| refuse(machine, Construct::Machine, Unserved::Call(reason));
    let mut queries = Vec::new();
    for machine in &module.machines {
        let tables = check_tables.machines.get(machine.id).ok_or_else(|| {
            unavailable(
                machine.id,
                "reference parent publication lost its machine tables",
            )
        })?;
        let demanded = tables
            .obligations
            .iter()
            .any(|(_, row)| matches!(row.kind, ObligationKind::Loan { .. }));
        if !demanded {
            continue;
        }
        let loan_index =
            MachineLoanIndex::build(tables).map_err(|reason| unavailable(machine.id, reason))?;
        let mut children = tables.obligations.map(|_, _| None);
        for (id, row) in tables.obligations.iter() {
            if let ObligationKind::Loan {
                captured, lender, ..
            } = row.kind
            {
                let (root, segments) = captured_path(tables, captured)
                    .map_err(|reason| unavailable(machine.id, reason))?;
                let root_type = machine.structural_place_type(root).ok_or_else(|| {
                    unavailable(machine.id, "loan parent has no source storage type")
                })?;
                let element =
                    terminal_semantics::descriptor_lifetimes::view_element_descriptor_prefix(
                        &module.types.structural_types,
                        root_type,
                        &segments,
                    )
                    .map_err(|reason| unavailable(machine.id, reason))?;
                if let Some(crossing) = element.or_else(|| {
                    segments.iter().rposition(|step| {
                        *step == terminal_check_tables::bindings::Segment::Referent
                    })
                }) {
                    let parent =
                        terminal_semantics::descriptor_lifetimes::DescriptorLocation::captured(
                            root,
                            &segments[..crossing],
                        )
                        .ok_or_else(|| {
                            unavailable(
                                machine.id,
                                "reference parent has no exact static descriptor path",
                            )
                        })?;
                    *children
                        .get_mut(id)
                        .ok_or_else(|| unavailable(machine.id, "parent child row disappeared"))? =
                        Some((row.created_at, parent));
                } else if lender == Lender::Incoming {
                    // An invocation input's parent belongs to its caller. A
                    // block input or new edge reborrow can instead descend
                    // from a loan formed inside this machine.
                    let declaration = usize::try_from(root.get())
                        .ok()
                        .and_then(|position| position.checked_sub(1))
                        .and_then(|position| machine.structural_places.get(position))
                        .filter(|place| place.id == root)
                        .ok_or_else(|| {
                            unavailable(machine.id, "loan parent has no actual storage declaration")
                        })?;
                    if !matches!(
                        declaration.kind,
                        semantic_vocabulary::StructuralPlaceKind::BlockParameter { .. }
                    ) && !matches!(row.created_at, ProgramPoint::Edge(_))
                    {
                        continue;
                    }
                    // The header carries the parent permission. The child
                    // still borrows its complete captured path, including any
                    // runtime index; that suffix does not name the parent's
                    // descriptor. The independent loan checker reconstructs
                    // this same header origin in `source_lender`.
                    let parent = DescriptorLocation::whole(root);
                    *children
                        .get_mut(id)
                        .ok_or_else(|| unavailable(machine.id, "parent child row disappeared"))? =
                        Some((row.created_at, parent));
                }
            }
        }
        // Reborrowing an existing descriptor needs its immediate parent even
        // when the machine only observes the new view locally.
        let mut view_reborrows = Vec::new();
        let mut publish_call_payloads = |arguments, formals, before| {
            publish_payload_children(
                module,
                machine,
                &loan_index,
                arguments,
                formals,
                before,
                &mut children,
            )
        };
        for block in &machine.blocks {
            let mut before = ProgramPoint::BlockEntry(block.id);
            for operation in &block.operations {
                if let terminal_psi::OperationKind::EstablishElementView { source, .. }
                | terminal_psi::OperationKind::EstablishMutableElementView { source, .. } =
                    &operation.kind
                    && let Some(root) = machine.structural_place_type(source.place)
                    && let Some(tip) = terminal_semantics::runtime_structural_path_tip(
                        module.types.structural_types.iter(),
                        root,
                        &source.path,
                    )
                    && terminal_semantics::descriptor_results::is_descriptor(
                        module.types.structural_types.iter(),
                        tip.id,
                    )
                {
                    view_reborrows.push(operation.id);
                }
                if let Some(call) = operation.declared_invocation() {
                    publish_call_payloads(
                        call.structural_arguments,
                        module
                            .invocation_structural_parameters(call.callee, call.required_contract),
                        before,
                    )?;
                } else if let Some(call) = operation.kind.callable_invocation() {
                    publish_call_payloads(
                        call.structural_arguments,
                        module
                            .execution
                            .callables
                            .contract(call.contract)
                            .map(|contract| contract.structural_parameters.as_slice()),
                        before,
                    )?;
                }
                before = ProgramPoint::AfterOperation(operation.id);
            }
            if let Some((_, call)) = block.terminator.declared_invocation() {
                publish_call_payloads(
                    call.structural_arguments,
                    module.invocation_structural_parameters(call.callee, call.required_contract),
                    before,
                )?;
            }
        }
        for successor in successor_loans
            .iter()
            .filter(|loan| loan.machine == machine.id)
        {
            let parameter = machine
                .block(&successor.target.0)
                .and_then(|block| {
                    block
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.place == successor.parameter)
                })
                .ok_or_else(|| {
                    unavailable(machine.id, "successor payload parent lost its header")
                })?;
            if terminal_semantics::borrowed_view_payload_parameter(
                parameter,
                module.types.structural_types.iter(),
            ) {
                let row = tables
                    .obligations
                    .get(successor.obligation)
                    .ok_or_else(|| unavailable(machine.id, "successor parent lost its loan"))?;
                let ObligationKind::Loan { captured, .. } = row.kind else {
                    unreachable!()
                };
                let (root, path) = captured_path(tables, captured)
                    .map_err(|reason| unavailable(machine.id, reason))?;
                let descriptor =
                    terminal_semantics::descriptor_lifetimes::DescriptorLocation::captured(
                        root, &path,
                    )
                    .ok_or_else(|| {
                        unavailable(
                            machine.id,
                            "successor payload parent has no exact descriptor",
                        )
                    })?;
                *children.get_mut(successor.obligation).ok_or_else(|| {
                    unavailable(machine.id, "successor parent child row disappeared")
                })? = Some((row.created_at, descriptor));
            }
        }
        if view_reborrows.is_empty() && children.iter().all(|(_, child)| child.is_none()) {
            continue;
        }
        view_reborrows.sort_unstable();
        queries.push((machine, loan_index, children, view_reborrows));
    }
    if queries.is_empty() {
        return Ok(());
    }
    let roots = queries.iter().map(|(machine, _, _, _)| machine.id);
    let graph = terminal_borrowed_storage_graph_from(module, roots, &[])
        .map_err(|error| graph_unavailable(module, error))?;
    let solve_machine = |query: (
        &terminal_psi::TerminalMachine,
        MachineLoanIndex,
        arena::IdTable<
            semantic_vocabulary::ObligationId,
            Option<(ProgramPoint, DescriptorLocation)>,
        >,
        Vec<semantic_vocabulary::OperationId>,
    )| {
        let (machine, loan_index, mut children, view_reborrows) = query;
        // Calls share one immutable graph and the module's callee summaries.
        // Saved storage states are needed only while finding the parents of
        // this machine's loans.
        let mut analysis = shared_analysis(module, &graph, summaries);
        let flow = analysis
            .flow_arc(machine.id)
            .map_err(|error| graph_unavailable(module, error))?;
        let predecessors = FlowPredecessors::new(&flow);
        let cutoff = terminal_semantics::reference_ends::dead_tail_cutoff(
            module,
            machine,
            &flow,
            &predecessors,
        );
        let mut op_positions = Vec::new();
        let mut parents = Vec::new();
        for block in &machine.blocks {
            let mut before = ProgramPoint::BlockEntry(block.id);
            for (position, operation) in block.operations.iter().enumerate() {
                let pos_u32 = u32::try_from(position).map_err(|_| {
                    unavailable(machine.id, "reference formation position exceeds its graph")
                })?;
                if let Some(after_pos) = pos_u32.checked_add(1) {
                    op_positions.push((operation.id, (block.id, after_pos)));
                }
                if let terminal_psi::OperationKind::EstablishReference { source, .. }
                | terminal_psi::OperationKind::EstablishElementView { source, .. }
                | terminal_psi::OperationKind::EstablishMutableElementView { source, .. } =
                    &operation.kind
                {
                    let result = operation.result.as_structural().ok_or_else(|| {
                        unavailable(
                            machine.id,
                            "reference formation has no actual descriptor result",
                        )
                    })?;
                    let ty = machine.structural_place_type(source.place).ok_or_else(|| {
                        unavailable(machine.id, "reference formation has no source storage type")
                    })?;
                    let source_path = terminal_semantics::resolved_structural_path(
                        module.types.structural_types.iter(),
                        ty,
                        &source.path,
                    )
                    .map_err(|reason| unavailable(machine.id, reason))?;
                    let expected_access = reference_access(source.access).ok_or_else(|| {
                        unavailable(machine.id, "reference formation has no borrowed access")
                    })?;
                    let mut matching = Vec::new();
                    for (_, loan) in loan_index.at(before) {
                        if loan.access == expected_access
                            && loan.root == source.place
                            && loan.path == source_path
                        {
                            matching.push(loan.id);
                        }
                    }
                    let [identity] = matching.as_slice() else {
                        return Err(unavailable(
                            machine.id,
                            "reference formation has no unique exact Loan row",
                        ));
                    };
                    if view_reborrows.binary_search(&operation.id).is_ok() {
                        let source_location = DescriptorLocation::captured(
                            source.place,
                            &source_path,
                        )
                        .ok_or_else(|| {
                            unavailable(machine.id, "view formation has no exact parent descriptor")
                        })?;
                        *children.get_mut(*identity).ok_or_else(|| {
                            unavailable(machine.id, "view formation lost its parent child row")
                        })? = Some((before, source_location));
                    }
                    let at = (block.id, pos_u32);
                    let lifetime = descriptor_location_lifetime_indexed(
                        &flow,
                        &predecessors,
                        Some(&cutoff),
                        at,
                        &DescriptorLocation::whole(result.place),
                    );
                    if lifetime.live.contains(&at) && !lifetime.reborrows_previous() {
                        let mut refused = refuse(
                            machine.id,
                            Construct::Operation(operation.id),
                            Unserved::Call(
                                "reference formation overlaps another activation of itself",
                            ),
                        );
                        refused.source_span = sources_ref
                            .get(machine.id)
                            .and_then(|source| source.operations.get(operation.id))
                            .and_then(|origin| origin.span());
                        return Err(refused);
                    }
                    parents.push((*identity, lifetime));
                }
                let retained = terminal_semantics::reference_call_lifetimes_indexed(
                    module,
                    machine,
                    operation,
                    &mut analysis,
                    terminal_semantics::CallLifetimeIndex {
                        predecessors: &predecessors,
                        cutoff: Some(&cutoff),
                        at: (block.id, pos_u32),
                        include_surviving: true,
                    },
                )
                .map_err(|error| graph_unavailable(module, error))?;
                if !retained.is_empty() {
                    let arguments = operation.kind.structural_arguments();
                    // Each argument's own row first, in argument order, then
                    // each narrower retained region's row
                    // (`publish_retained_call_loans`).
                    let mut claimed = Vec::new();
                    let mut own = Vec::with_capacity(arguments.len());
                    for source in arguments {
                        let Some(access) = reference_access(source.access) else {
                            own.push(None);
                            continue;
                        };
                        let id = retained_loan_row_indexed(
                            module,
                            machine,
                            &loan_index,
                            before,
                            source,
                            access,
                            &claimed,
                        )
                        .map_err(|reason| unavailable(machine.id, reason))?
                        .ok_or_else(|| {
                            unavailable(
                                machine.id,
                                "retained call loan has no exact occurrence row",
                            )
                        })?;
                        claimed.push(id);
                        own.push(Some(id));
                    }
                    for retained in retained {
                        let argument = arguments.get(retained.position).ok_or_else(|| {
                            unavailable(machine.id, "retained call loan lost its argument")
                        })?;
                        let id = if retained.path == argument.path {
                            own.get(retained.position).copied().flatten()
                        } else {
                            let access = reference_access(argument.access).ok_or_else(|| {
                                unavailable(machine.id, "retained call loan has no borrowed input")
                            })?;
                            let region = terminal_psi::StructuralArgument {
                                place: argument.place,
                                path: retained.path,
                                access: argument.access,
                            };
                            let id = retained_loan_row_indexed(
                                module,
                                machine,
                                &loan_index,
                                before,
                                &region,
                                access,
                                &claimed,
                            )
                            .map_err(|reason| unavailable(machine.id, reason))?;
                            claimed.extend(id);
                            id
                        }
                        .ok_or_else(|| {
                            unavailable(
                                machine.id,
                                "returned reference lifetime has no borrowed call occurrence",
                            )
                        })?;
                        parents.push((id, retained.lifetime));
                    }
                }
                before = ProgramPoint::AfterOperation(operation.id);
            }
        }
        op_positions.sort_unstable_by_key(|(id, _)| *id);
        for successor in successor_loans
            .iter()
            .filter(|loan| loan.machine == machine.id)
        {
            let lifetime =
                terminal_semantics::descriptor_lifetimes::descriptor_successor_lifetime_indexed(
                    &flow,
                    &predecessors,
                    Some(&cutoff),
                    successor.birth,
                    successor.target,
                    &DescriptorLocation::whole(successor.parameter),
                )
                .ok_or_else(|| {
                    unavailable(
                        machine.id,
                        "successor loan has no complete transported lifetime",
                    )
                })?;
            parents.push((successor.obligation, lifetime));
        }
        let mut parent_positions: Vec<(
            terminal_semantics::descriptor_lifetimes::DescriptorPosition,
            usize,
        )> = Vec::new();
        for (idx, (_, life)) in parents.iter().enumerate() {
            if life
                .authorized
                .values()
                .all(std::collections::BTreeSet::is_empty)
            {
                continue;
            }
            for &pos in life.authorized.keys().chain(life.surviving.keys()) {
                parent_positions.push((pos, idx));
            }
        }
        parent_positions.sort_unstable();
        parent_positions.dedup();
        let mut local_updates = Vec::new();
        for (child, parent) in children.iter() {
            let Some((point, descriptor)) = parent else {
                continue;
            };
            let at =
                formation_position_indexed(machine, &op_positions, *point).ok_or_else(|| {
                    unavailable(
                        machine.id,
                        "reference reborrow has no exact formation point",
                    )
                })?;
            let start = parent_positions.partition_point(|(pos, _)| *pos < at);
            let end = parent_positions.partition_point(|(pos, _)| *pos <= at);
            use terminal_semantics::{BorrowedLoanSource, BorrowedLoanSources};
            let lender = terminal_semantics::borrowed_loan_sources(
                module,
                machine,
                &mut analysis,
                at,
                descriptor,
                parent_positions[start..end]
                    .iter()
                    .map(|&(_, idx)| &parents[idx])
                    .filter(|(id, _)| *id != child)
                    .map(|(id, life)| (*id, life)),
            )
            .and_then(|sources| match sources {
                BorrowedLoanSources::Selected(_) => Ok(Lender::Selected),
                other => Ok(match other.single()? {
                    BorrowedLoanSource::Owner => Lender::Owner,
                    BorrowedLoanSource::Incoming => Lender::Incoming,
                    BorrowedLoanSource::Loan(parent) => Lender::Loan(parent),
                    BorrowedLoanSource::Provider => Lender::Provider,
                }),
            })
            .map_err(|reason| {
                let mut refusal = refuse(
                    machine.id,
                    Construct::Obligation(child),
                    Unserved::Call(reason),
                );
                refusal.source_span = sources_ref
                    .get(machine.id)
                    .and_then(|source| source.obligations.get(child))
                    .and_then(|origin| origin.span());
                if refusal.source_span.is_none()
                    && let Some(name) = names_ref.get(machine.id)
                {
                    refusal.detail = format!(
                        "{} in machine `{}` at block {} position {}",
                        refusal.detail,
                        name.machine,
                        at.0.get(),
                        at.1,
                    );
                }
                refusal
            })?;
            local_updates.push((machine.id, child, lender));
        }
        Ok(local_updates)
    };
    let per_machine: Vec<Checked<Vec<_>>> = if !parallel || queries.len() < 2 {
        queries.into_iter().map(solve_machine).collect()
    } else {
        use rayon::prelude::*;
        queries
            .into_par_iter()
            .with_max_len(1)
            .map(solve_machine)
            .collect()
    };
    let mut updates = Vec::new();
    for machine_updates in per_machine {
        updates.extend(machine_updates?);
    }
    for (machine, child, lender) in updates {
        let row = check_tables
            .machines
            .get_mut(machine)
            .and_then(|tables| tables.obligations.get_mut(child))
            .ok_or_else(|| {
                unavailable(
                    machine,
                    "reference parent publication lost its exact Loan row",
                )
            })?;
        let ObligationKind::Loan { lender: slot, .. } = &mut row.kind else {
            return Err(unavailable(
                machine,
                "reference parent publication changed obligation kind",
            ));
        };
        *slot = lender;
    }
    Ok(())
}

/// The unclaimed `Loan` row formed at `before` that captures exactly
/// `source` with `access`, looked up through the pre-indexed loan table.
fn retained_loan_row_indexed(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    loan_index: &MachineLoanIndex,
    before: terminal_check_tables::ProgramPoint,
    source: &terminal_psi::StructuralArgument,
    access: language_core::ReferenceAccess,
    claimed: &[semantic_vocabulary::ObligationId],
) -> Result<Option<semantic_vocabulary::ObligationId>, &'static str> {
    let ty = machine
        .structural_place_type(source.place)
        .ok_or("retained call loan has no source type")?;
    let source_path = terminal_semantics::resolved_structural_path(
        module.types.structural_types.iter(),
        ty,
        &source.path,
    )?;
    for (_, loan) in loan_index.at(before) {
        if loan.access == access
            && !claimed.contains(&loan.id)
            && loan.root == source.place
            && loan.path == source_path
        {
            return Ok(Some(loan.id));
        }
    }
    Ok(None)
}

/// A view formal borrows payload permission. A reference formal instead
/// borrows the descriptor's storage and keeps its ordinary storage parent.
fn publish_payload_children(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    loan_index: &MachineLoanIndex,
    arguments: &[terminal_psi::StructuralArgument],
    formals: Option<&[terminal_psi::StructuralParameterDeclaration]>,
    before: terminal_check_tables::ProgramPoint,
    children: &mut arena::IdTable<
        semantic_vocabulary::ObligationId,
        Option<(
            terminal_check_tables::ProgramPoint,
            terminal_semantics::descriptor_lifetimes::DescriptorLocation,
        )>,
    >,
) -> Checked<()> {
    use terminal_semantics::descriptor_lifetimes::descriptor_location;
    let unavailable = |reason| refuse(machine.id, Construct::Machine, Unserved::Call(reason));
    let formals =
        formals.ok_or_else(|| unavailable("payload loan has no exact callable contract"))?;
    if formals.len() != arguments.len() {
        return Err(unavailable(
            "payload loan arguments differ from callable formals",
        ));
    }
    let mut claimed = Vec::new();
    for (argument, formal) in arguments.iter().zip(formals) {
        let Some(access) = reference_access(argument.access) else {
            continue;
        };
        if !terminal_semantics::borrowed_view_payload_parameter(
            formal,
            module.types.structural_types.iter(),
        ) {
            continue;
        }
        let ty = machine
            .structural_place_type(argument.place)
            .ok_or_else(|| unavailable("payload loan has no source type"))?;
        let location = descriptor_location(
            &module.types.structural_types,
            argument.place,
            ty,
            &argument.path,
            None,
        )
        .ok_or_else(|| unavailable("payload loan has no exact source descriptor"))?;
        let mut found = None;
        for (_, loan) in loan_index.at(before) {
            if loan.access != access || claimed.contains(&loan.id) {
                continue;
            }
            if terminal_semantics::descriptor_lifetimes::DescriptorLocation::captured(
                loan.root, &loan.path,
            )
            .as_ref()
                == Some(&location)
            {
                found = Some(loan.id);
                break;
            }
        }
        let id =
            found.ok_or_else(|| unavailable("payload call loan has no exact occurrence row"))?;
        claimed.push(id);
        *children
            .get_mut(id)
            .ok_or_else(|| unavailable("payload child row disappeared"))? =
            Some((before, location));
    }
    Ok(())
}

fn reference_access(
    access: terminal_psi::StructuralAccess,
) -> Option<language_core::ReferenceAccess> {
    Some(match access {
        terminal_psi::StructuralAccess::SharedBorrow => language_core::ReferenceAccess::Shared,
        terminal_psi::StructuralAccess::MutableBorrow => language_core::ReferenceAccess::Mutable,
        terminal_psi::StructuralAccess::WriteOnlyBorrow => {
            language_core::ReferenceAccess::WriteOnly
        }
        terminal_psi::StructuralAccess::Owned => return None,
    })
}

fn captured_path(
    tables: &terminal_check_tables::MachineTables,
    mut place: terminal_check_tables::ExactPlaceId,
) -> Result<
    (
        semantic_vocabulary::PlaceId,
        Vec<terminal_check_tables::bindings::Segment>,
    ),
    &'static str,
> {
    use terminal_check_tables::bindings::PlaceOrigin;
    let mut path = Vec::new();
    for _ in 0..tables.places.len() {
        let row = tables
            .places
            .get(place)
            .ok_or("reference capture has no live place row")?;
        match row.origin {
            PlaceOrigin::Root(root) => {
                path.reverse();
                return Ok((root, path));
            }
            PlaceOrigin::Projection { parent, segment } => {
                path.push(segment);
                place = parent;
            }
        }
    }
    Err("reference capture does not reach its exact storage root")
}

fn formation_position_indexed(
    machine: &terminal_psi::TerminalMachine,
    op_positions: &[(
        semantic_vocabulary::OperationId,
        terminal_semantics::descriptor_lifetimes::DescriptorPosition,
    )],
    point: terminal_check_tables::ProgramPoint,
) -> Option<terminal_semantics::descriptor_lifetimes::DescriptorPosition> {
    match point {
        // Cleanup invokes a Unit consumer; it is not a source descriptor
        // formation. Its ordered state belongs to invocation replay.
        terminal_check_tables::ProgramPoint::BeforeCleanup { .. }
        | terminal_check_tables::ProgramPoint::AfterCleanup { .. } => None,
        terminal_check_tables::ProgramPoint::BlockEntry(block) => Some((block, 0)),
        terminal_check_tables::ProgramPoint::AfterOperation(operation) => op_positions
            .binary_search_by_key(&operation, |(id, _)| *id)
            .ok()
            .map(|pos| op_positions[pos].1),
        terminal_check_tables::ProgramPoint::Edge(edge) => {
            machine.blocks.iter().find_map(|block| {
                block
                    .terminator
                    .successor_edges()
                    .iter()
                    .any(|(candidate, _)| *candidate == edge)
                    .then(|| {
                        u32::try_from(block.operations.len())
                            .ok()
                            .map(|position| (block.id, position))
                    })
                    .flatten()
            })
        }
    }
}
