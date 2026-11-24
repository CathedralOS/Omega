//! Determine whether a checked root can return before entering a particular
//! synchronous callee. A caller only resumes past an ordinary direct call if
//! that callee has a path back that still avoids the required invocation.

use super::{
    CompletionGraphError, NormalCompletionDisposition, completion_disposition,
    validate_control_graph,
};
use semantic_vocabulary::{BlockId, MachineId};
use terminal_psi::TerminalModule;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationOnReturn {
    /// An invocation is reachable and every normal return passes through it.
    Required,
    /// There is an invocation, but another path can return before it.
    Bypassed,
    /// No direct path of synchronous calls enters the required machine.
    Unreached,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct BeforeInvocation {
    enters: bool,
    returns: bool,
}

/// Read an exact checked Psi module. Unknown dispatch can return without
/// entering the requested machine. It earns no credit for a selected call.
/// Crashes, process exits and infinite executions produce no normal return.
pub fn invocation_on_return(
    module: &TerminalModule,
    root: MachineId,
    required: MachineId,
) -> Result<InvocationOnReturn, CompletionGraphError> {
    // Keep execution state indexed by catalog position. Decoded modules may
    // use sparse identities; their numeric magnitude never controls allocation.
    let mut machines: Vec<_> = module.machines.iter().collect();
    machines.sort_unstable_by_key(|machine| machine.id);
    if let Some(pair) = machines.windows(2).find(|pair| pair[0].id == pair[1].id) {
        return Err(CompletionGraphError::AmbiguousMachine(pair[0].id));
    }
    let position = |id| {
        machines
            .binary_search_by_key(&id, |machine| machine.id)
            .map_err(|_| CompletionGraphError::UnknownMachine(id))
    };
    let root = position(root)?;
    let required = position(required)?;
    let target = machines[required];
    validate_control_graph(target).map_err(|error| CompletionGraphError::InvalidControl {
        machine: target.id,
        error,
    })?;

    // Index only the direct call closure that can precede the selected call.
    let mut callers = vec![Vec::new(); machines.len()];
    let mut summaries = vec![BeforeInvocation::default(); machines.len()];
    let mut bodies: Vec<Option<DirectBody>> = (0..machines.len()).map(|_| None).collect();
    let mut reached = vec![false; machines.len()];
    let mut pending = vec![root];
    while let Some(index) = pending.pop() {
        if std::mem::replace(&mut reached[index], true) {
            continue;
        }
        let machine = machines[index];
        if index == required || machine.blocks.is_empty() {
            summaries[index] = BeforeInvocation {
                enters: index == required,
                returns: index != required,
            };
            continue;
        }
        validate_control_graph(machine).map_err(|error| CompletionGraphError::InvalidControl {
            machine: machine.id,
            error,
        })?;
        let mut blocks: Vec<_> = machine.blocks.iter().collect();
        blocks.sort_unstable_by_key(|block| block.id);
        let block_position = |id: BlockId| {
            blocks
                .binary_search_by_key(&id, |block| block.id)
                .expect("the control graph was validated")
        };
        let mut body = DirectBody {
            entry: block_position(machine.entry),
            blocks: Vec::with_capacity(blocks.len()),
        };
        for block in &blocks {
            let mut direct = DirectBlock::default();
            let mut call = |callee| -> Result<(), CompletionGraphError> {
                let callee = position(callee)?;
                direct.calls.push(callee);
                if callers[callee].last() != Some(&index) {
                    callers[callee].push(index);
                    pending.push(callee);
                }
                Ok(())
            };
            for callee in block
                .operations
                .iter()
                .filter_map(|op| op.kind.direct_callee())
            {
                call(callee)?;
            }
            match completion_disposition(&block.terminator) {
                NormalCompletionDisposition::Return => direct.returns = true,
                NormalCompletionDisposition::Stop => {}
                NormalCompletionDisposition::Invoke(callee) => call(callee)?,
                NormalCompletionDisposition::Successors(targets) => direct
                    .successors
                    .extend(targets.into_iter().map(block_position)),
            }
            body.blocks.push(direct);
        }
        bodies[index] = Some(body);
    }

    // Both facts describe finite paths. Start with no such paths and grow
    // them through recursive calls until no caller learns another outcome.
    let mut queued: Vec<_> = bodies.iter().map(Option::is_some).collect();
    let mut pending: Vec<_> = (0..machines.len()).filter(|&index| queued[index]).collect();
    while let Some(index) = pending.pop() {
        queued[index] = false;
        let body = bodies[index]
            .as_ref()
            .expect("only a checked body has direct calls");
        let answer = body.before_invocation(&summaries);
        if summaries[index] != answer {
            summaries[index] = answer;
            for &caller in &callers[index] {
                if !std::mem::replace(&mut queued[caller], true) {
                    pending.push(caller);
                }
            }
        }
    }
    let answer = summaries[root];
    Ok(if !answer.enters {
        InvocationOnReturn::Unreached
    } else if answer.returns {
        InvocationOnReturn::Bypassed
    } else {
        InvocationOnReturn::Required
    })
}

struct DirectBody {
    entry: usize,
    blocks: Vec<DirectBlock>,
}

#[derive(Default)]
struct DirectBlock {
    calls: Vec<usize>,
    returns: bool,
    successors: Vec<usize>,
}

impl DirectBody {
    fn before_invocation(&self, summaries: &[BeforeInvocation]) -> BeforeInvocation {
        let mut answer = BeforeInvocation::default();
        let mut visited = vec![false; self.blocks.len()];
        let mut pending = vec![self.entry];
        'blocks: while let Some(index) = pending.pop() {
            if std::mem::replace(&mut visited[index], true) {
                continue;
            }
            let block = &self.blocks[index];
            for &callee in &block.calls {
                let callee = summaries[callee];
                answer.enters |= callee.enters;
                if !callee.returns {
                    continue 'blocks;
                }
            }
            answer.returns |= block.returns;
            pending.extend(&block.successors);
        }
        answer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{ContractId, EdgeId, OperationId, ScalarType, ValueId};
    use terminal_psi::{
        Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
        TerminalMachine, Terminator, ValueDeclaration,
    };

    fn machine(id: MachineId, blocks: Vec<Block>) -> TerminalMachine {
        let mut contract = MachineContract::empty(ContractId::new(id.get()).unwrap());
        contract.parameters.clone_from(&blocks[0].parameters);
        contract.parameter_order =
            vec![terminal_psi::BoundaryParameterKind::Scalar; contract.parameters.len()];
        TerminalMachine {
            call_formations: vec![],
            assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
            id,
            attachment: None,
            ranked_scc: None,
            structural_places: vec![],
            block_entry_claims: vec![],
            loan_completions: vec![],
            content_identity_reshuffles: vec![],
            content_partition_compositions: vec![],
            entry: blocks[0].id,
            blocks,
            contract,
        }
    }

    fn returning(block: u64, callees: &[MachineId]) -> Block {
        let mut body = Block::plain(
            BlockId::new(block).unwrap(),
            vec![],
            Terminator::ReturnUnit {
                edge: EdgeId::new(block).unwrap(),
                cleanup_actions: vec![],
            },
        );
        for (index, &callee) in callees.iter().enumerate() {
            body.operations.push(Operation::unsuspended(
                OperationId::new(block + index as u64).unwrap(),
                OperationResult::Unit,
                OperationKind::CallUnit {
                    callee,
                    arguments: vec![],
                    erased_arguments: vec![],
                    erased_proof_arguments: vec![],
                    structural_arguments: vec![],
                    claim_transfers: vec![],
                    requirement_obligations: vec![],
                    crash_continuations: vec![],
                },
            ));
        }
        body
    }

    fn split() -> Block {
        let successor = |edge, target| SuccessorEdge {
            edge: EdgeId::new(edge).unwrap(),
            target: BlockId::new(target).unwrap(),
            arguments: vec![],
            erased_arguments: vec![],
            erased_proof_arguments: vec![],
            structural_arguments: vec![],
            cleanup_actions: vec![],
        };
        let condition = ValueDeclaration {
            id: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        };
        Block::plain(
            BlockId::new(1).unwrap(),
            vec![condition],
            Terminator::Conditional {
                condition: condition.id,
                when_true: successor(4, 2),
                when_false: successor(5, 3),
            },
        )
    }

    #[test]
    fn forwarding_reaches_the_exact_callee_and_a_returning_bypass_is_reported() {
        let (root, helper, required, sibling) = (
            MachineId::new(1).unwrap(),
            MachineId::new(2).unwrap(),
            MachineId::new(3).unwrap(),
            MachineId::new(4).unwrap(),
        );
        let mut module = TerminalModule::declarations();
        module.machines = vec![
            machine(root, vec![returning(1, &[helper])]),
            machine(helper, vec![returning(1, &[required])]),
            machine(required, vec![returning(1, &[])]),
            machine(sibling, vec![returning(1, &[])]),
        ];
        assert_eq!(
            invocation_on_return(&module, root, required),
            Ok(InvocationOnReturn::Required)
        );
        assert_eq!(
            invocation_on_return(&module, root, sibling),
            Ok(InvocationOnReturn::Unreached)
        );
        module.machines[0] = machine(
            root,
            vec![split(), returning(2, &[helper]), returning(3, &[sibling])],
        );
        assert_eq!(
            invocation_on_return(&module, root, required),
            Ok(InvocationOnReturn::Bypassed)
        );
        module.machines[0].blocks[2] = returning(3, &[required]);
        assert_eq!(
            invocation_on_return(&module, root, required),
            Ok(InvocationOnReturn::Required)
        );
        // A crash or an external completion cannot return to the caller.
        module.machines[0].blocks[2] = returning(3, &[]);
        for terminator in [
            Terminator::Crash {
                edge: EdgeId::new(3).unwrap(),
                cause: semantic_vocabulary::CrashCause::Trap,
                site_guard: vec![],
                frontier_lower_bound: vec![],
            },
            Terminator::ExternalCompletion {
                edge: EdgeId::new(3).unwrap(),
                boundary: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
                kind: semantic_vocabulary::ExternalCompletionKind::ProcessExit,
                arguments: vec![],
                structural_arguments: vec![],
                requirement_obligations: vec![],
                frontier_lower_bound: vec![],
            },
        ] {
            module.machines[0].blocks[2].terminator = terminator;
            assert_eq!(
                invocation_on_return(&module, root, required),
                Ok(InvocationOnReturn::Required)
            );
        }
        // This call site exists textually, but unbounded recursion never reaches it.
        module.machines[0] = machine(root, vec![returning(1, &[helper])]);
        module.machines[1].blocks = vec![returning(1, &[helper, required])];
        assert_eq!(
            invocation_on_return(&module, root, required),
            Ok(InvocationOnReturn::Unreached)
        );
    }
}
