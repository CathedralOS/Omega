//! Read-only decomposition of the canonical direct in-module call vocabulary.
//! Producers and public consumers share these exact operand/result roles;
//! boundary and dynamic calls keep their independent public contract owners.
use crate::{OperationKind, OutcomeSpecificCallEvidence, StructuralArgument};
use semantic_vocabulary::{MachineId, ValueId};

/// What successful return of an emitted direct call binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectCallRole {
    Unit,
    Scalar,
    Structural,
    NoNormal,
}

/// A direct call's callee, result role and operand lanes, borrowed from
/// whichever of the five Terminal operations it took. A lane the operation
/// does not carry reads as empty.
#[derive(Clone, Copy, Debug)]
pub struct DirectCallParts<'a> {
    pub callee: MachineId,
    pub role: DirectCallRole,
    pub arguments: &'a [ValueId],
    pub structural_arguments: &'a [StructuralArgument],
    pub selected_evidence: &'a [OutcomeSpecificCallEvidence],
}

/// The direct in-module call `kind` spells, or `None` for any other
/// operation, including boundary and dynamic calls.
pub fn direct_call_parts(kind: &OperationKind) -> Option<DirectCallParts<'_>> {
    let (callee, role, arguments, structural_arguments, selected_evidence) = match kind {
        OperationKind::Call {
            callee, arguments, ..
        } => (
            callee,
            DirectCallRole::Scalar,
            arguments.as_slice(),
            &[][..],
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
            &[][..],
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
            &[][..],
        ),
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            selected_evidence,
            ..
        } => (
            callee,
            DirectCallRole::Structural,
            &[][..],
            structural_arguments.as_slice(),
            selected_evidence.as_slice(),
        ),
        OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments,
            structural_arguments,
            selected_evidence,
            ..
        } => (
            callee,
            DirectCallRole::Structural,
            arguments.as_slice(),
            structural_arguments.as_slice(),
            selected_evidence.as_slice(),
        ),
        _ => return None,
    };
    Some(DirectCallParts {
        callee: *callee,
        role,
        arguments,
        structural_arguments,
        selected_evidence,
    })
}

/// The structural arguments passed by direct, boundary or dynamic calls.
/// Operand custody is independent of dispatch; non-call operations are empty.
pub fn call_structural_arguments(kind: &OperationKind) -> &[StructuralArgument] {
    match kind {
        OperationKind::BoundaryCall {
            structural_arguments,
            ..
        }
        | OperationKind::CallDynamic {
            structural_arguments,
            ..
        } => structural_arguments,
        kind => direct_call_parts(kind).map_or(&[], |call| call.structural_arguments),
    }
}
