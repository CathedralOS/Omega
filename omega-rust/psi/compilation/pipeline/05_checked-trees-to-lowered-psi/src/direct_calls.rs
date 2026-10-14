//! Direct in-module calls: the one mapping between a call's facts and the
//! Terminal operation that spells it.
//!
//! Terminal Psi spells a direct call to an in-module machine as one of five
//! operations — `Call`, `CallUnit`, `CallStructuralScalar`, `CallStructural`
//! and `CallStructuralWithScalarArguments`. Which one a call takes follows
//! from what successful return binds (nothing, one scalar value or one
//! structural place) and which operand lanes cross the call.
//! [`DirectCall::into_operation_kind`] is the only place this stage's
//! lowering produces those operations. The operation decides the emitted
//! Terminal bytes. A scalar result always takes `CallStructuralScalar`: every
//! scalar callee is a Unit body that owns structural storage behind its scalar
//! signature, so this stage never emits the scalar-only `Call`, which Terminal
//! still decodes. [`DirectCallResult::Structural`] chooses between the two
//! structural-result forms.
//!
//! Consumers go the other way: [`direct_call_parts`] reads the callee, result
//! role and operand lanes back from whichever operation a call took, and
//! [`call_structural_arguments`] adds boundary calls for the consumers that
//! walk every structural argument a direct or boundary call passes. A
//! consumer that needs only those facts never lists the five operations.

use crate::emission::operation_emission::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::lowering_error::LoweringError;
use lowered_psi::LoweredSourceFrontierBindings;
use semantic_vocabulary::{MachineId, ObligationId, ProofTerm, ScalarTerm, ValueId};
use terminal_psi::{
    ClaimTransfer, CrashRouteBucket, Operation, OperationKind, OperationResult, StructuralArgument,
    StructuralMultiplicity, ValueDeclaration,
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

impl DirectCall {
    /// The one direct-call construction every lowering route shares; a route
    /// fills the lanes it prepared and hands the record to
    /// [`emit_direct_call`].
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        callee: MachineId,
        arguments: Vec<ValueId>,
        erased_arguments: Vec<ScalarTerm>,
        erased_proof_arguments: Vec<ProofTerm>,
        structural_arguments: Vec<StructuralArgument>,
        claim_transfers: Vec<ClaimTransfer>,
        requirement_obligations: Vec<ObligationId>,
        crash_continuations: Vec<CrashRouteBucket>,
        result: DirectCallResult,
    ) -> Self {
        DirectCall {
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            result,
        }
    }
}

/// How an emitted call operation records its source-call occurrence: against
/// the caller's lowered frontier rows, or against the scalar values live in
/// its operand frame before the call.
pub(crate) enum CallRecord<'a> {
    /// A separately checked sealed source request, never an ordinary call fact.
    Quotient(checked_trees::CheckedStructuralValueHandle),
    Frontier(&'a LoweredSourceFrontierBindings),
    Values(&'a [ValueDeclaration]),
}

/// The one call emitter: every direct-call route resolves its callee, mints
/// its result, prepares the operand lanes and lands here to record the
/// source-call occurrence and push the operation. The boundary route keeps
/// its own operation kind — a boundary application is not a machine call.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_direct_call(
    operations: &mut OperationBuffer,
    coordinate: SourceCallCoordinate,
    source_site: Option<checked_trees::NominalMachineUseSite>,
    source_target: symbols::SymbolHandle,
    record: CallRecord<'_>,
    result: OperationResult,
    call: DirectCall,
) -> Result<semantic_vocabulary::OperationId, LoweringError> {
    let operation = operations.allocate();
    match record {
        CallRecord::Quotient(value) => operations.quotient_calls.push(
            crate::emission::operation_emission::buffer::SourceQuotientCall {
                value,
                call: operation,
            },
        ),
        CallRecord::Frontier(frontier_before_call) => operations.record_source_call(
            coordinate,
            source_site,
            operation,
            source_target,
            frontier_before_call,
        )?,
        CallRecord::Values(source_values_before_call) => operations
            .record_source_call_with_values(
                coordinate,
                source_site,
                operation,
                source_target,
                source_values_before_call,
            )?,
    }
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result,
        kind: call.into_operation_kind(),
    });
    Ok(operation)
}

/// What successful return binds, with the facts that choose its operation.
pub(crate) enum DirectCallResult {
    /// Completion binds nothing: `CallUnit`, which carries every lane.
    Unit,
    /// Return binds one scalar value: `CallStructuralScalar`.
    Scalar,
    /// Return establishes one structural place. A linear result with no
    /// scalar or proof-only actual keeps the scalar-free `CallStructural`
    /// (which owns no erased lane); every other result — affine, unrestricted,
    /// or with any scalar or proof-only actual — takes
    /// `CallStructuralWithScalarArguments`.
    Structural {
        multiplicity: StructuralMultiplicity,
    },
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
            DirectCallResult::Scalar => OperationKind::CallStructuralScalar {
                callee,
                arguments,
                erased_arguments,
                erased_proof_arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            },
            DirectCallResult::Structural { multiplicity }
                if multiplicity == StructuralMultiplicity::Linear
                    && arguments.is_empty()
                    && erased_arguments.is_empty()
                    && erased_proof_arguments.is_empty() =>
            {
                OperationKind::CallStructural {
                    callee,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                    selected_evidence: Vec::new(),
                }
            }
            DirectCallResult::Structural { .. } => {
                OperationKind::CallStructuralWithScalarArguments {
                    callee,
                    arguments,
                    erased_arguments,
                    erased_proof_arguments,
                    structural_arguments,
                    claim_transfers,
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
