//! An inspected supplier can require an incoming cell to become ready.
//! The runnable consumer has to establish progress for its actual cell.

#[path = "support/program.rs"]
mod program;

const SOURCE: &str = r#"
public data Flag { word: AtomicU32; }

machine await_release(flag: @Flag) {
    let word: u32 = flag.word.load(Receive);
    transition word == 1 {
        true -> await_release(flag),
        _ -> ready()
    }
    machine ready() {}
}

public machine supplied(flag: @Flag) { await_release(flag); }
machine main() {
    let mutable flag: Flag;
    supplied(@flag);
}
"#;

#[test]
fn an_independent_supplier_leaves_the_incoming_cell_to_its_consumer() {
    let (inspect, report) = program::invoke(SOURCE, &["psi", "inspect", "--machine", "supplied"]);
    assert!(inspect.status.success(), "{report}");
    program::assert_silent_return(SOURCE);

    let held = SOURCE.replace(
        "supplied(@flag);",
        "flag.word.store(1, Publish); supplied(@flag);",
    );
    let (check, report) = program::invoke(&held, &["--check"]);
    assert!(!check.status.success());
    assert!(report.contains("synchronization deadlock"), "{report}");
}

#[test]
fn inspecting_an_external_wait_keeps_checking_the_continuation() {
    let source = SOURCE.replace(
        "await_release(flag); }",
        "await_release(flag); let mutable held: Flag; held.word.store(1, Publish); await_release(@held); }",
    );
    let (inspect, report) = program::invoke(&source, &["psi", "inspect", "--machine", "supplied"]);
    assert!(!inspect.status.success());
    assert!(report.contains("synchronization deadlock"), "{report}");
}

#[test]
fn an_open_supplier_must_still_discharge_its_private_alternative() {
    let source = SOURCE
        .replace(
            "public machine supplied(flag: @Flag) { await_release(flag); }",
            "data Choice<'s> { flag: @'s Flag; }
         public machine supplied(flag: @Flag, first: bool) {
             let mutable local: Flag;
             local.word.store(0, Publish);
             let chosen: Choice = match first {
                 true -> Choice { flag: flag }, false -> Choice { flag: @local }
             };
             await_release(chosen.flag);
         }",
        )
        .replace("supplied(@flag);", "supplied(@flag, false);");
    let inspect = |source| program::invoke(source, &["psi", "inspect", "--machine", "supplied"]);
    let (released, report) = inspect(&source);
    assert!(released.status.success(), "{report}");
    let held = source.replace("local.word.store(0", "local.word.store(1");
    let (blocked, report) = inspect(&held);
    assert!(!blocked.status.success(), "{report}");
    assert!(report.contains("synchronization"), "{report}");
}

#[test]
fn an_exclusive_input_can_defer_only_bytes_its_supplier_has_preserved() {
    let untouched = SOURCE
        .replace("word: AtomicU32;", "word: AtomicU32; sibling: AtomicU32;")
        .replace(
            "supplied(flag: @Flag) { await_release(flag); }",
            "supplied(flag: @mutable Flag, input: u32) {
                flag.sibling.store(input +% 1u32, Publish); await_release(flag);
            }",
        )
        .replace("supplied(@flag);", "supplied(@mutable flag, 1);");
    let inspect = |source| program::invoke(source, &["psi", "inspect", "--machine", "supplied"]);
    let (accepted, report) = inspect(&untouched);
    assert!(accepted.status.success(), "{report}");
    let replaced = untouched.replace("flag.sibling.store", "flag.word.store");
    let (rejected, report) = inspect(&replaced);
    assert!(!rejected.status.success(), "{report}");
    assert!(
        report.contains("cannot prove synchronization progress"),
        "{report}"
    );
}

#[test]
fn authorizing_an_inspected_artifact_still_checks_its_raw_execution_entry() {
    use std::sync::Arc;
    use terminal_interpreter::{
        TerminalArtifactInterpretError, TerminalExecution, TerminalStructuralInputs,
        TerminalStructuralValue,
    };

    let scratch = platform_custody::temporary_directory::TemporaryDirectory::create(
        "omega-raw-wait-entry",
        false,
    )
    .unwrap();
    let output = scratch.path().join("supplier.psi");
    let (inspect, report) = program::invoke(
        SOURCE,
        &[
            "psi",
            "inspect",
            "--machine",
            "supplied",
            "--output",
            output.to_str().unwrap(),
        ],
    );
    assert!(inspect.status.success(), "{report}");
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&std::fs::read(output).unwrap())
            .unwrap();
    let Some(terminal_psi::TargetMachineCallable::Machine(entry)) =
        artifact.semantic_module().entry
    else {
        panic!("expected supplied as the artifact entry")
    };
    let signal = artifact
        .semantic_module()
        .machine(entry)
        .unwrap()
        .structural_parameters[0]
        .structural_type;
    let authorized = psi_authorization::authorize_terminal_artifact(
        Arc::new(artifact),
        Some(&psi_authorization::HostProducerTrust::accept(
            "test compiler",
        )),
    )
    .unwrap();
    let signal = TerminalStructuralValue::minted(1, signal, Vec::new());
    let inputs = TerminalStructuralInputs {
        arguments: std::slice::from_ref(&signal),
        zero_initialized_arguments: &[0],
        ..Default::default()
    };
    match TerminalExecution::start_artifact(&authorized, &[], inputs) {
        Err(TerminalArtifactInterpretError::Synchronization(failure)) => {
            assert_eq!(
                failure.cause,
                terminal_semantics::synchronization::ProgressFailure::UnprovedExternal
            );
        }
        Err(error) => panic!("raw entry lost its synchronization diagnosis: {error}"),
        Ok(_) => panic!("borrowed wait reached raw execution without a discharge"),
    }
}
