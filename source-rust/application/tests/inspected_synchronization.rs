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
