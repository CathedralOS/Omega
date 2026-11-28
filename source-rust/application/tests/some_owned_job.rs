//! The provider supplies an established resident. The stored operation accepts
//! the complete owner by value and returns it in a new closed package.
#[path = "support/program.rs"]
mod program;

const SOURCE: &str = include_str!("fixtures/callables/run/some_owned_job/main.omg");

#[test]
fn a_stored_operation_consumes_an_owner_and_repackages_its_custody() {
    let (output, report) = program::invoke(SOURCE, &["psi", "inspect", "--machine", "complete"]);
    assert!(output.status.success(), "{report}");
    assert!(report.contains("psi selected_machine=complete"), "{report}");
}

#[test]
fn the_copying_operation_refuses_an_owning_package() {
    let command = ["psi", "inspect", "--machine", "collect"];
    let (output, report) = program::invoke(SOURCE, &command);
    assert!(output.status.success(), "{report}");
    let (line, original) = SOURCE
        .lines()
        .enumerate()
        .find(|(_, line)| line.starts_with("machine collect("))
        .unwrap();
    let copied = SOURCE.replace(original, &original.replace("u8", "Job"));
    let (output, report) = program::invoke(&copied, &command);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains(
            "type argument `Job` for `T` has `[linear]` but the binder requires `[copy]`"
        ),
        "{report}"
    );
    assert!(
        report.contains(&format!("main.omg:{}:", line + 1)),
        "{report}"
    );
}
