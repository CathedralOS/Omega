use super::*;
use crate::{
    BuildMachineEntry, InterpretOptions, VirtualClockBinding, VirtualClockOperand, interpret_entry,
};
use checked_trees::CheckedTrees;
use typed_trees::statement::StatementNode;

fn source(arguments: &str) -> String {
    format!(
        "boundary trait Timer {{ machine sleep(milliseconds: u32) reaches Timer; }}
         data Delay {{}}
         boundary requirement Delay::wait(pointer: u64, count: u64, milliseconds: i32) reaches Timer;
         boundary requirement Delay::other(pointer: u64, count: u64, milliseconds: i32) reaches Timer;
         data Main {{}}
         machine Main::main() reaches Timer {{ Delay::wait({arguments}); }}"
    )
}

fn program(arguments: &str) -> CheckedTrees {
    crate::front_end::checked_program(&source(arguments))
}

fn call(checked: &CheckedTrees) -> TableCall {
    let machine = checked.realized_machine_named("Main::main").expect("main");
    let entry = &checked.machine_states(machine)[0];
    checked
        .statement_table
        .statements(entry.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::Call(call) => Some(call.clone()),
            _ => None,
        })
        .expect("wait call")
}

fn binding(checked: &CheckedTrees) -> VirtualClockBinding {
    VirtualClockBinding::from_compiler_resolved_requirement(
        checked,
        checked
            .realized_machine_named("Delay::wait")
            .expect("wait")
            .symbol,
        &[
            VirtualClockOperand::Ignored,
            VirtualClockOperand::ExactInteger(0),
            VirtualClockOperand::Milliseconds,
        ],
        crate::VirtualClockOperation::Wait {
            maximum_milliseconds: i64::from(i32::MAX),
            result: crate::VirtualClockResult::Unit,
        },
    )
    .expect("exact wait binding")
}

#[test]
fn virtual_clock_wait_preserves_zero_and_exact_duration_with_host_accounting() {
    for (arguments, expected) in [
        ("0, 0, 0", 0),
        ("42, 0, 70", 70),
        ("0, 0, 18446744073709551616 - 18446744073709551615", 1),
        ("0, 0, 2147483647", i64::from(i32::MAX)),
    ] {
        let checked = program(arguments);
        let mut evaluator = Evaluator::new_checked(&checked, &[]);
        evaluator.virtual_clock_bindings.push(binding(&checked));
        let cell = evaluator
            .allocate_cell(Value::Unit)
            .unwrap_or_else(|_| panic!("frame cell"));
        let mut frame = Frame::bare(cell, SymbolHandle::invalid());
        assert!(matches!(
            evaluator.try_host_call(&call(&checked), &mut frame),
            Ok(Some(Value::Unit))
        ));
        assert_eq!(evaluator.virtual_ticks, expected);
        assert!(evaluator.host_boundary_touched);
        assert!(evaluator.non_fs_host_boundary_touched);
    }
}

#[test]
fn virtual_clock_wait_rejects_unsupported_operands_without_advancing_time() {
    for arguments in ["0, 1, 10", "0, 0, -1"] {
        let checked = program(arguments);
        let mut evaluator = Evaluator::new_checked(&checked, &[]);
        evaluator.virtual_clock_bindings.push(binding(&checked));
        evaluator.virtual_ticks = 23;
        let cell = evaluator
            .allocate_cell(Value::Unit)
            .unwrap_or_else(|_| panic!("frame cell"));
        let mut frame = Frame::bare(cell, SymbolHandle::invalid());
        assert!(matches!(
            evaluator.try_host_call(&call(&checked), &mut frame),
            Err(Halt::Unsupported(_))
        ));
        assert_eq!(evaluator.virtual_ticks, 23);
        assert!(evaluator.non_fs_host_boundary_touched);
    }
}

