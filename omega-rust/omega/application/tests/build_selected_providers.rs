/*

End-to-end CLI check that a provider a Build selects runs on both engines
(`omega run --both`).

`Build::select_provider<Service, Native>(builder, target)` chooses which
implementation answers a boundary requirement on one target
(`wiki/spec/build/provider_selection.md`). Native application assembly once
refused every such program before execution, even after the selected body was
checked. The settled plan now reaches the boundary settlement with one
provenance row per canonical plan row, so the call runs the selected machine.

The control's entry traps unless `Service::go(3)` returns 4. Its twins keep
the entry and change only the provider: an inline body runs the same way, and
a provider that returns its input unchanged makes both engines trap at the
entry's own check rather than being refused before execution.

*/

use std::process::Command;

const REQUIREMENT: &str = "\
boundary trait Service {
    machine go(value: u32) -> u32 requires value <= 10 ensures result <= 11 reaches none crashes none;
}
";

const ENTRY: &str = "\
machine entry() crashes Trap {
    transition Service::go(3) == 4 { true -> done() _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
";

const BUILD: &str = "\
machine build(builder: &mut Build) {
    Build::application(builder, \"selected_provider\");
    Build::select_provider<Service, Native>(builder, linux_x86_64);
    Build::select_provider<Service, Native>(builder, linux_arm64);
    Build::select_provider<Service, Native>(builder, macos_arm64);
    Build::select_provider<Service, Native>(builder, windows_x86_64);
    Build::bind(builder, linux_x86_64::ProgramEntry, entry);
    Build::bind(builder, linux_arm64::ProgramEntry, entry);
    Build::bind(builder, macos_arm64::ProgramEntry, entry);
    Build::bind(builder, windows_x86_64::ProgramEntry, entry);
}
";

const CONTRACT: &str = "requires value <= 10 ensures result <= 11 reaches none crashes none";

fn referenced(body: &str) -> String {
    format!(
        "{REQUIREMENT}machine provider_go(value: u32) -> u32 {CONTRACT} {{ {body} }}
Native satisfies Service {{
    Service::go = provider_go;
}}
{ENTRY}"
    )
}

fn run_both(name: &str, product: &str) -> String {
    let root = std::env::temp_dir().join(format!(
        "omega-selected-provider-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staged project");
    std::fs::write(root.join("main.omg"), product).expect("staged program");
    std::fs::write(root.join("build.omg"), BUILD).expect("staged Build");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--both", "main.omg"])
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    let _ = std::fs::remove_dir_all(&root);
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_agreed_exit_zero(report: &str) {
    assert!(report.contains("native exit 0"), "{report}");
    assert!(report.contains("interp exit: 0"), "{report}");
}

#[test]
fn a_selected_referenced_provider_runs_on_both_engines() {
    assert_agreed_exit_zero(&run_both("referenced", &referenced("value + 1")));
}

#[test]
fn a_selected_inline_provider_runs_on_both_engines() {
    let inline = format!(
        "{REQUIREMENT}Native satisfies Service {{
    machine go(value: u32) -> u32 {CONTRACT} {{ value + 1 }}
}}
{ENTRY}"
    );
    assert_agreed_exit_zero(&run_both("inline", &inline));
}

#[test]
fn a_selected_provider_with_another_answer_traps_at_the_entry_check() {
    let report = run_both("wrong-answer", &referenced("value"));
    assert!(!report.contains("no product route yet"), "{report}");
    assert!(report.contains("interp crash: Trap"), "{report}");
    assert!(!report.contains("native exit 0"), "{report}");
}
