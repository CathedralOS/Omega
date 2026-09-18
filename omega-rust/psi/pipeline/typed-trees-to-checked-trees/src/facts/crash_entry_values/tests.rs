use super::{entry_operand, has_stable_observable_contents, operand_entry_provenance};
use checked_trees::CrashPredicateExpression;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionTargetNode};

fn typed_program(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn named_state(
    program: &TypedTrees,
    machine_name: &str,
    state_name: &str,
) -> (SymbolHandle, SymbolHandle) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap_or_else(|| panic!("machine {machine_name}"));
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == state_name)
        .unwrap_or_else(|| panic!("state {machine_name}::{state_name}"));
    (machine.symbol, state.symbol)
}

fn call_argument(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    argument_index: usize,
) -> (usize, ExpressionHandle) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .unwrap();
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .unwrap();
    for (index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let argument = match statement {
            StatementNode::Call(call) => program
                .statement_table
                .expression_handles(call.arguments)
                .get(argument_index)
                .copied(),
            StatementNode::LocalData(local) => {
                match program.expression_table.expression(local.initial_value) {
                    ExpressionNode::Call(call) => program
                        .expression_table
                        .expression_handles(call.arguments)
                        .get(argument_index)
                        .copied(),
                    _ => None,
                }
            }
            StatementNode::Expression(expression) => {
                match program.expression_table.expression(*expression) {
                    ExpressionNode::Call(call) => program
                        .expression_table
                        .expression_handles(call.arguments)
                        .get(argument_index)
                        .copied(),
                    _ => None,
                }
            }
            // A state's tail call is its transition's named or value target.
            StatementNode::Transition(transition) => [transition.target, transition.continuation]
                .into_iter()
                .filter(|target| target.is_valid())
                .find_map(
                    |target| match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Named { arguments, .. } => program
                            .statement_table
                            .expression_handles(*arguments)
                            .get(argument_index)
                            .copied(),
                        TransitionTargetNode::Value(value) => {
                            match program.expression_table.expression(*value) {
                                ExpressionNode::Call(call) => program
                                    .expression_table
                                    .expression_handles(call.arguments)
                                    .get(argument_index)
                                    .copied(),
                                _ => None,
                            }
                        }
                        _ => None,
                    },
                ),
            _ => None,
        };
        if let Some(argument) = argument {
            return (index, argument);
        }
    }
    panic!("expected a call carrying an argument");
}