#[test]
fn virtual_clock_wait_is_explicit_and_rejects_other_programs_and_duplicate_bindings() {
    let checked = program("0, 0, 5");
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("Main::main"),
        &[],
        InterpretOptions::default(),
    );
    assert!(
        outcome
            .error
            .as_deref()
            .is_some_and(|message| message.contains("bodyless")),
        "{outcome:?}"
    );
    let wait = binding(&checked);
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("Main::main"),
        &[],
        InterpretOptions {
            virtual_clock_bindings: vec![wait.clone()],
            ..Default::default()
        },
    );
    assert!(outcome.error.is_none(), "{outcome:?}");
    let cloned = checked.clone();
    let outcome = interpret_entry(
        &cloned,
        BuildMachineEntry::Name("Main::main"),
        &[],
        InterpretOptions {
            virtual_clock_bindings: vec![wait.clone()],
            ..Default::default()
        },
    );
    assert!(
        outcome
            .error
            .as_deref()
            .is_some_and(|message| message.contains("different checked program"))
    );
    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("Main::main"),
        &[],
        InterpretOptions {
            virtual_clock_bindings: vec![wait.clone(), wait],
            ..Default::default()
        },
    );
    assert!(
        outcome
            .error
            .as_deref()
            .is_some_and(|message| message.contains("duplicate bindings"))
    );
}

#[test]
fn virtual_clock_wait_does_not_grant_a_same_shaped_requirement() {
    let checked = program("0, 0, 5");
    let mut evaluator = Evaluator::new_checked(&checked, &[]);
    evaluator.virtual_clock_bindings.push(binding(&checked));
    let cell = evaluator
        .allocate_cell(Value::Unit)
        .unwrap_or_else(|_| panic!("frame cell"));
    let mut frame = Frame::bare(cell, SymbolHandle::invalid());
    let mut substituted = call(&checked);
    substituted.target_symbol = checked.machine_states(
        checked
            .realized_machine_named("Delay::other")
            .expect("other requirement"),
    )[0]
    .symbol;
    assert!(matches!(
        evaluator.try_host_call(&substituted, &mut frame),
        Ok(None)
    ));
    assert_eq!(evaluator.virtual_ticks, 0);
    assert!(!evaluator.host_boundary_touched);
}

#[test]
fn virtual_clock_wait_rejects_clock_overflow_without_committing_time() {
    let checked = program("0, 0, 5");
    let mut evaluator = Evaluator::new_checked(&checked, &[]);
    evaluator.virtual_clock_bindings.push(binding(&checked));
    evaluator.virtual_ticks = i64::MAX - 1;
    let cell = evaluator
        .allocate_cell(Value::Unit)
        .unwrap_or_else(|_| panic!("frame cell"));
    let mut frame = Frame::bare(cell, SymbolHandle::invalid());
    assert!(matches!(
        evaluator.try_host_call(&call(&checked), &mut frame),
        Err(Halt::Unsupported(_))
    ));
    assert_eq!(evaluator.virtual_ticks, i64::MAX - 1);
}

#[test]
fn virtual_clock_wait_cannot_replace_a_checked_body() {
    let checked = program("0, 0, 5");
    let main = checked.realized_machine_named("Main::main").expect("main");
    assert!(
        VirtualClockBinding::from_compiler_resolved_requirement(
            &checked,
            main.symbol,
            &[VirtualClockOperand::Milliseconds],
            crate::VirtualClockOperation::Wait {
                maximum_milliseconds: 100,
                result: crate::VirtualClockResult::Unit,
            },
        )
        .is_err()
    );
}

#[test]
fn virtual_clock_wait_followed_by_time_read_rejects_overflow() {
    let checked = program("0, 0, 5");
    let mut evaluator = Evaluator::new_checked(&checked, &[]);
    evaluator.virtual_clock_bindings.push(binding(&checked));
    evaluator.virtual_ticks = i64::MAX - 5;
    let cell = evaluator
        .allocate_cell(Value::Unit)
        .unwrap_or_else(|_| panic!("frame cell"));
    let mut frame = Frame::bare(cell, SymbolHandle::invalid());
    assert!(matches!(
        evaluator.try_host_call(&call(&checked), &mut frame),
        Ok(Some(Value::Unit))
    ));
    assert_eq!(evaluator.virtual_ticks, i64::MAX);
    assert!(
        evaluator
            .virtual_time_host_value("wall_clock_raw")
            .is_none()
    );
    assert!(matches!(
        evaluator.serve_host_operation(
            HostOperation::TickCount,
            SymbolHandle::invalid(),
            &[],
            &mut frame
        ),
        Err(Halt::Unsupported(_))
    ));
    assert_eq!(evaluator.virtual_ticks, i64::MAX);
}

