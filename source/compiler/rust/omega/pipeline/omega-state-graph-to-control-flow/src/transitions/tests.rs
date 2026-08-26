use super::remap_transition_owned;
use omega_control_flow::{PlannedTransitionTarget, TransitionExpressionRefs};
use psi_arena::{Handle, HandleSpan};
use psi_checked_trees::expression::ExpressionHandle;
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;

#[test]
fn remap_owned_transition_preserves_targets_and_expression_refs() {
    let machine = SymbolHandle::from_arena_index(2);
    let state = SymbolHandle::from_arena_index(3);
    let key = omega_state_graph::StateKey {
        machine,
        state,
        segment_index: 4,
    };
    let target_arguments =
        HandleSpan::<ExpressionHandle>::from_parts(Handle::from_arena_index(8), 2);
    let continuation_arguments =
        HandleSpan::<ExpressionHandle>::from_parts(Handle::from_arena_index(10), 1);
    let target_value = Handle::from_arena_index(12);
    let continuation_value = Handle::from_arena_index(13);
    let guard = Handle::from_arena_index(14);

    let transition = remap_transition_owned(omega_state_graph::TransitionEdge {
        statement_index: 9,
        target: omega_state_graph::PlannedTransitionTarget::State {
            index: 6,
            key,
            name: Identifier::generated("resolved"),
        },
        continuation: omega_state_graph::PlannedTransitionTarget::Nested {
            receiver_symbol: SymbolHandle::from_arena_index(15),
            state_symbol: SymbolHandle::from_arena_index(16),
            receiver: Identifier::generated("child"),
            state: Identifier::generated("tick"),
        },
        expressions: omega_state_graph::TransitionExpressionRefs {
            target_arguments,
            target_value,
            continuation_arguments,
            continuation_value,
            guard,
        },
    });

    assert_eq!(transition.statement_index, 9);
    assert_eq!(
        transition.target,
        PlannedTransitionTarget::State {
            index: 6,
            key: omega_control_flow::StateKey {
                machine,
                state,
                segment_index: 4,
            },
            name: Identifier::generated("resolved"),
        }
    );
    assert_eq!(
        transition.continuation,
        PlannedTransitionTarget::Nested {
            receiver_symbol: SymbolHandle::from_arena_index(15),
            state_symbol: SymbolHandle::from_arena_index(16),
            receiver: Identifier::generated("child"),
            state: Identifier::generated("tick"),
        }
    );
    assert_eq!(
        transition.expressions,
        TransitionExpressionRefs {
            target_arguments,
            target_value,
            continuation_arguments,
            continuation_value,
            guard,
        }
    );
}
