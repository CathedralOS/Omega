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
         machine Main::main(&mut self) -> i32 {
             self.untouched = 41;
             self.counter.store(10, NoOrdering);
             let prior: u32 in Wrapping = self.counter.fetch_add(5, NoOrdering);
             transition prior == 10 && self.counter == 15 && self.untouched == 41 {
                 true -> exchange() false -> 71
             }
             state exchange(&mut self) -> i32 {
                 let prior: u32 in Wrapping = self.counter.swap(23, ReceivePublish);
                 transition prior == 15 && self.counter == 23 && self.untouched == 41 {
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
         machine Main::main(&mut self) -> i32 {
             self.untouched = 41;
             self.counter.store(10, NoOrdering);
             let prior: u32 in Wrapping = self.counter.compare_exchange(10, 99, ReceivePublish, Receive);
             transition prior == 10 && self.counter == 99 && self.untouched == 41 {
                 true -> mismatch() false -> 71
             }
             state mismatch(&mut self) -> i32 {
                 let prior: u32 in Wrapping = self.counter.compare_exchange(10, 42, ReceivePublish, Receive);
                 transition prior == 99 && self.counter == 99 && self.untouched == 41 {
                     true -> 70 false -> 72
                 }
             }
         }",
    );
}

#[test]
fn atomic_result_local_cannot_shadow_a_receiver_field() {
    let diagnostics = crate::front_end::checked_program_result(
        "data Main { counter: AtomicU32; prior: u32 in Wrapping; }
         machine Main::main(&mut self) -> i32 {
             let prior: u32 in Wrapping = self.counter.fetch_add(5, NoOrdering);
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
