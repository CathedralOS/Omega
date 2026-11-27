//! Exercise file-qualified declarations in calls and conformance member selections.
#[path = "support/project.rs"]
mod project;
mod support;

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
    let scratch = project::stage("direct-file-imports", &[("main.omg", source)]);
    std::fs::create_dir(scratch.path().join("m")).unwrap();
    for (engine, receipt) in [("native", "native exit 0"), ("psi", "Psi exit 0")] {
        let mut arguments = vec!["run", "--engine", engine, "--build-input", "main.omg"];
        for file in ["build.omg", "m/b.omg", "m/value-cell.omg"] {
            std::fs::copy(fixture.join(file), scratch.path().join(file)).unwrap();
            arguments.extend(["--build-input", file]);
        }
        arguments.push("main.omg");
        let output = project::omega(scratch.path(), &arguments);
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
    let contract = "public trait Step { machine apply(value: u32) -> u32; }";
    let scratch = project::stage(
        "file-selected-requirement",
        &[
            (
                "build.omg",
                "machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: \"rows\" }; }",
            ),
            ("selected.omg", contract),
            ("other.omg", contract),
            (
                "action.omg",
                "public machine keep(value: u32) -> u32 { value }",
            ),
        ],
    );
    let root = scratch.path();
    for (owner, accepted) in [
        ("|renamed|::Step", true),
        ("|other|::Step", false),
        ("Step", false),
    ] {
        let source = format!(
            "use selected.omg; use selected.omg as renamed; use other.omg; use action.omg;\n\
             Whole satisfies |selected|::Step {{ {owner}::apply = |action|::keep; }}"
        );
        project::write(root, &[("main.omg", &source)]);
        let output = project::omega(root, &["--check", "--offline", "main.omg"]);
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
    let scratch = project::stage(
        "file-domain-carriers",
        &[(
            "limits.omg",
            "public domain u16::Small requires self < 10u16;\n\
             public domain u32::Small requires self >= 10u32;",
        )],
    );
    let root = scratch.path();
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
        project::write(root, &[("main.omg", &source)]);
        let output = project::omega(root, &["--check", "--offline", "main.omg"]);
        let diagnostics = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.success(), error.is_none(), "{diagnostics}");
        if let Some(error) = error {
            assert!(diagnostics.contains(error), "{diagnostics}");
        }
    }
}
