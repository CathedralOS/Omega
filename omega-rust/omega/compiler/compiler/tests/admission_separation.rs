use compiler::{CompileOptions, CompileRequest};

#[test]
fn ordinary_compilation_reports_obligations_without_creating_or_updating_policy() {
    let project = std::env::temp_dir().join(format!(
        "omega-compiler-admission-separation-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project");
    std::fs::write(
        project.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("admission-separation");
    builder.accept_boundary<admitted>();
}
"#,
    )
    .expect("write build declaration");
    std::fs::write(
        project.join("main.omg"),
        r#"boundary machine admitted() ensures true;
data Main {}
machine Main::exercise(&mut self) {}
"#,
    )
    .expect("write source");

    let options = || CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(project.join("build")),
        target_name: None,
    };
    let report = compiler::compile(CompileRequest::new(options()))
        .expect("compilation returns unresolved policy obligations as evidence");
    let settlement = report.trust_admission_settlement();
    assert!(settlement.consumed().is_empty());
    assert_eq!(settlement.unresolved().len(), 1);
    assert_eq!(settlement.required(), settlement.unresolved());
    assert!(!project.join("omega.lock").exists());

    let sentinel = "owner policy bytes that are not a legacy trust lock\n";
    std::fs::write(project.join("omega.lock"), sentinel).expect("seed owner policy");
    let report = compiler::compile(
        CompileRequest::new(options())
            .with_accepted_trust_admissions(settlement.required().to_vec()),
    )
    .expect("explicit in-memory admission set should be consumed");
    assert!(report.trust_admission_settlement().is_exactly_admitted());
    assert_eq!(
        std::fs::read_to_string(project.join("omega.lock")).expect("read sentinel"),
        sentinel,
        "compiler must not inspect, repair, or rewrite policy bytes"
    );

    let _ = std::fs::remove_dir_all(project);
}
