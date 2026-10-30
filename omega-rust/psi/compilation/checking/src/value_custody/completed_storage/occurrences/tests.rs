use super::consumer_captures;
use arena::Handle;
use checked_trees::{FlowCallFact, FlowControlFacts, FlowOperandFact, FlowStateFact};

#[test]
fn captures_follow_evaluated_sources_but_not_unrelated_history() {
    let state = Handle::from_arena_index(1);
    let expression = Handle::from_arena_index(2);
    let mut control = FlowControlFacts::default();
    let source = FlowOperandFact {
        state_symbol: state,
        statement_index: 3,
        expression,
        ..Default::default()
    };
    let sources = control.operands.insert_many([source.clone()]);
    let source_handle = sources.handles().next().unwrap();
    // Neither a detached occurrence nor prior-statement identity supplies
    // this consumer, even though their expression handles match.
    control.operands.insert(source.clone());
    let previous = control.operands.insert(FlowOperandFact {
        statement_index: 2,
        ..source
    });
    let actuals = control.operands.insert_many([FlowOperandFact {
        state_symbol: state,
        statement_index: 3,
        expression: Handle::from_arena_index(4),
        evaluated_inputs: sources,
        value_source: previous,
        ..Default::default()
    }]);
    let calls = control.calls.insert_many([FlowCallFact {
        statement_index: 3,
        operands: actuals,
        ..Default::default()
    }]);
    control.states.insert(FlowStateFact {
        state_symbol: state,
        calls,
        ..Default::default()
    });
    assert_eq!(
        consumer_captures(&control, state, 3, expression),
        Some(vec![source_handle])
    );
    assert_eq!(consumer_captures(&control, state, 2, expression), None);
}
