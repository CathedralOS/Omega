//! Check where control continues when an acknowledged local call completes.
//! This judgment binds a control location to one unit. Storage, live values,
//! prefix correspondence and the right to execute need their own evidence.
use omega_psi::{BlockTerminator, CallSite, CallTarget, CompletionContinuation, LoweredMachine, LoweredPackage, MachineId, OperationKind, SuspensionSite};

#[derive(Debug)]
pub struct CheckedSuspensionControl<'unit> {
    caller: &'unit LoweredMachine,
    callee: &'unit LoweredMachine,
    crossing: &'unit SuspensionSite,
}

impl<'unit> CheckedSuspensionControl<'unit> {
    pub fn caller(&self) -> &'unit LoweredMachine {
        self.caller
    }
    pub fn callee(&self) -> &'unit LoweredMachine {
        self.callee
    }
    pub fn call(&self) -> CallSite {
        self.crossing.call
    }
    pub fn continuation(&self) -> CompletionContinuation {
        self.crossing.continuation
    }
}

/// Judge the selected instruction and its immediate normal successor. The
/// caller supplies the call the activation reached, rather than a source name.
pub fn check_local_suspension_control(
    unit: &LoweredPackage,
    caller: MachineId,
    at: CallSite,
) -> Result<CheckedSuspensionControl<'_>, &'static str> {
    let machine = |id| {
        let mut found = unit.machines.iter().filter(|m| m.id == id);
        match (found.next(), found.next()) {
            (Some(machine), None) => Ok(machine),
            _ => Err("suspension needs a unique machine in this unit"),
        }
    };
    let caller = machine(caller)?;
    let body = &caller.body;
    let mut sites = body.suspension_sites.iter().filter(|row| row.call == at);
    let (Some(crossing), None) = (sites.next(), sites.next()) else {
        return Err("suspending call needs exactly one suspension site");
    };
    let mut acknowledgements = body.call_acknowledgements.iter().filter(|row| row.call == at);
    if !matches!((acknowledgements.next(), acknowledgements.next()), (Some(row), None) if row.effects.suspend) {
        return Err("suspending call needs its own suspension acknowledgement");
    }
    let (target, continuation) = match at {
        CallSite::Operation(id) => {
            let position = id.index();
            if body.operations.iter().filter(|op| op.id == id).count() != 1 {
                return Err("parked operation has ambiguous identity");
            }
            let op = body
                .operations
                .get(position)
                .filter(|op| op.id == id)
                .ok_or("unknown parked operation")?;
            let OperationKind::Call { callee, .. } = &op.kind else {
                return Err("parked operation is not a call");
            };
            let mut owners = body.blocks.iter().filter(|b| b.operations.contains(&position));
            let (Some(block), None) = (owners.next(), owners.next()) else {
                return Err("parked operation needs exactly one enclosing block");
            };
            if body.blocks.iter().filter(|b| b.id == block.id).count() != 1 {
                return Err("parked block has ambiguous identity");
            }
            let sequence = body
                .operations
                .get(block.operations.clone())
                .ok_or("invalid parked block operation range")?;
            let next = match sequence.get(position - block.operations.start + 1) {
                Some(op) if op.id.index() == position + 1 && body.operations.iter().filter(|other| other.id == op.id).count() == 1 => {
                    CompletionContinuation::Operation(op.id)
                }
                Some(_) => return Err("successor operation has a different identity"),
                None => CompletionContinuation::Terminator(block.id),
            };
            (*callee, next)
        }
        CallSite::Edge(id) => {
            let mut calls = body
                .blocks
                .iter()
                .flat_map(|block| {
                    use BlockTerminator::*;
                    let edges = match &block.terminator {
                        TerminalCall { id, callee, .. } => [Some((*id, Some(*callee))), None],
                        Return { id, .. } => [Some((*id, None)), None],
                        Jump(edge) => [Some((edge.id, None)), None],
                        ExternalExit(exit) => [Some((exit.id, None)), None],
                        Conditional { when_true, when_false, .. } => [Some((when_true.id, None)), Some((when_false.id, None))],
                    };
                    edges.into_iter().flatten()
                })
                .filter(|(edge, _)| *edge == id);
            let (Some((_, Some(target))), None) = (calls.next(), calls.next()) else {
                return Err("parked edge needs exactly one terminal call");
            };
            (target, CompletionContinuation::Return)
        }
        CallSite::Cleanup(_) => return Err("local suspension control does not support cleanup calls"),
    };
    if crossing.callee != target || crossing.continuation != continuation {
        return Err("suspension site differs from the selected call or its normal continuation");
    }
    let CallTarget::Machine(id) = target else {
        return Err("suspension control requires a local machine call");
    };
    let callee = machine(id)?;
    if !callee.contract.realized_pause.suspend || !caller.contract.realized_pause.suspend {
        return Err("caller and callee must admit suspension");
    }
    Ok(CheckedSuspensionControl { caller, callee, crossing })
}
