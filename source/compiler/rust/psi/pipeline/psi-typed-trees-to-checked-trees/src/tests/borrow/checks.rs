use crate::checks::check_checked_facts;
use crate::flow::canonical_place_overlaps_segments;
use crate::semantic_calls::{call_site_argument_expressions, find_call_site};
use crate::{
    build_borrow_facts, build_domain_facts, build_flow_facts, build_proof_facts,
    build_semantic_facts,
};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

/// Body-level escape analysis: returning a view of a machine-body local that
/// holds no loan is a dangling borrow and is rejected. (A local that borrows a
/// parameter — like `cells` in `accepts_view_return_disambiguated_by_explicit_lifetime`
/// — is fine; the loan fact distinguishes the two.)
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

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
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

/// Run a source program through the full frontend check, returning the borrow
/// checker's verdict.
pub(super) fn check_program(source: &str) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };
    check_checked_facts(&typed, &facts)
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

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
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
                    psi_checked_trees::expression::ExpressionNode::Borrow(_)
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

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
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

#[test]
fn rejects_direct_mutable_borrow_while_local_alias_is_active() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            self.use_value(&mut self.value);
            self.write_alias(alias);
        }

        machine Main::use_value(&mut self, value: &mut i32) {
            value = 1;
        }

        machine Main::write_alias(&mut self, value: &mut i32) {
            value = 2;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("active local alias should block direct mutable borrow");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(combined.contains("local borrow `alias` is still active"));
    assert!(combined.contains("borrowed at statement 0"));
    assert!(combined.contains("its last use is at statement 2"));
    assert!(combined.contains("released at state exit"));
}

#[test]
fn rejects_direct_mutable_borrow_while_helper_alias_is_active() {
    let source = r#"
        data Exit {
            destination: i32;
        }

        data Room {
            exits: [Exit; 1];
        }

        data Main {
            room: Room;
        }

        machine Main::main(&mut self) {
            let alias: &mut [Exit] = self.room.exits.as_mut_slice();
            self.use_exit(&mut self.room.exits[0]);
            self.write_alias(alias);
        }

        machine Main::use_exit(&mut self, exit: &mut Exit) {
            exit = Exit { destination: 1 };
        }

        machine Main::write_alias(&mut self, exits: &mut [Exit]) {
            exits[0] = Exit { destination: 2 };
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("helper-returned local alias should block direct mutable borrow");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(combined.contains("local borrow `alias` is still active"));
    assert!(combined.contains("borrowed at statement 0"));
    assert!(combined.contains("its last use is at statement 2"));
    assert!(combined.contains("released at state exit"));
}

#[test]
fn accepts_adjacent_mutable_windows_with_one_symbolic_half_open_boundary() {
    let source = r#"
        data Main { items: [i32; 4]; }

        machine Main::split(&mut self) -> u64 {
            let mid: u64 = 2;
            let cut: u64 = mid;
            let left: &mut [i32] = self.items[0..cut];
            let right: &mut [i32] = self.items[mid..4];
            left.len + right.len
        }
    "#;

    check_program(source)
        .expect("the exact shared symbolic boundary proves half-open window adjacency");
}

#[test]
fn changing_symbolic_window_start_to_zero_restores_overlap_rejection() {
    let source = r#"
        data Main { items: [i32; 4]; }

        machine Main::split(&mut self) -> u64 {
            let mid: u64 = 2;
            let cut: u64 = mid;
            let left: &mut [i32] = self.items[0..cut];
            let right: &mut [i32] = self.items[0..4];
            left.len + right.len
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("changing the second start from `mid` to `0` makes the windows overlap");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("creates local borrow `right` while local borrow `left` is still active"),
        "expected the exact symbolic-bound mutation conflict, got:\n{combined}"
    );
}

#[test]
fn rejects_local_borrow_creation_while_prior_alias_is_active() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let first: &mut i32 = &mut self.value;
            let second: &mut i32 = &mut self.value;
            self.write_alias(first);
            self.write_alias(second);
        }

        machine Main::write_alias(&mut self, value: &mut i32) {
            value = 2;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("second local borrow alias should be rejected while the first is live");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined
            .contains("creates local borrow `second` while local borrow `first` is still active")
    );
    assert!(combined.contains("borrowed at statement 0"));
    assert!(combined.contains("its last use is at statement 2"));
}

#[test]
fn accepts_stable_mutable_reborrow_chain_from_local_alias() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Main {
            cell: Cell;
        }

        machine write_cell(cell: &mut Cell) {
            cell.value = 2;
        }

        machine Main::main(&mut self) {
            let first: &mut Cell = &mut self.cell;
            let second: &mut Cell = &mut first;
            write_cell(second);
        }
    "#;

    check_program(source).expect(
        "a mutable reborrow may derive from its active source alias and retain the ultimate place",
    );
}

#[test]
fn accepts_stable_mutable_aliases_from_fixed_and_dynamic_indexed_places() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Main {
            cells: [Cell; 2];
        }

        machine write_cell(cell: &mut Cell) {
            cell.value = 2;
        }

        machine Main::fixed(&mut self) {
            let cell: &mut Cell = &mut self.cells[0];
            write_cell(cell);
        }

        machine Main::dynamic(&mut self, index: u64)
        requires
            index < 2
        {
            let cell: &mut Cell = &mut self.cells[index];
            write_cell(cell);
        }
    "#;

    check_program(source).expect(
        "fixed and range-checked dynamic indexed places may back stable local mutable aliases",
    );
}

