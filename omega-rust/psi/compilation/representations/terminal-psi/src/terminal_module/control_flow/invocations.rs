//! Ordinary invocation inputs, independent of result publication and completion.
//!
//! A normal call operation and a no-normal invocation edge owe the same exact
//! callee-entry contract and argument custody. Consumers borrow this common
//! presentation instead of manufacturing a Unit call or a result place for an
//! invocation which cannot return. Result-specific and completion-specific
//! validation remains with the owning operation or terminator.

use super::{OperationKind, Terminator};
use crate::{ClaimTransfer, CrashRouteBucket, StructuralArgument};
use semantic_vocabulary::{MachineId, ObligationId, ProofTerm, ScalarTerm, ValueId};

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
