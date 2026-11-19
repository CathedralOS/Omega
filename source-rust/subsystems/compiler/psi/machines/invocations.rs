use super::{OperationKind, Terminator};
use crate::{ClaimTransfer, CrashRouteBucket, StructuralArgument};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ExternalCompletionKind, InvocationSite, MachineId, ObligationId,
    ProofTerm, ScalarTerm, ValueId,
};

#[derive(Clone, Copy)]
pub struct BoundaryInvocation<'a> {
    pub block: BlockId,
    pub site: InvocationSite,
    pub boundary: BoundaryMachineId,
    pub arguments: &'a [ValueId],
    pub structural_arguments: &'a [StructuralArgument],
    pub requirement_obligations: &'a [ObligationId],
    pub completion: BoundaryInvocationCompletion<'a>,
}

#[derive(Clone, Copy)]
pub enum BoundaryInvocationCompletion<'a> {
    Normal {
        result: &'a super::OperationResult,
        receipts: &'a [crate::CompletionReceipt],
    },
    External(ExternalCompletionKind),
}

impl super::Block {
    pub fn boundary_invocations(&self) -> impl Iterator<Item = BoundaryInvocation<'_>> {
        self.operations
            .iter()
            .filter_map(|operation| {
                let OperationKind::BoundaryCall {
                    boundary,
                    arguments,
                    structural_arguments,
                    completion_receipts,
                    requirement_obligations,
                } = &operation.kind
                else {
                    return None;
                };
                Some(BoundaryInvocation {
                    block: self.id,
                    site: InvocationSite::Operation(operation.id),
                    boundary: *boundary,
                    arguments,
                    structural_arguments,
                    requirement_obligations,
                    completion: BoundaryInvocationCompletion::Normal {
                        result: &operation.result,
                        receipts: completion_receipts,
                    },
                })
            })
            .chain(match &self.terminator {
                Terminator::ExternalCompletion {
                    edge,
                    boundary,
                    kind,
                    arguments,
                    structural_arguments,
                    requirement_obligations,
                    ..
                } => Some(BoundaryInvocation {
                    block: self.id,
                    site: InvocationSite::ExternalCompletion(*edge),
                    boundary: *boundary,
                    arguments,
                    structural_arguments,
                    requirement_obligations,
                    completion: BoundaryInvocationCompletion::External(*kind),
                }),
                _ => None,
            })
    }
}

impl super::TerminalMachine {
    pub fn boundary_invocations(&self) -> impl Iterator<Item = BoundaryInvocation<'_>> {
        self.blocks
            .iter()
            .flat_map(super::Block::boundary_invocations)
    }
}

#[derive(Clone, Copy)]
pub struct CallableInvocationInputs<'a> {
    pub callee: ValueId,
    pub contract: semantic_vocabulary::CallableContractId,
    pub arguments: &'a [ValueId],
    pub erased_arguments: &'a [ScalarTerm],
    pub erased_proof_arguments: &'a [ProofTerm],
    pub structural_arguments: &'a [StructuralArgument],
    pub claim_transfers: &'a [ClaimTransfer],
    pub requirement_obligations: &'a [ObligationId],
    pub crash_continuations: &'a [CrashRouteBucket],
}

#[derive(Clone, Copy)]
pub struct CallableInvocation<'a> {
    pub site: semantic_vocabulary::InvocationSite,
    pub inputs: CallableInvocationInputs<'a>,
}

impl OperationKind {
    pub fn callable_invocation(&self) -> Option<CallableInvocationInputs<'_>> {
        let Self::CallMachine {
            callee,
            contract,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } = self
        else {
            return None;
        };
        Some(CallableInvocationInputs {
            callee: *callee,
            contract: *contract,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        })
    }
}

impl super::TerminalMachine {
    pub fn callable_invocations(&self) -> impl Iterator<Item = CallableInvocation<'_>> {
        self.operations().filter_map(|operation| {
            operation
                .kind
                .callable_invocation()
                .map(|inputs| CallableInvocation {
                    site: semantic_vocabulary::InvocationSite::Operation(operation.id),
                    inputs,
                })
        })
    }
}

#[derive(Clone, Copy)]
pub struct ModuleInvocationInputs<'a, Callable = MachineId> {
    pub callee: Callable,
    /// A selected call uses this public contract while executing `callee`.
    /// A direct call leaves it absent and uses the body's own contract.
    pub required_contract: Option<semantic_vocabulary::ContractId>,
    pub arguments: &'a [ValueId],
    pub erased_arguments: &'a [ScalarTerm],
    pub erased_proof_arguments: &'a [ProofTerm],
    pub structural_arguments: &'a [StructuralArgument],
    pub claim_transfers: &'a [ClaimTransfer],
    pub completion_receipts: &'a [crate::CompletionReceipt],
    pub requirement_obligations: &'a [ObligationId],
    pub crash_continuations: &'a [CrashRouteBucket],
}