#[test]
fn accepts_direct_mutable_borrow_after_local_alias_last_use() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            self.write_alias(alias);
            self.use_value(&mut self.value);
        }

        machine Main::write_alias(&mut self, value: &mut i32) {
            value = 1;
        }

        machine Main::use_value(&mut self, value: &mut i32) {
            value = 2;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    check_checked_facts(&typed, &facts).expect("loan should end after alias last use");
}

#[test]
fn rejects_direct_assignment_while_local_alias_is_active() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            self.value = 3;
            self.write_alias(alias);
        }

        machine Main::write_alias(&mut self, value: &mut i32) {
            value = 2;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("direct assignment should not overlap a live local alias");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(
            "statement 1 mutates `self.value` while local borrow `alias` is still active"
        )
    );
    assert!(combined.contains("borrowed at statement 0"));
}

#[test]
fn rejects_mutating_call_through_owner_while_view_is_active() {
    // A `&mut self` call that writes the owner field is a *call* statement, not
    // an assignment. The Vec-views / owner-mutation-through-a-call rule must
    // reject it while a borrowed view of that field is still live. This is the
    // call-statement analogue of the array/slice/string owner-write rule and the
    // mechanism behind the Vec `push`-while-borrowed rejection.
    let source = r#"
        data Entry {
            value: i32;
        }

        data Main {
            entries: [Entry; 2];
        }

        machine Main::main(&mut self) {
            let view: &[Entry] = self.entries.as_slice();
            self.clear_entries();
            self.read_alias(view);
        }

        machine Main::clear_entries(&mut self) {
            self.entries[0] = Entry { value: 0 };
        }

        machine Main::read_alias(&self, entries: &[Entry]) {
            let count: u64 = entries.len;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("a mutating call through the owner must conflict with a live view");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("while local borrow `view` is still active"),
        "expected owner-mutation-through-call conflict, got:\n{combined}"
    );
}

#[test]
fn rejects_vec_push_while_slice_view_is_active() {
    let source = r#"
        data Vec<T> {
        }

        machine Vec::as_slice<T>(&self) -> &[T] {
        }

        machine Vec::push<T>(&mut self, value: T) {
        }

        data Main {
            items: Vec<u8>;
        }

        machine Main::main(&mut self) {
            let view: &[u8] = self.items.as_slice();
            self.items.push(7);
            self.read_alias(view);
        }

        machine Main::read_alias(&self, items: &[u8]) {
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("Vec::push through the owner must conflict with a live view");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("while local borrow `view` is still active"),
        "expected Vec push conflict, got:\n{combined}"
    );
}