#[test]
fn virtual_clock_wait_executes_in_value_position() {
    let checked = crate::front_end::checked_program(
        &source("0, 0, 70").replace("Delay::wait(0, 0, 70);", "_ = Delay::wait(0, 0, 70);"),
    );
    let mut evaluator = Evaluator::new_checked(&checked, &[]);
    evaluator.virtual_clock_bindings.push(binding(&checked));
    assert!(evaluator.run_entry("Main::main", None).is_ok());
    assert_eq!(evaluator.virtual_ticks, 70);
    assert!(evaluator.non_fs_host_boundary_touched);
}

#[test]
fn virtual_clock_wait_evaluates_operands_once_in_source_order() {
    let text = source("Timer::tick_count(), Timer::tick_count() - 2, Timer::tick_count()")
        .replace(
            "machine sleep(milliseconds: u32)",
            "machine tick_count() -> i32",
        )
        .replace("pointer: u64, count: u64", "pointer: i32, count: i32");
    let checked = crate::front_end::checked_program(&text);
    let mut evaluator = Evaluator::new_checked(&checked, &[]);
    evaluator.virtual_clock_bindings.push(binding(&checked));
    assert!(evaluator.run_entry("Main::main", None).is_ok());
    // Reads observe 1, 2 and 3; only that order makes the count zero. Then
    // the exact third value advances the clock from 3 to 6 milliseconds.
    assert_eq!(evaluator.virtual_ticks, 6);
}

#[test]
fn virtual_clock_wait_propagates_an_operand_error_without_a_default_duration() {
    let mut checked = program("0, 0, 5");
    let wait_call = call(&checked);
    let duration = checked
        .statement_table
        .expression_handles(wait_call.arguments)[2];
    *checked.typed.expression_table.expression_mut(duration) =
        typed_trees::expression::ExpressionNode::Name(typed_trees::expression::TableNamePath {
            members: arena::HandleSpan::empty(),
            member_symbols: arena::HandleSpan::empty(),
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        });
    let mut evaluator = Evaluator::new_checked(&checked, &[]);
    evaluator.virtual_clock_bindings.push(binding(&checked));
    let cell = evaluator
        .allocate_cell(Value::Unit)
        .unwrap_or_else(|_| panic!("frame cell"));
    let mut frame = Frame::bare(cell, SymbolHandle::invalid());
    assert!(evaluator.try_host_call(&wait_call, &mut frame).is_err());
    assert_eq!(evaluator.virtual_ticks, 0);
    assert!(evaluator.non_fs_host_boundary_touched);
}

#[test]
fn virtual_clock_wait_returns_the_exact_integer_result_and_rechecks_its_contract() {
    use crate::VirtualClockResult;
    use typed_trees::types::PrimitiveType;

    let checked = crate::front_end::checked_program(
        "boundary trait Timer { machine mark() reaches Timer; }
         data Delay {}
         boundary requirement Delay::wait(milliseconds: i32) -> i32 reaches Timer;
         machine main() -> i32 reaches Timer { Delay::wait(70) }",
    );
    let requirement = checked
        .realized_machine_named("Delay::wait")
        .expect("wait")
        .symbol;
    let make_binding = |result| {
        VirtualClockBinding::from_compiler_resolved_requirement(
            &checked,
            requirement,
            &[VirtualClockOperand::Milliseconds],
            crate::VirtualClockOperation::Wait {
                maximum_milliseconds: i64::from(i32::MAX),
                result,
            },
        )
    };
    let wait = make_binding(VirtualClockResult::Integer(PrimitiveType::I32))
        .expect("exact integer result");
    for result in [
        VirtualClockResult::Unit,
        VirtualClockResult::Integer(PrimitiveType::U32),
        VirtualClockResult::Integer(PrimitiveType::I64),
        VirtualClockResult::Integer(PrimitiveType::F32),
        VirtualClockResult::Integer(PrimitiveType::Bool),
    ] {
        assert!(make_binding(result).is_err(), "incorrect result {result:?}");
        let mut corrupted = wait.clone();
        corrupted.operation = crate::VirtualClockOperation::Wait {
            maximum_milliseconds: i64::from(i32::MAX),
            result,
        };
        assert!(corrupted.validate_for(&checked).is_err());
    }
    let mut evaluator = Evaluator::new_checked(&checked, &[]);
    evaluator.virtual_clock_bindings.push(wait);
    assert!(matches!(
        evaluator.run_entry("main", None),
        Err(Halt::Exit(0))
    ));
    assert_eq!(evaluator.virtual_ticks, 70);
}

