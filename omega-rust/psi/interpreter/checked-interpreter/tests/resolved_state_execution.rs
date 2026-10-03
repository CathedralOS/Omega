use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;

#[test]
fn qualified_user_calls_do_not_probe_build_facet_storage() {
    for (write, read) in [("write", "read"), ("close", "open"), ("create", "resolve")] {
        let helper = format!(
            "module helpers;
             pub data Counter {{}}
             pub machine Counter::{write}(value: &mut i32) {{ value = 70; }}
             pub machine Counter::{read}(value: &i32) -> i32 {{ value }}"
        );
        let caller = format!(
            "use helpers;
             machine main() -> i32 {{
                 let mut value: i32 = 0;
                 helpers::Counter::{write}(&mut value);
                 helpers::Counter::{read}(&value)
             }}"
        );
        let mut sources = source::SourceMap::default();
        let helper_source = sources.add("helpers.omg".into(), helper.clone()).source_id;
        let caller_source = sources.add("main.omg".into(), caller.clone()).source_id;
        let typed = crate::front_end::typed_program_from_source_map(
            sources,
            &[(helper_source, &helper), (caller_source, &caller)],
        );
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .expect("namespace calls check");
        let outcome = interpret_entry(
            &checked,
            BuildMachineEntry::Name("main"),
            &[],
            InterpretOptions::default(),
        );
        assert_eq!(outcome.error, None, "{write}/{read}");
        assert_eq!(outcome.exit_code, 70, "{write}/{read}");
    }
}

#[test]
fn qualified_static_generic_calls_preserve_distinct_local_instances() {
    let mut checked = crate::front_end::checked_program_with_generic_data_result(
        "data Counter<T> { value: T; times: i32 in Wrapping; }
         machine Counter::record<T>(counter: &mut Counter<T>, value: T) {
             counter.value = value;
             counter.times = counter.times + 1;
         }
         machine Counter::current<T [copy]>(counter: &Counter<T>) -> T {
             counter.value
         }
         machine Counter::count<T>(counter: &Counter<T>) -> i32 in Wrapping {
             counter.times
         }
         machine main() -> i32 {
             let mut score: Counter<i32>;
             let mut flag: Counter<bool>;
             Counter::record(&mut score, 30);
             Counter::record(&mut score, 12);
             Counter::record(&mut flag, true);
             let total: i32 = Counter::current(&score);
             let times_score: i32 in Wrapping = Counter::count(&score);
             let times_flag: i32 in Wrapping = Counter::count(&flag);
             let latched: bool = Counter::current(&flag);
             transition latched {
                 true -> tally(total, times_score, times_flag)
                 false -> 3
             }
             state tally(total: i32, times_score: i32 in Wrapping,
                         times_flag: i32 in Wrapping) -> i32 {
                 ((total as i32 in Wrapping) + times_score + 1 + times_flag) as i32
             }
         }",
    )
    .expect("static generic counter source checks");
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 16);

    // Both statement and value calls retain the selected specialization even
    // when every helper's declaration and entry display names collide.
    let helpers: Vec<_> = checked
        .typed
        .machines()
        .iter()
        .filter(|machine| machine.name.as_str() != "main")
        .cloned()
        .collect();
    for machine in &helpers {
        for state in checked.typed.machine_states_mut(machine) {
            state.name = "shared".into();
        }
    }
    for machine in checked.typed.machines_mut() {
        if helpers.iter().any(|helper| helper.symbol == machine.symbol) {
            machine.name = "shared".into();
        }
    }
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 16);
}

#[test]
fn qualified_statement_without_target_does_not_select_a_namesake() {
    let mut checked = crate::front_end::checked_program(
        "data Commands {}
         machine Commands::write(value: &mut i32) { value = 70; }
         machine write(value: &mut i32) { value = 9; }
         machine main() -> i32 {
             let mut value: i32 = 0;
             Commands::write(&mut value);
             value
         }",
    );
    let main = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .expect("main exists");
    let statements = checked.machine_states(main)[0].statement_nodes;
    let call = checked
        .typed
        .statement_table
        .statements_mut(statements)
        .iter_mut()
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::Call(call) => Some(call),
            _ => None,
        })
        .expect("qualified statement exists");
    assert!(call.target_symbol.is_valid());
    call.target_symbol = symbols::SymbolHandle::invalid();
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(
        outcome.error.as_deref(),
        Some("qualified call target `write` has no resolved state")
    );
}