#[test]
fn accepts_mutating_call_through_owner_on_disjoint_field() {
    // A mutating call through the owner that writes a DISJOINT field is accepted
    // while a borrowed view of a different field is live. The call-mutation rule
    // reuses the loan-overlap engine, so a call whose summarized writes do not
    // overlap the live view's place does not conflict.
    let source = r#"
        data Entry {
            value: i32;
        }

        data Main {
            left: [Entry; 2];
            right: [Entry; 2];
        }

        machine Main::main(&mut self) {
            let view: &[Entry] = self.left.as_slice();
            self.touch_right();
            self.read_alias(view);
        }

        machine Main::touch_right(&mut self) {
            self.right[0] = Entry { value: 1 };
        }

        machine Main::read_alias(&self, entries: &[Entry]) {
            let count: u64 = entries.len;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    check_checked_facts(&typed, &facts)
        .expect("a mutating call on a disjoint field should not conflict with the view");
}

#[test]
fn accepts_known_pure_mutable_receiver_call_while_view_is_active() {
    // `&mut self` in the signature is not by itself a write. Once the target is
    // known, an empty mutation summary means this helper is read-only for borrow
    // invalidation purposes; only unknown calls need the conservative receiver
    // fallback.
    let source = r#"
        data Entry {
            value: i32;
        }

        data Main {
            entries: [Entry; 2];
        }

        machine Main::main(&mut self) {
            let view: &[Entry] = self.entries.as_slice();
            let value: i32 = self.identity(1);
            self.read_alias(view, value);
        }

        machine Main::identity(&mut self, value: i32) -> i32 {
            transition {
                _ -> value
            }
        }

        machine Main::read_alias(&self, entries: &[Entry], value: i32) {
            let count: u64 = entries.len;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    check_checked_facts(&typed, &facts)
        .expect("known pure mutable receiver helper should not invalidate a live view");
}

#[test]
fn accepts_mutable_slice_alias_index_from_fixed_array_field() {
    let source = r#"
        data Exit {
            destination: i32;
        }

        data Room {
            exits: [Exit; 4];
        }

        data Main {
            room: Room;
        }

        machine Main::main(&mut self) {
            let exits: &mut [Exit] = self.room.exits.as_mut_slice();
            exits[0] = Exit { destination: 1 };
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    check_checked_facts(&typed, &facts)
        .expect("mutable slice alias from fixed array field should keep its fixed length");
}

#[test]
fn accepts_recursive_slice_parameter_index_proof_from_guard() {
    let source = r#"
        data Entry {
            value: i32;
        }

        data Main {
            entries: [Entry; 2];
        }

        machine Main::main(&mut self) {
            let entries: &[Entry] = self.entries.as_slice();
            transition {
                _ -> self.visit(entries, 0)
            }

            state visit(&mut self, entries: &[Entry], index: u64) {
                let value: i32 = entries[index].value;
                let next_index: u64 = index + 1;
                let has_next: bool = next_index < entries.len;

                transition {
                    has_next -> self.visit(entries, next_index)
                    _ -> {}
                }
            }
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    check_checked_facts(&typed, &facts)
        .expect("recursive slice parameter should keep index proof from length guard");
}

#[test]
fn accepts_direct_mutable_borrow_after_local_alias_reassignment() {
    let source = r#"
        data Main {
            value: i32;
            other: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            alias = &mut self.other;
            self.use_value(&mut self.value);
        }

        machine Main::use_value(&mut self, value: &mut i32) {
            value = 1;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    check_checked_facts(&typed, &facts)
        .expect("reassigned local alias should no longer block later direct mutable borrow");
}

/// Lifetimes stage 1 (elision rule 1): a free machine returning a view with
/// exactly one ref input links the returned view's loan to THAT input, so
/// mutating the linked source while the view is live is rejected. Before
/// stage 1 no loan was tracked for a free-machine call result at all.
#[test]
fn rejects_linked_input_mutation_while_free_machine_view_is_active() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Bag {
            cells: [Cell; 4];
        }

        data Main {
            bag: Bag;
        }

        machine pick(bag: &mut Bag) -> &mut Cell {
            let cells: &mut [Cell] = bag.cells.as_mut_slice();
            transition {
                _ -> &mut cells[2]
            }
        }

        machine Main::main(&mut self) {
            let cell: &mut Cell = pick(&mut self.bag);
            self.bag.cells[0] = Cell { value: 1 };
            cell.value = 7;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("mutating the elision-linked input while the view is live should reject");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(
            "statement 1 mutates `self.bag.cells[0]` while local borrow `cell` is still active"
        ),
        "expected the linked-input mutation rejection, got:\n{combined}"
    );
}

/// Lifetimes stage 1 (elision rule 1, the win): the returned view borrows ONLY
/// the single ref input it was linked to -- mutating a DIFFERENT ref input of
/// the caller while the view is live compiles.
#[test]
fn accepts_unlinked_ref_input_mutation_while_free_machine_view_is_active() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Bag {
            cells: [Cell; 4];
        }

        data Main {
            first: Bag;
            second: Bag;
        }

        machine pick(bag: &mut Bag) -> &mut Cell {
            let cells: &mut [Cell] = bag.cells.as_mut_slice();
            transition {
                _ -> &mut cells[2]
            }
        }

        machine fill(a: &mut Bag, b: &mut Bag) {
            let cell: &mut Cell = pick(a);
            b.cells[0] = Cell { value: 1 };
            cell.value = 7;
        }

        machine Main::main(&mut self) {
            fill(&mut self.first, &mut self.second);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    check_checked_facts(&typed, &facts)
        .expect("mutating the unlinked ref input while the view is live should compile");
}

/// A view-returning machine with MULTIPLE non-self ref inputs and an ELIDED
/// output lifetime is ambiguous and rejected at the declaration: the checker
/// cannot infer which input the view borrows, and now points at explicit
/// lifetimes (decision 15 stage 2) as the fix. A `&self` method with extra ref
/// params stays accepted (elision rule 3 links the output to self).
#[test]
fn rejects_ambiguous_view_return_with_multiple_ref_inputs() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Bag {
            cells: [Cell; 4];
        }

        data Main {
            first: Bag;
            second: Bag;
        }

        machine pick_either(a: &mut Bag, b: &mut Bag) -> &mut Cell {
            let cells: &mut [Cell] = a.cells.as_mut_slice();
            transition {
                _ -> &mut cells[2]
            }
        }

        machine Main::main(&mut self) {
            let cell: &mut Cell = pick_either(&mut self.first, &mut self.second);
            cell.value = 7;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    let diagnostics = check_checked_facts(&typed, &facts)
        .expect_err("two non-self ref inputs with a view output should be ambiguous");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("cannot infer which input the returned view borrows"),
        "expected the elision ambiguity rejection, got:\n{combined}"
    );
}

/// Bodyless provider requirements create caller-side loans too, so declaration
/// validation must reject their ambiguous view sources before any call is
/// attributed.
#[test]
fn rejects_ambiguous_view_return_from_boundary_trait_signature() {
    let source = r#"
        boundary trait Storage {
            machine view(first: &u8, second: &u8) -> &u8;
        }

        data Main {}

        machine Main::main(&mut self) {}
    "#;

    let diagnostics = check_program(source)
        .expect_err("a bodyless view requirement with two ref inputs should be ambiguous");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("cannot infer which input the returned view borrows"),
        "expected the bodyless-signature elision rejection, got:\n{combined}"
    );
}

/// Lifetimes stage 2 (frozen decision 15): an EXPLICIT output lifetime resolves
/// the otherwise-ambiguous two-ref-input case by naming the input the view
/// borrows. `pick_either<'bag>(a: &'bag mut Bag, b: &mut Bag) -> &'bag mut Cell`
/// says the view comes from `a`, so the declaration is accepted and the loan
/// follows `a`'s argument (here `self.first`).
#[test]
fn accepts_view_return_disambiguated_by_explicit_lifetime() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Bag {
            cells: [Cell; 4];
        }

        data Main {
            first: Bag;
            second: Bag;
        }

        machine pick_either<'bag>(a: &'bag mut Bag, b: &mut Bag) -> &'bag mut Cell {
            let cells: &mut [Cell] = a.cells.as_mut_slice();
            transition {
                _ -> &mut cells[2]
            }
        }

        machine Main::main(&mut self) {
            let cell: &mut Cell = pick_either(&mut self.first, &mut self.second);
            cell.value = 7;
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
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
    let facts = psi_checked_trees::CheckFacts {
        semantic,
        proof,
        values: Default::default(),
        borrow,
        domains,
        dynamic_conformances: Default::default(),
        nominal_machine_uses: Default::default(),
        operators: Default::default(),
        capabilities: Default::default(),
        flow,
        index_compatibility: Default::default(),
        mutation: Default::default(),
        service_reaches: Default::default(),
        synchronous_invocations: Default::default(),
        suspensions: Default::default(),
        blocking: Default::default(),
        termination: Default::default(),
        qualifications: Default::default(),
        contract_plans: Default::default(),
        carry: Default::default(),
        fact_call_projections: Vec::new(),
    };

    check_checked_facts(&typed, &facts)
        .expect("an explicit output lifetime naming one input should compile");
}