fn first_call_argument(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> (usize, ExpressionHandle) {
    call_argument(program, machine_symbol, state_symbol, 0)
}

/// Like `call_argument`, but selects the call whose target is named
/// `target`, so earlier calls in the same state do not shadow it.
fn targeted_call_argument(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    target: &str,
) -> (usize, ExpressionHandle) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .unwrap();
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .unwrap();
    for (index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let call_arguments = |call: &typed_trees::expression::TableCallExpression| {
            (call.target.as_str() == target)
                .then(|| {
                    program
                        .expression_table
                        .expression_handles(call.arguments)
                        .first()
                        .copied()
                })
                .flatten()
        };
        let argument = match statement {
            StatementNode::Call(call) => (call.target.as_str() == target)
                .then(|| {
                    program
                        .statement_table
                        .expression_handles(call.arguments)
                        .first()
                        .copied()
                })
                .flatten(),
            StatementNode::LocalData(local) => {
                match program.expression_table.expression(local.initial_value) {
                    ExpressionNode::Call(call) => call_arguments(call),
                    _ => None,
                }
            }
            StatementNode::Expression(expression) => {
                match program.expression_table.expression(*expression) {
                    ExpressionNode::Call(call) => call_arguments(call),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(argument) = argument {
            return (index, argument);
        }
    }
    panic!("expected a `{target}` call carrying an argument");
}

#[test]
fn state_parameter_arrival_transports_its_named_transition_argument() {
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value(flag: bool) -> bool {
             transition flag { true -> next(flag) false -> false }
             state next(input: bool) -> bool { sink(input); input }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        Some(CrashPredicateExpression::Parameter(0)),
        "the state parameter forwards the invocation actual, not a read of `next`'s storage"
    );
}

#[test]
fn state_parameter_arrival_transports_a_literal_argument() {
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value() -> bool {
             transition true { true -> next(false) false -> false }
             state next(input: bool) -> bool { sink(input); input }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        Some(CrashPredicateExpression::Boolean(false)),
    );
}

#[test]
fn divergent_state_arrivals_keep_provenance_unknown() {
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value(take: bool) -> bool {
             transition take { true -> next(false) false -> next(true) }
             state next(input: bool) -> bool { sink(input); input }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        None,
        "two arrivals binding different values are not one saved actual"
    );
}

#[test]
fn unreachable_state_parameters_keep_provenance_unknown() {
    // `next` is never targeted, so its parameter has no arrival at all.
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value() -> bool {
             transition { _ -> false }
             state next(input: bool) -> bool { sink(input); input }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        None
    );
}

#[test]
fn mutable_state_parameters_transport_their_bound_snapshot() {
    // `input` arrives only as `false` and its storage is never touched, so
    // the read at the call sees the bound snapshot across the state join.
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value() -> bool {
             transition true { true -> next(false) false -> false }
             state next(mut input: bool) -> bool { sink(input); input }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        Some(CrashPredicateExpression::Boolean(false)),
    );
}

#[test]
fn mutable_entry_parameters_transport_the_invocation_actual() {
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value(mut flag: bool) -> bool { sink(flag); flag }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let entry = program.machine_states(machine)[0].symbol;
    let (call_index, argument) = first_call_argument(&program, machine.symbol, entry);
    assert_eq!(
        entry_operand(&program, machine.symbol, entry, call_index, argument),
        Some(CrashPredicateExpression::Parameter(0)),
    );
}

#[test]
fn shared_borrow_transports_the_referents_entry_operand() {
    // A guard observing through `cell` reads `flag`'s storage, so the
    // operand's entry identity is `flag`'s bound value — the machine
    // parameter's invocation actual.
    let program = typed_program(
        "machine sink(cell: &i32) -> bool { true }
         machine value(flag: i32) -> bool { sink(&flag); flag }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let entry = program.machine_states(machine)[0].symbol;
    let (call_index, argument) = first_call_argument(&program, machine.symbol, entry);
    assert_eq!(
        entry_operand(&program, machine.symbol, entry, call_index, argument),
        Some(CrashPredicateExpression::Parameter(0)),
    );
}

#[test]
fn shared_borrow_of_a_written_referent_keeps_provenance_unknown() {
    // `flag`'s bound snapshot ends at the write, so a borrow taken after it
    // cannot claim the entry actual.
    let program = typed_program(
        "machine sink(cell: &i32) -> bool { true }
         machine value(mut flag: i32) -> bool { flag = flag + 1; sink(&flag); flag }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let entry = program.machine_states(machine)[0].symbol;
    let (call_index, argument) = first_call_argument(&program, machine.symbol, entry);
    assert_eq!(
        entry_operand(&program, machine.symbol, entry, call_index, argument),
        None,
    );
}

#[test]
fn mutable_snapshots_end_at_writes_and_exclusive_borrows() {
    for source in [
        // A write before the read replaces the bound snapshot.
        "machine sink(input: bool) -> bool { input }
         machine value() -> bool {
             transition true { true -> next(false) false -> false }
             state next(mut input: bool) -> bool { input = true; sink(input); input }
         }",
        // An exclusive borrow can overwrite the storage before the read.
        "machine corrupt(value: &mut bool) { value = true; }
         machine sink(input: bool) -> bool { input }
         machine value() -> bool {
             transition true { true -> next(false) false -> false }
             state next(mut input: bool) -> bool { corrupt(&mut input); sink(input); input }
         }",
        // A same-state edge forwarding the parameter after a write binds the
        // written value, not the arrival's bound snapshot.
        "machine sink(input: bool) -> bool { input }
         machine value() -> bool {
             transition true { true -> next(false) false -> false }
             state next(mut input: bool) -> bool {
                 sink(input);
                 input = true;
                 transition true { true -> next(input) false -> false }
             }
         }",
        // `-> self` after a write carries the written storage into the next
        // arrival, so no single bound snapshot exists.
        "machine sink(input: bool) -> bool { input }
         machine value() -> bool {
             transition true { true -> next(false) false -> false }
             state next(mut input: bool) -> bool {
                 sink(input);
                 input = true;
                 transition input { true -> self false -> false }
             }
         }",
    ] {
        let program = typed_program(source);
        let (machine, next) = named_state(&program, "value", "next");
        let (call_index, argument) = first_call_argument(&program, machine, next);
        assert_eq!(
            entry_operand(&program, machine, next, call_index, argument),
            None,
            "{source}"
        );
    }

    // An earlier operand of the same call already ran the borrow: the second
    // argument reads the overwritten storage, not the bound snapshot.
    let program = typed_program(
        "machine touch(slot: &mut bool) -> bool { slot = true; slot }
         machine pair(first: bool, second: bool) -> bool { second }
         machine value() -> bool {
             transition true { true -> next(false) false -> false }
             state next(mut input: bool) -> bool { pair(touch(&mut input), input); input }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = call_argument(&program, machine, next, 1);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        None,
        "`touch(&mut input)` runs before `input` is read as the second argument"
    );
}

#[test]
fn a_pristine_self_edge_still_transports_the_bound_snapshot() {
    // The `-> self` edge sees no writes, so it forwards the bound snapshot
    // itself and `input` stays `flag` on every iteration.
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value(flag: bool) -> bool {
             transition flag { true -> next(flag) false -> false }
             state next(mut input: bool) -> bool {
                 sink(input);
                 transition input { true -> self false -> false }
             }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        Some(CrashPredicateExpression::Parameter(0)),
    );
}

#[test]
fn mutable_stable_carrier_parameters_transport_field_snapshots() {
    // A `mut` record parameter with stable contents keeps its bound snapshot:
    // `rec.value` is the arrival's field while `rec`'s storage is pristine.
    let program = typed_program(
        "data Holder { value: bool; }
         machine sink(input: bool) -> bool { input }
         machine value(h: Holder) -> bool {
             transition true { true -> next(h) false -> false }
             state next(mut rec: Holder) -> bool { sink(rec.value); rec.value }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "value".to_owned(),
        }),
    );

    // A field write before the read dirties the whole binding's snapshot.
    let program = typed_program(
        "data Holder { value: bool; }
         machine sink(input: bool) -> bool { input }
         machine value(h: Holder) -> bool {
             transition true { true -> next(h) false -> false }
             state next(mut rec: Holder) -> bool { rec.value = true; sink(rec.value); rec.value }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        None,
    );
}

#[test]
fn mutable_field_versions_transport_unwritten_fields() {
    // Writes and exclusive borrows confined to a sibling projection leave the
    // read field's bound snapshot intact: `rec.value` still transports the
    // arrival's field.
    for source in [
        // A write to a sibling field.
        "data Holder { value: bool; other: bool; }
         machine sink(input: bool) -> bool { input }
         machine value(h: Holder) -> bool {
             transition true { true -> next(h) false -> false }
             state next(mut rec: Holder) -> bool {
                 rec.other = true;
                 sink(rec.value);
                 rec.value
             }
         }",
        // An exclusive borrow of a sibling field cannot write `value`.
        "data Holder { value: bool; other: bool; }
         machine corrupt(slot: &mut bool) { slot = true; }
         machine sink(input: bool) -> bool { input }
         machine value(h: Holder) -> bool {
             transition true { true -> next(h) false -> false }
             state next(mut rec: Holder) -> bool {
                 corrupt(&mut rec.other);
                 sink(rec.value);
                 rec.value
             }
         }",
        // A write below a different projection (`inner.flag`) does not reach
        // the sibling `value` field.
        "data Inner { flag: bool; }
         data Holder { inner: Inner; value: bool; }
         machine sink(input: bool) -> bool { input }
         machine value(h: Holder) -> bool {
             transition true { true -> next(h) false -> false }
             state next(mut rec: Holder) -> bool {
                 rec.inner.flag = true;
                 sink(rec.value);
                 rec.value
             }
         }",
        // An indexed write inside `items` is opaque below `items`, which
        // still diverges from the sibling `value` at the first segment.
        "data Holder { value: bool; items: [bool; 2]; }
         machine sink(input: bool) -> bool { input }
         machine value(h: Holder) -> bool {
             transition true { true -> next(h) false -> false }
             state next(mut rec: Holder) -> bool {
                 rec.items[0] = true;
                 sink(rec.value);
                 rec.value
             }
         }",
    ] {
        let program = typed_program(source);
        let (machine, next) = named_state(&program, "value", "next");
        let (call_index, argument) = targeted_call_argument(&program, machine, next, "sink");
        assert_eq!(
            entry_operand(&program, machine, next, call_index, argument),
            Some(CrashPredicateExpression::Member {
                receiver: Box::new(CrashPredicateExpression::Parameter(0)),
                member: "value".to_owned(),
            }),
            "{source}"
        );
    }

    // A `let mut` local versions its bound record's fields the same way.
    let program = typed_program(
        "data Holder { value: bool; other: bool; }
         machine sink(input: bool) -> bool { input }
         machine value(h: Holder) -> bool {
             let mut rec: Holder = h;
             rec.other = true;
             sink(rec.value);
             rec.value
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let entry = program.machine_states(machine)[0].symbol;
    let (call_index, argument) = first_call_argument(&program, machine.symbol, entry);
    assert_eq!(
        entry_operand(&program, machine.symbol, entry, call_index, argument),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "value".to_owned(),
        }),
    );
}

#[test]
fn mutable_field_versions_end_at_the_read_projection() {
    // Any write or exclusive reach that can touch the read projection ends
    // its provenance.
    for (source, target) in [
        // A write to the read field itself.
        (
            "data Holder { value: bool; other: bool; }
             machine sink(input: bool) -> bool { input }
             machine value(h: Holder) -> bool {
                 transition true { true -> next(h) false -> false }
                 state next(mut rec: Holder) -> bool {
                     rec.value = true;
                     sink(rec.value);
                     rec.value
                 }
             }",
            "sink",
        ),
        // A whole-binding write covers every field.
        (
            "data Holder { value: bool; other: bool; }
             machine sink(input: bool) -> bool { input }
             machine value(h: Holder) -> bool {
                 transition true { true -> next(h) false -> false }
                 state next(mut rec: Holder) -> bool {
                     rec = h;
                     sink(rec.value);
                     rec.value
                 }
             }",
            "sink",
        ),
        // An exclusive borrow of the read field may write through it.
        (
            "data Holder { value: bool; other: bool; }
             machine corrupt(slot: &mut bool) { slot = true; }
             machine sink(input: bool) -> bool { input }
             machine value(h: Holder) -> bool {
                 transition true { true -> next(h) false -> false }
                 state next(mut rec: Holder) -> bool {
                     corrupt(&mut rec.value);
                     sink(rec.value);
                     rec.value
                 }
             }",
            "sink",
        ),
        // An exclusive borrow of the whole binding reaches the field.
        (
            "data Holder { value: bool; other: bool; }
             machine consume(slot: &mut Holder) { slot.other = true; }
             machine sink(input: bool) -> bool { input }
             machine value(h: Holder) -> bool {
                 transition true { true -> next(h) false -> false }
                 state next(mut rec: Holder) -> bool {
                     consume(&mut rec);
                     sink(rec.value);
                     rec.value
                 }
             }",
            "sink",
        ),
        // Reading a whole field covers writes inside it: `rec.inner` still
        // observes the `inner.flag` write.
        (
            "data Inner { flag: bool; }
             data Holder { inner: Inner; }
             machine sink_inner(input: Inner) -> bool { input.flag }
             machine value(h: Holder) -> bool {
                 transition true { true -> next(h) false -> false }
                 state next(mut rec: Holder) -> bool {
                     rec.inner.flag = true;
                     sink_inner(rec.inner);
                     true
                 }
             }",
            "sink_inner",
        ),
        // An indexed position cannot be separated below its field, so a read
        // covering `items` still observes the element write.
        (
            "data Holder { value: bool; items: [bool; 2]; }
             machine sink_items(input: [bool; 2]) -> bool { input[0] }
             machine value(h: Holder) -> bool {
                 transition true { true -> next(h) false -> false }
                 state next(mut rec: Holder) -> bool {
                     rec.items[0] = true;
                     sink_items(rec.items);
                     rec.value
                 }
             }",
            "sink_items",
        ),
    ] {
        let program = typed_program(source);
        let (machine, next) = named_state(&program, "value", "next");
        let (call_index, argument) = targeted_call_argument(&program, machine, next, target);
        assert_eq!(
            entry_operand(&program, machine, next, call_index, argument),
            None,
            "{source}"
        );
    }
}

#[test]
fn mutable_field_versions_track_uniform_projections_across_self_edges() {
    // `-> self` rebinds the whole storage, but the produced operand only
    // asserts `value` is uniform across arrivals — a sibling-field drift does
    // not move it. The projection stays transportable.
    let program = typed_program(
        "data Holder { value: bool; other: bool; }
         machine sink(input: bool) -> bool { input }
         machine value(h: Holder) -> bool {
             transition true { true -> next(h) false -> false }
             state next(mut rec: Holder) -> bool {
                 sink(rec.value);
                 rec.other = true;
                 transition true { true -> self false -> false }
             }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "value".to_owned(),
        }),
        "the `-> self` edge preserves `value` even though `other` drifted"
    );

    // A write to the read projection itself before the edge still ends the
    // snapshot: the next arrival binds a different `value`.
    let program = typed_program(
        "data Holder { value: bool; other: bool; }
         machine sink(input: bool) -> bool { input }
         machine value(h: Holder) -> bool {
             transition true { true -> next(h) false -> false }
             state next(mut rec: Holder) -> bool {
                 sink(rec.value);
                 rec.value = true;
                 transition true { true -> self false -> false }
             }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        None,
        "the `-> self` edge would rebind `value` to the written storage"
    );
}

#[test]
fn mutable_field_snapshots_discharge_the_selected_crash_route() {
    // `rec.value` keeps its bound snapshot across the sibling-field write, so
    // the surviving Trap route is exactly `h.value` — covered only when
    // `value` publishes a same-cause route for that projection.
    for (declaration, expect_ok) in [
        ("crashes Trap h.value", true),
        ("crashes Trap h.other", false),
        ("", false),
    ] {
        let program = typed_program(&format!(
            "pub data Holder {{ value: bool; other: bool; }}
             {TRIGGER}
             pub machine value(h: Holder) -> bool
             {declaration}
             {{
                 transition true {{ true -> next(h) false -> false }}
                 state next(mut rec: Holder) -> bool {{
                     rec.other = true;
                     let r: bool = trigger(rec.value);
                     r
                 }}
             }}"
        ));
        match crate::lower_typed_trees(program) {
            Ok(_) => assert!(expect_ok, "{declaration} must not check"),
            Err(diagnostics) => {
                assert!(!expect_ok, "{declaration}: {diagnostics:#?}");
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("uncovered Trap crash route")),
                    "{declaration}: {diagnostics:#?}"
                );
            }
        }
    }
}

#[test]
fn mutable_locals_transport_their_initializer_until_written() {
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value(flag: bool) -> bool {
             let mut kept: bool = flag;
             sink(kept);
             kept = !kept;
             kept
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let entry = program.machine_states(machine)[0].symbol;
    let (call_index, argument) = first_call_argument(&program, machine.symbol, entry);
    assert_eq!(
        entry_operand(&program, machine.symbol, entry, call_index, argument),
        Some(CrashPredicateExpression::Parameter(0)),
        "the read precedes the later write, so `kept` still holds `flag`"
    );

    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value(flag: bool) -> bool {
             let mut kept: bool = flag;
             kept = !kept;
             sink(kept);
             kept
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let entry = program.machine_states(machine)[0].symbol;
    let (call_index, argument) = first_call_argument(&program, machine.symbol, entry);
    assert_eq!(
        entry_operand(&program, machine.symbol, entry, call_index, argument),
        None,
        "the write precedes the read, so the initializer is not the actual"
    );
}

#[test]
fn a_self_forwarded_arrival_is_tautological() {
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value(flag: bool) -> bool {
             transition flag { true -> next(flag) false -> false }
             state next(input: bool) -> bool {
                 sink(input);
                 transition true { true -> next(input) false -> false }
             }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        Some(CrashPredicateExpression::Parameter(0)),
        "the `-> next(input)` edge binds the parameter to its own arrival value"
    );
}

#[test]
fn a_named_self_cycle_with_a_rebound_argument_stays_unproven() {
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value(flag: bool) -> bool {
             transition flag { true -> next(flag) false -> false }
             state next(input: bool) -> bool {
                 sink(input);
                 transition input { true -> next(!input) false -> false }
             }
         }",
    );
    let (machine, next) = named_state(&program, "value", "next");
    let (call_index, argument) = first_call_argument(&program, machine, next);
    assert_eq!(
        entry_operand(&program, machine, next, call_index, argument),
        None,
        "`-> next(!input)` rebinds the parameter, so no single saved actual exists"
    );
}

#[test]
fn immutable_receiver_transports_its_field_entry_identity() {
    // `self.count` reads the receiver's storage, and an immutable receiver is
    // bound once at the invocation and never rebound or written, so the field
    // is the entry field — rendered as the entry telescope's self parameter.
    let program = typed_program(
        "data Main { count: i32; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(&self) -> bool { sink(self.count); true }",
    );
    let (machine, run) = named_state(&program, "Main::run", "run");
    let (call_index, argument) = first_call_argument(&program, machine, run);
    assert_eq!(
        entry_operand(&program, machine, run, call_index, argument),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "count".to_owned(),
        }),
    );
}

#[test]
fn immutable_receiver_transports_its_identity_and_shared_field_borrows() {
    // A bare `self` operand names the receiver itself; `&self.count` is a
    // shared borrow whose referent is the same entry field.
    let program = typed_program(
        "data Main { count: i32; }
         machine sink(owner: &Main) -> bool { true }
         machine Main::run(&self) -> bool { sink(self); true }",
    );
    let (machine, run) = named_state(&program, "Main::run", "run");
    let (call_index, argument) = first_call_argument(&program, machine, run);
    assert_eq!(
        entry_operand(&program, machine, run, call_index, argument),
        Some(CrashPredicateExpression::Parameter(0)),
        "the whole immutable receiver is the invocation's entry receiver"
    );

    let program = typed_program(
        "data Main { count: i32; }
         machine sink(cell: &i32) -> bool { true }
         machine Main::run(&self) -> bool { sink(&self.count); true }",
    );
    let (machine, run) = named_state(&program, "Main::run", "run");
    let (call_index, argument) = first_call_argument(&program, machine, run);
    assert_eq!(
        entry_operand(&program, machine, run, call_index, argument),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "count".to_owned(),
        }),
        "a shared borrow of the receiver field transports the field's entry identity"
    );
}

#[test]
fn immutable_receiver_identity_holds_in_non_entry_states() {
    // The receiver is never rebound by a transition, so `self.count` in a
    // later state still names the invocation's entry storage.
    let program = typed_program(
        "data Main { count: i32; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(&self) -> bool {
             transition true { true -> work() false -> true }
             state work(&self) -> bool { sink(self.count); true }
         }",
    );
    let (machine, work) = named_state(&program, "Main::run", "work");
    let (call_index, argument) = first_call_argument(&program, machine, work);
    assert_eq!(
        entry_operand(&program, machine, work, call_index, argument),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "count".to_owned(),
        }),
    );
}

