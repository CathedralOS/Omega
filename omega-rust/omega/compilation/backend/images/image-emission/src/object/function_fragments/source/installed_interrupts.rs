//! Check installation guards, live storage and publication against the program.
//! This audit runs at explicit artifact admission. It neither re-emits Scope
//! nor grants authority from instruction names or descriptive memory records.
use super::Error;
use installation_evidence::InstalledInterruptStackRequirement;
use provider_admission::AdmittedInstalledInterruptCohort;
use selected_instructions::{
    SelectedBlockId, SelectedFunction, SelectedImageAddressReference, SelectedInstructionId,
};
use semantic_vocabulary::OperationId;
use target_operations::{CallSiteOwner, InstallationCallRole, TargetFunction, TargetUnitOperation};
mod control;
#[cfg(test)]
mod control_tests;
mod expressions;
mod guards;
mod inputs;
mod instructions;
mod pairs;
mod publication;
mod refusal_result;
mod table;
mod writes;
use control::Graph;
use expressions::{Binary, Expr, Word};
use guards::{Contract, EntryInput};
use inputs::Inputs;
use writes::Writes;

fn unique<'a, T>(
    mut rows: impl Iterator<Item = &'a T>,
    reason: &'static str,
) -> Result<&'a T, Error> {
    let first = rows.next().ok_or(Error::Mismatch(reason))?;
    if rows.next().is_some() {
        return Err(Error::Mismatch(reason));
    }
    Ok(first)
}
fn add(left: Expr, right: Expr) -> Expr {
    Expr::binary(Binary::Add, left, right)
}
fn equal(left: Expr, right: Expr) -> Expr {
    Expr::binary(Binary::Equal, left, right)
}

