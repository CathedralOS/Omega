use super::{entry_operand, has_stable_observable_contents};
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