#[test]
fn mutable_receiver_field_transports_entry_identity_when_never_written() {
    // `&mut self` still binds once at the invocation: when no statement in
    // the machine can write `self.count`, the field read is the entry field
    // even though the receiver itself is mutable.
    let program = typed_program(
        "data Main { count: i32; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(&mut self) -> bool { sink(self.count); true }",
    );
    let (machine, run) = named_state(&program, "Main::run", "run");
    let (call_index, argument) = first_call_argument(&program, machine, run);
    assert_eq!(
        entry_operand(&program, machine, run, call_index, argument),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "count".to_owned(),
        }),
    );
}

#[test]
fn mutable_receiver_field_survives_writes_confined_to_siblings() {
    // The write-escape scan is per field: `self.other = 1` and a mutating
    // call on `self.inner`'s attached machine never reach `self.count`, whose
    // provenance stays intact in a non-entry state too.
    let program = typed_program(
        "data Inner { n: i32; }
         machine Inner::bump(&mut self) { self.n = 1; }
         data Main { count: i32; other: i32; inner: Inner; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(&mut self) -> bool {
             self.other = 1;
             self.inner.bump();
             transition true { true -> work() false -> true }
             state work(&mut self) -> bool { sink(self.count); true }
         }",
    );
    let (machine, work) = named_state(&program, "Main::run", "work");
    let (call_index, argument) = first_call_argument(&program, machine, work);
    assert_eq!(
        entry_operand(&program, machine, work, call_index, argument),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "count".to_owned(),
        }),
    );
}

