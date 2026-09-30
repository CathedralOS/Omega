//! Direct in-module calls: the one mapping between a call's facts and the
//! Terminal operation that spells it.
//!
//! Terminal Psi spells a direct call to an in-module machine as one of five
//! operations — `Call`, `CallUnit`, `CallStructuralScalar`, `CallStructural`
//! and `CallStructuralWithScalarArguments`. Which one a call takes follows
//! from what successful return binds (nothing, one scalar value or one
//! structural place) and which operand lanes cross the call.
//! [`DirectCall::into_operation_kind`] is the only place this stage's
//! lowering produces those five operations. The operation decides the emitted
//! Terminal bytes, so the choice has exactly two rules, each written once:
//! [`ScalarCallFrame::for_call`] for a scalar result and
//! [`DirectCallResult::Structural`] for a structural one. A route whose form
//! is fixed by construction names that form instead of re-deriving it.
//!
//! Consumers go the other way: [`direct_call_parts`] reads the callee, result
//! role and operand lanes back from whichever operation a call took, and
//! [`call_structural_arguments`] adds boundary calls for the consumers that
//! walk every structural argument a direct or boundary call passes. A
//! consumer that needs only those facts never lists the five operations.

use semantic_vocabulary::{MachineId, ObligationId, ProofTerm, ScalarTerm, ValueId};
use terminal_psi::{
    ClaimTransfer, CrashRouteBucket, OperationKind, StructuralArgument, StructuralMultiplicity,
    StructuralResultClaimTransfer,
};

/// One direct in-module call, before its Terminal spelling is chosen.
/// Lanes the chosen operation does not carry must be empty; the result mode
/// only selects them away after its rule found them empty.
pub(crate) struct DirectCall {
    pub(crate) callee: MachineId,
    pub(crate) arguments: Vec<ValueId>,
    pub(crate) erased_arguments: Vec<ScalarTerm>,
    pub(crate) erased_proof_arguments: Vec<ProofTerm>,
    pub(crate) structural_arguments: Vec<StructuralArgument>,
    pub(crate) claim_transfers: Vec<ClaimTransfer>,
    pub(crate) requirement_obligations: Vec<ObligationId>,
    pub(crate) crash_continuations: Vec<CrashRouteBucket>,
    pub(crate) result: DirectCallResult,
}

/// What successful return binds, with the facts that choose its operation.
pub(crate) enum DirectCallResult {
    /// Completion binds nothing: `CallUnit`, which carries every lane.
    Unit,
    /// Return binds one scalar value through the chosen frame.
    Scalar(ScalarCallFrame),
    /// Return establishes one structural place. A linear result with no
    /// scalar or proof-only actual keeps the scalar-free `CallStructural`
    /// (which owns no erased lane); every other result — affine, unrestricted,
    /// or with any scalar or proof-only actual — takes
    /// `CallStructuralWithScalarArguments`.
    Structural {
        multiplicity: StructuralMultiplicity,
        returned_claim_transfers: Vec<StructuralResultClaimTransfer>,
    },
    /// Terminal's bounded payloadless `CallStructural` form: no operand,
    /// claim or contract lane crosses, and return establishes one unrestricted
    /// exact structural case whose outcome evidence a later pass selects.
    PayloadlessCase,
}

/// The frame a scalar-result call crosses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScalarCallFrame {
    /// Only positional scalar and erased actuals cross: `Call`.
    ScalarOnly,
    /// Structural custody crosses, or the callee's body owns structural
    /// storage behind a scalar signature: `CallStructuralScalar`.
    Structural,
}

impl ScalarCallFrame {
    /// The ordinary rule: any structural argument or claim transfer crossing
    /// the call, or a callee whose selected body requires a structural frame,
    /// takes the structural frame. A caller decides this from its checked
    /// facts before lowering operands, since only a structural frame lowers
    /// structural arguments and their transfers.
    pub(crate) fn for_call(
        crosses_structural_argument: bool,
        crosses_claim_transfer: bool,
        callee_requires_structural_frame: bool,
    ) -> Self {
        if crosses_structural_argument || crosses_claim_transfer || callee_requires_structural_frame
        {
            Self::Structural
        } else {
            Self::ScalarOnly
        }
    }
}

