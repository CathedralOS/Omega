//! Source correspondence is checked before the instruction graph is retired.
use machine_code::fragment_origins::*;
use machine_code::{
    FunctionFragmentConditionalBranchPredicate, FunctionFragmentControlProvenance,
    FunctionFragmentSuccessorProvenance,
};
use selected_instructions::{
    SelectedBlock, SelectedFunction, SelectedInstruction, SelectedInstructionKind,
    SelectedSuccessor, SelectedTerminator, terminator_instruction,
};

pub fn prepare_fragment_origins(
    program: &post_allocation_machine::PostAllocationMachineProgram,
) -> FragmentOrigins {
    FragmentOrigins {
        psi: program.psi,
        work_schedule: program.work_schedule,
        selected: program.machine.selected,
        target: program.target,
        entry: program.entry,
        functions: program
            .functions
            .iter()
            .map(prepare_function_origin)
            .collect(),
    }
}
pub fn prepare_function_origin(function: &SelectedFunction) -> FragmentFunctionOrigin {
    FragmentFunctionOrigin {
        entry_block: function.entry_block,
        machine: function.machine,
        attachment: function.attachment,
        provenance: function.provenance.clone(),
        blocks: function
            .blocks
            .iter()
            .map(|block| FragmentBlockOrigin {
                id: block.id,
                instructions: block
                    .instructions
                    .iter()
                    .chain(std::iter::once(terminator_instruction(&block.terminator)))
                    .map(|instruction| {
                        let control =
                            control_provenance(function, block, instruction).and_then(|value| {
                                check_control_provenance(function, block, instruction, &value)?;
                                Ok(value)
                            });
                        let import_free = if let SelectedInstructionKind::NormalizedForeignCall {
                            boundary,
                            ordinal,
                        } = instruction.kind
                        {
                            function
                                .normalized_foreign_calls
                                .get(ordinal as usize)
                                .filter(|call| {
                                    call.instruction == instruction.id
                                        && call.call.boundary == boundary
                                })
                                .map(|call| call.call.binding.import_locator().is_none())
                                .ok_or(ResolvedFragmentEmissionError::RootMismatch)
                        } else {
                            Ok(false)
                        };
                        let mut origin = prepare_fixup_origin(instruction);
                        origin.control = control;
                        origin.import_free = import_free;
                        origin
                    })
                    .collect(),
            })
            .collect(),
    }
}
pub fn prepare_fixup_origin(instruction: &SelectedInstruction) -> FragmentInstructionOrigin {
    FragmentInstructionOrigin {
        id: instruction.id,
        provenance: instruction.provenance.clone(),
        control: Ok(FunctionFragmentControlProvenance::None),
        callee: instruction.kind.machine_callee(),
        foreign: if let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } =
            instruction.kind
        {
            Some((boundary, ordinal))
        } else {
            None
        },
        image_reference: if let SelectedInstructionKind::ImageAddress { reference } =
            instruction.kind
        {
            Some(reference)
        } else {
            None
        },
        import_free: Ok(false),
    }
}
fn require(condition: bool) -> Result<(), ResolvedFragmentEmissionError> {
    if condition {
        Ok(())
    } else {
        Err(ResolvedFragmentEmissionError::ArtifactMismatch)
    }
}