#[test]
fn mutable_receiver_field_keeps_no_identity_past_its_own_write() {
    // A `self.count` write — even in another state the invocation may never
    // visit — can precede the read through an earlier arrival, so the operand
    // keeps no entry identity.
    let same_state = typed_program(
        "data Main { count: i32; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(&mut self) -> bool {
             self.count = 1;
             sink(self.count); true
         }",
    );
    let (machine, run) = named_state(&same_state, "Main::run", "run");
    let (call_index, argument) = first_call_argument(&same_state, machine, run);
    assert_eq!(
        entry_operand(&same_state, machine, run, call_index, argument),
        None,
    );

    // A write in another state that can still reach the read — here `drift`
    // re-enters `work` — may already have run on an earlier arrival.
    let other_state = typed_program(
        "data Main { count: i32; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(&mut self) -> bool {
             transition true { true -> work() false -> true }
             state work(&mut self) -> bool {
                 sink(self.count);
                 transition true { true -> drift() false -> true }
             }
             state drift(&mut self) -> bool {
                 self.count = 1;
                 transition true { true -> work() false -> true }
             }
         }",
    );
    let (machine, work) = named_state(&other_state, "Main::run", "work");
    let (call_index, argument) = first_call_argument(&other_state, machine, work);
    assert_eq!(
        entry_operand(&other_state, machine, work, call_index, argument),
        None,
        "a predecessor state's write still escapes the field's entry identity"
    );
}

