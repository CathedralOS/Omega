use super::check_program;
use crate::borrow::build_borrow_facts;
use crate::checks::check_unretained_borrow_fixture_facts as check_checked_facts;
use crate::flow::build_domain_facts;
use crate::flow::build_flow_facts;
use crate::flow::canonical_place_overlaps_segments;
use crate::proof::build_proof_facts;
use crate::semantic::facts::build_semantic_facts;
use crate::tests::front_end::typed_program;

/// The escape companion: a borrow-carrying value whose borrow comes from a
/// machine-body local does not outlive the call and is rejected.
#[test]
fn rejects_borrow_carrying_data_returned_from_local() {
    let source = r#"
        data Message {
            body: &string;
        }

        machine bad(seed: &string) -> Message {
            let owned: string = "local";
            let msg: Message = Message { body: &owned };
            transition {
                _ -> msg
            }
        }
    "#;
    let diagnostics =
        check_program(source).expect_err("a borrow-carrying value borrowing a local should reject");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("returns a view borrowing the local `msg`"),
        "expected the escape rejection for the borrow-carrying value, got:\n{combined}"
    );
}

/// A dynamic index can select any element, so it must keep every candidate
/// loan and reject a sibling write while the projected view remains live.
#[test]
fn rejects_dynamic_fixed_array_projection_as_potentially_aliasing() {
    let source = r#"
        data Cell {
            value: &mut i32;
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise(first: &mut i32, second: &mut i32, index: u64 [0..=1]) {
            let pair: [Cell; 2] = [
                Cell { value: first },
                Cell { value: second }
            ];
            let selected: &mut i32 = pair[index].value;
            write(second);
            write(selected);
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("dynamic array projection must conservatively retain sibling loans");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("mutates `second` while local borrow `selected` is still active"),
        "expected the dynamic element's sibling-loan conflict, got:\n{combined}"
    );
}

/// Projection paths through a concrete generic wrapper retain the substituted
/// field's source rather than appearing rooted in the temporary wrapper.
#[test]
fn accepts_projected_borrow_through_generic_wrapper() {
    let source = r#"
        data Message {
            body: &mut i32;
        }

        data Wrapper<T> {
            value: T;
        }

        machine pick(input: &mut i32) -> &mut i32 {
            let wrapper: Wrapper<Message> = Wrapper {
                value: Message { body: input }
            };
            let selected: &mut i32 = wrapper.value.body;
            transition {
                _ -> selected
            }
        }
    "#;

    check_program(source)
        .expect("projection through a generic wrapper must retain the input source");
}

/// The active payload of a sum is part of the carrier. Selecting a case cannot
/// hide the input loan stored inside that payload.
#[test]
fn accepts_sum_payload_carrying_an_input_borrow() {
    let source = r#"
        data Message {
            body: &i32;
        }

        data Envelope {
            case Empty;
            case Message(message: Message);
        }

        machine wrap(input: &i32) -> Envelope {
            let envelope: Envelope = Envelope::Message {
                message: Message { body: input }
            };
            transition {
                _ -> envelope
            }
        }
    "#;

    check_program(source).expect("an active sum payload must retain its input loan");
}

#[test]
fn accepts_disjoint_member_borrow_arguments() {
    let source = r#"
        data Player {
            health: i32;
            stamina: i32;
        }

        data Main {
            player: Player;
        }

        machine Main::main(&mut self) {
            self.use_stats(&mut self.player.health, self.player.stamina);
        }

        machine Main::use_stats(&mut self, health: &mut i32, stamina: i32) {
            health = stamina;
        }
    "#;

    let typed = typed_program(source);
    let proof_plan = proof::obligations::build_proof_plan(&typed);
    let operations = validation::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );
    let facts = checked_trees::CheckFacts {
        semantic,
        proof,
        borrow,
        domains,
        flow,
        ..Default::default()
    };

    let main_machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let main_state = typed
        .machine_states(main_machine)
        .iter()
        .find(|state| state.name.as_str() == "main")
        .expect("main state");
    let borrow_state = facts
        .borrow
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == main_machine.symbol && state.state_symbol == main_state.symbol)
                .then_some(state)
        })
        .expect("main borrow state");
    let use_stats_call = facts.borrow.calls.span_or_empty(borrow_state.calls)[0].clone();
    let accesses = facts
        .borrow
        .argument_accesses
        .span_or_empty(use_stats_call.accesses);
    assert_eq!(accesses.len(), 2);
    assert_eq!(accesses[0].root_symbol, accesses[1].root_symbol);
    assert_ne!(
        facts
            .borrow
            .access_segments
            .span_or_empty(accesses[0].segments),
        facts
            .borrow
            .access_segments
            .span_or_empty(accesses[1].segments),
    );
    assert!(!canonical_place_overlaps_segments(
        facts
            .borrow
            .access_segments
            .span_or_empty(accesses[0].segments),
        facts
            .borrow
            .access_segments
            .span_or_empty(accesses[1].segments),
    ));

    check_checked_facts(&typed, &facts).expect("disjoint member borrows should not conflict");
}
