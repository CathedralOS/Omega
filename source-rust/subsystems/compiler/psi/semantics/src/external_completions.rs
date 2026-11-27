//! A checked machine that realizes a requirement whose completion ends the
//! process must end it with the caller's own arguments.
//!
//! A call of such a requirement is "a terminal transfer retaining the exact
//! requirement and ordered arguments" (`wiki/spec/terminal-psi/calls_and_outcomes.md`,
//! "External process exit"), and "the exact exit argument survive[s]"
//! (`wiki/spec/language/process_exit.md`). A requirement that never returns
//! has no `ensures` to state this, so contract refinement cannot see a
//! provider that passes a different status to the primitive. This judgment
//! reads the realizing body instead: every external completion it reaches
//! has the requirement's kind and passes the machine's formals, in order, as
//! the very values it received. A value computed in the body,
//! even one that would equal the argument, is not the argument: the rule is
//! identity of values, which needs no proof search.
//!
//! Which machine realizes which requirement comes from the module's provider
//! candidate rows, the rows provider settlement and native admission bind
//! through.

use semantic_vocabulary::{
    BlockId, BoundaryCompletion, EdgeId, ExternalCompletionKind, MachineId, ValueId,
};
use terminal_psi::{TerminalMachine, TerminalModule, Terminator};

/// How a realizing body departs from its requirement's terminal transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalRealizationFault {
    /// The completion passes other values than the machine's own arguments.
    ChangedArguments { edge: EdgeId },
    /// The completion is of another kind than the requirement's.
    OtherKind { edge: EdgeId },
    /// A path returns although the requirement has no normal completion.
    Returns { block: BlockId },
    /// The body ends through a form this judgment does not follow.
    Unfollowed { block: BlockId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalRealizationError {
    pub machine: MachineId,
    pub fault: ExternalRealizationFault,
}

/// The external completion kind of a requirement `machine` realizes, if any.
/// A provider candidate row is the published statement that its machine
/// stands for its boundary requirement; settlement and native admission
/// choose a checked provider only from these rows.
pub(crate) fn realized_external_completion(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Option<ExternalCompletionKind> {
    module
        .interfaces
        .provider_candidates
        .iter()
        .filter(|candidate| candidate.candidate == machine.id)
        .find_map(|candidate| {
            match module.boundary_machine(candidate.boundary)?.completion {
                BoundaryCompletion::External(kind) => Some(kind),
                BoundaryCompletion::NormalReturn => None,
            }
        })
}

/// Judge one machine. A machine that realizes no such requirement passes.
pub fn validate_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), ExternalRealizationError> {
    let Some(kind) = realized_external_completion(module, machine) else {
        return Ok(());
    };
    let fault = |fault| ExternalRealizationError {
        machine: machine.id,
        fault,
    };
    // Scalar arguments only: the judgment compares value identities, and a
    // structural argument has none to compare.
    if !machine.contract.structural_parameters.is_empty() {
        return Err(fault(ExternalRealizationFault::Unfollowed {
            block: machine.entry,
        }));
    }
    let received: Vec<ValueId> = machine
        .contract
        .parameters
        .iter()
        .map(|value| value.id)
        .collect();
    let passed_on = |arguments: &[ValueId]| {
        arguments.len() == received.len()
            && arguments.iter().zip(&received).all(|(argument, received)| {
                received_value(machine, *argument, machine.blocks.len()) == Some(*received)
            })
    };
    for block in &machine.blocks {
        match &block.terminator {
            Terminator::ExternalCompletion {
                edge,
                kind: completed,
                arguments,
                structural_arguments,
                ..
            } => {
                if *completed != kind {
                    return Err(fault(ExternalRealizationFault::OtherKind { edge: *edge }));
                }
                if !structural_arguments.is_empty() || !passed_on(arguments) {
                    return Err(fault(ExternalRealizationFault::ChangedArguments {
                        edge: *edge,
                    }));
                }
            }
            // Control flow inside the body, and a crash, which the crash
            // refinement of the conformance witness judges.
            Terminator::Jump { .. }
            | Terminator::Conditional { .. }
            | Terminator::StructuralCase { .. }
            | Terminator::EliminateEmpty { .. }
            | Terminator::Crash { .. } => {}
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnStructural { .. } => {
                return Err(fault(ExternalRealizationFault::Returns { block: block.id }));
            }
            Terminator::InvokeNoNormal { .. } => {
                return Err(fault(ExternalRealizationFault::Unfollowed {
                    block: block.id,
                }));
            }
        }
    }
    Ok(())
}

/// The formal of `machine` that `value` is. A value handed from block to
/// block stays the same value when every arrival at a block passes it in the
/// same parameter position; anything an operation computed is another value.
fn received_value(machine: &TerminalMachine, value: ValueId, budget: usize) -> Option<ValueId> {
    if machine
        .contract
        .parameters
        .iter()
        .any(|formal| formal.id == value)
    {
        return Some(value);
    }
    let (block, position) = machine.blocks.iter().find_map(|block| {
        block
            .parameters
            .iter()
            .position(|parameter| parameter.id == value)
            .map(|position| (block, position))
    })?;
    // One step per block bounds any chain; a cycle of arrivals runs out.
    let budget = budget.checked_sub(1)?;
    let mut origin = None;
    for source in &machine.blocks {
        let arrivals: Vec<&[ValueId]> = match &source.terminator {
            Terminator::Jump {
                target, arguments, ..
            } if *target == block.id => vec![arguments],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => [when_true, when_false]
                .into_iter()
                .filter(|edge| edge.target == block.id)
                .map(|edge| edge.arguments.as_slice())
                .collect(),
            // A case edge binds payload fields, not the caller's arguments.
            Terminator::StructuralCase { cases, .. }
                if cases.iter().any(|case| case.target == block.id) =>
            {
                return None;
            }
            _ => Vec::new(),
        };
        for arguments in arrivals {
            let arrived = received_value(machine, *arguments.get(position)?, budget)?;
            if origin.is_some_and(|known| known != arrived) {
                return None;
            }
            origin = Some(arrived);
        }
    }
    origin
}

/// Judge every machine of a module, for artifact ingress.
pub fn validate(module: &TerminalModule) -> Result<(), ExternalRealizationError> {
    module
        .machines
        .iter()
        .try_for_each(|machine| validate_machine(module, machine))
}