pub(super) fn validate(
    module: &terminal_psi::TerminalModule,
    cohort: &AdmittedInstalledInterruptCohort,
    requirements: &[InstalledInterruptStackRequirement],
    native: target::NativeTarget,
    targeted: &TargetFunction,
    selected: &SelectedFunction,
) -> Result<Vec<OperationId>, Error> {
    let mut completed = Vec::new();
    for operation in targeted.graph.operations() {
        let TargetUnitOperation::InstallStaticInterrupt {
            psi_operation,
            boundary,
            arguments,
            result_home,
            ..
        } = operation
        else {
            continue;
        };
        let scope = unique(
            cohort
                .scopes()
                .iter()
                .filter(|row| row.operation() == *psi_operation),
            "installation has no unique admitted Scope",
        )?;
        if scope.boundary() != *boundary || scope.arguments() != arguments {
            return Err(Error::Mismatch(
                "installation substituted its admitted boundary or operands",
            ));
        }
        let input = Inputs::derive(targeted, selected, arguments)?;
        let anchor = scope.context_anchor().ok_or(Error::Mismatch(
            "installation has no exact acknowledgement context anchor",
        ))?;
        let mut entries: Vec<_> = cohort
            .entries()
            .iter()
            .filter(|row| row.operation == *psi_operation)
            .collect();
        entries.sort_unstable_by_key(|entry| entry.placement.application_index);
        if entries.len() != scope.fatal_entries().len() + 1
            || anchor.application() != scope.acknowledgement_application()
            || anchor.table_bytes() != input.layout.table_bytes
        {
            return Err(Error::Mismatch(
                "installation lost its complete admitted roster or anchor extent",
            ));
        }
        let mut retained: Vec<EntryInput<'_>> = Vec::with_capacity(entries.len());
        let mut materializations = Vec::with_capacity(entries.len());
        let mut acknowledgement = None;
        for (ordinal, entry) in entries.iter().enumerate() {
            let requirement = unique(
                requirements.iter().filter(|row| {
                    row.operation == *psi_operation && row.placement_index == entry.placement_index
                }),
                "installation lost its exact measured stack requirement",
            )?;
            let callback = module
                .callback_application(entry.placement.application_index)
                .ok_or(Error::Mismatch(
                    "installation measured entry lost its callback application",
                ))?;
            let body = unique(
                module
                    .machines
                    .iter()
                    .filter(|body| body.id == requirement.entry),
                "installation measured entry has no unique Terminal body",
            )?;
            if !body.declaration.as_ref().is_some_and(|declaration| {
                declaration.same_identity_as(&module.identities, &callback.body, &module.identities)
            }) || callback.registrar != *psi_operation
                || callback.body.contract != entry.placement.contract_commitment
                || !callback.entry_admitted
                || entry.placement.boundary_entry_plan.call.entry_control
                    != calling_conventions::EntryControl::InterruptReturn
                || requirement.entry != entry.entry().machine()
                || requirement.psi != cohort.psi()
                || requirement.psi != entry.entry().psi()
                || requirement.target != native
                || requirement.target != entry.entry().target()
                || !requirement.entry_plan.has_canonical_geometry()
                || requirement.required_bytes().is_none_or(|bytes| {
                    bytes < u64::from(requirement.entry_plan.root_slot_from_top)
                })
                || requirement.alignment < 16
                || !requirement.alignment.is_power_of_two()
            {
                return Err(Error::Mismatch(
                    "installation measured stack has a different body, target or arrival geometry",
                ));
            }
            match entry.entry().inbound().state.stack {
                calling_conventions::EntryStack::Dedicated { .. } => {}
                calling_conventions::EntryStack::Interrupted => {
                    let demand = requirement.arrival.as_ref().ok_or(Error::Mismatch(
                        "interrupted arrival has no composed physical-root demand",
                    ))?;
                    if requirement.fault_exclusion.is_none()
                        || requirement.entry_plan.frame != runtime_abi::static_interrupts::InstalledInterruptFrame::OpaqueFatal
                        || !matches!(requirement.entry_plan.protocol, runtime_abi::static_interrupts::InstalledInterruptProtocol::FatalContext { .. })
                        || demand.root_reserve_bytes < requirement.own_bytes().ok_or(Error::Overflow)?
                    {
                        return Err(Error::Mismatch("interrupted arrival lacks its complete conditional entry demand"));
                    }
                }
                calling_conventions::EntryStack::ProviderSelected => {
                    return Err(Error::Mismatch(
                        "installed entry has no selected stack domain",
                    ));
                }
            }
            let materialization =
                entry
                    .placement
                    .private_materialization
                    .as_ref()
                    .ok_or(Error::Mismatch(
                        "installation entry lacks table materialization",
                    ))?;
            let placements = materialization
                .destination_application
                .placements()
                .ok_or(Error::Mismatch("installation entry is not a table field"))?;
            let gate = table::gate(placements.as_slice(), input.layout.table_bytes)?;
            if entries[..ordinal].iter().any(|prior| {
                prior.placement.application_index == entry.placement.application_index
                    || prior.placement_index == entry.placement_index
            }) || retained.iter().any(|prior| prior.gate == gate)
            {
                return Err(Error::Mismatch(
                    "installation repeats an application, placement or gate",
                ));
            }
            let address = Expr::new(Word::Image(
                SelectedImageAddressReference::PrivateFunction {
                    identity: entry.callback_function,
                },
            ));
            let expected = if entry.placement.application_index == anchor.application() {
                if gate != anchor.gate_byte_offset()
                    || entry.callback_function != anchor.function()
                    || acknowledgement.replace(address.clone()).is_some()
                {
                    return Err(Error::Mismatch(
                        "installation acknowledgement differs from its exact anchor",
                    ));
                }
                runtime_abi::static_interrupts::InstalledInterruptEntryPlan::new(
                    runtime_abi::static_interrupts::InstalledInterruptFrame::NoErrorCode,
                )
            } else {
                let fatal = unique(
                    scope.fatal_entries().iter().filter(|fatal| {
                        fatal.application() == entry.placement.application_index
                            && fatal.machine() == entry.entry().machine()
                            && fatal.gate_byte_offset() == gate
                    }),
                    "installation entry has no exact admitted fatal occurrence",
                )?;
                runtime_abi::static_interrupts::InstalledInterruptEntryPlan::fatal_context(
                    anchor.gate_byte_offset(),
                    fatal.gate_byte_offset(),
                    anchor.table_bytes(),
                )
                .ok_or(Error::Mismatch(
                    "fatal entry has invalid retained gate geometry",
                ))?
            };
            if requirement.entry_plan != expected {
                return Err(Error::Mismatch(
                    "installation stack requirement names another entry protocol",
                ));
            }
            materializations.push((placements, address.clone()));
            retained.push(EntryInput {
                gate,
                address,
                requirement,
                stack: entry.entry().inbound().state.stack,
            });
        }
        let acknowledgement = acknowledgement.ok_or(Error::Mismatch(
            "installation omitted its acknowledgement anchor",
        ))?;
        let graph = Graph::new(selected, *psi_operation)?;
        let mut expressions = expressions::Expressions::new(selected, &graph)?;
        let mut stores = Writes::read(selected, *psi_operation, &mut expressions)?;
        let validation = unique(
            selected.calls.iter().filter(|row| {
                row.owner
                    == CallSiteOwner::Installation {
                        operation: *psi_operation,
                        role: InstallationCallRole::Validate,
                    }
            }),
            "installation has no unique validation call",
        )?;
        let work = unique(
            selected.calls.iter().filter(|row| {
                row.owner
                    == CallSiteOwner::Installation {
                        operation: *psi_operation,
                        role: InstallationCallRole::Work,
                    }
            }),
            "installation has no unique work call",
        )?;
        let validation_instruction = unique(
            selected
                .blocks
                .iter()
                .flat_map(|block| {
                    block.instructions.iter().chain(std::iter::once(
                        selected_instructions::terminator_instruction(&block.terminator),
                    ))
                })
                .filter(|row| row.id == validation.instruction),
            "installation validator has no selected call",
        )?;
        let work_instruction = unique(
            selected
                .blocks
                .iter()
                .flat_map(|block| {
                    block.instructions.iter().chain(std::iter::once(
                        selected_instructions::terminator_instruction(&block.terminator),
                    ))
                })
                .filter(|row| row.id == work.instruction),
            "installation work has no selected call",
        )?;
        instructions::validate(
            selected,
            &expressions,
            *psi_operation,
            validation.instruction,
            work.instruction,
        )?;
        for (helper, instruction, arity, expected) in [
            (validation, validation_instruction, 2, &input.pointers[0]),
            (work, work_instruction, 1, &input.pointers[5]),
        ] {
            let actual = if instruction.operands.len() == arity {
                Some(expressions.input(instruction, 0)?)
            } else {
                None
            };
            if actual.as_ref() != Some(expected) {
                return Err(Error::InstallationHelperReference {
                    machine: targeted.machine,
                    operation: *psi_operation,
                    detail: format!(
                        "owner={:?}, instruction={:?}, target={:?}, operand count={} (expected {arity}), pointer={actual:?} (expected {expected:?})",
                        helper.owner,
                        instruction.id,
                        helper.call.target,
                        instruction.operands.len(),
                    ),
                });
            }
        }
        refusal_result::validate(
            targeted.machine,
            *psi_operation,
            result_home.source_value,
            selected,
            validation.instruction,
            &graph,
            &mut expressions,
        )?;
        let refusal = refusal(selected, &graph, &mut expressions, validation.instruction)?;
        graph.protected_interval(
            validation.instruction,
            work.instruction,
            refusal,
            &mut expressions,
        )?;
        let contract = Contract::derive(*psi_operation, &input, &retained, validation.instruction)?;
        let publication = publication::validate(
            selected,
            *psi_operation,
            &input,
            &contract,
            acknowledgement,
            scope.completion().get(),
            work.instruction,
            &graph,
            &mut expressions,
            &mut stores,
            refusal,
        )?;
        for predicate in contract.before_publication.iter().chain(
            contract
                .entries
                .iter()
                .flat_map(|entry| &entry.before_publication),
        ) {
            let actual = graph.require_guard(&mut expressions, predicate, publication, refusal)?;
            publication::fresh_observations(&actual, &input, validation.instruction, &graph)?;
        }
        for (placements, address) in materializations {
            table::materialization(
                placements.as_slice(),
                &input.pointers[0],
                &address,
                validation.instruction,
                &graph,
                &mut stores,
            )?;
        }
        loops(
            &contract,
            publication,
            refusal,
            &graph,
            &mut expressions,
            &mut stores,
            &input,
            validation.instruction,
            work.instruction,
        )?;
        stores.complete()?;
        // Reads and ABI footprints are checked against the same actual operand
        // expressions after all loop/guard relationships have been established.
        publication::reads(
            selected,
            *psi_operation,
            &input,
            &contract,
            validation.instruction,
            &graph,
            &mut expressions,
        )?;
        completed.push(*psi_operation);
    }
    completed.sort_unstable();
    if completed.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::Mismatch(
            "installation target operation is duplicated",
        ));
    }
    Ok(completed)
}

