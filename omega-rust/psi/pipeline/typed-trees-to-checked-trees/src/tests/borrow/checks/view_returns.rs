use super::check_program;
use crate::borrow::build_borrow_facts;
use crate::checks::check_unretained_borrow_fixture_facts as check_checked_facts;
use crate::flow::build_domain_facts;
use crate::flow::build_flow_facts;
use crate::flow::canonical_place_overlaps_segments;
use crate::proof::build_proof_facts;
use crate::semantic::build_semantic_facts;
use crate::semantic_calls::{call_site_argument_expressions, find_call_site};
use crate::tests::front_end::typed_program;

#[test]
fn rejects_view_return_of_body_local() {
    let source = r#"
        data Cell { value: i32; }

        machine leak(seed: &Cell) -> &Cell {
            let local: Cell = Cell { value: 9 };
            transition {
                _ -> &local
            }
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

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("returning a view of a body-local should be rejected as a dangling borrow");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("returns a view borrowing the local `local`"),
        "expected the escape rejection, got:\n{combined}"
    );
}

/// Borrow-carrying data (decision 15 stage 2): a `data` value holding a
/// reference field may be returned when its borrow comes from an input —
/// the constructed value's loan follows the borrowed source.
#[test]
fn accepts_borrow_carrying_data_returned_from_input() {
    let source = r#"
        data Message {
            body: &string;
        }

        machine wrap(input: &string) -> Message {
            let msg: Message = Message { body: input };
            transition {
                _ -> msg
            }
        }
    "#;
    let facts_result = check_program(source);
    facts_result.expect("a borrow-carrying value borrowing an input should compile");
}

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

/// The escape rule also sees through an aggregate literal built directly at
/// the return position: `View { body: &local }` dangles exactly like
/// `&local` does, whether or not a `let` named the carrier first.
#[test]
fn rejects_inline_aggregate_literal_carrying_a_local_borrow() {
    let source = r#"
        data Message {
            body: &i32;
        }

        machine bad(seed: &i32) -> Message {
            let owned: i32 = 9;
            transition {
                _ -> Message { body: &owned }
            }
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("an inline carrier borrowing a body-local should reject");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("returns a view borrowing the local `owned`"),
        "expected the inline-carrier escape rejection, got:\n{combined}"
    );
}

/// The same inline construction is accepted when the carried borrow reaches an
/// input, matching the named-`let` form field for field.
#[test]
fn accepts_inline_aggregate_literal_carrying_an_input_borrow() {
    let source = r#"
        data Message {
            body: &i32;
        }

        machine wrap(input: &i32) -> Message {
            transition {
                _ -> Message { body: input }
            }
        }
    "#;

    check_program(source).expect("an inline carrier borrowing an input should compile");
}

/// A nested record cannot erase a loan. Returning `Envelope` is sound because
/// its nested `Message` ultimately borrows the machine input.
#[test]
fn accepts_nested_borrow_carrying_data_returned_from_input() {
    let source = r#"
        data Message {
            body: &i32;
        }

        data Envelope {
            message: Message;
        }

        machine wrap(input: &i32) -> Envelope {
            let envelope: Envelope = Envelope {
                message: Message { body: input }
            };
            transition {
                _ -> envelope
            }
        }
    "#;

    check_program(source)
        .expect("a nested borrow-carrying value borrowing an input should compile");
}

/// Several fields may carry the same valid source; the aggregate remains
/// returnable when every carried loan reaches the input.
#[test]
fn accepts_multiple_borrowing_fields_returned_from_input() {
    let source = r#"
        data Pair {
            first: &i32;
            second: &i32;
        }

        machine wrap(input: &i32) -> Pair {
            let pair: Pair = Pair {
                first: input,
                second: input
            };
            transition {
                _ -> pair
            }
        }
    "#;

    check_program(source).expect("all carried input loans should outlive the result");
}