impl<'a> ModuleInvocationInputs<'a> {
    pub fn declared(self) -> ModuleInvocationInputs<'a, super::TargetMachineCallable> {
        ModuleInvocationInputs {
            callee: super::TargetMachineCallable::Machine(self.callee),
            required_contract: self.required_contract,
            arguments: self.arguments,
            erased_arguments: self.erased_arguments,
            erased_proof_arguments: self.erased_proof_arguments,
            structural_arguments: self.structural_arguments,
            claim_transfers: self.claim_transfers,
            completion_receipts: self.completion_receipts,
            requirement_obligations: self.requirement_obligations,
            crash_continuations: self.crash_continuations,
        }
    }
}

impl OperationKind {
    /// The two declaration namespaces share call operands and obligation
    /// custody. Machine-only consumers keep `module_invocation`; callers
    /// judging interface contracts use the exact declared target here.
    pub fn declared_invocation(
        &self,
    ) -> Option<ModuleInvocationInputs<'_, super::TargetMachineCallable>> {
        if let Self::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            completion_receipts,
            requirement_obligations,
        } = self
        {
            return Some(ModuleInvocationInputs {
                callee: super::TargetMachineCallable::Boundary(*boundary),
                required_contract: None,
                arguments,
                erased_arguments: &[],
                erased_proof_arguments: &[],
                structural_arguments,
                claim_transfers: &[],
                completion_receipts,
                requirement_obligations,
                crash_continuations: &[],
            });
        }
        self.module_invocation()
            .map(ModuleInvocationInputs::declared)
    }

    pub fn module_invocation(&self) -> Option<ModuleInvocationInputs<'_>> {
        let (
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        ) = match self {
            Self::Call {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                requirement_obligations,
                crash_continuations,
            } => (
                *callee,
                arguments.as_slice(),
                erased_arguments.as_slice(),
                erased_proof_arguments.as_slice(),
                &[][..],
                &[][..],
                requirement_obligations.as_slice(),
                crash_continuations.as_slice(),
            ),
            Self::CallUnit {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            }
            | Self::CallStructuralScalar {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            }
            | Self::CallStructuralWithScalarArguments {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            } => (
                *callee,
                arguments.as_slice(),
                erased_arguments.as_slice(),
                erased_proof_arguments.as_slice(),
                structural_arguments.as_slice(),
                claim_transfers.as_slice(),
                requirement_obligations.as_slice(),
                crash_continuations.as_slice(),
            ),
            Self::CallStructural {
                callee,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            } => (
                *callee,
                &[][..],
                &[][..],
                &[][..],
                structural_arguments.as_slice(),
                claim_transfers.as_slice(),
                requirement_obligations.as_slice(),
                crash_continuations.as_slice(),
            ),
            _ => return None,
        };
        Some(ModuleInvocationInputs {
            callee,
            required_contract: None,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            completion_receipts: &[],
            requirement_obligations,
            crash_continuations,
        })
    }
}

impl Terminator {
    /// A terminator that is a call, with its invocation site: a machine
    /// call with no normal return, or a boundary requirement that
    /// completes externally. The two declaration namespaces share call
    /// operands and obligation custody here as they do for calls that
    /// return (`OperationKind::declared_invocation`).
    pub fn declared_invocation(
        &self,
    ) -> Option<(
        InvocationSite,
        ModuleInvocationInputs<'_, super::TargetMachineCallable>,
    )> {
        if let Self::ExternalCompletion {
            edge,
            boundary,
            arguments,
            structural_arguments,
            requirement_obligations,
            ..
        } = self
        {
            return Some((
                InvocationSite::ExternalCompletion(*edge),
                ModuleInvocationInputs {
                    callee: super::TargetMachineCallable::Boundary(*boundary),
                    required_contract: None,
                    arguments,
                    erased_arguments: &[],
                    erased_proof_arguments: &[],
                    structural_arguments,
                    claim_transfers: &[],
                    completion_receipts: &[],
                    requirement_obligations,
                    crash_continuations: &[],
                },
            ));
        }
        self.module_invocation().map(|inputs| {
            (
                InvocationSite::InvokeNoNormal(self.edge()),
                inputs.declared(),
            )
        })
    }

    pub fn module_invocation(&self) -> Option<ModuleInvocationInputs<'_>> {
        let Self::InvokeNoNormal {
            callee,
            required_contract,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        } = self
        else {
            return None;
        };
        Some(ModuleInvocationInputs {
            callee: *callee,
            required_contract: *required_contract,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            completion_receipts: &[],
            requirement_obligations,
            crash_continuations,
        })
    }
}

#[derive(Clone, Copy)]
pub struct ModuleInvocation<'a> {
    pub site: semantic_vocabulary::InvocationSite,
    pub static_reach_binding: Option<u32>,
    pub inputs: ModuleInvocationInputs<'a>,
}

