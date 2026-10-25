//! Operand custody is independent of dispatch and result shape. Cleanup and
//! projected-move accounting must see the same owned transfers for direct,
//! boundary and dynamic invocations; omitting one leaves stale disposal debt.
//! Direct callee identity remains with `direct_calls`.

use terminal_psi::{OperationKind, StructuralArgument};

pub(crate) fn structural_arguments(kind: &OperationKind) -> &[StructuralArgument] {
    match kind {
        OperationKind::BoundaryCall {
            structural_arguments,
            ..
        }
        | OperationKind::CallDynamic {
            structural_arguments,
            ..
        } => structural_arguments,
        kind => crate::direct_calls::direct_call_parts(kind)
            .map_or(&[], |call| call.structural_arguments),
    }
}
