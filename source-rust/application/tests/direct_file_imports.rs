//! Exercise file-qualified declarations in calls and conformance member selections.
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};

#[test]
fn invoke_a_returned_machine_in_a_transition_arm() {
    run_example(include_str!(
        "../../../samples/cli/probes/direct_file_imports/main.omg"
    ));
}

#[test]
fn the_file_type_and_value_each_select_their_own_member() {
    run_example(
        r#"
use m/value-cell.omg as cell;
use m/b.omg;

data Reader { read: i32; }
data cell { case Stored; case Supplied; }

machine read(cell: Reader, item: |b|::B, kind: cell) -> i32 {
    transition kind {
        cell::Stored -> |cell|::read(item.value),
        cell::Supplied -> cell.read
    }
}

machine read_case(kind: cell) -> i32 {
    let item: |b|::B = |b|::B { value: |cell|::A { value: 7 } };
    let local: Reader = Reader { read: 9 };
    read(local, item, kind)
}

machine main() crashes Trap {
    transition read_case(cell::Stored) == 7 && read_case(cell::Supplied) == 9 {
        true -> done(), _ -> bad()
    }
    machine done() {}
    machine bad() { crash Trap; }
}
"#,
    );
}

fn run_example(source: &str) {
    let fixture = support::repository_root().join("samples/cli/probes/direct_file_imports");
    let scratch = TemporaryDirectory::create("direct-file-imports", false).unwrap();
    std::fs::create_dir(scratch.path().join("m")).unwrap();
    for (engine, receipt) in [("native", "native exit 0"), ("psi", "Psi exit 0")] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
        command.args(["run", "--engine", engine]);
        std::fs::write(scratch.path().join("main.omg"), source).unwrap();
        command.args(["--build-input", "main.omg"]);
        for file in ["build.omg", "m/b.omg", "m/value-cell.omg"] {
            std::fs::copy(fixture.join(file), scratch.path().join(file)).unwrap();
            command.args(["--build-input", file]);
        }
        let output = command
            .arg("main.omg")
            .current_dir(scratch.path())
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "{report}");
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn a_selection_row_names_the_exact_file_that_declares_its_requirement() {
    let scratch = TemporaryDirectory::create("file-selected-requirement", false).unwrap();
    let root = scratch.path();
    let contract = "public trait Step { machine apply(value: u32) -> u32; }";
    for (path, source) in [
        (
            "build.omg",
            "machine build(builder: @mutable Build) { Build::package(builder, \"rows\"); }",
        ),
        ("selected.omg", contract),
        ("other.omg", contract),
        (
            "action.omg",
            "public machine keep(value: u32) -> u32 { value }",
        ),
    ] {
        std::fs::write(root.join(path), source).unwrap();
    }
    for (owner, accepted) in [
        ("|renamed|::Step", true),
        ("|other|::Step", false),
        ("Step", false),
    ] {
        let source = format!(
            "use selected.omg; use selected.omg as renamed; use other.omg; use action.omg;\n\
             Whole satisfies |selected|::Step {{ {owner}::apply = |action|::keep; }}"
        );
        std::fs::write(root.join("main.omg"), source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(["--check", "--offline", "main.omg"])
            .current_dir(root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let report = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.success(), accepted, "{report}");
        if !accepted {
            assert!(
                report.contains(&format!("no inherited requirement slot `{owner}::apply`")),
                "{report}"
            );
        }
    }
}

#[test]
fn imported_domains_with_the_same_name_select_the_subjects_carrier() {
    let scratch = TemporaryDirectory::create("file-domain-carriers", false).unwrap();
    let root = scratch.path();
    std::fs::write(
        root.join("limits.omg"),
        "public domain u16::Small requires self < 10u16;\n\
         public domain u32::Small requires self >= 10u32;",
    )
    .unwrap();
    for (carrier, value, error) in [
        ("u16", 3, None),
        ("u32", 30, None),
        ("u8", 3, Some("does not qualify carrier `u8`")),
        ("u32", 3, Some("domain `u32::Small` requires `self >= 10`")),
    ] {
        let source = format!(
            "use limits.omg;\n\
             machine small(value: {carrier} in |limits|::Small) -> bool {{ value in |limits|::Small }}\n\
             public machine main() {{ _ = small({value}{carrier} as + |limits|::Small); }}"
        );
        std::fs::write(root.join("main.omg"), source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(["--check", "--offline", "main.omg"])
            .current_dir(root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let diagnostics = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.success(), error.is_none(), "{diagnostics}");
        if let Some(error) = error {
            assert!(diagnostics.contains(error), "{diagnostics}");
        }
    }
}