/// A single erased lifetime argument on borrow-carrying data has the same
/// linking role as a direct reference lifetime. The aggregate result borrows
/// only `first`, so mutating `second` while it remains live is sound.
#[test]
fn accepts_aggregate_return_disambiguated_by_explicit_lifetime_argument() {
    let source = r#"
        data View<'buf> {
            body: &'buf mut i32;
        }

        machine select<'left, 'right>(
            first: &'left mut i32,
            second: &'right mut i32
        ) -> View<'left> {
            let selected: View<'left> = View { body: first };
            transition {
                _ -> selected
            }
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'left, 'right>(
            first: &'left mut i32,
            second: &'right mut i32
        ) {
            let selected: View<'left> = select(first, second);
            write(second);
            write(selected.body);
        }
    "#;

    check_program(source)
        .expect("the aggregate lifetime argument should retain only its named source");
}

/// The same aggregate result keeps its named source loan active at the call
/// site; mutating that source before the result's last use must reject.
#[test]
fn rejects_linked_source_mutation_for_explicit_aggregate_lifetime_argument() {
    let source = r#"
        data View<'buf> {
            body: &'buf mut i32;
        }

        machine select<'left, 'right>(
            first: &'left mut i32,
            second: &'right mut i32
        ) -> View<'left> {
            let selected: View<'left> = View { body: first };
            transition {
                _ -> selected
            }
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'left, 'right>(
            first: &'left mut i32,
            second: &'right mut i32
        ) {
            let selected: View<'left> = select(first, second);
            write(first);
            write(selected.body);
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("the aggregate result must retain its named source");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("mutates `first` while local borrow `selected` is still active"),
        "expected the aggregate result's linked-source conflict, got:\n{combined}"
    );
}

/// Moving a borrow-carrying aggregate through another local transfers its
/// source loan rather than laundering it through ordinary data assignment.
#[test]
fn rejects_source_mutation_after_borrow_carrying_local_transfer() {
    let source = r#"
        data View<'buf> {
            body: &'buf mut i32;
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'source>(source: &'source mut i32) {
            let first: View<'source> = View { body: source };
            let second: View<'source> = first;
            write(source);
            write(second.body);
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("transferring the aggregate must transfer its loan");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("mutates `source` while local borrow `second` is still active"),
        "expected the transferred aggregate's source conflict, got:\n{combined}"
    );
}

/// Selecting a borrow-carrying field from a larger aggregate strips the
/// selected owner-path prefix while retaining that field's source.
#[test]
fn rejects_source_mutation_after_borrow_carrying_field_transfer() {
    let source = r#"
        data View<'buf> {
            body: &'buf mut i32;
        }

        data Pair<'left, 'right> {
            left: View<'left>;
            right: View<'right>;
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'left, 'right>(
            left: &'left mut i32,
            right: &'right mut i32
        ) {
            let pair: Pair<'left, 'right> = Pair {
                left: View { body: left },
                right: View { body: right },
            };
            let selected: View<'right> = pair.right;
            write(right);
            write(selected.body);
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("projecting the aggregate field must retain its loan");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("mutates `right` while local borrow `selected` is still active"),
        "expected the projected aggregate's source conflict, got:\n{combined}"
    );
}