/// Projecting one reference field out of a multi-loan carrier rebases through
/// that field only; a disjoint sibling source remains independently writable
/// after the carrier's last use.
#[test]
fn accepts_projected_field_from_multi_loan_carrier() {
    let source = r#"
        data Pair {
            first: &mut i32;
            second: &mut i32;
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise(first: &mut i32, second: &mut i32) {
            let pair: Pair = Pair {
                first: first,
                second: second
            };
            let selected: &mut i32 = pair.first;
            write(second);
            write(selected);
        }
    "#;

    check_program(source).expect("field projection must not retain a disjoint sibling source");
}

/// A literal fixed-array position is as precise as a named field: selecting a
/// constant element retains only that element's loan.
#[test]
fn accepts_projected_fixed_array_element_from_multi_loan_carrier() {
    let source = r#"
        data Cell {
            value: &mut i32;
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise(first: &mut i32, second: &mut i32) {
            let pair: [Cell; 2] = [
                Cell { value: first },
                Cell { value: second }
            ];
            let selected: &mut i32 = pair[0].value;
            write(second);
            write(selected);
        }
    "#;

    check_program(source)
        .expect("constant array projection must not retain a disjoint element source");
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

/// Generic storage participates in the same structural walk: substituting a
/// borrow-carrying argument into an otherwise ordinary field keeps the loan.
#[test]
fn accepts_generic_wrapper_of_borrow_carrying_data() {
    let source = r#"
        data Message {
            body: &i32;
        }

        data Envelope<T> {
            value: T;
        }

        machine wrap(input: &i32) -> Envelope<Message> {
            let envelope: Envelope<Message> = Envelope {
                value: Message { body: input }
            };
            transition {
                _ -> envelope
            }
        }
    "#;

    check_program(source)
        .expect("a generic wrapper must retain its concrete argument's input loan");
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

/// Fixed arrays compose borrow carrying structurally just like records and sum
/// payloads.
#[test]
fn accepts_fixed_array_carrying_an_input_borrow() {
    let source = r#"
        data Message {
            body: &i32;
        }

        machine wrap(input: &i32) -> [Message; 1] {
            let messages: [Message; 1] = [
                Message { body: input }
            ];
            transition {
                _ -> messages
            }
        }
    "#;

    check_program(source).expect("a fixed array must retain its element's input loan");
}

/// The nested escape companion: wrapping a local borrow in two records does not
/// make it outlive the call.
#[test]
fn rejects_nested_borrow_carrying_data_returned_from_local() {
    let source = r#"
        data Message {
            body: &i32;
        }

        data Envelope {
            message: Message;
        }

        machine bad(seed: &i32) -> Envelope {
            let owned: i32 = 9;
            let envelope: Envelope = Envelope {
                message: Message { body: &owned }
            };
            transition {
                _ -> envelope
            }
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("a nested borrow-carrying value borrowing a local should reject");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("returns a view borrowing the local `envelope`"),
        "expected the nested escape rejection, got:\n{combined}"
    );
}

/// Escape safety is universal over the carrier's loans, not existential: one
/// valid input loan cannot hide a dangling sibling field.
#[test]
fn rejects_mixed_input_and_local_loans_in_returned_data() {
    let source = r#"
        data Pair {
            first: &i32;
            second: &i32;
        }

        machine bad(input: &i32) -> Pair {
            let owned: i32 = 9;
            let pair: Pair = Pair {
                first: input,
                second: &owned
            };
            transition {
                _ -> pair
            }
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("a valid first field must not hide a dangling sibling loan");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("returns a view borrowing the local `pair`"),
        "expected the mixed-source escape rejection, got:\n{combined}"
    );
}

/// Fixed arrays carry every element loan as well; a valid first element cannot
/// hide a dangling later element.
#[test]
fn rejects_mixed_input_and_local_loans_in_returned_array() {
    let source = r#"
        data Message {
            body: &i32;
        }

        machine bad(input: &i32) -> [Message; 2] {
            let owned: i32 = 9;
            let messages: [Message; 2] = [
                Message { body: input },
                Message { body: &owned }
            ];
            transition {
                _ -> messages
            }
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("a valid first element must not hide a dangling later loan");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("returns a view borrowing the local `messages`"),
        "expected the mixed-array escape rejection, got:\n{combined}"
    );
}

/// Recursive data without a reference terminates the structural borrow walk
/// and remains an ordinary owned value.
#[test]
fn accepts_recursive_data_without_a_borrow() {
    let source = r#"
        data Chain {
            case End;
            case Next(next: Chain);
        }

        machine make() -> Chain {
            transition {
                _ -> Chain::End
            }
        }
    "#;

    check_program(source).expect("recursive owned data must not look borrow-carrying");
}

/// A recursive edge encountered before a borrowing payload must not terminate
/// the whole structural search early.
#[test]
fn accepts_recursive_data_with_a_later_borrowing_payload() {
    let source = r#"
        data Chain {
            case Next(next: Chain);
            case End(value: &i32);
        }

        machine wrap(input: &i32) -> Chain {
            let chain: Chain = Chain::End { value: input };
            transition {
                _ -> chain
            }
        }
    "#;

    check_program(source).expect("cycle detection must still find a later borrowing payload");
}

#[test]
fn constrained_u8_call_argument_does_not_manufacture_owned_transfer() {
    let source = r#"
        data Counter {}

        data Main {
            values: [i32; 5];
            counter: Counter;
        }

        machine Counter::observe(&mut self, value: u8 [0..=20]) {}

        machine Main::main(&mut self) {
            let view: &[i32] = self.values.as_slice();
            self.counter.observe(0);
            self.consume(view);
        }

        machine Main::consume(&self, values: &[i32]) {
            let length: u64 = values.len;
        }
    "#;

    check_program(source).expect(
        "a constrained copy primitive is not an owned operand and the receiver field is disjoint",
    );
}

#[test]
fn accepts_mutable_local_named_place_arguments() {
    let source = r#"
        data Main {}

        machine Main::main(&mut self, flag: bool, out: &mut u32) {
            out = self.pick(flag);
        }

        machine Main::pick(&mut self, flag: bool) -> u32 {
            let choice: bool;
            self.copy(flag, &mut choice);
            transition {
                choice -> self.branch(choice)
                _ -> 0
            }
        }

        machine Main::copy(&mut self, flag: bool, out: &mut bool) {
            out = flag;
        }

        machine Main::branch(&mut self, flag: bool) -> u32 {
            transition {
                flag -> 1
                _ -> 2
            }
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

    let pick_machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::pick")
        .expect("pick machine");
    let pick_state = typed
        .machine_states(pick_machine)
        .iter()
        .find(|state| state.name.as_str() == "pick")
        .expect("pick state");
    let borrow_state = facts
        .borrow
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == pick_machine.symbol && state.state_symbol == pick_state.symbol)
                .then_some(state)
        })
        .expect("pick borrow state");
    let state_calls = facts.borrow.calls.span_or_empty(borrow_state.calls);
    assert_eq!(state_calls.len(), 2);
    let copy_call = state_calls
        .iter()
        .find(|call| {
            typed
                .machines()
                .iter()
                .flat_map(|machine| typed.machine_states(machine).iter())
                .find(|state| state.symbol == call.target_symbol)
                .is_some_and(|state| state.name.as_str() == "copy")
        })
        .expect("copy borrow call");

    assert_eq!(
        facts
            .borrow
            .argument_accesses
            .span_or_empty(copy_call.accesses)
            .len(),
        2
    );
    let call_site = find_call_site(
        &typed,
        pick_machine.symbol,
        pick_state.symbol,
        copy_call.statement_index,
        copy_call.call_ordinal,
    )
    .expect("copy call site");
    assert_eq!(call_site_argument_expressions(&typed, &call_site).len(), 2);
    assert_eq!(
        call_site_argument_expressions(&typed, &call_site)
            .iter()
            .filter(|argument| {
                matches!(
                    typed.expression_table.expression(**argument),
                    checked_trees::expression::ExpressionNode::Borrow(_)
                )
            })
            .count(),
        1
    );

    check_checked_facts(&typed, &facts)
        .expect("mutable local named place should pass borrow checks");
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
