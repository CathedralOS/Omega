use checked_interpreter::{BuildMachineEntry, InterpretOptions, interpret_entry};
use checked_trees::expression::ExpressionNode;

fn assert_atomic_results(source: &str) {
    let checked = crate::front_end::checked_program(source);
    let mut result_symbols = Vec::new();
    for (_, expression) in checked.expression_table.iter_expressions() {
        let ExpressionNode::Atomic(atomic) = expression else {
            continue;
        };
        if !atomic.result.is_valid() {
            continue;
        }
        let ExpressionNode::Name(path) = checked.expression_table.expression(atomic.result) else {
            panic!("the authored atomic result is a local name");
        };
        assert!(
            path.head_symbol.is_valid(),
            "atomic result lost its local symbol"
        );
        result_symbols.push(path.head_symbol);
    }
    assert_eq!(result_symbols.len(), 2);
    // Both states spell their local `prior`, but their identities stay distinct.
    assert_ne!(result_symbols[0], result_symbols[1]);
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("Main::main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

#[test]
fn fetch_and_swap_bind_priors_in_their_own_state_scope() {
    assert_atomic_results(
        "data Main { counter: AtomicU32; untouched: u32 in Wrapping; }
         machine Main::main() -> i32 {
             let mut context: Main;
             context.untouched = 41;
             context.counter.store(10, NoOrdering);
             let prior: u32 in Wrapping = context.counter.fetch_add(5, NoOrdering);
             transition prior == 10 && context.counter == 15 && context.untouched == 41 {
                 true -> exchange(&mut context) false -> 71
             }
             state exchange(context: &mut Main) -> i32 {
                 let prior: u32 in Wrapping = context.counter.swap(23, ReceivePublish);
                 transition prior == 15 && context.counter == 23 && context.untouched == 41 {
                     true -> 70 false -> 72
                 }
             }
         }",
    );
}

#[test]
fn compare_exchange_binds_the_observed_prior_on_success_and_mismatch() {
    assert_atomic_results(
        "data Main { counter: AtomicU32; untouched: u32 in Wrapping; }
         machine Main::main() -> i32 {
             let mut context: Main;
             context.untouched = 41;
             context.counter.store(10, NoOrdering);
             let prior: u32 in Wrapping = context.counter.compare_exchange(10, 99, ReceivePublish, Receive);
             transition prior == 10 && context.counter == 99 && context.untouched == 41 {
                 true -> mismatch(&mut context) false -> 71
             }
             state mismatch(context: &mut Main) -> i32 {
                 let prior: u32 in Wrapping = context.counter.compare_exchange(10, 42, ReceivePublish, Receive);
                 transition prior == 99 && context.counter == 99 && context.untouched == 41 {
                     true -> 70 false -> 72
                 }
             }
         }",
    );
}

#[test]
fn atomic_result_local_cannot_shadow_a_parameter() {
    let diagnostics = crate::front_end::checked_program_result(
        "machine Main::main(prior: u32 in Wrapping) -> i32 {
             let mut counter: AtomicU32;
             let prior: u32 in Wrapping = counter.fetch_add(5, NoOrdering);
             0
         }",
    )
    .expect_err("atomic results obey ordinary local declaration rules");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("local data `prior` conflicts with an existing name")
    }));
}
