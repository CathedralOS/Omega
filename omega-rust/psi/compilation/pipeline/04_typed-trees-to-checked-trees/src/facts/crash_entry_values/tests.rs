use super::{
    entry_operand, entry_operand_projected, formal_member_projection, operand_entry_provenance,
};
use crate::tests::front_end::typed_program;
use checked_trees::CrashPredicateExpression;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionTargetNode};

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
        // A guarded back-edge after a write passes the written value into
        // the next arrival, so no single bound snapshot exists.
        "machine sink(input: bool) -> bool { input }
         machine value() -> bool {
             transition true { true -> next(false) false -> false }
             state next(mut input: bool) -> bool {
                 sink(input);
                 input = true;
                 transition input { true -> next(input) false -> false }
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
fn a_pristine_named_back_edge_still_transports_the_bound_snapshot() {
    // The explicit back-edge sees no writes, so it forwards the bound snapshot
    // itself and `input` stays `flag` on every iteration.
    let program = typed_program(
        "machine sink(input: bool) -> bool { input }
         machine value(flag: bool) -> bool {
             transition flag { true -> next(flag) false -> false }
             state next(mut input: bool) -> bool {
                 sink(input);
                 transition input { true -> next(input) false -> false }
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
fn mutable_parameter_field_survives_writes_confined_to_siblings() {
    // The write-escape scan is per field: `context.other = 1` and a mutating
    // call on `context.inner` never reach `context.count`, whose
    // provenance stays intact in a non-entry state too.
    let program = typed_program(
        "data Inner { n: i32; }
         machine Inner::bump(inner: &mut Inner) { inner.n = 1; }
         data Main { count: i32; other: i32; inner: Inner; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(context: &mut Main) -> bool {
             context.other = 1;
             Inner::bump(&mut context.inner);
             transition true { true -> work(context) false -> true }
             state work(context: &mut Main) -> bool { sink(context.count); true }
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
fn mutable_parameter_field_direct_call_retires_entry_value() {
    for argument in [
        "context",
        "match flag { true -> context, false -> context }",
    ] {
        let program = typed_program(&format!(
            "data Main {{ count: i32; }}
         machine overwrite(context: &mut Main) {{ context.count = -1; }}
         machine sink(input: i32) -> bool {{ true }}
         machine Main::run(context: &mut Main, flag: bool) -> bool {{
             overwrite({argument});
             transition {{ _ -> work(context) }}
             state work(context: &mut Main) -> bool {{ sink(context.count); true }}
         }}"
        ));
        let (machine, work) = named_state(&program, "Main::run", "work");
        let (call_index, argument) = first_call_argument(&program, machine, work);
        assert_eq!(
            entry_operand(&program, machine, work, call_index, argument),
            None,
            "passing a mutable reference can change the field without an explicit borrow expression",
        );
    }
}

#[test]
fn mutable_parameter_field_later_write_still_escapes_once_the_state_reenters() {
    // The named back-edge re-enters `run`, so its post-read statements may already have
    // run on the earlier arrival: the whole state stays in the window and the
    // write ends entry identity.
    let program = typed_program(
        "data Main { count: i32; }
         machine sink(input: i32) -> bool { true }
         machine Main::run(context: &mut Main) -> bool {
             transition { _ -> work(context) }
             state work(context: &mut Main) -> bool {
                 sink(context.count);
                 context.count = -1;
                 transition true { true -> work(context) false -> true }
             }
         }",
    );
    let (machine, run) = named_state(&program, "Main::run", "work");
    let (call_index, argument) = first_call_argument(&program, machine, run);
    assert_eq!(
        entry_operand(&program, machine, run, call_index, argument),
        None,
    );
}

/// The index itself must carry entry provenance: a rebound selector cannot
/// name the element the leaf read at the invocation, so the operand keeps no
/// provenance even while the collection is pristine.
#[test]
fn an_indexed_operand_requires_the_index_entry_operand() {
    let program = typed_program(
        "data Cell { count: u64 }
         machine sink(value: u64) -> u64 { value }
         machine probe(cell: Cell) -> u64 { sink(cell.count) }
         machine value(items: [Cell; 4], index: u64) -> u64
         requires index < 4 {
             let mut slot: u64 = index;
             slot = 0;
             sink(items[slot])
         }",
    );
    let (machine, entry) = named_state(&program, "value", "entry");
    let (call_index, operand) = targeted_call_argument(&program, machine, entry, "sink");
    let (probe_machine, probe_entry) = named_state(&program, "probe", "entry");
    let (_, leaf) = targeted_call_argument(&program, probe_machine, probe_entry, "sink");
    assert!(
        !operand_entry_provenance(&program, machine, entry, call_index, operand, leaf),
        "a rebound selector cannot name the element the leaf read",
    );
}

/// A literal-initialized local recovers its indexed element through the same
/// literal projection `entry_operand_at` uses for the bare operand: the saved
/// initializer names `input` for `cells[1]`, so a projected leaf still binds
/// the entry actual rather than falling off the structural walk.
#[test]
fn a_literal_indexed_operand_recovers_its_element_below_a_leaf() {
    let program = typed_program(
        "data Cell { count: u64 }
         machine sink(value: u64) -> u64 { value }
         machine probe(cell: Cell) -> u64 { sink(cell.count) }
         machine value(input: Cell) -> u64 {
             let cells: [Cell; 2] = [Cell { count: 1 }, input];
             sink(cells[1u64])
         }",
    );
    let (machine, entry) = named_state(&program, "value", "entry");
    let (call_index, operand) = targeted_call_argument(&program, machine, entry, "sink");
    let (probe_machine, probe_entry) = named_state(&program, "probe", "entry");
    let (_, leaf) = targeted_call_argument(&program, probe_machine, probe_entry, "sink");
    assert!(
        operand_entry_provenance(&program, machine, entry, call_index, operand, leaf),
        "the literal element `input` still proves the projected read",
    );
    let probe = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == probe_machine)
        .unwrap();
    let state = program
        .machine_states(probe)
        .iter()
        .find(|state| state.name.as_str() == "entry")
        .unwrap();
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "cell")
        .unwrap();
    let projection =
        formal_member_projection(&program, parameter.type_reference, &["count".into()]);
    assert_eq!(
        entry_operand_projected(&program, machine, entry, call_index, operand, &projection,),
        Some(CrashPredicateExpression::Parameter(0)),
        "the literal initializer supplies `input` as the element's entry operand",
    );
}

/// A statically fixed element selection keeps its normalized `FixedIndex`
/// identity: a write to a provably disjoint element no longer retires the
/// whole collection's bound snapshot, so the projected leaf still binds the
/// entry operand rather than widening the surviving route to `Truth`.
#[test]
fn a_fixed_index_operand_separates_a_disjoint_element_write() {
    let program = typed_program(
        "data Cell { count: u64 }
         machine sink(value: u64) -> u64 { value }
         machine probe(cell: Cell) -> u64 { sink(cell.count) }
         machine value(mut items: [Cell; 4], spare: Cell) -> u64 {
             items[0u64] = spare;
             sink(items[1u64])
         }",
    );
    let (machine, entry) = named_state(&program, "value", "entry");
    let (call_index, operand) = targeted_call_argument(&program, machine, entry, "sink");
    assert!(
        matches!(
            program.expression_table.expression(operand),
            ExpressionNode::Indexed(_)
        ),
        "the operand is the caller's `items[1]` read"
    );
    let (probe_machine, probe_entry) = named_state(&program, "probe", "entry");
    let (_, leaf) = targeted_call_argument(&program, probe_machine, probe_entry, "sink");
    assert!(
        operand_entry_provenance(&program, machine, entry, call_index, operand, leaf),
        "a disjoint fixed element write does not retire the read element's snapshot",
    );
    let probe = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == probe_machine)
        .unwrap();
    let state = program
        .machine_states(probe)
        .iter()
        .find(|state| state.name.as_str() == "entry")
        .unwrap();
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "cell")
        .unwrap();
    let projection =
        formal_member_projection(&program, parameter.type_reference, &["count".into()]);
    assert!(
        matches!(
            entry_operand_projected(&program, machine, entry, call_index, operand, &projection,),
            Some(CrashPredicateExpression::Indexed { .. })
        ),
        "the produced entry operand keeps the structured `items[1]` element read",
    );

    // A write to the read element itself still retires the snapshot.
    let program = typed_program(
        "data Cell { count: u64 }
         machine sink(value: u64) -> u64 { value }
         machine probe(cell: Cell) -> u64 { sink(cell.count) }
         machine value(mut items: [Cell; 4], spare: Cell) -> u64 {
             items[1u64] = spare;
             sink(items[1u64])
         }",
    );
    let (machine, entry) = named_state(&program, "value", "entry");
    let (call_index, operand) = targeted_call_argument(&program, machine, entry, "sink");
    let (probe_machine, probe_entry) = named_state(&program, "probe", "entry");
    let (_, leaf) = targeted_call_argument(&program, probe_machine, probe_entry, "sink");
    assert!(
        !operand_entry_provenance(&program, machine, entry, call_index, operand, leaf),
        "a write to the read element retires its snapshot",
    );
}

/// A `start..end` index operand keeps its normalized `FixedRange` window: a
/// write to an element outside the window leaves the read's bound snapshot
/// intact, while a write inside the window still retires it.
#[test]
fn a_fixed_range_operand_separates_disjoint_element_writes() {
    let program = typed_program(
        "data Cell { count: u64 }
         machine sink(value: u64) -> u64 { value }
         machine probe(cell: Cell) -> u64 { sink(cell.count) }
         machine value(mut items: [Cell; 4], spare: Cell) -> u64 {
             items[0u64] = spare;
             sink(items[1u64..3u64])
         }",
    );
    let (machine, entry) = named_state(&program, "value", "entry");
    let (call_index, operand) = targeted_call_argument(&program, machine, entry, "sink");
    let (probe_machine, probe_entry) = named_state(&program, "probe", "entry");
    let (_, leaf) = targeted_call_argument(&program, probe_machine, probe_entry, "sink");
    assert!(
        operand_entry_provenance(&program, machine, entry, call_index, operand, leaf),
        "a write outside the fixed window does not reach the range read",
    );

    // An element write inside the window still interferes.
    let program = typed_program(
        "data Cell { count: u64 }
         machine sink(value: u64) -> u64 { value }
         machine probe(cell: Cell) -> u64 { sink(cell.count) }
         machine value(mut items: [Cell; 4], spare: Cell) -> u64 {
             items[2u64] = spare;
             sink(items[1u64..3u64])
         }",
    );
    let (machine, entry) = named_state(&program, "value", "entry");
    let (call_index, operand) = targeted_call_argument(&program, machine, entry, "sink");
    let (probe_machine, probe_entry) = named_state(&program, "probe", "entry");
    let (_, leaf) = targeted_call_argument(&program, probe_machine, probe_entry, "sink");
    assert!(
        !operand_entry_provenance(&program, machine, entry, call_index, operand, leaf),
        "a write inside the fixed window retires the range read's snapshot",
    );
}
