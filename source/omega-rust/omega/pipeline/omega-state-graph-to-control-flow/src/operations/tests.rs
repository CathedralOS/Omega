use super::remap_operation_owned;
use omega_control_flow::{OperationExpressionRefs, OperationKind};
use psi_arena::{Handle, HandleSpan};
use psi_checked_trees::expression::ExpressionHandle;
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;

#[test]
fn remap_owned_call_operation_preserves_call_shape_and_argument_span() {
    let arguments = HandleSpan::<ExpressionHandle>::from_parts(Handle::from_arena_index(7), 3);
    let receiver_symbol = SymbolHandle::from_arena_index(11);
    let target_symbol = SymbolHandle::from_arena_index(12);

    let operation = remap_operation_owned(omega_state_graph::Operation {
        statement_index: 5,
        kind: omega_state_graph::OperationKind::Call {
            receiver_symbol,
            target_symbol,
            has_receiver: true,
            receiver: Identifier::generated("deck"),
            target: Identifier::generated("draw"),
        },
        expressions: omega_state_graph::OperationExpressionRefs::Call { arguments },
    });

    assert_eq!(operation.statement_index, 5);
    assert_eq!(
        operation.kind,
        OperationKind::Call {
            receiver_symbol,
            target_symbol,
            has_receiver: true,
            receiver: Identifier::generated("deck"),
            target: Identifier::generated("draw"),
        }
    );
    assert_eq!(
        operation.expressions,
        OperationExpressionRefs::Call { arguments }
    );
}
