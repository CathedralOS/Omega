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

/// Read a direct call through the operands its invocation declares, adding
/// only the call's result role and its outcome-specific evidence.
pub fn direct_call_parts(kind: &OperationKind) -> Option<DirectCallParts<'_>> {
    let (role, selected_evidence) = match kind {
        OperationKind::Call { .. } | OperationKind::CallStructuralScalar { .. } => {
            (DirectCallRole::Scalar, &[][..])
        }
        OperationKind::CallUnit { .. } => (DirectCallRole::Unit, &[][..]),
        OperationKind::CallStructural {
            selected_evidence, ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            selected_evidence, ..
        } => (DirectCallRole::Structural, selected_evidence.as_slice()),
        _ => return None,
    };
    let invocation = kind.module_invocation()?;
    Some(DirectCallParts {
        callee: invocation.callee,
        role,
        arguments: invocation.arguments,
        structural_arguments: invocation.structural_arguments,
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