#[test]
fn qualified_value_without_target_does_not_select_by_namespace_spelling() {
    for target in ["choose", "tick_count"] {
        let mut checked = crate::front_end::checked_program(&format!(
            "data Choices {{}}
         machine Choices::{target}() -> i32 {{ 70 }}
         machine {target}() -> i32 {{ 9 }}
         machine main() -> i32 {{ Choices::{target}() }}",
        ));
        let expression = checked
            .expression_table
            .iter_expressions()
            .find_map(|(handle, expression)| match expression {
                typed_trees::expression::ExpressionNode::Call(call) if call.receiver.is_valid() => {
                    Some(handle)
                }
                _ => None,
            })
            .expect("qualified value call exists");
        let typed_trees::expression::ExpressionNode::Call(call) =
            checked.typed.expression_table.expression_mut(expression)
        else {
            unreachable!("selected call expression");
        };
        assert!(call.target_symbol.is_valid());
        call.target_symbol = symbols::SymbolHandle::invalid();
        let outcome = interpret_entry(
            &checked,
            BuildMachineEntry::Name("main"),
            &[],
            InterpretOptions::default(),
        );
        assert_eq!(
            outcome.error,
            Some(format!(
                "qualified call target `{target}` has no resolved state"
            ))
        );
    }
}

#[test]
fn namespace_operator_keeps_its_identity_backed_intrinsic_route() {
    for (namespace, carrier) in [("F32", "f32"), ("F64", "f64")] {
        let typed = crate::front_end::typed_program(&format!(
            "pub data {namespace} {{}}
             pub boundary operator {namespace}::square_root(value: {carrier}) -> {carrier};
             machine main() -> i32 {{ {namespace}::square_root(4.0{carrier}) as i32 }}",
        ));
        assert_eq!(
            checked_interpreter::evaluate_const_machine(&typed, "main"),
            Ok(2)
        );
    }
}

#[test]
fn static_calls_mutate_the_explicit_local_instance() {
    let checked = crate::front_end::checked_program(
        "data Counter { value: i32 in Wrapping; }
         machine Counter::increment(counter: &mut Counter) { counter.value = counter.value + 1; }
         machine Counter::twice(counter: &mut Counter) {
             Counter::increment(counter);
             Counter::increment(counter);
         }
         machine main() -> i32 {
             let mut counter: Counter;
             Counter::twice(&mut counter);
             counter.value as i32
         }",
    );
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 2);
}

#[test]
fn resolved_calls_keep_exact_states_when_debug_spellings_collide() {
    let mut checked = crate::front_end::checked_program(
        "machine first() -> i32 { transition { _ -> chosen() } state chosen() -> i32 { 3 } }
         machine second() -> i32 { transition { _ -> chosen() } state chosen() -> i32 { 4 } }
         machine main() -> i32 { transition first() == 3 && second() == 4 { true -> 7 false -> 0 } }",
    );
    // Resolved symbols, not retained debug spelling, identify these calls. Make
    // both machine and state spellings collide after checking to expose a lost
    // target identity at the interpreter's resolution-to-execution boundary.
    let machines = checked.typed.machines()[..2].to_vec();
    for machine in &machines {
        for state in checked.typed.machine_states_mut(machine) {
            state.name = "shared".into();
        }
    }
    for machine in &mut checked.typed.machines_mut()[..2] {
        machine.name = "shared".into();
    }
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn sibling_transitions_preserve_explicit_borrow_and_arguments() {
    let checked = crate::front_end::checked_program(
        "data Counter { value: i32 in Wrapping; }
         machine Counter::advance(counter: &mut Counter, amount: i32 in Wrapping) -> i32 {
             let carried: i32 in Wrapping = amount + 1;
             transition true { true -> finish(counter, carried) false -> 0 }
             state finish(counter: &mut Counter, carried: i32 in Wrapping) -> i32 {
                 counter.value = counter.value + carried;
                 counter.value as i32
             }
         }
         machine main() -> i32 {
             let mut counter: Counter = Counter { value: 2 };
             let result: i32 = Counter::advance(&mut counter, 4);
             transition result == 7 && counter.value == 7 { true -> 7 false -> 0 }
         }",
    );
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn resolved_calls_evaluate_arguments_once_in_source_order() {
    let checked = crate::front_end::checked_program(
        "machine step(value: &mut i32 in Wrapping) -> i32 { value = value + 1; value as i32 }
         machine combine(left: i32 in Wrapping, right: i32 in Wrapping) -> i32 {
             (left * 10 + right) as i32
         }
         machine main() -> i32 {
             let mut count: i32 in Wrapping = 0;
             let result: i32 = combine(
                 step(&mut count) as i32 in Wrapping,
                 step(&mut count) as i32 in Wrapping
             );
             transition result == 12 && count == 2 { true -> 7 false -> 0 }
         }",
    );
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}
