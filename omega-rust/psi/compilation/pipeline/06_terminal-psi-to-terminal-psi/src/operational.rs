//! A call owes its caller two independent promises: its possible waits fit the
//! caller's contract, and the source acknowledges each possibility. Markers
//! describe a call; they never turn a synchronous operation into a pause.
//!
//! Read canonical calls, not assertions in auxiliary invocation tables. A
//! source tail call retains its interface on the jump which reuses its frame.
//! Inferred contracts are the least fixed point of those same call envelopes.

use semantic_vocabulary::{ContractId, MachineId, TargetMachineCallable};
use terminal_check_tables::{Checked, Construct, RefusalReason};
use terminal_psi::{CallAcknowledgement, CallOrigin, CallSite, TerminalMachine, Terminator};

use crate::{Elaboration, refusal};

#[derive(Clone, Copy)]
struct Call {
    site: CallSite,
    callee: TargetMachineCallable,
    required_contract: Option<ContractId>,
    acknowledgement: CallAcknowledgement,
    tail: bool,
}

fn at(site: CallSite) -> Construct {
    match site {
        CallSite::Operation(id) => Construct::Operation(id),
        CallSite::Edge(id) => Construct::Edge(id),
    }
}

fn fault(machine: MachineId, site: Construct, detail: &str) -> terminal_check_tables::Refusal {
    refusal(RefusalReason::ForeignIdentity, machine, site, detail)
}

fn published(machine: &TerminalMachine) -> [bool; 2] {
    [machine.published_may_suspend, machine.published_may_block]
}

fn declared(machine: &TerminalMachine) -> [Option<bool>; 2] {
    [machine.declared_suspension, machine.declared_blocking]
}

fn calls(elaboration: &Elaboration, machine: &TerminalMachine) -> Checked<Vec<Call>> {
    let malformed = |detail| fault(machine.id, Construct::Machine, detail);
    if machine
        .call_formations
        .windows(2)
        .any(|pair| pair[0].site >= pair[1].site)
    {
        return Err(malformed(
            "call acknowledgements are not strictly ordered by site",
        ));
    }
    let formation = |site| {
        machine
            .call_formations
            .binary_search_by_key(&site, |row| row.site)
            .ok()
            .map(|position| &machine.call_formations[position])
    };
    let mut blocks: Vec<_> = machine.blocks.iter().collect();
    blocks.sort_unstable_by_key(|block| block.id);
    let mut calls = Vec::new();
    for block in &machine.blocks {
        for operation in &block.operations {
            if let Some(inputs) = operation.declared_invocation() {
                calls.push(Call {
                    site: CallSite::Operation(operation.id),
                    callee: inputs.callee,
                    required_contract: inputs.required_contract,
                    acknowledgement: CallAcknowledgement::default(),
                    tail: false,
                });
            }
        }
        if let Some((_, inputs)) = block.terminator.declared_invocation() {
            calls.push(Call {
                site: CallSite::Edge(block.terminator.edge()),
                callee: inputs.callee,
                required_contract: inputs.required_contract,
                acknowledgement: CallAcknowledgement::default(),
                tail: false,
            });
        } else if let Terminator::Jump {
            edge,
            target,
            arguments,
            structural_arguments,
            erased_arguments,
            erased_proof_arguments,
            ..
        } = &block.terminator
            && let Some(row) = formation(CallSite::Edge(*edge))
            && let Some(callee) = row.tail_callee
        {
            let callee = elaboration
                .module
                .machine(callee)
                .ok_or_else(|| malformed("tail call names an unavailable machine"))?;
            let head = blocks
                .binary_search_by_key(target, |block| block.id)
                .ok()
                .map(|position| blocks[position])
                .ok_or_else(|| malformed("tail call names an unavailable loop head"))?;
            // Reusing the frame still calls the same interface. Both argument
            // lanes must match its formals, including access and qualifications;
            // ordinary arrival and borrow checking judge the actual transfer.
            if !erased_arguments.is_empty()
                || !erased_proof_arguments.is_empty()
                || !callee.erased_scalar_formals.is_empty()
                || head.parameters.len() != arguments.len()
                || head.parameters.len() != callee.parameters.len()
                || head
                    .parameters
                    .iter()
                    .zip(&callee.parameters)
                    .any(|(actual, formal)| actual.value_type() != formal.value_type())
                || head.structural_parameters.len() != structural_arguments.len()
                || head.structural_parameters.len() != callee.structural_parameters.len()
                || head
                    .structural_parameters
                    .iter()
                    .zip(&callee.structural_parameters)
                    .any(|(actual, formal)| {
                        actual.position != formal.position
                            || actual.structural_type != formal.structural_type
                            || actual.multiplicity != formal.multiplicity
                            || actual.access != formal.access
                            || actual.qualifications != formal.qualifications
                            || actual.projected_qualifications != formal.projected_qualifications
                            || actual.additional_input_qualifications
                                != formal.additional_input_qualifications
                    })
            {
                return Err(malformed(
                    "tail call's loop head does not carry its callable's formals",
                ));
            }
            calls.push(Call {
                site: row.site,
                callee: callee.id.into(),
                required_contract: None,
                acknowledgement: CallAcknowledgement::default(),
                tail: true,
            });
        }
    }
    calls.sort_unstable_by_key(|call| call.site);
    for row in &machine.call_formations {
        let position = calls
            .binary_search_by_key(&row.site, |call| call.site)
            .map_err(|_| malformed("call acknowledgement names no canonical call"))?;
        let call = &mut calls[position];
        if row
            .tail_callee
            .is_some_and(|callee| call.callee != callee.into())
            || (row.tail_callee.is_some() && !matches!(row.site, CallSite::Edge(_)))
        {
            return Err(malformed(
                "tail call acknowledgement names a different callable",
            ));
        }
        // A real invocation carries its target in the operation/terminator;
        // only a frame-reusing jump may repeat it in the formation row.
        if row.tail_callee.is_some() != call.tail {
            return Err(malformed("ordinary invocation has a tail-call interface"));
        }
        call.acknowledgement = row.acknowledgement;
    }
    Ok(calls)
}