/// An explicitly multi-lifetime result derives one source mapping per field;
/// using `right` does not keep the unrelated `left` loan active.
#[test]
fn accepts_field_specific_sources_for_multi_lifetime_result() {
    let source = r#"
        data Pair<'left, 'right> {
            left: &'left mut i32;
            right: &'right mut i32;
        }

        machine pair<'left, 'right>(
            left: &'left mut i32,
            right: &'right mut i32
        ) -> Pair<'left, 'right> {
            let result: Pair<'left, 'right> = Pair {
                left: left,
                right: right,
            };
            transition {
                _ -> result
            }
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'left, 'right>(
            left: &'left mut i32,
            right: &'right mut i32
        ) {
            let result: Pair<'left, 'right> = pair(left, right);
            write(left);
            write(result.right);
        }
    "#;

    check_program(source)
        .expect("a field-specific use should retain only that field's named source");
}

/// The field-specific mapping still rejects mutation of the source retained by
/// the field used later.
#[test]
fn rejects_linked_field_source_for_multi_lifetime_result() {
    let source = r#"
        data Pair<'left, 'right> {
            left: &'left mut i32;
            right: &'right mut i32;
        }

        machine pair<'left, 'right>(
            left: &'left mut i32,
            right: &'right mut i32
        ) -> Pair<'left, 'right> {
            let result: Pair<'left, 'right> = Pair {
                left: left,
                right: right,
            };
            transition {
                _ -> result
            }
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'left, 'right>(
            left: &'left mut i32,
            right: &'right mut i32
        ) {
            let result: Pair<'left, 'right> = pair(left, right);
            write(right);
            write(result.right);
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("the right result field must retain the right input");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("mutates `right` while local borrow `result` is still active"),
        "expected the multi-lifetime field conflict, got:\n{combined}"
    );
}

/// Replacing a borrow-carrying field releases the source carried by the old
/// field and makes the replacement source the field's active loan.
#[test]
fn accepts_precise_borrow_carrying_field_reassignment() {
    let source = r#"
        data View<'buf> {
            body: &'buf mut i32;
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'source>(
            first: &'source mut i32,
            second: &'source mut i32
        ) {
            let mut selected: View<'source> = View { body: first };
            selected.body = second;
            write(first);
            selected.body = 2;
        }
    "#;

    check_program(source)
        .expect("field replacement should release the old source and retain the new source");
}

/// The replacement source cannot be mutated while the reassigned aggregate
/// field remains live.
#[test]
fn rejects_replacement_source_of_borrow_carrying_field_reassignment() {
    let source = r#"
        data View<'buf> {
            body: &'buf mut i32;
        }

        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'source>(
            first: &'source mut i32,
            second: &'source mut i32
        ) {
            let mut selected: View<'source> = View { body: first };
            selected.body = second;
            write(second);
            selected.body = 2;
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("the reassigned field must retain its replacement source");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("mutates `second` while local borrow `selected` is still active"),
        "expected the reassigned field's source conflict, got:\n{combined}"
    );
}

#[test]
fn rejects_persistent_borrow_storage_until_cross_state_loans_are_propagated() {
    let source = r#"
        data Main<'storage> {
            stored: &'storage mut i32;
        }

        machine Main::store(
            &mut self,
            source: &'storage mut i32
        ) {
            self.stored = source;
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("persistent borrow storage must fail closed");
    let combined = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("assignment stores a borrow-carrying value in persistent field `stored`"),
        "expected the persistent-loan fence, got:\n{combined}"
    );
}

#[test]
fn accepts_program_static_literal_in_persistent_borrow_storage() {
    let source = r#"
        data Main {
            stored: &[u8];
        }

        machine Main::store(&mut self) {
            self.stored = "program static";
        }
    "#;

    check_program(source).expect("a literal view needs no state-local source loan");
}

#[test]
fn accepts_folded_static_literal_join_in_persistent_borrow_storage() {
    let source = r#"
        data Main {
            stored: &[u8];
        }

        machine Main::store(&mut self) {
            self.stored = "program " + "static";
        }
    "#;

    check_program(source).expect("a folded literal join remains program-static storage");
}

#[test]
fn accepts_nested_program_static_literal_in_persistent_aggregate_storage() {
    let source = r#"
        data Message {
            body: &[u8];
            code: i32;
        }

        data Main {
            stored: Message;
        }

        machine Main::store(&mut self) {
            self.stored = Message { body: "program static", code: 7 };
        }
    "#;

    check_program(source).expect("only the aggregate's borrow-carrying field needs classification");
}

#[test]
fn accepts_static_view_call_result_in_persistent_storage() {
    let source = r#"
        data Main {
            stored: &[u8];
        }

        machine Main::pick(&self, first: bool) -> &[u8] {
            transition first {
                true -> "first"
                false -> "second"
            }
        }

        machine Main::store(&mut self) {
            self.stored = self.pick(true);
        }
    "#;

    check_program(source).expect("every value exit of pick is program-static storage");
}

