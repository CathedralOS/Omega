use crate::tests::front_end::{checked_program, checked_program_result, typed_program};
use checked_trees::{FlowClaimJoinExitKind, FlowPermissionEvaluation};
use language_semantics::{PermissionEventKind, PermissionEventSource};
use typed_trees::statement::StatementNode;

const TOKEN: &str = "data Token { value: bool; } machine Token::drop(token: &mut Token) {}";

fn checked_with_continuation(source: &str, combined: bool) -> checked_trees::CheckedTrees {
    let mut program = typed_program(source);
    if combined {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "choose")
            .unwrap()
            .clone();
        let state = &program.machine_states(&machine)[0];
        let nodes = state.statement_nodes;
        let count = nodes.count() as usize;
        let StatementNode::Transition(other) =
            &program.statement_table.statements(nodes)[count - 1]
        else {
            panic!("fallback transition")
        };
        let continuation = other.target;
        let StatementNode::Transition(first) =
            &mut program.statement_table.statements_mut(nodes)[count - 2]
        else {
            panic!("guarded transition")
        };
        first.continuation = continuation;
        program.machine_states_mut(&machine)[0].statement_nodes =
            arena::HandleSpan::from_parts(nodes.start(), nodes.count() - 1);
    }
    crate::lower_typed_trees(program, &crate::CheckingRequest::settled())
        .expect("separate custody on each authored target")
}

#[test]
fn permission_regions_preserve_each_target_and_continuation_cleanup() {
    for named in [false, true] {
        for combined in [false, true] {
            let body = if named {
                "machine choose(flag: bool, first: Token, second: Token) {
                    transition flag { true -> done(first) false -> done(second) }
                    state done(value: Token) {}
                }"
            } else {
                "machine choose(flag: bool, first: Token, second: Token) -> Token {
                    transition flag { true -> (first) false -> (second) }
                }"
            };
            let checked = checked_with_continuation(&format!("{TOKEN} {body}"), combined);
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "choose")
                .unwrap();
            let state = &checked.machine_states(machine)[0];
            let parameters = checked.state_parameters(state);
            let statements = checked.statement_table.statements(state.statement_nodes);
            let dispositions = checked
                .facts
                .contextual_cleanup
                .dispositions
                .iter()
                .filter(|(_, row)| row.machine == machine.symbol && row.state == state.symbol)
                .map(|(_, row)| row)
                .collect::<Vec<_>>();
            assert_eq!(dispositions.len(), 2, "named={named}, combined={combined}");
            if named {
                assert!(
                    checked
                        .facts
                        .flow
                        .control
                        .exits
                        .iter()
                        .all(|(_, exit)| { exit.state_symbol != state.symbol }),
                    "named transfers must not become machine-contract returns"
                );
            }
            for (ordinal, disposition) in dispositions.iter().enumerate() {
                let StatementNode::Transition(transition) =
                    &statements[disposition.exit.statement_index]
                else {
                    panic!("cleanup transition")
                };
                let target = match disposition.exit.kind {
                    FlowClaimJoinExitKind::TransitionTarget => transition.target,
                    FlowClaimJoinExitKind::TransitionContinuation => transition.continuation,
                    _ => panic!("a selected target has an exact exit coordinate"),
                };
                assert!(target.is_valid());
                assert_eq!(
                    disposition.root,
                    facts::PlaceRoot::Symbol(parameters[2 - ordinal].symbol)
                );
                assert!(
                    checked.facts.flow.control.outgoing.iter().any(|(_, exit)| {
                        exit.state_symbol == state.symbol
                            && exit.statement_index == disposition.exit.statement_index
                            && exit.transition_target == target
                    }),
                    "cleanup must join the exact post-operand, pre-arrival semantic frontier"
                );
            }
            if combined {
                assert_eq!(
                    dispositions[0].exit.kind,
                    FlowClaimJoinExitKind::TransitionTarget
                );
                assert_eq!(
                    dispositions[1].exit.kind,
                    FlowClaimJoinExitKind::TransitionContinuation
                );
                assert_eq!(
                    dispositions[0].exit.statement_index,
                    dispositions[1].exit.statement_index
                );
            }
            // A different valid sibling coordinate is still the wrong source
            // occurrence; independent source replay must reject the retagging.
            let mut forged = checked.facts.clone();
            let transfers = forged
                .flow
                .ownership
                .permissions
                .iter()
                .filter_map(|(handle, event)| {
                    (event.state_symbol == state.symbol
                        && event.kind == PermissionEventKind::Transfer
                        && matches!(
                            event.evaluation,
                            FlowPermissionEvaluation::TransitionTarget(_)
                        ))
                    .then_some((handle, event.evaluation))
                })
                .collect::<Vec<_>>();
            assert_eq!(transfers.len(), 2);
            forged
                .flow
                .ownership
                .permissions
                .get_mut(transfers[0].0)
                .evaluation = transfers[1].1;
            assert!(
                crate::checks::multiplicity::validate_linear_permission_events(
                    &checked.typed,
                    &forged
                )
                .is_err()
            );
        }
    }
}

