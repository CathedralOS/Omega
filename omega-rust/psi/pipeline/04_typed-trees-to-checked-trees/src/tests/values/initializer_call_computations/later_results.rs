use super::{ExpressionNode, StatementNode};
use crate::tests::front_end::typed_program;

/// A caller whose computed initializer arguments feed two calls to one
/// ordinary scalar machine.
fn sequence_source() -> String {
    "pub data Sink {} data Root {}
     machine numeric(input: u32) -> u32 { input }
     machine Sink::observe(input: u32) {}
     boundary machine Sink::record(input: u32) ensures true;
     machine Sink::produce(first: u32, second: u32) -> u32 { first }
     machine Root::enter(input: u32) {
         let seed: u32 = input;
         Sink::observe(seed);
         let first: u32 = Sink::produce(numeric(seed), seed);
         Sink::record(first);
         let copied: u32 = first;
         let second: u32 = Sink::produce(numeric(copied), first);
         Sink::observe(second);
     }"
    .to_owned()
}

#[test]
fn later_result_eligibility_rejects_mutability_receivers_and_semantic_modifiers() {
    let source = sequence_source();
    let program = typed_program(&source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let StatementNode::LocalData(local) =
        &program.statement_table.statements(state.statement_nodes)[5]
    else {
        panic!("later initializer")
    };
    assert!(validation::unit_result_initializer_call_is_supported(
        &program,
        machine,
        local.initial_value
    ));
    for mutation in 0..5 {
        let mut changed = program.clone();
        if mutation == 0 {
            let StatementNode::LocalData(local) = &mut changed
                .statement_table
                .statements_mut(state.statement_nodes)[5]
            else {
                unreachable!()
            };
            local.is_mutable = true;
        } else {
            let ExpressionNode::Call(call) =
                changed.expression_table.expression_mut(local.initial_value)
            else {
                unreachable!()
            };
            let selected = typed_trees::expression::StaticMachineArgument {
                path: Box::new([]),
                application: None,
                type_reference: Default::default(),
                const_literal: None,
                evidence_projection: None,
                symbol: call.target_symbol,
            };
            match mutation {
                1 => call.receiver = local.initial_value,
                2 => call.machine_arguments = Box::new([selected]),
                3 => {
                    call.quotient_operation =
                        Some(typed_trees::expression::QuotientOperationRequest {
                            kind: typed_trees::expression::QuotientOperationKind::Lift,
                            representative_operation: selected,
                            theorem_evidence: Box::new([]),
                        })
                }
                4 => {
                    call.private_layout_operation =
                        Some(typed_trees::expression::PrivateLayoutOperationRequest {
                            selected_slot: selected,
                        })
                }
                _ => unreachable!(),
            }
        }
        assert!(
            !validation::unit_result_initializer_call_is_supported(
                &changed,
                machine,
                local.initial_value
            ),
            "later initializer mutation={mutation}"
        );
    }
}