#[test]
fn rejects_parameter_backed_view_call_result_in_persistent_storage() {
    let source = r#"
        data Main {
            stored: &[u8];
        }

        machine Main::forward(&self, text: &[u8]) -> &[u8] {
            text
        }

        machine Main::store(&mut self, text: &[u8]) {
            self.stored = self.forward(text);
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("the returned view still borrows the call parameter");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `stored`")),
        "expected the persistent-loan fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn accepts_same_state_copy_from_static_persistent_storage() {
    let source = r#"
        data Message {
            body: &[u8];
            code: i32;
        }

        data Main {
            first: Message;
            second: Message;
        }

        machine Main::store(&mut self) {
            self.first = Message { body: "program static", code: 7 };
            self.second = self.first;
        }
    "#;

    check_program(source).expect("the copy retains the established static provenance");
}

#[test]
fn accepts_cross_state_copy_from_static_persistent_storage() {
    let source = r#"
        data Main {
            first: &[u8];
            second: &[u8];
        }

        machine Main::store(&mut self) {
            self.first = "program static";
            transition { _ -> copy() }

            state copy(&mut self) {
                self.second = self.first;
            }
        }
    "#;

    check_program(source).expect("program-static persistent provenance crosses a graph-state edge");
}

#[test]
fn rejects_cross_state_static_provenance_missing_on_one_predecessor() {
    let source = r#"
        data Main {
            first: &[u8];
            second: &[u8];
        }

        machine Main::store(&mut self, choose_static: bool) {
            transition choose_static {
                true -> establish()
                false -> bypass()
            }

            state establish(&mut self) {
                self.first = "program static";
                transition { _ -> join() }
            }

            state bypass(&mut self) {
                transition { _ -> join() }
            }

            state join(&mut self) {
                self.second = self.first;
            }
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("static provenance must hold on every predecessor");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `second`")),
        "expected the cross-state must-analysis fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn accepts_cross_state_static_aggregate_frontier_accumulation() {
    let source = r#"
        data Message {
            first: &[u8];
            second: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
        }

        machine Main::store(&mut self) {
            self.source.first = "first";
            transition { _ -> establish_second() }

            state establish_second(&mut self) {
                self.source.second = "second";
                transition { _ -> copy_complete() }
            }

            state copy_complete(&mut self) {
                self.copy = self.source;
            }
        }
    "#;

    check_program(source)
        .expect("each stable borrowed leaf crosses state edges into a complete frontier");
}

#[test]
fn accepts_cross_state_static_fixed_index_copy() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self) {
            self.messages[1].body = "program static";
            transition { _ -> copy_element() }

            state copy_element(&mut self) {
                self.copy = self.messages[1];
            }
        }
    "#;

    check_program(source)
        .expect("a literal fixed-index path retains static provenance across states");
}

#[test]
fn accepts_cross_state_static_runtime_index_forwarded_through_state_parameter() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self, index: u64 [0..2]) {
            self.messages[index].body = "program static";
            transition { _ -> copy_element(index) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    check_program(source).expect(
        "an immutable runtime index forwarded unchanged to a state parameter retains identity",
    );
}

#[test]
fn accepts_cross_state_static_runtime_index_forwarded_from_immutable_local() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
            code: i32;
        }

        machine Main::touch_code(&mut self) {
            self.code = 7;
        }

        machine Main::store(&mut self) {
            let index: u64 [0..2] = 1;
            self.messages[index].body = "program static";
            self.touch_code();
            transition { _ -> copy_element(index) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    check_program(source).expect(
        "an immutable local runtime index forwarded directly to a state parameter retains identity",
    );
}

#[test]
fn accepts_cross_state_static_runtime_index_through_immutable_local_alias() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self, source: u64 [0..2]) {
            let index: u64 [0..2] = source;
            let forwarded: u64 [0..2] = index;
            self.messages[index].body = "program static";
            transition { _ -> copy_element(forwarded) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    check_program(source).expect(
        "a direct immutable local-copy alias retains the runtime index identity across states",
    );
}

#[test]
fn rejects_cross_state_static_runtime_index_from_mutable_local() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self) {
            let mut index: u64 [0..2] = 1;
            self.messages[index].body = "program static";
            transition { _ -> copy_element(index) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("a mutable local index cannot identify one persistent source across states");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `copy`")),
        "expected the mutable-index persistent fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn rejects_cross_state_static_runtime_index_through_mutable_local_alias() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self, source: u64 [0..2]) {
            let index: u64 [0..2] = source;
            let mut forwarded: u64 [0..2] = index;
            self.messages[index].body = "program static";
            transition { _ -> copy_element(forwarded) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("a mutable copy cannot establish stable runtime-index identity");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `copy`")),
        "expected the mutable-alias persistent fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn rejects_cross_state_static_runtime_index_through_computed_local_alias() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self, source: u64 [0..2]) {
            let index: u64 [0..2] = source;
            let forwarded: u64 [0..2] = index + 0;
            self.messages[index].body = "program static";
            transition { _ -> copy_element(forwarded) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("a computed copy needs equality proof beyond direct-name identity");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `copy`")),
        "expected the computed-alias persistent fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn rejects_cross_state_static_runtime_index_rewritten_on_transition() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self, index: u64 [0..2]) {
            self.messages[index].body = "program static";
            transition { _ -> copy_element(0) }

            state copy_element(&mut self, index: u64 [0..2]) {
                self.copy = self.messages[index];
            }
        }
    "#;

    let diagnostics = check_program(source)
        .expect_err("rewriting an index does not preserve the established persistent leaf");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `copy`")),
        "expected the rewritten-index persistent fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn accepts_cross_state_static_leaf_across_disjoint_scalar_mutation() {
    let source = r#"
        data Message {
            body: &[u8];
            code: i32;
        }

        data Main {
            source: Message;
            copy: Message;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            transition { _ -> mutate_scalar() }

            state mutate_scalar(&mut self) {
                self.source.code = 7;
                transition { _ -> copy_complete() }
            }

            state copy_complete(&mut self) {
                self.copy = self.source;
            }
        }
    "#;

    check_program(source)
        .expect("a disjoint scalar mutation does not invalidate the static borrowed leaf");
}

#[test]
fn accepts_static_persistent_copy_across_disjoint_call_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine Main::touch_code(&mut self) {
            self.code = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("an exact disjoint call frame preserves static persistent provenance");
}

#[test]
fn accepts_static_persistent_copy_across_disjoint_cyclic_alias_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine identity(value: &mut i32) -> &mut i32 {
            value
        }

        machine Main::touch_code_cycle(&mut self) {
            let alias: &mut i32 = &mut self.code;
            transition { _ -> cycle(alias) }

            state cycle(&mut self, value: &mut i32) {
                value = 7;
                transition { _ -> cycle(identity(value)) }
            }
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code_cycle();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a transparent helper preserves the exact cyclic alias permutation");
}

#[test]
fn accepts_static_persistent_copy_across_attached_transparent_result_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine Main::forward_alias(&self, value: &mut i32) -> &mut i32 {
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut i32 = self.forward_alias(&mut self.code);
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("an attached transparent result preserves its explicit argument's disjoint frame");
}

#[test]
fn accepts_static_persistent_copy_across_local_alias_helper_result_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine identity(value: &mut i32) -> &mut i32 {
            value
        }

        machine forward_alias(value: &mut i32) -> &mut i32 {
            let first: &mut i32 = identity(value);
            let second: &mut i32 = &mut first;
            second
        }

        machine write(value: &mut i32) {
            value = 7;
        }

        machine Main::touch_code(&mut self) {
            write(forward_alias(&mut self.code));
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a transparent helper result supplied directly to a statement call stays exact");
}

#[test]
fn accepts_static_persistent_copy_across_local_index_helper_result_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: [u64; 2];
        }

        machine return_local_index(cells: &mut [u64; 2]) -> &mut u64 {
            let index: u64 = 0;
            &mut cells[index]
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut u64 = return_local_index(&mut self.code);
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("an effect-free local-index helper result preserves its collection frame");
}

#[test]
fn accepts_static_persistent_copy_across_local_index_alias_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: [u64; 2];
        }

        machine Main::touch_code(&mut self) {
            let index: u64 = 0;
            let alias: &mut u64 = &mut self.code[index];
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source).expect("an effect-free local-index alias preserves its collection frame");
}