#[test]
fn permission_guard_transfer_survives_missed_arm_without_replaying_target_moves() {
    let source = format!(
        "{TOKEN}
        machine inspect(value: Token) -> bool {{ false }}
        machine choose(first: Token, second: Token) {{
            transition {{ inspect(first) -> done() _ -> done() }}
            state done() {{}}
        }}"
    );
    let checked = checked_program(&source);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let parameters = checked.state_parameters(state);
    let transfers = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.state_symbol == state.symbol
                && event.kind == PermissionEventKind::Transfer
                && event.root == facts::PlaceRoot::Symbol(parameters[0].symbol)
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    assert_eq!(
        transfers.len(),
        1,
        "the guard executes before the fork once"
    );
    assert_eq!(
        transfers[0].evaluation,
        FlowPermissionEvaluation::TransitionGuard
    );
    let residuals = checked
        .facts
        .contextual_cleanup
        .dispositions
        .iter()
        .filter(|(_, row)| row.state == state.symbol)
        .map(|(_, row)| row.root)
        .collect::<Vec<_>>();
    assert_eq!(
        residuals,
        vec![facts::PlaceRoot::Symbol(parameters[1].symbol); 2]
    );
    let repeated = source.replace("_ -> done()", "inspect(first) -> done() _ -> done()");
    assert!(
        checked_program_result(&repeated).is_err(),
        "a missed earlier guard already transferred first"
    );
}

#[test]
fn permission_constructed_argument_transfer_belongs_only_to_selected_target() {
    let source = format!(
        "{TOKEN}
        machine inspect(value: Token) -> bool {{ value.value }}
        machine choose(flag: bool) -> bool {{
            transition flag {{ true -> (inspect(Token {{ value: true }})) false -> (false) }}
        }}"
    );
    for combined in [false, true] {
        let checked = checked_with_continuation(&source, combined);
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "choose")
            .unwrap();
        let state = &checked.machine_states(machine)[0];
        let transition = checked
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| {
                if let StatementNode::Transition(transition) = statement {
                    Some(transition)
                } else {
                    None
                }
            })
            .unwrap();
        let temporary_transfers = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.state_symbol == state.symbol
                    && matches!(event.root, facts::PlaceRoot::Expression(_))
                    && event.kind == PermissionEventKind::Transfer
                    && matches!(event.source, PermissionEventSource::Call { .. })
            })
            .map(|(_, event)| event)
            .collect::<Vec<_>>();
        assert_eq!(temporary_transfers.len(), 1);
        assert_eq!(
            temporary_transfers[0].evaluation,
            FlowPermissionEvaluation::TransitionTarget(transition.target)
        );
    }
}

#[test]
fn permission_named_transition_drops_only_the_guarded_generic_sibling() {
    for keep_first in [false, true] {
        for discarded_ready in [false, true] {
            let first_ready = !keep_first && discarded_ready;
            let second_ready = keep_first && discarded_ready;
            let source = format!(
                "data Token {{ ready: bool; }}
                machine Token::drop(owner: &mut Token) requires owner.ready {{}}
                machine choose<T>(flag: bool, first: T, second: T) -> T {{
                    transition flag {{
                        true -> selected(first)
                        false -> selected(second)
                    }}
                    state selected(value: T) {{ value }}
                }}
                machine run() -> Token {{
                    choose({keep_first}, Token {{ ready: {first_ready} }},
                        Token {{ ready: {second_ready} }})
                }}"
            );
            let result = checked_program_result(&source);
            if discarded_ready {
                result.expect("the named transfer retains the guarded sibling's cleanup proof");
            } else {
                let diagnostics = result.err().expect("the discarded sibling is not ready");
                assert!(
                    diagnostics.iter().any(|diagnostic| {
                        diagnostic
                            .message
                            .contains("contextual cleanup prerequisite")
                            && diagnostic.message.contains("Token::drop")
                    }),
                    "keep_first={keep_first}: {diagnostics:#?}"
                );
            }
        }
    }
}
