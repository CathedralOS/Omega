#[path = "../support/program.rs"]
mod program;
#[path = "../support/mod.rs"]
mod support;

mod block_valued_arms;
mod copied_readers;
mod incoming_reference_match;
mod inlined_multiple_returns;
mod inlined_scalar_arguments;
mod parenthesized_guard;
mod record_callback_loop;
mod some_reader;
mod straight_line_loop_value;
mod unit_callable_alias;

#[test]
fn a_suspending_machine_parameter_preserves_its_argument_and_result() {
    program::assert_silent_return(include_str!(
        "../fixtures/callables/run/suspending_parameter/main.omg"
    ));
}
