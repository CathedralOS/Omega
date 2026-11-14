//! A call owes its caller two independent promises: its possible waits fit the
//! caller's contract, and the source acknowledges each possibility. Markers
//! describe a call; they never turn a synchronous operation into a pause.
//!
//! Read canonical calls, not assertions in auxiliary invocation tables. A
//! source tail call retains its interface on the jump which reuses its frame.
//! Inferred contracts are the least fixed point of those same call envelopes.

use semantic_vocabulary::{MachineId, TargetMachineCallable};
use terminal_check_tables::{Checked, Construct, RefusalReason};
use terminal_psi::{CallAcknowledgement, CallOrigin, CallSite, TerminalMachine, Terminator};

use crate::{Elaboration, refusal};

#[derive(Clone, Copy)]
struct Call {
    site: CallSite,
    callee: TargetMachineCallable,
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
            if let Some(inputs) = operation.kind.declared_invocation() {
                calls.push(Call {
                    site: CallSite::Operation(operation.id),
                    callee: inputs.callee,
                    acknowledgement: CallAcknowledgement::default(),
                    tail: false,
                });
            }
        }
        if let Some((_, inputs)) = block.terminator.declared_invocation() {
            calls.push(Call {
                site: CallSite::Edge(block.terminator.edge()),
                callee: inputs.callee,
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
            // Frame reuse currently carries scalar formals only. The block's
            // telescope and the retained callable must describe that same
            // arrival; ordinary arrival checking proves its predicates.
            if !structural_arguments.is_empty()
                || !erased_arguments.is_empty()
                || !erased_proof_arguments.is_empty()
                || !callee.structural_parameters.is_empty()
                || !callee.erased_scalar_formals.is_empty()
                || head.parameters.len() != arguments.len()
                || head.parameters.len() != callee.parameters.len()
                || head
                    .parameters
                    .iter()
                    .zip(&callee.parameters)
                    .any(|(actual, formal)| actual.scalar_type != formal.scalar_type)
            {
                return Err(malformed(
                    "tail call's loop head does not carry its callable's formals",
                ));
            }
            calls.push(Call {
                site: row.site,
                callee: callee.id.into(),
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
    let envelope = |callee, summaries: &[[bool; 2]], caller, site| -> Checked<[bool; 2]> {
        match callee {
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
                let flags = envelope(call.callee, &inferred, machine.id, at(call.site))?;
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
                    .and_then(|origin| match origin {
                        terminal_check_tables::program::SourceOrigin::Authored(span) => Some(span),
                        terminal_check_tables::program::SourceOrigin::Unpositioned(_) => None,
                    });
                refusal(RefusalReason::Violation(span), machine.id, site, detail)
            };
            let flags = envelope(call.callee, &inferred, machine.id, site)?;
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
                suspension.get_or_insert((machine.id, site));
            }
        }
    }
    if let Some((machine, site)) = suspension {
        return Err(refusal(
            RefusalReason::Unsupported,
            machine,
            site,
            "a possibly suspending call needs checked continuation, carry and loan preservation evidence",
        ));
    }
    Ok(())
}