#[test]
fn mutable_receiver_field_survives_writes_in_states_that_cannot_precede() {
    // `done` runs only after `work` has already read, and `drift` is never
    // visited at all: neither state's write can execute before the read, so
    // the escape window excludes them and `self.count` keeps entry identity.
    let downstream = typed_program(
        "data Main { count: i32; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(&mut self) -> bool {
             transition true { true -> work() false -> true }
             state work(&mut self) -> bool {
                 sink(self.count);
                 transition true { true -> done() false -> true }
             }
             state done(&mut self) -> bool { self.count = -1; true }
         }",
    );
    let (machine, work) = named_state(&downstream, "Main::run", "work");
    let (call_index, argument) = first_call_argument(&downstream, machine, work);
    assert_eq!(
        entry_operand(&downstream, machine, work, call_index, argument),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "count".to_owned(),
        }),
        "a write in a state that cannot reach the read never precedes it",
    );

    let unreachable = typed_program(
        "data Main { count: i32; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(&mut self) -> bool {
             transition true { true -> work() false -> true }
             state work(&mut self) -> bool { sink(self.count); true }
             state drift(&mut self) { self.count = 1; }
         }",
    );
    let (machine, work) = named_state(&unreachable, "Main::run", "work");
    let (call_index, argument) = first_call_argument(&unreachable, machine, work);
    assert_eq!(
        entry_operand(&unreachable, machine, work, call_index, argument),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "count".to_owned(),
        }),
        "an unvisited state's write is outside the escape window",
    );
}

