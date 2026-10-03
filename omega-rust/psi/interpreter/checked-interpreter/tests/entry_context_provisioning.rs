//! The arrival bridge's provisioned entry context
//! (wiki/spec/build/entry_roots.md#entry-shape-and-arrival-bridge): the entry
//! shape names one exact parameter occurrence, and the interpreter supplies it
//! as an exclusive borrow of a zero-initialized record of that parameter's
//! declared referent type. No other entry parameter receives a value.

use checked_interpreter::{BuildMachineEntry, InterpretOptions, InterpretOutcome, interpret_entry};
use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;

fn checked(source: &str) -> CheckedTrees {
    crate::front_end::checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

/// The entry-state parameter occurrence a target entry shape would select.
fn entry_parameter(
    checked: &CheckedTrees,
    machine_name: &str,
    selects: impl Fn(&StateParameter) -> bool,
) -> SymbolHandle {
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("entry machine");
    let state = checked
        .typed
        .machine_states(machine)
        .first()
        .expect("entry state");
    checked
        .typed
        .state_parameters(state)
        .iter()
        .find(|parameter| selects(parameter))
        .expect("entry parameter")
        .symbol
}

fn named(name: &str) -> impl Fn(&StateParameter) -> bool {
    move |parameter| parameter.name.as_str() == name
}

fn run(checked: &CheckedTrees, entry: &str, context: Option<SymbolHandle>) -> InterpretOutcome {
    let mut options = InterpretOptions::default();
    if let Some(context) = context {
        options = options.with_provisioned_context_parameter(context);
    }
    interpret_entry(checked, BuildMachineEntry::Name(entry), &[], options)
}

// The migrated static entry: an ordinary `main: &mut Main` parameter reads
// and writes a zeroed record, and the same borrow travels into a sibling
// state. Before provisioning this bound `Unit` and trapped on the first field.
#[test]
fn provisioned_context_parameter_starts_zeroed_and_carries_across_states() {
    let program = checked(
        "data Main { values: [u8; 10]; count: u32; ready: bool; }
        machine Main::main(main: &mut Main) -> i32 {
            main.values[9] = 7;
            transition main.values[9] == 7 && main.count == 0 && !main.ready {
                true -> finish(main)
                false -> 0
            }
            state finish(main: &mut Main) -> i32 {
                main.count = main.count + 3;
                (main.values[9] as i32) * 10 + (main.count as i32)
            }
        }",
    );
    let context = entry_parameter(&program, "Main::main", named("main"));
    let outcome = run(&program, "Main::main", Some(context));
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 73);
}

// The parameter's declared referent type determines the record: a free entry
// with no declaration attachment still receives its zeroed context, including
// nested record and array fields.
#[test]
fn provisioned_context_record_is_the_parameter_referent_type() {
    let program = checked(
        "data Totals { sum: u8; cells: [u8; 3]; }
        data Context { totals: Totals; limit: u8; }
        machine main(context: &mut Context) -> i32 {
            let zeroed: bool = context.limit == 0 && context.totals.sum == 0
                && context.totals.cells[0] == 0 && context.totals.cells[2] == 0;
            context.totals.cells[2] = 5;
            context.totals.sum = 2;
            transition zeroed {
                true -> (context.totals.sum as i32) + (context.totals.cells[2] as i32)
                false -> 0
            }
        }",
    );
    let context = entry_parameter(&program, "main", named("context"));
    let outcome = run(&program, "main", Some(context));
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

// A nonprovisioned entry receives no implicit state: a parameter the entry
// shape did not name declines interpretation instead of binding `Unit`.
#[test]
fn unprovisioned_entry_parameter_declines_instead_of_binding_unit() {
    let program = checked(
        "data Main { count: u8; }
        machine Main::main(main: &mut Main) -> i32 { main.count as i32 }",
    );
    let outcome = run(&program, "Main::main", None);
    let error = outcome
        .error
        .expect("an unsupplied entry parameter declines");
    assert!(
        error.contains("parameter `main` is not the selected entry shape's provisioned context"),
        "{error}"
    );
}

// The provisioned occurrence must belong to the entry being run; a symbol
// from another machine is not silently ignored.
#[test]
fn provisioned_context_from_another_machine_declines() {
    let program = checked(
        "data Main { count: u8; }
        machine helper(other: &mut Main) -> i32 { other.count as i32 }
        machine main() -> i32 { 7 }",
    );
    let foreign = entry_parameter(&program, "helper", named("other"));
    let outcome = run(&program, "main", Some(foreign));
    let error = outcome
        .error
        .expect("a foreign context occurrence declines");
    assert!(
        error.contains("provisioned context is not a parameter of entry machine `main`"),
        "{error}"
    );
}

// Declaration attachment is a namespace, not an implicit instance. The entry
// context remains the explicitly selected parameter even for an attached machine.
#[test]
fn attached_entry_uses_its_explicit_context_parameter() {
    let program = checked(
        "data Main { count: u8; }
        machine Main::main(context: &mut Main) -> i32 {
            transition context.count == 0 { true -> 7 false -> 0 }
        }",
    );
    let context = entry_parameter(&program, "Main::main", named("context"));
    let outcome = run(&program, "Main::main", Some(context));
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}
