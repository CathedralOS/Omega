use omega_compiler::compile_to_checked;

#[test]
fn checked_program_retains_the_exact_selected_provider_plan() {
    let project = std::env::temp_dir().join(format!(
        "omega-selected-provider-facts-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create test project");
    let source = project.join("main.omg");
    std::fs::write(
        &source,
        r#"boundary trait Pair {
    machine first(code: i32) -> i32;
    machine second(code: i32) -> i32;
}

machine first_leaf(code: i32) -> i32
    satisfies Pair::first via Binding::VtableSlot(1);
machine second_leaf(code: i32) -> i32
    satisfies Pair::second via Binding::VtableSlot(2);

data Main { }
machine Main::main(&mut self) { }
"#,
    )
    .expect("write test program");

    let checked = compile_to_checked(&source, None).expect("provider program should check");
    let facts = checked.selected_provider_plans();
    assert_eq!(facts.plans().len(), 1);
    let plan = facts
        .plan_by_name("satisfies::Pair")
        .expect("the unique covering Pair plan should be selected");
    assert_eq!(plan.rows.len(), 2);
    assert!(plan.covers_schema());
    assert_eq!(
        facts
            .plan_by_identity(plan.identity_fingerprint())
            .map(|selected| selected.name.as_str()),
        Some("satisfies::Pair")
    );

    let _ = std::fs::remove_dir_all(&project);
}