fn control_provenance(
    function: &selected_instructions::SelectedFunctionContracts,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
) -> Result<FunctionFragmentControlProvenance, ResolvedFragmentEmissionError> {
    if let selected_instructions::selected_call_pattern!(target, ..) = instruction.kind {
        let mut rows = function
            .calls
            .iter()
            .filter(|row| row.instruction == instruction.id);
        let row = rows
            .next()
            .ok_or(ResolvedFragmentEmissionError::ArtifactMismatch)?;
        if rows.next().is_some()
            || selected_instructions::SelectedCallTarget::from_native(row.call.target) != target
        {
            return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
        }
        if matches!(
            instruction.kind,
            SelectedInstructionKind::InvokeNoNormal { .. }
        ) {
            let (terminal, owner) = match &block.terminator {
                SelectedTerminator::InvokeNoNormal {
                    instruction,
                    psi_edge,
                } => (
                    instruction,
                    target_vocabulary::CallSiteOwner::InvokeNoNormal(*psi_edge),
                ),
                SelectedTerminator::OperationInvokeNoNormal {
                    instruction,
                    operation,
                } => (
                    instruction,
                    target_vocabulary::CallSiteOwner::Installation {
                        operation: *operation,
                        role: target_vocabulary::InstallationCallRole::Work,
                    },
                ),
                _ => return Err(ResolvedFragmentEmissionError::ArtifactMismatch),
            };
            if terminal.id != instruction.id || row.owner != owner {
                return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
            }
        } else if matches!(
            row.owner,
            target_vocabulary::CallSiteOwner::InvokeNoNormal(_)
                | target_vocabulary::CallSiteOwner::Installation {
                    role: target_vocabulary::InstallationCallRole::Work,
                    ..
                }
        ) {
            return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
        }
        return Ok(FunctionFragmentControlProvenance::InternalCall {
            target,
            owner: row.owner,
        });
    }
    if let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = instruction.kind {
        let row = function
            .normalized_foreign_calls
            .get(ordinal as usize)
            .ok_or(ResolvedFragmentEmissionError::ArtifactMismatch)?;
        if row.instruction != instruction.id || row.call.boundary != boundary {
            return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
        }
        return Ok(FunctionFragmentControlProvenance::NormalizedForeignCall {
            invocation_site: row.invocation_site,
            boundary,
            ordinal,
        });
    }
    Ok(match &block.terminator {
        SelectedTerminator::EliminateEmpty {
            instruction: terminal,
            psi_edge,
        } if terminal.id == instruction.id => FunctionFragmentControlProvenance::EliminateEmpty {
            psi_edge: *psi_edge,
        },
        SelectedTerminator::OperationTrap {
            instruction: terminal,
            operation,
        } if terminal.id == instruction.id => FunctionFragmentControlProvenance::OperationTrap {
            operation: *operation,
        },
        SelectedTerminator::Crash {
            instruction: terminal,
            psi_edge,
            cause,
            ..
        } if terminal.id == instruction.id => FunctionFragmentControlProvenance::Crash {
            psi_edge: *psi_edge,
            cause: *cause,
        },
        SelectedTerminator::Jump {
            instruction: jump,
            successor,
        } if jump.id == instruction.id => FunctionFragmentControlProvenance::Jump {
            successor: Box::new(successor_provenance(successor)),
        },
        SelectedTerminator::ConditionalBranch {
            instruction: branch,
            when_nonzero,
            when_zero,
        } if branch.id == instruction.id => FunctionFragmentControlProvenance::ConditionalBranch {
            predicate: FunctionFragmentConditionalBranchPredicate::NonZeroV1,
            when_taken: Box::new(successor_provenance(when_nonzero)),
            when_fallthrough: Box::new(successor_provenance(when_zero)),
        },
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction: branch,
            when_less,
            when_not_less,
        } if branch.id == instruction.id => FunctionFragmentControlProvenance::ConditionalBranch {
            predicate: FunctionFragmentConditionalBranchPredicate::U64LessThanV1,
            when_taken: Box::new(successor_provenance(when_less)),
            when_fallthrough: Box::new(successor_provenance(when_not_less)),
        },
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction: branch,
            when_less,
            when_not_less,
        } if branch.id == instruction.id => FunctionFragmentControlProvenance::ConditionalBranch {
            predicate: FunctionFragmentConditionalBranchPredicate::I64LessThanV1,
            when_taken: Box::new(successor_provenance(when_less)),
            when_fallthrough: Box::new(successor_provenance(when_not_less)),
        },
        SelectedTerminator::HostedExitProcess {
            instruction: exited,
            completion_edge,
        } if exited.id == instruction.id => FunctionFragmentControlProvenance::HostedExitProcess {
            completion_edge: *completion_edge,
        },
        SelectedTerminator::Return {
            instruction: returned,
            psi_return_edge,
        } if returned.id == instruction.id => FunctionFragmentControlProvenance::Return {
            psi_return_edge: *psi_return_edge,
        },
        _ => FunctionFragmentControlProvenance::None,
    })
}

fn successor_provenance(successor: &SelectedSuccessor) -> FunctionFragmentSuccessorProvenance {
    FunctionFragmentSuccessorProvenance {
        role: successor.role,
        psi_edge: successor.psi_edge,
        block: successor.block,
        source_target: successor.source_target,
        bindings: successor
            .bindings
            .iter()
            .map(|binding| binding.semantic)
            .collect(),
        work: successor.work.clone(),
    }
}