#[test]
fn mutable_receiver_field_keeps_no_identity_when_an_upstream_state_writes() {
    // `drift` is a real predecessor of `work` — the invocation runs it before
    // every `work` arrival — so its `self.count` write ends entry identity.
    let program = typed_program(
        "data Main { count: i32; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(&mut self) -> bool {
             transition true { true -> drift() false -> true }
             state drift(&mut self) -> bool {
                 self.count = 1;
                 transition true { true -> work() false -> true }
             }
             state work(&mut self) -> bool { sink(self.count); true }
         }",
    );
    let (machine, work) = named_state(&program, "Main::run", "work");
    let (call_index, argument) = first_call_argument(&program, machine, work);
    assert_eq!(
        entry_operand(&program, machine, work, call_index, argument),
        None,
    );
}

#[test]
fn mutable_receiver_field_keeps_no_identity_when_a_cycle_reaches_back() {
    // `done` writes `self.count` and then returns to `work`, so on the second
    // `work` arrival the write has already run: it stays inside the window.
    let program = typed_program(
        "data Main { count: i32; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(&mut self) -> bool {
             transition true { true -> work() false -> true }
             state work(&mut self) -> bool {
                 sink(self.count);
                 transition true { true -> done() false -> true }
             }
             state done(&mut self) -> bool {
                 self.count = -1;
                 transition true { true -> work() false -> true }
             }
         }",
    );
    let (machine, work) = named_state(&program, "Main::run", "work");
    let (call_index, argument) = first_call_argument(&program, machine, work);
    assert_eq!(
        entry_operand(&program, machine, work, call_index, argument),
        None,
    );
}

#[test]
fn mutable_receiver_field_survives_a_later_write_without_reentry() {
    // The write sits after the read in the same state, which never re-enters:
    // nothing after the read can have run before it, so the prefix is the
    // whole escape window and `self.count` keeps entry identity.
    let program = typed_program(
        "data Main { count: i32; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(&mut self) -> bool {
             sink(self.count);
             self.count = -1;
             true
         }",
    );
    let (machine, run) = named_state(&program, "Main::run", "run");
    let (call_index, argument) = first_call_argument(&program, machine, run);
    assert_eq!(
        entry_operand(&program, machine, run, call_index, argument),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "count".to_owned(),
        }),
    );
}

#[test]
fn mutable_receiver_field_later_write_still_escapes_once_the_state_reenters() {
    // `-> self` re-enters `run`, so its post-read statements may already have
    // run on the earlier arrival: the whole state stays in the window and the
    // write ends entry identity.
    let program = typed_program(
        "data Main { count: i32; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(&mut self) -> bool {
             sink(self.count);
             self.count = -1;
             transition true { true -> self false -> true }
         }",
    );
    let (machine, run) = named_state(&program, "Main::run", "run");
    let (call_index, argument) = first_call_argument(&program, machine, run);
    assert_eq!(
        entry_operand(&program, machine, run, call_index, argument),
        None,
    );
}

#[test]
fn mutable_receiver_field_keeps_no_identity_past_a_whole_receiver_escape() {
    // An exclusive `&mut self` loan or a `&mut self` receiver call can write
    // any field, so no field projection survives either escape while it can
    // still precede the read. A `&mut self` escape AFTER the read in a state
    // that cannot re-enter stays outside the window instead.
    for body in [
        "self.recompute(); sink(self.count); true",
        "let loan: &mut i32 = &mut self.count; sink(self.count); true",
    ] {
        let program = typed_program(&format!(
            "data Main {{ count: i32; }}
             machine sink(input: i32) -> bool {{ true }}
             machine peek(x: &mut Main) {{ }}
             machine Main::recompute(&mut self) {{ }}
             machine Main::run(&mut self) -> bool {{ {body} }}",
        ));
        let (machine, run) = named_state(&program, "Main::run", "run");
        let (call_index, argument) = first_call_argument(&program, machine, run);
        assert_eq!(
            entry_operand(&program, machine, run, call_index, argument),
            None,
            "{body}"
        );
    }

    let program = typed_program(
        "data Main { count: i32; }
         machine sink(input: i32) -> bool { true }
         machine peek(x: &mut Main) { }
         machine Main::run(&mut self) -> bool {
             sink(self.count);
             peek(&mut self);
             true
         }",
    );
    let (machine, run) = named_state(&program, "Main::run", "run");
    let (call_index, argument) = first_call_argument(&program, machine, run);
    assert_eq!(
        entry_operand(&program, machine, run, call_index, argument),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Parameter(0)),
            member: "count".to_owned(),
        }),
        "a whole-receiver escape after the read cannot precede it without re-entry",
    );
}

#[test]
fn a_receiver_with_unstable_contents_keeps_provenance_unknown() {
    // `Main` owns a mutable loan, so even the immutable `&self` receiver
    // cannot promise `self.slot`'s referent is the entry observation.
    let program = typed_program(
        "data Rec { flag: bool; }
         data Main { slot: &mut Rec; }
         machine sink(input: bool) -> bool { input }
         machine Main::run(&self) -> bool { sink(self.slot.flag); true }",
    );
    let (machine, run) = named_state(&program, "Main::run", "run");
    let (call_index, argument) = first_call_argument(&program, machine, run);
    assert_eq!(
        entry_operand(&program, machine, run, call_index, argument),
        None,
    );
}

const TRIGGER: &str = "machine trigger(input: bool) -> bool
         crashes Trap input
         {
             transition { !input -> false }
             crash Trap;
         }";

