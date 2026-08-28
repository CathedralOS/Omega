use omega_compiler::compile_to_checked;
use psi_language_semantics::CallOperationalAcknowledgementOrigin;
use std::fs;
use std::path::PathBuf;

fn write_program(name: &str, source: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "omega-call-acknowledgements-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create call acknowledgement test directory");
    let main_path = directory.join("main.omg");
    fs::write(&main_path, source).expect("write call acknowledgement test program");
    main_path
}

fn compile_error(name: &str, source: &str) -> String {
    let main_path = write_program(name, source);
    let diagnostics = compile_to_checked(&main_path, None)
        .expect_err("call acknowledgement violation must reject compilation");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary program directory"));
    rendered
}

const FOUR_ENVELOPES: &str = r#"
boundary trait Immediate { machine call(); }
boundary trait Parking { machine call() suspends; }
boundary trait Waiting { machine call() blocks; }
boundary trait Combined { machine call() suspends; blocks; }

data Main { }
machine run_immediate(immediate: &mut Immediate) reaches Immediate {
    immediate.call();
}
machine run_parking(suspend_source: &mut Parking) reaches Parking suspends; {
    suspend suspend_source.call();
}
machine run_waiting(block_source: &mut Waiting) reaches Waiting blocks; {
    block block_source.call();
}
machine run_combined(both_source: &mut Combined)
reaches Combined
suspends;
blocks;
{
    suspend block both_source.call();
}
machine Main::main(&mut self) { }
"#;

#[test]
fn exact_acknowledgements_cover_all_four_operational_envelopes() {
    let main_path = write_program("four-envelopes", FOUR_ENVELOPES);
    let checked = compile_to_checked(&main_path, None)
        .expect("all four exact acknowledgement combinations should compile");
    let calls = checked.facts.flow.control.calls.iter().collect::<Vec<_>>();
    assert_eq!(calls.len(), 4);
    assert_eq!(
        calls
            .iter()
            .map(|(_, call)| (
                call.operational_acknowledgement.acknowledges_suspend,
                call.operational_acknowledgement.acknowledges_block,
            ))
            .collect::<Vec<_>>(),
        vec![(false, false), (true, false), (false, true), (true, true)]
    );
    assert_eq!(
        calls
            .iter()
            .map(|(_, call)| (
                call.suspension.transitive_may_suspend,
                call.blocking.transitive_may_block,
            ))
            .collect::<Vec<_>>(),
        vec![(false, false), (true, false), (false, true), (true, true)]
    );
    assert!(calls.iter().all(|(_, call)| {
        call.operational_acknowledgement.origin == CallOperationalAcknowledgementOrigin::Source
    }));
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary program directory"));
}

#[test]
fn missing_partial_and_redundant_acknowledgements_reject() {
    for (name, call, expected) in [
        ("missing", "suspend_source.call();", "acknowledges neither"),
        (
            "partial",
            "suspend both_source.call();",
            "operational envelope `suspend block`",
        ),
        (
            "redundant",
            "suspend immediate.call();",
            "operational envelope neither",
        ),
    ] {
        let source = match name {
            "missing" => FOUR_ENVELOPES.replace("suspend suspend_source.call();", call),
            "partial" => FOUR_ENVELOPES.replace("suspend block both_source.call();", call),
            "redundant" => FOUR_ENVELOPES.replace("immediate.call();", call),
            _ => unreachable!(),
        };
        let error = compile_error(name, &source);
        assert!(error.contains(expected), "{name}: {error}");
    }
}

#[test]
fn suspension_rejects_nested_position_while_blocking_may_nest() {
    let nested_suspend = FOUR_ENVELOPES
        .replace(
            "boundary trait Immediate { machine call(); }",
            "boundary trait Immediate { machine call(); }\nboundary trait Value { machine get() -> u64 suspends; }",
        )
        .replace(
            "machine run_parking(suspend_source: &mut Parking) reaches Parking suspends; {\n    suspend suspend_source.call();\n}",
            "machine nested(value_source: &mut Value) -> u64 reaches Value suspends; {\n    let value: u64 = 1 + suspend value_source.get();\n    value\n}",
        );
    let error = compile_error("nested-suspend", &nested_suspend);
    assert!(
        error.contains("nested inside a partially evaluated expression"),
        "{error}"
    );

    let nested_block = FOUR_ENVELOPES
        .replace(
            "boundary trait Immediate { machine call(); }",
            "boundary trait Immediate { machine call(); }\nboundary trait Value { machine get() -> u64 blocks; }",
        )
        .replace(
            "machine run_waiting(block_source: &mut Waiting) reaches Waiting blocks; {\n    block block_source.call();\n}",
            "machine nested(value_source: &mut Value) -> u64 reaches Value blocks; {\n    let value: u64 = 1 + block value_source.get();\n    value\n}",
        );
    let main_path = write_program("nested-block", &nested_block);
    compile_to_checked(&main_path, None).expect("blocking-only call may nest");
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary program directory"));
}

