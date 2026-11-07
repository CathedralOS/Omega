//! Ordinary invocation inputs, independent of result publication and completion.
//!
//! A normal call operation and a no-normal invocation edge owe the same exact
//! callee-entry contract and argument custody. Consumers borrow this common
//! presentation instead of manufacturing a Unit call or a result place for an
//! invocation which cannot return. Result-specific and completion-specific
//! validation remains with the owning operation or terminator.

use super::{OperationKind, Terminator};
use crate::{ClaimTransfer, CrashRouteBucket, StructuralArgument};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ExternalCompletionKind, InvocationSite, MachineId, ObligationId,
    ProofTerm, ScalarTerm, ValueId,
};

/// One canonical boundary occurrence, borrowing its actual runtime operands.
/// A boundary identity alone cannot establish constraints on these operands or
/// the lifetime of a value passed at this site. This view grants no authority.
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

/// Result publication and external completion stay distinct. In particular an
/// external completion has neither a fabricated Unit result nor normal-return
/// receipts that a lifetime checker could use as a release obligation.
#[derive(Clone, Copy)]
pub enum BoundaryInvocationCompletion<'a> {
    Normal {
        result: &'a super::OperationResult,
        receipts: &'a [crate::CompletionReceipt],
    },
    External(ExternalCompletionKind),
}

impl super::Block {
    /// Every boundary occurrence in execution order, including the final
    /// external completion. The original slices remain in the canonical body.
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
pub struct ModuleInvocationInputs<'a> {
    pub callee: MachineId,
    pub arguments: &'a [ValueId],
    pub erased_arguments: &'a [ScalarTerm],
    pub erased_proof_arguments: &'a [ProofTerm],
    pub structural_arguments: &'a [StructuralArgument],
    pub claim_transfers: &'a [ClaimTransfer],
    pub requirement_obligations: &'a [ObligationId],
    pub crash_continuations: &'a [CrashRouteBucket],
}

impl OperationKind {
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

impl Terminator {
    pub fn module_invocation(&self) -> Option<ModuleInvocationInputs<'_>> {
        let Self::InvokeNoNormal {
            callee,
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

/// The call's exact producer identity stays separate from its entry operands.
#[derive(Clone, Copy)]
pub struct ModuleInvocation<'a> {
    pub site: semantic_vocabulary::InvocationSite,
    pub static_reach_binding: Option<u32>,
    pub inputs: ModuleInvocationInputs<'a>,
}

impl super::Block {
    /// Every ordinary invocation in execution order, including a no-normal
    /// final edge. Reachability and contract consumers share this inventory.
    pub fn module_invocations(&self) -> impl Iterator<Item = ModuleInvocation<'_>> {
        self.operations
            .iter()
            .filter_map(|operation| {
                operation
                    .kind
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