#[test]
fn a_state_arrival_actual_discharges_the_selected_crash_route() {
    // `input` arrives only as `false`, so the selected `crashes Trap input`
    // route is false at this invocation. `value` is public: an undischarged
    // route would have to appear on its published ceiling.
    for declaration in ["", "crashes Trap false"] {
        let program = typed_program(&format!(
            "{TRIGGER}
             pub machine value() -> bool
             {declaration}
             {{
                 transition true {{ true -> next(false) false -> false }}
                 state next(input: bool) -> bool {{ let r: bool = trigger(input); r }}
             }}"
        ));
        crate::lower_typed_trees(program)
            .unwrap_or_else(|diagnostics| panic!("{declaration}: {diagnostics:#?}"));
    }
}

#[test]
fn a_state_arrival_actual_retains_the_exact_entry_origin() {
    // `input` arrives as the caller's `flag`: the surviving Trap route is the
    // entry predicate `flag` itself, covered only when `value` publishes a
    // same-cause route for exactly that parameter.
    for (declaration, expect_ok) in [
        ("crashes Trap flag", true),
        ("crashes Trap !flag", false),
        ("", false),
    ] {
        let program = typed_program(&format!(
            "{TRIGGER}
             pub machine value(flag: bool) -> bool
             {declaration}
             {{
                 transition true {{ true -> next(flag) false -> false }}
                 state next(input: bool) -> bool {{ let r: bool = trigger(input); r }}
             }}"
        ));
        match crate::lower_typed_trees(program) {
            Ok(_) => assert!(expect_ok, "{declaration} must not check"),
            Err(diagnostics) => {
                assert!(!expect_ok, "{declaration}: {diagnostics:#?}");
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("uncovered Trap crash route")),
                    "{declaration}: {diagnostics:#?}"
                );
            }
        }
    }
}

#[test]
fn mutable_state_arrivals_refine_the_selected_crash_route() {
    // The mutable parameter's bound snapshot is the saved actual, so the
    // surviving Trap route refines to the arrival's entry-relative value.
    for (declaration, expect_ok) in [("", true), ("crashes Trap false", true)] {
        let program = typed_program(&format!(
            "{TRIGGER}
             pub machine value() -> bool
             {declaration}
             {{
                 transition true {{ true -> next(false) false -> false }}
                 state next(mut input: bool) -> bool {{ let r: bool = trigger(input); r }}
             }}"
        ));
        match crate::lower_typed_trees(program) {
            Ok(_) => assert!(expect_ok, "{declaration} must not check"),
            Err(diagnostics) => {
                assert!(!expect_ok, "{declaration}: {diagnostics:#?}");
            }
        }
    }
    // Bound from the caller's own parameter: the surviving route is exactly
    // `flag`, covered only by a same-cause published ceiling.
    for (declaration, expect_ok) in [
        ("crashes Trap flag", true),
        ("crashes Trap !flag", false),
        ("", false),
    ] {
        let program = typed_program(&format!(
            "{TRIGGER}
             pub machine value(flag: bool) -> bool
             {declaration}
             {{
                 transition true {{ true -> next(flag) false -> false }}
                 state next(mut input: bool) -> bool {{ let r: bool = trigger(input); r }}
             }}"
        ));
        match crate::lower_typed_trees(program) {
            Ok(_) => assert!(expect_ok, "{declaration} must not check"),
            Err(diagnostics) => {
                assert!(!expect_ok, "{declaration}: {diagnostics:#?}");
                assert!(
                    diagnostics.iter().any(|diagnostic| diagnostic
                        .message
                        .contains("uncovered Trap crash route")),
                    "{declaration}: {diagnostics:#?}"
                );
            }
        }
    }
}

#[test]
fn divergent_or_mutable_state_arrivals_still_reject_at_checking() {
    for source in [
        // Arrivals disagree: `input` is not one invocation value.
        "pub machine value(take: bool) -> bool
         crashes Trap take
         {
             transition take { true -> next(false) false -> next(true) }
             state next(input: bool) -> bool { let r: bool = trigger(input); r }
         }",
        // A mutable parameter written before the call no longer holds its
        // bound snapshot, so the actual stays unproven.
        "pub machine value(still: bool) -> bool
         crashes Trap still
         {
             transition true { true -> next(false) false -> false }
             state next(mut input: bool) -> bool {
                 input = true;
                 let r: bool = trigger(input);
                 r
             }
         }",
    ] {
        let program = typed_program(&format!(
            "{TRIGGER}
{source}"
        ));
        let diagnostics = crate::lower_typed_trees(program)
            .expect_err("unproven state provenance must stay conservative");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("uncovered Trap crash route")),
            "{source}: {diagnostics:#?}"
        );
    }
}

#[test]
fn structural_entry_identity_requires_plain_contents_through_generic_substitution() {
    for (carrier, stable) in [
        ("Holder<Flag>", true),
        ("Holder<Borrowed>", false),
        ("Holder<Holder<Borrowed>>", false),
        ("&Holder<Flag>", true),
        ("&mut Holder<Flag>", false),
    ] {
        let source = format!(
            "data Flag {{ enabled: bool; }}
             data Borrowed {{ flag: &mut Flag; }}
             data Holder<T> {{ value: T; }}
             machine inspect(holder: {carrier}) {{}}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "inspect")
            .unwrap();
        let parameter = &program.state_parameters(&program.machine_states(machine)[0])[0];
        assert_eq!(
            has_stable_observable_contents(&program, parameter.type_reference),
            stable,
            "{carrier}"
        );
    }
}