pub(crate) fn judge(elaboration: &Elaboration) -> Checked<()> {
    let module = &elaboration.module;
    let mut machines = module
        .machines
        .iter()
        .map(|machine| calls(elaboration, machine).map(|calls| (machine, calls)))
        .collect::<Checked<Vec<_>>>()?;
    machines.sort_unstable_by_key(|(machine, _)| machine.id);
    let index = |id| {
        machines
            .binary_search_by_key(&id, |(machine, _)| machine.id)
            .ok()
    };
    // Explicit bounds are fixed interfaces, including conservative `true`.
    // Omitted private clauses begin at false and grow through checked calls.
    let mut inferred: Vec<_> = machines
        .iter()
        .map(|(machine, _)| declared(machine).map(|bound| bound.unwrap_or(false)))
        .collect();
    let envelope = |call: &Call, summaries: &[[bool; 2]], caller, site| -> Checked<[bool; 2]> {
        if let (TargetMachineCallable::Machine(body), Some(required)) =
            (call.callee, call.required_contract)
        {
            let contract = module
                .invocation_machine_contract(body, Some(required))
                .ok_or_else(|| {
                    fault(
                        caller,
                        site,
                        "selected call has no matching public operational contract",
                    )
                })?;
            return Ok([contract.published_may_suspend, contract.published_may_block]);
        }
        match call.callee {
            TargetMachineCallable::Machine(id) => index(id)
                .map(|position| summaries[position])
                .ok_or_else(|| fault(caller, site, "operational callee is unavailable")),
            TargetMachineCallable::Boundary(id) => module
                .boundary_machine(id)
                .and_then(|boundary| boundary.declaration.as_ref())
                .map(|declaration| {
                    [
                        declaration.source.published_may_suspend,
                        declaration.source.published_may_block,
                    ]
                })
                .ok_or_else(|| fault(caller, site, "boundary has no checked operational contract")),
        }
    };
    loop {
        let mut changed = false;
        for (position, (machine, calls)) in machines.iter().enumerate() {
            let mut contributes = [false; 2];
            for call in calls {
                let flags = envelope(call, &inferred, machine.id, at(call.site))?;
                for axis in 0..2 {
                    contributes[axis] |= flags[axis];
                }
            }
            let expected =
                std::array::from_fn(|axis| declared(machine)[axis].unwrap_or(contributes[axis]));
            if inferred[position] != expected {
                inferred[position] = expected;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut suspension = None;
    let mut crossings = Vec::new();
    for (position, (machine, calls)) in machines.iter().enumerate() {
        if published(machine) != inferred[position] {
            return Err(fault(
                machine.id,
                Construct::Machine,
                "published operational contract differs from its declared or inferred envelope",
            ));
        }
        if machine.declaration.as_ref().is_some_and(|declaration| {
            [
                declaration.published_may_suspend,
                declaration.published_may_block,
            ] != published(machine)
        }) {
            return Err(fault(
                machine.id,
                Construct::Machine,
                "callable declaration and body disagree on their operational contract",
            ));
        }
        for call in calls {
            let site = at(call.site);
            let violation = |detail| {
                let span = elaboration
                    .sources
                    .get(machine.id)
                    .and_then(|sources| match site {
                        Construct::Operation(id) => sources.operations.get(id).copied(),
                        Construct::Edge(id) => sources.edges.get(id).copied(),
                        _ => None,
                    })
                    .and_then(terminal_check_tables::program::SourceOrigin::authored_span);
                refusal(RefusalReason::Violation, machine.id, site, detail).with_source_span(span)
            };
            let flags = envelope(call, &inferred, machine.id, site)?;
            let ack = call.acknowledgement;
            if ack.origin == CallOrigin::Authored
                && ack.acknowledges_suspend
                && !ack.suspension_position
            {
                return Err(violation(
                    "a suspend-marked call is nested outside a legal suspension position",
                ));
            }
            for axis in 0..2 {
                if !flags[axis] {
                    continue;
                }
                if !published(machine)[axis] {
                    return Err(violation(if axis == 0 {
                        "call may suspend but its caller's suspends ceiling is false"
                    } else {
                        "call may block but its caller's blocks ceiling is false"
                    }));
                }
                if ![ack.acknowledges_suspend, ack.acknowledges_block][axis] {
                    return Err(violation(if axis == 0 {
                        "call may suspend and requires a suspend acknowledgement"
                    } else {
                        "call may block and requires a block acknowledgement"
                    }));
                }
            }
            if flags[0] {
                match call.site {
                    CallSite::Operation(operation) if !call.tail => {
                        crossings.push((machine.id, operation));
                    }
                    // A crossing is a property of an operation. A call a
                    // terminator makes, or one lowered to a jump, has none.
                    CallSite::Operation(_) | CallSite::Edge(_) => {
                        suspension.get_or_insert((machine.id, site));
                    }
                }
            }
        }
    }
    if let Some((machine, site)) = suspension {
        return Err(refusal(
            RefusalReason::Unsupported,
            machine,
            site,
            "a possibly suspending terminal call needs a checked suspension crossing",
        ));
    }
    crossings_are_stated(elaboration, &crossings)
}

/// Every call operation that may suspend, and no other operation, carries a
/// suspension crossing, and the module states for each exactly the site and
/// plan this checker reconstructs from the operations themselves.
/// calls_and_outcomes.md, "Suspension": "Completeness requires an
/// independently established possibly-suspending crossing roster. Matching a
/// producer's site and plan is not enough: deleting both must not erase a
/// required crossing."
///
/// What a live value owes to cross (resources/carry.md): its carry policy
/// must allow suspension, which is checked row by row. The other duties do
/// not arise on this route. A resource claim is refused before it could be
/// live here (`activation_storage::validate_module`). An ownership window
/// cannot be open, because the reader refuses an owned extraction from
/// borrowed storage. A loan is not ended by the call, so the loan judgment
/// already holds every lender for as long as its borrower is live.
fn crossings_are_stated(
    elaboration: &Elaboration,
    expected: &[(
        semantic_vocabulary::MachineId,
        semantic_vocabulary::OperationId,
    )],
) -> Checked<()> {
    use terminal_semantics::suspension_frontier::{call_plan, call_site};
    let module = &elaboration.module;
    let mut stated = 0usize;
    for machine in &module.machines {
        for operation in machine.operations() {
            let site = Construct::Operation(operation.id);
            let owed = expected.contains(&(machine.id, operation.id));
            let Some(crossing) = operation.suspension_crossing else {
                if owed {
                    return Err(fault(
                        machine.id,
                        site,
                        "a possibly suspending call carries no suspension crossing",
                    ));
                }
                continue;
            };
            if !owed {
                return Err(fault(
                    machine.id,
                    site,
                    "a suspension crossing is on an operation that cannot suspend",
                ));
            }
            stated += 1;
            let plan = call_plan(module, machine, operation.id, crossing)
                .map_err(|reason| fault(machine.id, site, reason))?;
            let mut plans = module
                .suspension_call_plans
                .iter()
                .filter(|plan| plan.crossing == crossing);
            if plans.next() != Some(&plan) || plans.next().is_some() {
                return Err(fault(
                    machine.id,
                    site,
                    "a suspension plan is not the live frontier of its call",
                ));
            }
            let mut sites = module
                .suspension_call_sites
                .iter()
                .filter(|site| site.crossing == crossing);
            if sites.next() != Some(&call_site(module, &plan)) || sites.next().is_some() {
                return Err(fault(
                    machine.id,
                    site,
                    "a suspension site does not commit to the live frontier of its call",
                ));
            }
            // carry.md: "A suspension-forbidden live value rejects an
            // explicit semantic suspension point."
            if plan
                .live_values
                .iter()
                .any(|live| live.effective.suspension != language_core::CarrySuspension::Allowed)
            {
                return Err(refusal(
                    RefusalReason::Violation,
                    machine.id,
                    site,
                    "a value live across this call forbids suspension",
                ));
            }
        }
    }
    if stated != module.suspension_call_plans.len()
        || stated != module.suspension_call_sites.len()
        || u32::try_from(stated).ok() != Some(module.suspension_call_plan_count)
    {
        return Err(fault(
            semantic_vocabulary::MachineId::new(1).expect("nonzero diagnostic identity"),
            Construct::Machine,
            "the module states a suspension site or plan no call owns",
        ));
    }
    Ok(())
}