impl DirectCall {
    pub(crate) fn into_operation_kind(self) -> OperationKind {
        let Self {
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            result,
        } = self;
        match result {
            DirectCallResult::Unit => OperationKind::CallUnit {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            },
            DirectCallResult::Scalar(ScalarCallFrame::ScalarOnly) => {
                debug_assert!(
                    structural_arguments.is_empty() && claim_transfers.is_empty(),
                    "a scalar-only call frame carries no structural custody"
                );
                OperationKind::Call {
                    callee,
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    requirement_obligations,
                    crash_continuations,
                }
            }
            DirectCallResult::Scalar(ScalarCallFrame::Structural) => {
                OperationKind::CallStructuralScalar {
                    callee,
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                }
            }
            DirectCallResult::Structural {
                multiplicity,
                returned_claim_transfers,
            } if multiplicity == StructuralMultiplicity::Linear
                && arguments.is_empty()
                && erased_arguments.is_empty()
                && erased_proof_arguments.is_empty() =>
            {
                OperationKind::CallStructural {
                    callee,
                    structural_arguments,
                    claim_transfers,
                    returned_claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                    selected_evidence: Vec::new(),
                }
            }
            DirectCallResult::Structural {
                returned_claim_transfers,
                ..
            } => OperationKind::CallStructuralWithScalarArguments {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                returned_claim_transfers,
                requirement_obligations,
                crash_continuations,
            },
            DirectCallResult::PayloadlessCase => {
                debug_assert!(
                    arguments.is_empty()
                        && erased_arguments.is_empty()
                        && erased_proof_arguments.is_empty()
                        && structural_arguments.is_empty()
                        && claim_transfers.is_empty()
                        && requirement_obligations.is_empty()
                        && crash_continuations.is_empty(),
                    "the payloadless structural call form carries no lane"
                );
                OperationKind::CallStructural {
                    callee,
                    structural_arguments,
                    claim_transfers,
                    returned_claim_transfers: Vec::new(),
                    requirement_obligations,
                    crash_continuations,
                    selected_evidence: Vec::new(),
                }
            }
        }
    }
}

/// What successful return of an emitted direct call binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectCallRole {
    Unit,
    Scalar,
    Structural,
}

/// A direct call's callee, result role and operand lanes, borrowed from
/// whichever of the five Terminal operations it took. A lane the operation
/// does not carry reads as empty.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DirectCallParts<'a> {
    pub(crate) callee: MachineId,
    pub(crate) role: DirectCallRole,
    pub(crate) arguments: &'a [ValueId],
    pub(crate) structural_arguments: &'a [StructuralArgument],
}

/// The direct in-module call `kind` spells, or `None` for any other
/// operation, including boundary and dynamic calls.
pub(crate) fn direct_call_parts(kind: &OperationKind) -> Option<DirectCallParts<'_>> {
    let (callee, role, arguments, structural_arguments) = match kind {
        OperationKind::Call {
            callee, arguments, ..
        } => (
            callee,
            DirectCallRole::Scalar,
            arguments.as_slice(),
            &[][..],
        ),
        OperationKind::CallUnit {
            callee,
            arguments,
            structural_arguments,
            ..
        } => (
            callee,
            DirectCallRole::Unit,
            arguments.as_slice(),
            structural_arguments.as_slice(),
        ),
        OperationKind::CallStructuralScalar {
            callee,
            arguments,
            structural_arguments,
            ..
        } => (
            callee,
            DirectCallRole::Scalar,
            arguments.as_slice(),
            structural_arguments.as_slice(),
        ),
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            ..
        } => (
            callee,
            DirectCallRole::Structural,
            &[][..],
            structural_arguments.as_slice(),
        ),
        OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments,
            structural_arguments,
            ..
        } => (
            callee,
            DirectCallRole::Structural,
            arguments.as_slice(),
            structural_arguments.as_slice(),
        ),
        _ => return None,
    };
    Some(DirectCallParts {
        callee: *callee,
        role,
        arguments,
        structural_arguments,
    })
}

/// The structural arguments a direct or boundary call passes; empty for any
/// other operation.
pub(crate) fn call_structural_arguments(kind: &OperationKind) -> &[StructuralArgument] {
    match kind {
        OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments,
        kind => direct_call_parts(kind).map_or(&[], |call| call.structural_arguments),
    }
}

/// The structural argument lane of a direct or boundary call, for rewriting
/// argument places in place; `None` for an operation without one.
pub(crate) fn call_structural_arguments_mut(
    kind: &mut OperationKind,
) -> Option<&mut Vec<StructuralArgument>> {
    match kind {
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => Some(structural_arguments),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