#[test]
fn a_case_payload_projection_transports_the_entry_actual() {
    // `Outcome::Second { c }` destructures the scrutinee's payload: the
    // synthesized member carries only its `Second` case qualification — no
    // retained `member_symbol` — so `c.item` still resolves through
    // `value.Second::c.item` to the invocation actual's projection rather
    // than falling off the place walk.
    let program = typed_program(
        "pub data Cell { item: i32; }
         pub data Outcome { case First(c: Cell); case Second(c: Cell); }
         machine sink(input: i32) -> i32 { input }
         machine run(value: Outcome) -> i32 {
             transition value {
                 Outcome::Second { c } -> done(sink(c.item))
                 Outcome::First { c } -> done(0)
             }
             state done(result: i32) -> i32 { result }
         }",
    );
    let (machine, entry) = named_state(&program, "run", "entry");
    let (call_index, argument) = first_call_argument(&program, machine, entry);
    let ExpressionNode::Call(call) = program.expression_table.expression(argument) else {
        panic!("the `done` target argument is the `sink` call")
    };
    let operand = program.expression_table.expression_handles(call.arguments)[0];
    assert_eq!(
        entry_operand(&program, machine, entry, call_index, operand),
        Some(CrashPredicateExpression::Member {
            receiver: Box::new(CrashPredicateExpression::Member {
                receiver: Box::new(CrashPredicateExpression::Parameter(0)),
                member: "Second::c".to_owned(),
            }),
            member: "item".to_owned(),
        }),
        "the destructure-bound operand keeps its case-qualified entry identity",
    );
}

#[test]
fn a_case_projection_below_rewritten_storage_keeps_provenance_unknown() {
    // `held` is rebound before the transition reads it, so the payload the
    // destructure exposes is not the invocation actual's — provenance stays
    // unknown rather than naming `value`'s entry storage.
    let program = typed_program(
        "pub data Cell { item: i32; }
         pub data Outcome { case First(c: Cell); case Second(c: Cell); }
         machine sink(input: i32) -> i32 { input }
         machine run(value: Outcome) -> i32 {
             let mut held: Outcome = value;
             held = Outcome::First { c: Cell { item: 0 } };
             transition held {
                 Outcome::Second { c } -> done(sink(c.item))
                 Outcome::First { c } -> done(0)
             }
             state done(result: i32) -> i32 { result }
         }",
    );
    let (machine, entry) = named_state(&program, "run", "entry");
    let (call_index, argument) = first_call_argument(&program, machine, entry);
    let ExpressionNode::Call(call) = program.expression_table.expression(argument) else {
        panic!("the `done` target argument is the `sink` call")
    };
    let operand = program.expression_table.expression_handles(call.arguments)[0];
    assert_eq!(
        entry_operand(&program, machine, entry, call_index, operand),
        None,
        "a rebound scrutinee cannot claim the entry operand's payload",
    );
}

#[test]
fn a_leaf_read_below_a_case_payload_operand_keeps_entry_provenance() {
    // The named-route gate asks whether the callee's leaf read — `cell.item`
    // over a formal bound to the `Second` payload — still sees the
    // invocation-entry value. The operand `c` resolves through
    // `value.Second::c`, so the composed read `value.Second::c.item` holds
    // the entry snapshot while `value` is immutable.
    let program = typed_program(
        "pub data Cell { item: i32; }
         pub data Outcome { case First(c: Cell); case Second(c: Cell); }
         machine sink(input: i32) -> i32 { input }
         machine run(value: Outcome) -> i32 {
             transition value {
                 Outcome::Second { c } -> done(sink(c.item))
                 Outcome::First { c } -> done(0)
             }
             state done(result: i32) -> i32 { result }
         }",
    );
    let (machine, entry) = named_state(&program, "run", "entry");
    let (call_index, argument) = first_call_argument(&program, machine, entry);
    let ExpressionNode::Call(call) = program.expression_table.expression(argument) else {
        panic!("the `done` target argument is the `sink` call")
    };
    let leaf = program.expression_table.expression_handles(call.arguments)[0];
    let ExpressionNode::Member(item) = program.expression_table.expression(leaf) else {
        panic!("the `sink` argument reads `c.item`")
    };
    assert!(
        operand_entry_provenance(&program, machine, entry, call_index, item.receiver, leaf),
        "the payload operand's `item` read still names the entry actual",
    );
}

#[test]
fn indexed_read_transports_collection_and_index_entry_operands() {
    // `items[index]` is a structured operand: the collection contributes its
    // whole-storage entry identity and the index its own, so the call
    // argument keeps the exact `Parameter(0)[Parameter(1)]` read rather than
    // dropping to no provenance.
    let program = typed_program(
        "machine sink(input: i32) -> i32 { input }
         machine value(items: [i32; 4], index: u64) -> i32
         requires index < 4 {
             sink(items[index]); items[index]
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let entry = program.machine_states(machine)[0].symbol;
    let (call_index, argument) = first_call_argument(&program, machine.symbol, entry);
    assert_eq!(
        entry_operand(&program, machine.symbol, entry, call_index, argument),
        Some(CrashPredicateExpression::Indexed {
            collection: Box::new(CrashPredicateExpression::Parameter(0)),
            index: Box::new(CrashPredicateExpression::Parameter(1)),
        }),
    );
}

#[test]
fn indexed_read_below_rewritten_collection_storage_widens() {
    // `items[0] = 9` ends the collection's whole-storage bound snapshot —
    // element writes cannot be separated below the binding root — so the
    // indexed read keeps no entry operand even though the index is pristine.
    let program = typed_program(
        "machine sink(input: i32) -> i32 { input }
         machine value(mut items: [i32; 4]) -> i32 {
             items[0u64] = 9; sink(items[1u64]); items[1u64]
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let entry = program.machine_states(machine)[0].symbol;
    let (call_index, argument) = first_call_argument(&program, machine.symbol, entry);
    assert_eq!(
        entry_operand(&program, machine.symbol, entry, call_index, argument),
        None,
        "an element write retires the whole collection's entry snapshot",
    );
}