#[test]
fn virtual_clock_wait_does_not_invent_qualified_result_evidence() {
    let checked = crate::front_end::checked_program(
        "domain i32::Positive(value) requires value > 0;
         data Delay {}
         boundary requirement Delay::wait(milliseconds: i32) -> i32 in Positive;
         machine main() {}",
    );
    let requirement = checked
        .realized_machine_named("Delay::wait")
        .expect("wait")
        .symbol;
    assert!(
        VirtualClockBinding::from_compiler_resolved_requirement(
            &checked,
            requirement,
            &[VirtualClockOperand::Milliseconds],
            crate::VirtualClockOperation::Wait {
                maximum_milliseconds: 100,
                result: crate::VirtualClockResult::Integer(typed_trees::types::PrimitiveType::I32),
            },
        )
        .is_err()
    );
}

#[test]
fn virtual_clock_read_preserves_time_and_rejects_wrong_id_or_scaling_overflow() {
    for (clock_id, ticks, succeeds) in [(6, 4, true), (7, 4, false), (6, i64::MAX, false)] {
        let checked = crate::front_end::checked_program(&format!(
            "data Delay {{}}
             boundary requirement Delay::read(clock_id: u32) -> u64;
             machine Main::main() {{ _ = Delay::read({clock_id}); }}"
        ));
        let requirement = checked
            .realized_machine_named("Delay::read")
            .expect("read")
            .symbol;
        let operation = crate::VirtualClockOperation::Read {
            primitive: typed_trees::types::PrimitiveType::U64,
            units_per_millisecond: 1_000_000,
        };
        let binding = VirtualClockBinding::from_compiler_resolved_requirement(
            &checked,
            requirement,
            &[VirtualClockOperand::ExactInteger(6)],
            operation,
        )
        .expect("read binding");
        for invalid_operation in [
            crate::VirtualClockOperation::Read {
                primitive: typed_trees::types::PrimitiveType::U64,
                units_per_millisecond: 0,
            },
            crate::VirtualClockOperation::Read {
                primitive: typed_trees::types::PrimitiveType::U8,
                units_per_millisecond: 1,
            },
        ] {
            assert!(
                VirtualClockBinding::from_compiler_resolved_requirement(
                    &checked,
                    requirement,
                    &[VirtualClockOperand::ExactInteger(6)],
                    invalid_operation,
                )
                .is_err()
            );
        }
        assert!(
            VirtualClockBinding::from_compiler_resolved_requirement(
                &checked,
                requirement,
                &[VirtualClockOperand::Milliseconds],
                operation,
            )
            .is_err()
        );
        let main = checked.realized_machine_named("Main::main").expect("main");
        assert!(
            binding
                .clone()
                .with_compiler_resolved_realization(&checked, main.symbol)
                .is_err()
        );
        let mut evaluator = Evaluator::new_checked(&checked, &[]);
        evaluator.virtual_clock_bindings.push(binding);
        evaluator.virtual_ticks = ticks;
        assert_eq!(evaluator.run_entry("Main::main", None).is_ok(), succeeds);
        assert_eq!(evaluator.virtual_ticks, ticks);
    }
}