#[test]
fn accepts_static_persistent_copy_across_mutable_slice_view_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: [u64; 2];
        }

        machine return_slice(cells: &mut [u64; 2]) -> &mut [u64] {
            let view: &mut [u64] = cells.as_mut_slice();
            view
        }

        machine Main::touch_code(&mut self) {
            let view: &mut [u64] = return_slice(&mut self.code);
            transition view.len > 0 {
                true -> write(view)
                false -> {}
            }

            state write(&mut self, view: &mut [u64]) {
                view[0] = 7;
            }
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a mutable slice view preserves its backing array's disjoint frame");
}

#[test]
fn accepts_static_persistent_copy_across_mutable_slice_statement_argument_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: [u64; 2];
        }

        machine write_slice(view: &mut [u64]) {
            transition view.len > 0 {
                true -> write(view)
                false -> {}
            }

            state write(view: &mut [u64]) {
                view[0] = 7;
            }
        }

        machine Main::touch_code(&mut self) {
            write_slice(self.code.as_mut_slice());
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a direct mutable-slice statement argument preserves its backing array frame");
}

#[test]
fn accepts_static_persistent_copy_after_discarded_slice_view_expression() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: u64;
            cells: [u64; 2];
        }

        machine return_after_slice_length<'value, 'cells>(
            value: &'value mut u64,
            cells: &'cells mut [u64; 2]
        ) -> &'value mut u64 {
            cells.as_mut_slice().len;
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut u64 =
                return_after_slice_length(&mut self.code, &mut self.cells);
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a discarded slice-view length read does not obscure a disjoint exact frame");
}

#[test]
fn accepts_static_persistent_copy_after_discarded_shared_slice_view_expression() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: u64;
            cells: [u64; 2];
        }

        machine return_after_shared_slice_length<'value, 'cells>(
            value: &'value mut u64,
            cells: &'cells [u64; 2]
        ) -> &'value mut u64 {
            cells.as_slice().len;
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut u64 =
                return_after_shared_slice_length(&mut self.code, self.cells);
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a discarded shared-slice length read does not obscure a disjoint exact frame");
}

#[test]
fn accepts_static_persistent_copy_across_recast_local_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: u64;
        }

        machine recast_write_then_return(value: &mut u64) -> &mut u64 {
            let view: &mut f64 = &mut value as &mut f64;
            view = 3.0;
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut u64 = recast_write_then_return(&mut self.code);
            alias = 4;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a recast write does not obscure a helper's exact returned parameter origin");
}

#[test]
fn accepts_static_persistent_copy_across_value_write_helper_result_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine identity(value: &mut i32) -> &mut i32 {
            value
        }

        machine write_then_return(value: &mut i32) -> &mut i32 {
            identity(value) = 7;
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut i32 = write_then_return(&mut self.code);
            alias = 8;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a transparent call-produced assignment target preserves the helper result origin");
}