fn refusal(
    selected: &SelectedFunction,
    graph: &Graph<'_>,
    expressions: &mut expressions::Expressions<'_>,
    validation: SelectedInstructionId,
) -> Result<SelectedBlockId, Error> {
    let expected = Expr::new(Word::Call(validation));
    let mut result = None;
    for (index, block) in selected.blocks.iter().enumerate() {
        let Some((predicate, _, failure)) = graph.predicate(index, expressions)? else {
            continue;
        };
        if predicate == expected
            && graph.precedes(
                validation,
                selected_instructions::terminator_instruction(&block.terminator).id,
            )?
            && result
                .replace(graph.continuation(SelectedBlockId(failure as u32))?)
                .is_some()
        {
            return Err(Error::Mismatch(
                "installation validator has multiple refusal continuations",
            ));
        }
    }
    result.ok_or(Error::Mismatch(
        "installation does not branch on its actual validator result",
    ))
}

struct Scan {
    count_init: SelectedInstructionId,
    initialize: SelectedInstructionId,
    advance: SelectedInstructionId,
    count_next: SelectedInstructionId,
}
impl Scan {
    fn consume(home: &Expr, offset: u32, stores: &mut Writes<'_>) -> Result<Self, Error> {
        let counter = home.clone().offset(offset);
        let count = home.clone().offset(88);
        Ok(Self {
            count_init: stores.take(count.clone(), Expr::constant(0), 8)?,
            initialize: stores.take(counter.clone(), Expr::constant(0), 8)?,
            advance: stores.take(counter.clone(), add(counter.load(8), Expr::constant(1)), 8)?,
            count_next: stores.take(count.clone(), add(count.load(8), Expr::constant(1)), 8)?,
        })
    }
}