fn check_control_provenance(
    function: &SelectedFunction,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    actual: &FunctionFragmentControlProvenance,
) -> Result<(), ResolvedFragmentEmissionError> {
    use FunctionFragmentConditionalBranchPredicate as Predicate;
    use FunctionFragmentControlProvenance as Control;
    if let selected_instructions::selected_call_pattern!(target, ..) = instruction.kind {
        let mut rows = function
            .calls
            .iter()
            .filter(|row| row.instruction == instruction.id);
        let row = rows
            .next()
            .ok_or(ResolvedFragmentEmissionError::ArtifactMismatch)?;
        require(
            rows.next().is_none()
                && selected_instructions::SelectedCallTarget::from_native(row.call.target)
                    == target,
        )?;
        if matches!(
            instruction.kind,
            SelectedInstructionKind::InvokeNoNormal { .. }
        ) {
            let (terminal, owner) = match &block.terminator {
                SelectedTerminator::InvokeNoNormal {
                    instruction,
                    psi_edge,
                } => (
                    instruction,
                    target_vocabulary::CallSiteOwner::InvokeNoNormal(*psi_edge),
                ),
                SelectedTerminator::OperationInvokeNoNormal {
                    instruction,
                    operation,
                } => (
                    instruction,
                    target_vocabulary::CallSiteOwner::Installation {
                        operation: *operation,
                        role: target_vocabulary::InstallationCallRole::Work,
                    },
                ),
                _ => return Err(ResolvedFragmentEmissionError::ArtifactMismatch),
            };
            if terminal.id != instruction.id || row.owner != owner {
                return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
            }
        } else if matches!(
            row.owner,
            target_vocabulary::CallSiteOwner::InvokeNoNormal(_)
                | target_vocabulary::CallSiteOwner::Installation {
                    role: target_vocabulary::InstallationCallRole::Work,
                    ..
                }
        ) {
            return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
        }
        return require(
            matches!(actual, Control::InternalCall { target: actual_target, owner } if *actual_target == target && *owner == row.owner),
        );
    }
    if let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = instruction.kind {
        let row = function
            .normalized_foreign_calls
            .get(ordinal as usize)
            .ok_or(ResolvedFragmentEmissionError::ArtifactMismatch)?;
        require(row.instruction == instruction.id && row.call.boundary == boundary)?;
        return require(matches!(
            actual,
            Control::NormalizedForeignCall {
                invocation_site,
                boundary: actual_boundary,
                ordinal: actual_ordinal,
            } if *actual_boundary == boundary && *actual_ordinal == ordinal
                && *invocation_site == row.invocation_site
        ));
    }
    let (terminal, predicate, successors) = match &block.terminator {
        SelectedTerminator::InvokeNoNormal {
            instruction: terminal,
            ..
        }
        | SelectedTerminator::OperationInvokeNoNormal {
            instruction: terminal,
            ..
        } => {
            return require(terminal.id != instruction.id && matches!(actual, Control::None));
        }

        SelectedTerminator::OperationTrap {
            instruction: terminal,
            operation,
        } => {
            return if terminal.id == instruction.id {
                require(
                    matches!(actual, Control::OperationTrap { operation: actual } if actual == operation),
                )
            } else {
                require(matches!(actual, Control::None))
            };
        }
        SelectedTerminator::EliminateEmpty {
            instruction: terminal,
            psi_edge,
        } => {
            return if terminal.id == instruction.id {
                require(
                    matches!(actual, Control::EliminateEmpty { psi_edge: edge } if edge == psi_edge),
                )
            } else {
                require(matches!(actual, Control::None))
            };
        }
        SelectedTerminator::Crash {
            instruction: terminal,
            psi_edge,
            cause,
            ..
        } => {
            return if terminal.id == instruction.id {
                require(
                    matches!(actual, Control::Crash { psi_edge: edge, cause: actual_cause }
                    if edge == psi_edge && actual_cause == cause),
                )
            } else {
                require(matches!(actual, Control::None))
            };
        }
        SelectedTerminator::Jump {
            instruction: terminal,
            successor: target,
        } => {
            return if terminal.id == instruction.id {
                let Control::Jump { successor: actual } = actual else {
                    return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
                };
                check_successor_provenance(target, actual)
            } else {
                require(matches!(actual, Control::None))
            };
        }
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => (instruction, Predicate::NonZeroV1, (when_nonzero, when_zero)),
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => (
            instruction,
            Predicate::U64LessThanV1,
            (when_less, when_not_less),
        ),
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => (
            instruction,
            Predicate::I64LessThanV1,
            (when_less, when_not_less),
        ),
        SelectedTerminator::HostedExitProcess {
            instruction: terminal,
            completion_edge,
        } => {
            return if terminal.id == instruction.id {
                require(
                    matches!(actual, Control::HostedExitProcess { completion_edge: edge } if edge == completion_edge),
                )
            } else {
                require(matches!(actual, Control::None))
            };
        }
        SelectedTerminator::Return {
            instruction: terminal,
            psi_return_edge,
        } => {
            return if terminal.id == instruction.id {
                require(
                    matches!(actual, Control::Return { psi_return_edge: edge } if edge == psi_return_edge),
                )
            } else {
                require(matches!(actual, Control::None))
            };
        }
    };
    if terminal.id != instruction.id {
        return require(matches!(actual, Control::None));
    }
    let Control::ConditionalBranch {
        predicate: actual_predicate,
        when_taken,
        when_fallthrough,
    } = actual
    else {
        return Err(ResolvedFragmentEmissionError::ArtifactMismatch);
    };
    let (taken, fallthrough) = successors;
    require(*actual_predicate == predicate)?;
    check_successor_provenance(taken, when_taken)?;
    check_successor_provenance(fallthrough, when_fallthrough)
}

pub fn check_successor_provenance(
    source: &SelectedSuccessor,
    actual: &FunctionFragmentSuccessorProvenance,
) -> Result<(), ResolvedFragmentEmissionError> {
    require(
        actual.role == source.role
            && actual.psi_edge == source.psi_edge
            && (actual.role == selected_instructions::SelectedSuccessorRole::Semantic
                || actual.work.is_empty())
            && actual.block == source.block
            && actual.source_target == source.source_target
            && actual
                .bindings
                .iter()
                .copied()
                .eq(source.bindings.iter().map(|binding| binding.semantic))
            && actual.work == source.work,
    )
}