#[test]
fn accepts_static_persistent_copy_across_isolated_scratch_helper_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine return_with_scratch(value: &mut i32) -> &mut i32 {
            let mut scratch: [i32; 2] = [0, 1];
            scratch[0] = 2;
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut i32 = return_with_scratch(&mut self.code);
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source).expect(
        "a reference-free scratch local cannot alter a helper's exact returned-alias origin",
    );
}

#[test]
fn accepts_static_persistent_copy_across_rebound_helper_result_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            spare: Message;
            copy: Message;
        }

        machine identity_message<'source>(
            value: &'source mut Message
        ) -> &'source mut Message {
            value
        }

        machine choose_second<'first, 'second>(
            first: &'first mut Message,
            second: &'second mut Message
        ) -> &'second mut Message {
            let mut selected: &mut Message = &mut first;
            selected = identity_message(second);
            selected
        }

        machine Main::touch_spare(&mut self) {
            let selected: &mut Message =
                choose_second(&mut self.source, &mut self.spare);
            selected.body = "spare static";
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_spare();
            self.copy = self.source;
        }
    "#;

    check_program(source).expect(
        "a transparent call-produced rebind selects only the replacement argument's write frame",
    );
}

#[test]
fn accepts_static_persistent_copy_across_pure_expression_helper_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine return_after_read(value: &mut i32) -> &mut i32 {
            value == value;
            value
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut i32 = return_after_read(&mut self.code);
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("an effect-free discarded expression cannot alter the returned alias origin");
}

#[test]
fn accepts_static_persistent_copy_across_receiver_result_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            code: i32;
        }

        machine Main::code_alias(&mut self) -> &mut i32 {
            &mut self.code
        }

        machine Main::touch_code(&mut self) {
            let alias: &mut i32 = self.code_alias();
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_code();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("an exact receiver-rooted result preserves its disjoint caller frame");
}

#[test]
fn accepts_static_persistent_copy_across_isolated_record_local_frame() {
    let source = r#"
        data Cell {
            value: i32;
        }

        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
        }

        machine Main::touch_local_record(&mut self) {
            let local: [Cell; 2] = [Cell { value: 0 }, Cell { value: 1 }];
            let alias: &mut i32 = &mut local[0].value;
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_local_record();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a primitive-only record local contributes no caller-visible write frame");
}

#[test]
fn accepts_static_persistent_copy_across_stable_rebound_alias_frame() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
            first: i32;
            second: i32;
        }

        machine Main::touch_second(&mut self) {
            let alias: &mut i32 = &mut self.first;
            alias = &mut self.second;
            alias = 7;
        }

        machine Main::store(&mut self) {
            self.source.body = "program static";
            self.touch_second();
            self.copy = self.source;
        }
    "#;

    check_program(source)
        .expect("a direct stable alias replacement publishes its replacement origin's frame");
}

#[test]
fn accepts_same_place_reassignment_from_static_persistent_storage() {
    let source = r#"
        data Main {
            stored: &[u8];
        }

        machine Main::store(&mut self) {
            self.stored = "program static";
            self.stored = self.stored;
        }
    "#;

    check_program(source)
        .expect("assignment reads established static provenance before replacing the same place");
}

#[test]
fn accepts_indexed_aggregate_copy_after_all_borrowed_leaves_become_static() {
    let source = r#"
        data Message {
            body: &[u8];
            code: i32;
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self) {
            let index: u64 = 1;
            self.messages[index].body = "program static";
            self.messages[index].code = 7;
            self.copy = self.messages[index];
        }
    "#;

    check_program(source)
        .expect("an immutable indexed aggregate copy retains complete static leaf provenance");
}

#[test]
fn rejects_aggregate_copy_with_only_partial_static_leaf_coverage() {
    let source = r#"
        data Message {
            first: &[u8];
            second: &[u8];
        }

        data Main {
            source: Message;
            copy: Message;
        }

        machine Main::store(&mut self) {
            self.source.first = "program static";
            self.copy = self.source;
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("every borrowed source leaf needs static provenance");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `copy`")),
        "expected the incomplete aggregate-copy fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn rejects_indexed_static_copy_through_mutable_index_binding() {
    let source = r#"
        data Message {
            body: &[u8];
        }

        data Main {
            messages: [Message; 2];
            copy: Message;
        }

        machine Main::store(&mut self) {
            let mut index: u64 = 0;
            self.messages[index].body = "program static";
            index = 1;
            self.copy = self.messages[index];
        }
    "#;

    let diagnostics =
        check_program(source).expect_err("a mutable index cannot identify one persistent source");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("assignment stores a borrow-carrying value in persistent field `copy`")),
        "expected the mutable-index provenance fence, got:\n{}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The same replacement rule applies to a direct reference local: its old
/// source is released and its new source remains borrowed through later use.
#[test]
fn accepts_precise_reference_local_reassignment() {
    let source = r#"
        machine write(value: &mut i32) {
            value = 1;
        }

        machine exercise<'source>(
            first: &'source mut i32,
            second: &'source mut i32
        ) {
            let mut selected: &'source mut i32 = first;
            selected = second;
            write(first);
            write(selected);
        }
    "#;

    check_program(source)
        .expect("reference replacement should release the old source and retain the new source");
}