fn loops(
    contract: &Contract,
    publication: SelectedInstructionId,
    refusal: SelectedBlockId,
    graph: &Graph<'_>,
    expressions: &mut expressions::Expressions<'_>,
    stores: &mut Writes<'_>,
    input: &Inputs,
    validate: SelectedInstructionId,
    work: SelectedInstructionId,
) -> Result<(), Error> {
    let gate_counter = contract.home.clone().offset(64);
    let gate_init = stores.take(gate_counter.clone(), Expr::constant(0), 8)?;
    let gate_next = stores.take(
        gate_counter.clone(),
        add(contract.table_index.clone(), Expr::constant(1)),
        8,
    )?;
    let gate_header = graph.counted_loop(
        expressions,
        &Expr::binary(
            Binary::Less,
            contract.table_index.clone(),
            contract.table_limit.clone(),
        ),
        gate_init,
        gate_next,
        publication,
    )?;
    increment_observation(
        stores.value(gate_next)?,
        &gate_counter,
        gate_header,
        gate_next,
        graph,
    )?;
    let actual = graph.require_guard_between(
        expressions,
        &contract.each_gate,
        gate_init,
        gate_next,
        refusal,
    )?;
    publication::fresh_observations(&actual, input, validate, graph)?;
    iteration_observations(&actual, &contract.home, gate_header, graph)?;
    let mut backedges = vec![(gate_next, gate_header)];
    let root_init = stores.take(contract.home.clone().offset(96), Expr::constant(0), 8)?;
    graph.require_loop_exit(gate_header, root_init, expressions)?;
    let mut scans = Vec::new();
    for (ordinal, entry) in contract.entries.iter().enumerate() {
        scans.push((ordinal, 0, Scan::consume(&contract.home, 72, stores)?));
        if entry.dedicated {
            scans.push((ordinal, 1, Scan::consume(&contract.home, 80, stores)?));
        }
    }
    let pair_scans = pairs::consume(&contract.home, contract.pairs.len(), stores)?;
    let mut previous_header = gate_header;
    let mut acknowledgement_header = None;
    for (phase, (ordinal, kind, scan)) in scans.iter().enumerate() {
        let entry = &contract.entries[*ordinal];
        let kind = *kind;
        let (offset, counter, limit, matching, guards) = if kind == 0 {
            (
                72,
                &contract.arrival_index,
                &contract.arrival_limit,
                &entry.matching_arrival,
                std::slice::from_ref(&entry.matched_arrival),
            )
        } else {
            (
                80,
                &contract.stack_index,
                &contract.stack_limit,
                &entry.matching_stack,
                entry.matched_stack.as_slice(),
            )
        };
        let next = scans
            .get(phase + 1)
            .map(|row| row.2.count_init)
            .unwrap_or_else(|| {
                pair_scans
                    .first()
                    .map_or(publication, |pair| pair[0].count_init)
            });
        if !graph.precedes(root_init, scan.count_init)?
            || !graph.precedes(scan.count_init, scan.initialize)?
        {
            return Err(Error::Mismatch(
                "installation changed sequential scan initialization",
            ));
        }
        graph.require_loop_exit(previous_header, scan.count_init, expressions)?;
        let header = graph.counted_loop(
            expressions,
            &Expr::binary(Binary::Less, counter.clone(), limit.clone()),
            scan.initialize,
            scan.advance,
            next,
        )?;
        increment_observation(
            stores.value(scan.advance)?,
            &contract.home.clone().offset(offset),
            header,
            scan.advance,
            graph,
        )?;
        increment_observation(
            stores.value(scan.count_next)?,
            &contract.home.clone().offset(88),
            header,
            scan.count_next,
            graph,
        )?;
        let actual = graph.selected_iteration(
            expressions,
            matching,
            header,
            scan.count_next,
            scan.advance,
        )?;
        publication::fresh_observations(&actual, input, validate, graph)?;
        iteration_observations(&actual, &contract.home, header, graph)?;
        if kind == 1 {
            for predicate in &contract.each_stack {
                let actual = graph.require_guard_between(
                    expressions,
                    predicate,
                    scan.initialize,
                    scan.advance,
                    refusal,
                )?;
                publication::fresh_observations(&actual, input, validate, graph)?;
                iteration_observations(&actual, &contract.home, header, graph)?;
            }
        }
        for predicate in guards {
            let actual = graph.require_iteration_guard(
                expressions,
                predicate,
                header,
                scan.count_next,
                scan.advance,
                refusal,
            )?;
            publication::fresh_observations(&actual, input, validate, graph)?;
            iteration_observations(&actual, &contract.home, header, graph)?;
        }
        let actual = graph.require_guard_between(
            expressions,
            &equal(contract.home.clone().offset(88).load(8), Expr::constant(1)),
            graph.header_instruction(header),
            next,
            refusal,
        )?;
        actual.visit_loads(&mut |address, bytes, instruction| {
            if *address == contract.home.clone().offset(88) && bytes == 8 {
                graph.require_loop_exit(
                    header,
                    instruction.ok_or(Error::Mismatch(
                        "installation count observation lacks an instruction",
                    ))?,
                    expressions,
                )?;
            }
            Ok(())
        })?;
        if kind == 1
            && let Some(root) = &entry.acknowledgement_root
        {
            let root_write = stores.take(contract.home.clone().offset(96), root.clone(), 8)?;
            let top_write = stores.take(
                contract.home.clone().offset(104),
                contract.stack_top.clone(),
                8,
            )?;
            if !graph.precedes(scan.count_next, root_write)?
                || !graph.precedes(root_write, top_write)?
                || acknowledgement_header.replace(header).is_some()
            {
                return Err(Error::Mismatch(
                    "installation changed its unique acknowledgement root publication",
                ));
            }
            graph.selected_iteration(expressions, matching, header, root_write, scan.advance)?;
            for predicate in guards {
                let actual = graph.require_guard_between(
                    expressions,
                    predicate,
                    scan.count_next,
                    root_write,
                    refusal,
                )?;
                iteration_observations(&actual, &contract.home, header, graph)?;
            }
            iteration_observations(stores.value(root_write)?, &contract.home, header, graph)?;
            iteration_observations(stores.value(top_write)?, &contract.home, header, graph)?;
        }
        previous_header = header;
        backedges.push((scan.advance, header));
    }
    pairs::replay(
        contract,
        &pair_scans,
        previous_header,
        publication,
        refusal,
        graph,
        expressions,
        stores,
        input,
        validate,
        &mut backedges,
    )?;
    let header = acknowledgement_header.ok_or(Error::Mismatch(
        "installation has no acknowledgement root scan",
    ))?;
    for store in &stores.rows {
        store
            .address
            .visit_loads(&mut |address, bytes, instruction| {
                if *address == contract.home.clone().offset(96) && bytes == 8 {
                    graph.require_loop_exit(
                        header,
                        instruction.ok_or(Error::Mismatch(
                            "installation root observation lacks an instruction",
                        ))?,
                        expressions,
                    )?;
                }
                Ok(())
            })?;
    }
    graph.bounded_region(validate, work, refusal, &backedges)
}

