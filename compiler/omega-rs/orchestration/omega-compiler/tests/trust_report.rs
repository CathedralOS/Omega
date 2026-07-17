//! GR5 (chapter-10 carrier): the trust report writes one row per admitted
//! semantic commitment. Today's rows are sealed-domain introductions --
//! own-package declarations are dev-active (grant locality v1) and carry
//! the standing warning until a root grant (GR3) flips their provenance.

use omega_compiler::{CompileOptions, compile};

#[test]
fn trust_report_rows_dev_active_domain_introductions() {
    let project = std::env::temp_dir().join(format!("omega-trust-report-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"domain u32::Meters {}
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::main(&mut self) {
    let d: u32 in Meters = (7 as u32 in Meters);
    self.console.exit_process(70);
}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("domain program should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    assert!(
        report.contains("admitted commitments: 1"),
        "expected one commitment row:\n{report}"
    );
    assert!(
        report.contains("domain introduction: u32::Meters -- own-package (dev-active)"),
        "expected the dev-active domain row:\n{report}"
    );
    assert!(
        report.contains("STANDING WARNING"),
        "dev-active rows carry the standing warning until granted:\n{report}"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn trust_report_empty_without_commitments() {
    let project =
        std::env::temp_dir().join(format!("omega-trust-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("main.omg"),
        r#"boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::main(&mut self) {
    self.console.exit_process(70);
}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("plain program should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("the empty report is still written -- the honest no-commitments statement");
    assert!(
        report.contains("admitted commitments: 0"),
        "expected zero rows:\n{report}"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn root_grant_flips_domain_row_and_retires_warning() {
    // GR3: `b.accept_boundary<Meters>();` in build.omg harvests as a root
    // grant; the granted domain's trust row flips provenance and drops the
    // standing warning. A grant naming no declared domain surfaces as an
    // accepted-fact row (the report sees every grant).
    let project = std::env::temp_dir().join(format!("omega-trust-grant-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project dir");
    std::fs::write(
        project.join("build.omg"),
        r#"data Subsystem { case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }
data Build { subsystem: Subsystem; freestanding: bool; }

machine build(b: &mut Build) {
    b.accept_boundary<Meters>();
    b.accept_boundary<walker_lib::collatz_cert_checked>();
}
"#,
    )
    .expect("write build.omg");
    std::fs::write(
        project.join("main.omg"),
        r#"domain u32::Meters {}
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::main(&mut self) {
    let d: u32 in Meters = (7 as u32 in Meters);
    self.console.exit_process(70);
}
"#,
    )
    .expect("write main.omg");

    let build_dir = project.join("build");
    compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("granted project should compile");

    let report = std::fs::read_to_string(build_dir.join("trust_report.md"))
        .expect("trust report should be written");
    assert!(
        report.contains("domain introduction: u32::Meters -- root grant (build.omg)"),
        "expected the granted domain row:\n{report}"
    );
    assert!(
        report.contains("accepted fact: walker_lib::collatz_cert_checked -- root grant (build.omg)"),
        "expected the accepted-fact row:\n{report}"
    );
    let meters_row = report
        .lines()
        .find(|line| line.contains("u32::Meters"))
        .unwrap_or_default();
    assert!(
        !meters_row.contains("STANDING WARNING"),
        "a root-granted domain drops the standing warning:\n{report}"
    );

    let _ = std::fs::remove_dir_all(&project);
}