impl super::Block {
    pub fn module_invocations(&self) -> impl Iterator<Item = ModuleInvocation<'_>> {
        self.operations
            .iter()
            .filter_map(|operation| {
                operation
                    .module_invocation()
                    .map(|inputs| ModuleInvocation {
                        site: semantic_vocabulary::InvocationSite::Operation(operation.id),
                        static_reach_binding: operation.static_reach_binding,
                        inputs,
                    })
            })
            .chain(
                self.terminator
                    .module_invocation()
                    .map(|inputs| ModuleInvocation {
                        site: semantic_vocabulary::InvocationSite::InvokeNoNormal(
                            self.terminator.edge(),
                        ),
                        static_reach_binding: match self.terminator {
                            Terminator::InvokeNoNormal {
                                static_reach_binding,
                                ..
                            } => static_reach_binding,
                            _ => None,
                        },
                        inputs,
                    }),
            )
            .chain(
                self.terminator
                    .cleanup_rosters()
                    .flat_map(|(edge, actions)| {
                        actions
                            .iter()
                            .enumerate()
                            .filter_map(move |(ordinal, action)| {
                                let crate::TerminalAffineCleanupAction::InvokeOwned(cleanup) =
                                    action
                                else {
                                    return None;
                                };
                                Some(ModuleInvocation {
                                    site: semantic_vocabulary::InvocationSite::Cleanup {
                                        edge,
                                        action_ordinal: u32::try_from(ordinal).expect(
                                            "cleanup action ordinal exceeds its portable identity",
                                        ),
                                    },
                                    static_reach_binding: None,
                                    inputs: cleanup.module_invocation(),
                                })
                            })
                    }),
            )
    }
}

impl super::TerminalMachine {
    pub fn module_invocations(&self) -> impl Iterator<Item = ModuleInvocation<'_>> {
        self.blocks
            .iter()
            .flat_map(super::Block::module_invocations)
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    use crate::{Block, Operation, OperationResult};
    use semantic_vocabulary::{EdgeId, OperationId};

    #[test]
    fn cleanup_calls_keep_their_real_action_positions_and_owned_actuals() {
        let edge = EdgeId::new(9).unwrap();
        let place = semantic_vocabulary::PlaceId::new(3).unwrap();
        let callee = MachineId::new(2).unwrap();
        let subject = crate::StructuralArgument {
            place,
            path: Vec::new(),
            access: crate::StructuralAccess::Owned,
        };
        let cleanup = crate::OwnedCleanupInvocation {
            subject: subject.clone(),
            structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            cleanup_machine: callee,
            claim_transfers: Vec::new(),
            requirement_obligations: vec![semantic_vocabulary::ObligationId::new(7).unwrap()],
        };
        let block = Block::plain(
            BlockId::new(1).unwrap(),
            Vec::new(),
            Terminator::ReturnUnit {
                edge,
                cleanup_actions: vec![
                    crate::TerminalAffineCleanupAction::RetireHome(place),
                    crate::TerminalAffineCleanupAction::InvokeOwned(cleanup.clone()),
                    crate::TerminalAffineCleanupAction::InvokeOwned(cleanup),
                ],
            },
        );
        let calls = block.module_invocations().collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        for (call, action_ordinal) in calls.iter().zip([1, 2]) {
            assert_eq!(
                call.site,
                InvocationSite::Cleanup {
                    edge,
                    action_ordinal
                }
            );
            assert_eq!(call.inputs.callee, callee);
            assert_eq!(
                call.inputs.structural_arguments,
                std::slice::from_ref(&subject)
            );
            assert_eq!(call.inputs.requirement_obligations.len(), 1);
        }
    }

    #[test]
    fn repeated_boundary_keeps_distinct_operands_and_completions() {
        let boundary = BoundaryMachineId::new(1).unwrap();
        let normal_value = ValueId::new(1).unwrap();
        let external_value = ValueId::new(2).unwrap();
        let operation = OperationId::new(1).unwrap();
        let edge = EdgeId::new(1).unwrap();
        let mut block = Block::plain(
            BlockId::new(1).unwrap(),
            Vec::new(),
            Terminator::ExternalCompletion {
                edge,
                boundary,
                kind: ExternalCompletionKind::ProcessExit,
                arguments: vec![external_value],
                structural_arguments: Vec::new(),
                requirement_obligations: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
        );
        block.operations.push(Operation::unsuspended(
            operation,
            OperationResult::Unit,
            OperationKind::BoundaryCall {
                boundary,
                arguments: vec![normal_value],
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            },
        ));
        let occurrences: Vec<_> = block.boundary_invocations().collect();
        assert_eq!(occurrences.len(), 2);
        assert_eq!(occurrences[0].boundary, occurrences[1].boundary);
        assert_eq!(occurrences[0].site, InvocationSite::Operation(operation));
        assert_eq!(occurrences[0].arguments, [normal_value]);
        assert!(matches!(
            occurrences[0].completion,
            BoundaryInvocationCompletion::Normal {
                result: OperationResult::Unit,
                receipts: [],
            }
        ));
        assert_eq!(
            occurrences[1].site,
            InvocationSite::ExternalCompletion(edge)
        );
        assert_eq!(occurrences[1].arguments, [external_value]);
        assert!(matches!(
            occurrences[1].completion,
            BoundaryInvocationCompletion::External(ExternalCompletionKind::ProcessExit)
        ));
    }
}
