//! A returned permission keeps the actual input storage its body returned.
//! Publish those existing canonical origin mappings from the shared descriptor
//! analysis; this derives a contract, never establishes its loan or authority.
//! The checker must independently reconstruct formation and completion.

use crate::{Unserved, refuse};
use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct, Refusal};
use terminal_psi::TerminalModule;
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorGraphError, terminal_borrowed_storage_graph_from,
};

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
) -> Checked<()> {
    publish_results(&mut elaboration.module, selections)?;
    let successor_loans = publish_successor_loans(elaboration)?;
    publish_parents(elaboration, &successor_loans)
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
            .declared_result_lifetimes(requirement, signature, &module.structural_types)
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

fn publish_successor_loans(elaboration: &mut crate::Elaboration) -> Checked<Vec<SuccessorLoan>> {
    use terminal_check_tables::ProgramPoint;
    use terminal_check_tables::obligations::{Lender, ObligationKind, ObligationRow};
    use terminal_psi::StructuralAccess;
    let mut published = Vec::new();
    for machine in &elaboration.module.machines {
        let unavailable = |reason| refuse(machine.id, Construct::Machine, Unserved::Call(reason));
        let tables = elaboration
            .tables
            .machines
            .get_mut(machine.id)
            .ok_or_else(|| unavailable("successor loan publication lost its machine tables"))?;
        let sources = elaboration
            .sources
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
                        &elaboration.module.structural_types,
                        argument,
                    )
                    .ok_or_else(|| unavailable("successor loan has no actual source authority"))?;
                    if authority == parameter.access {
                        continue;
                    }
                    let lender = if authority == StructuralAccess::Owned {
                        Lender::Owner
                    } else {
                        Lender::Incoming
                    };
                    let captured = capture_argument(
                        machine,
                        &elaboration.module.structural_types,
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

fn publish_results(module: &mut TerminalModule, selections: &[crate::Selected]) -> Checked<()> {
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
    let graph = terminal_borrowed_storage_graph_from(module, selected.iter().copied(), &[])
        .map_err(|error| graph_unavailable(module, error))?;
    let mut analysis = DescriptorAnalysis::with_reference_slots(&module.structural_types, &graph);
    let mut published = Vec::with_capacity(selected.len());
    for id in selected {
        let machine = module
            .machine(id)
            .ok_or_else(|| unavailable(id, "reference result lost its actual machine"))?;
        let result = machine
            .result
            .structural()
            .ok_or_else(|| unavailable(id, "reference result lost its structural declaration"))?;
        let slots = analysis.result_slots(id).ok_or_else(|| {
            unavailable(
                id,
                "reference result has no complete actual return-source correspondence",
            )
        })?;
        let mappings = terminal_semantics::reference_result_sources(
            &module.structural_types,
            &machine.structural_parameters,
            result,
            &slots,
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

/// Complete the existing lender column after all descriptor transports are
/// available. These rows describe actual source occurrences; 06 reconstructs
/// them before deciding any loan and grants no authority from this publication.
fn publish_parents(
    elaboration: &mut crate::Elaboration,
    successor_loans: &[SuccessorLoan],
) -> Checked<()> {
    use terminal_check_tables::ProgramPoint;
    use terminal_check_tables::obligations::{Lender, ObligationKind};
    use terminal_semantics::descriptor_lifetimes::{
        DescriptorLocation, descriptor_location, descriptor_location_lifetime,
    };
    let module = &elaboration.module;
    let unavailable = |machine, reason| refuse(machine, Construct::Machine, Unserved::Call(reason));
    let mut updates = Vec::new();
    for machine in &module.machines {
        let tables = elaboration.tables.machines.get(machine.id).ok_or_else(|| {
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
        let mut children = tables.obligations.map(|_, _| None);
        for (id, row) in tables.obligations.iter() {
            if let ObligationKind::Loan {
                captured, lender, ..
            } = row.kind
            {
                let (root, segments) = captured_path(tables, captured)
                    .map_err(|reason| unavailable(machine.id, reason))?;
                if let Some(crossing) = segments
                    .iter()
                    .rposition(|step| *step == terminal_check_tables::bindings::Segment::Referent)
                {
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
                    let parent =
                        terminal_semantics::descriptor_lifetimes::DescriptorLocation::captured(
                            root, &segments,
                        )
                        .ok_or_else(|| {
                            unavailable(
                                machine.id,
                                "borrowed header parent has no exact descriptor path",
                            )
                        })?;
                    *children
                        .get_mut(id)
                        .ok_or_else(|| unavailable(machine.id, "parent child row disappeared"))? =
                        Some((row.created_at, parent));
                }
            }
        }
        for block in &machine.blocks {
            let mut before = ProgramPoint::BlockEntry(block.id);
            for operation in &block.operations {
                if let Some(call) = operation.declared_invocation() {
                    publish_payload_children(module, machine, tables, call, before, &mut children)?;
                }
                before = ProgramPoint::AfterOperation(operation.id);
            }
            if let Some((_, call)) = block.terminator.declared_invocation() {
                publish_payload_children(module, machine, tables, call, before, &mut children)?;
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
            if terminal_semantics::descriptor_results::is_descriptor(
                module.structural_types.iter(),
                parameter.structural_type,
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
        if children.iter().all(|(_, child)| child.is_none()) {
            continue;
        }
        let graph = terminal_borrowed_storage_graph_from(module, [machine.id], &[])
            .map_err(|error| graph_unavailable(module, error))?;
        let mut analysis =
            DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
        let flow = analysis.flow(machine.id).map_err(|error| {
            unavailable(
                error.machine(),
                error
                    .detail()
                    .unwrap_or("reference parent has no complete descriptor flow"),
            )
        })?;
        let mut parents = Vec::new();
        for block in &machine.blocks {
            let mut before = ProgramPoint::BlockEntry(block.id);
            for (position, operation) in block.operations.iter().enumerate() {
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
                    let source_location = descriptor_location(
                        &module.structural_types,
                        source.place,
                        ty,
                        &source.path,
                        None,
                    )
                    .ok_or_else(|| {
                        unavailable(machine.id, "reference formation has no exact source place")
                    })?;
                    let mut matching = Vec::new();
                    for (id, row) in tables.obligations.iter() {
                        let ObligationKind::Loan {
                            captured, access, ..
                        } = row.kind
                        else {
                            continue;
                        };
                        if row.created_at != before
                            || !access.forms(reference_access(source.access).ok_or_else(|| {
                                unavailable(
                                    machine.id,
                                    "reference formation has no borrowed access",
                                )
                            })?)
                        {
                            continue;
                        }
                        let (root, path) = captured_path(tables, captured)
                            .map_err(|reason| unavailable(machine.id, reason))?;
                        if DescriptorLocation::captured(root, &path).as_ref()
                            == Some(&source_location)
                        {
                            matching.push(id);
                        }
                    }
                    let [identity] = matching.as_slice() else {
                        return Err(unavailable(
                            machine.id,
                            "reference formation has no unique exact Loan row",
                        ));
                    };
                    let position = u32::try_from(position).map_err(|_| {
                        unavailable(machine.id, "reference formation position exceeds its graph")
                    })?;
                    let at = (block.id, position);
                    let lifetime = descriptor_location_lifetime(
                        &flow,
                        at,
                        &DescriptorLocation::whole(result.place),
                    );
                    if lifetime.live.contains(&at) {
                        return Err(unavailable(
                            machine.id,
                            "reference formation overlaps another activation of itself",
                        ));
                    }
                    parents.push((*identity, lifetime));
                }
                let retained = terminal_semantics::reference_call_lifetimes(
                    module,
                    machine,
                    operation,
                    &mut analysis,
                )
                .map_err(|reason| unavailable(machine.id, reason))?;
                if !retained.is_empty() {
                    let call = operation.kind.declared_invocation().ok_or_else(|| {
                        unavailable(machine.id, "retained reference lost its actual call")
                    })?;
                    let mut claimed = Vec::new();
                    let mut retained = retained.into_iter().peekable();
                    for (position, source) in call.structural_arguments.iter().enumerate() {
                        let Some(access) = reference_access(source.access) else {
                            continue;
                        };
                        let ty = machine.structural_place_type(source.place).ok_or_else(|| {
                            unavailable(machine.id, "retained call loan has no source type")
                        })?;
                        let location = descriptor_location(
                            &module.structural_types,
                            source.place,
                            ty,
                            &source.path,
                            None,
                        )
                        .ok_or_else(|| {
                            unavailable(machine.id, "retained call loan has no exact source")
                        })?;
                        let mut found = None;
                        for (id, row) in tables.obligations.iter() {
                            let ObligationKind::Loan {
                                captured,
                                access: actual,
                                ..
                            } = row.kind
                            else {
                                continue;
                            };
                            if row.created_at != before
                                || !actual.forms(access)
                                || claimed.contains(&id)
                            {
                                continue;
                            }
                            let (root, path) = captured_path(tables, captured)
                                .map_err(|reason| unavailable(machine.id, reason))?;
                            if DescriptorLocation::captured(root, &path).as_ref() == Some(&location)
                            {
                                found = Some(id);
                                break;
                            }
                        }
                        let id = found.ok_or_else(|| {
                            unavailable(
                                machine.id,
                                "retained call loan has no exact occurrence row",
                            )
                        })?;
                        claimed.push(id);
                        if retained.peek().is_some_and(|(input, _)| *input == position) {
                            let (_, lifetime) = retained.next().ok_or_else(|| {
                                unavailable(machine.id, "retained call loan lifetime disappeared")
                            })?;
                            parents.push((id, lifetime));
                        }
                    }
                    if retained.next().is_some() {
                        return Err(unavailable(
                            machine.id,
                            "returned reference lifetime has no borrowed call occurrence",
                        ));
                    }
                }
                before = ProgramPoint::AfterOperation(operation.id);
            }
        }
        for successor in successor_loans
            .iter()
            .filter(|loan| loan.machine == machine.id)
        {
            let lifetime = terminal_semantics::descriptor_lifetimes::descriptor_successor_lifetime(
                &flow,
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
        for (child, parent) in children.iter() {
            let Some((point, descriptor)) = parent else {
                continue;
            };
            let at = formation_position(machine, *point).ok_or_else(|| {
                unavailable(
                    machine.id,
                    "reference reborrow has no exact formation point",
                )
            })?;
            let parent = terminal_semantics::borrowed_loan_source(
                module,
                machine,
                &mut analysis,
                at,
                descriptor,
                parents
                    .iter()
                    .filter(|(id, _)| *id != child)
                    .map(|(id, life)| (*id, life)),
            )
            .map_err(|reason| unavailable(machine.id, reason))?;
            updates.push((
                machine.id,
                child,
                match parent {
                    terminal_semantics::BorrowedLoanSource::Owner => Lender::Owner,
                    terminal_semantics::BorrowedLoanSource::Incoming => Lender::Incoming,
                    terminal_semantics::BorrowedLoanSource::Loan(parent) => Lender::Loan(parent),
                    terminal_semantics::BorrowedLoanSource::Provider => Lender::Provider,
                },
            ));
        }
    }
    for (machine, child, lender) in updates {
        let row = elaboration
            .tables
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

/// A view formal borrows payload permission. A reference formal instead
/// borrows the descriptor's storage and keeps its ordinary storage parent.
fn publish_payload_children(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    tables: &terminal_check_tables::MachineTables,
    call: terminal_psi::ModuleInvocationInputs<'_, semantic_vocabulary::TargetMachineCallable>,
    before: terminal_check_tables::ProgramPoint,
    children: &mut semantic_vocabulary::IdTable<
        semantic_vocabulary::ObligationId,
        Option<(
            terminal_check_tables::ProgramPoint,
            terminal_semantics::descriptor_lifetimes::DescriptorLocation,
        )>,
    >,
) -> Checked<()> {
    use terminal_check_tables::obligations::ObligationKind;
    use terminal_semantics::descriptor_lifetimes::descriptor_location;
    let unavailable = |reason| refuse(machine.id, Construct::Machine, Unserved::Call(reason));
    let formals = module
        .invocation_structural_parameters(call.callee, call.required_contract)
        .ok_or_else(|| unavailable("payload loan has no exact callable contract"))?;
    if formals.len() != call.structural_arguments.len() {
        return Err(unavailable(
            "payload loan arguments differ from callable formals",
        ));
    }
    let mut claimed = Vec::new();
    for (argument, formal) in call.structural_arguments.iter().zip(formals) {
        let Some(access) = reference_access(argument.access) else {
            continue;
        };
        if !terminal_semantics::descriptor_results::is_descriptor(
            module.structural_types.iter(),
            formal.structural_type,
        ) {
            continue;
        }
        let ty = machine
            .structural_place_type(argument.place)
            .ok_or_else(|| unavailable("payload loan has no source type"))?;
        let location = descriptor_location(
            &module.structural_types,
            argument.place,
            ty,
            &argument.path,
            None,
        )
        .ok_or_else(|| unavailable("payload loan has no exact source descriptor"))?;
        let mut found = None;
        for (id, row) in tables.obligations.iter() {
            let ObligationKind::Loan {
                captured,
                access: actual,
                ..
            } = row.kind
            else {
                continue;
            };
            if row.created_at != before || actual != access || claimed.contains(&id) {
                continue;
            }
            let (root, path) = captured_path(tables, captured).map_err(unavailable)?;
            if terminal_semantics::descriptor_lifetimes::DescriptorLocation::captured(root, &path)
                .as_ref()
                == Some(&location)
            {
                found = Some(id);
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

fn formation_position(
    machine: &terminal_psi::TerminalMachine,
    point: terminal_check_tables::ProgramPoint,
) -> Option<terminal_semantics::descriptor_lifetimes::DescriptorPosition> {
    match point {
        terminal_check_tables::ProgramPoint::BlockEntry(block) => Some((block, 0)),
        terminal_check_tables::ProgramPoint::AfterOperation(operation) => {
            machine.blocks.iter().find_map(|block| {
                block
                    .operations
                    .iter()
                    .position(|row| row.id == operation)
                    .and_then(|position| u32::try_from(position + 1).ok())
                    .map(|position| (block.id, position))
            })
        }
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
