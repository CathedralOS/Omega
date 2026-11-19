use crate::{OperationKind, OutcomeSpecificCallEvidence, StructuralArgument};
use semantic_vocabulary::{MachineId, ValueId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectCallRole {
    Unit,
    Scalar,
    Structural,
    NoNormal,
}

#[derive(Clone, Copy, Debug)]
pub struct DirectCallParts<'a> {
    pub callee: MachineId,
    pub role: DirectCallRole,
    pub arguments: &'a [ValueId],
    pub structural_arguments: &'a [StructuralArgument],
    pub selected_evidence: &'a [OutcomeSpecificCallEvidence],
}

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