#[test]
fn suspension_accepts_each_direct_continuation_position() {
    let source = r#"
boundary trait Event { machine park() suspends; }
boundary trait Value { machine get() -> u64 suspends; }
boundary trait Scheduler { machine wait() -> u64 suspends; }

data Subject { scheduler: Scheduler; }
data Main { }
machine Subject::wait(&mut self) -> u64 reaches Scheduler suspends; {
    suspend self.scheduler.wait()
}
machine Subject::subject(&mut self) reaches Scheduler suspends; {
    transition suspend self.wait() {
        0 -> { }
        _ -> { }
    }
}
machine statement(event: &mut Event) reaches Event suspends; {
    suspend event.park();
}
machine local(value_source: &mut Value) -> u64 reaches Value suspends; {
    let value: u64 = suspend value_source.get();
    value
}
machine terminal(value_source: &mut Value) -> u64 reaches Value suspends; {
    suspend value_source.get()
}
machine Main::main(&mut self) { }
"#;
    let main_path = write_program("direct-positions", source);
    compile_to_checked(&main_path, None).expect("all direct suspension positions should compile");
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary program directory"));
}

#[test]
fn local_checked_summary_can_narrow_an_authored_operational_ceiling() {
    let source = r#"
data Worker { }
data Main { }
machine Worker::locally_narrow(&mut self) suspends; blocks; { }
machine caller(worker: &mut Worker) {
    worker.locally_narrow();
}
machine Main::main(&mut self) { }
"#;
    let main_path = write_program("local-checked-narrowing", source);
    let checked = compile_to_checked(&main_path, None)
        .expect("a locally checked empty body should narrow its authored operational ceiling");
    let calls = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .map(|(_, call)| call)
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let call = calls[0];
    assert!(!call.suspension.transitive_may_suspend);
    assert!(!call.blocking.transitive_may_block);
    assert!(!call.operational_acknowledgement.acknowledges_suspend);
    assert!(!call.operational_acknowledgement.acknowledges_block);
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary program directory"));
}

#[test]
fn reversed_combined_order_rejects_during_parsing() {
    let error = compile_error(
        "reversed",
        &FOUR_ENVELOPES.replace(
            "suspend block both_source.call();",
            "block suspend both_source.call();",
        ),
    );
    assert!(error.contains("canonical order `suspend block`"), "{error}");
}

#[test]
fn task_start_acknowledges_only_the_start_operation_not_the_target_machine() {
    let canary = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../../../../../tests/omega/pass/tasks/task_runtime_machine_selection_compile/main.omg",
    );
    let checked = compile_to_checked(&canary, None)
        .expect("task-start canary should compile with unmarked immediate start calls");
    let start_symbols = checked
        .task_activations()
        .as_slice()
        .iter()
        .map(|activation| activation.start_requirement)
        .collect::<Vec<_>>();
    assert_eq!(start_symbols.len(), 2);
    let start_calls = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .filter(|(_, call)| start_symbols.contains(&call.target_symbol))
        .collect::<Vec<_>>();
    assert_eq!(start_calls.len(), 2);
    assert!(start_calls.iter().all(|(_, call)| {
        !call.suspension.transitive_may_suspend
            && !call.blocking.transitive_may_block
            && !call.operational_acknowledgement.acknowledges_suspend
            && !call.operational_acknowledgement.acknowledges_block
    }));

    let worker = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Worker::run")
        .expect("worker target");
    let worker_suspension = checked
        .facts
        .suspensions
        .for_machine(worker.symbol)
        .expect("worker suspension plan");
    assert!(worker_suspension.checked_may_suspend);
}

#[test]
fn compiler_synthesized_calls_record_acknowledgements_without_source_tokens() {
    let canary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../../tests/omega/pass/traits/equatable_record_equality_exit/main.omg");
    let checked =
        compile_to_checked(&canary, None).expect("synthesized equality-call canary should compile");
    let synthesized = checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .filter(|(_, call)| {
            call.operational_acknowledgement.origin
                == CallOperationalAcknowledgementOrigin::CompilerSynthesized
        })
        .collect::<Vec<_>>();
    assert!(
        !synthesized.is_empty(),
        "the authored equality override should be reached through a compiler-synthesized call"
    );
    assert!(synthesized.iter().all(|(_, call)| {
        call.operational_acknowledgement.acknowledges_suspend
            == call.suspension.transitive_may_suspend
            && call.operational_acknowledgement.acknowledges_block
                == call.blocking.transitive_may_block
    }));
}