/// Equal counter expressions from other scans do not identify current storage.
fn iteration_observations(
    value: &Expr,
    home: &Expr,
    header: usize,
    graph: &Graph<'_>,
) -> Result<(), Error> {
    value.visit_loads(&mut |address, bytes, instruction| {
        if bytes == 8
            && [64, 72, 80, 88]
                .iter()
                .any(|offset| *address == home.clone().offset(*offset))
        {
            let observed = instruction.ok_or(Error::Mismatch(
                "installation iteration has no load instruction",
            ))?;
            if graph.position(observed)?.0 != header
                && !graph.precedes(graph.header_instruction(header), observed)?
            {
                return Err(Error::Mismatch(
                    "installation guard reuses a different counter incarnation",
                ));
            }
        }
        Ok(())
    })
}

fn increment_observation(
    value: &Expr,
    counter: &Expr,
    header: usize,
    advance: SelectedInstructionId,
    graph: &Graph<'_>,
) -> Result<(), Error> {
    let mut observations = 0;
    value.visit_loads(&mut |address, bytes, instruction| {
        if address == counter && bytes == 8 {
            let load = instruction.ok_or(Error::Mismatch(
                "installation counter update has no observation",
            ))?;
            if (graph.position(load)?.0 != header
                && !graph.precedes(graph.header_instruction(header), load)?)
                || !graph.precedes(load, advance)?
            {
                return Err(Error::Mismatch(
                    "installation counter update reuses a different iteration",
                ));
            }
            observations += 1;
        }
        Ok(())
    })?;
    if observations == 0 {
        return Err(Error::Mismatch(
            "installation counter update does not read its own counter",
        ));
    }
    Ok(())
}
