use omega_compiler::{compile_to_checked, selected_external_root_provider_plan_id};

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
    let [plan] = facts.plans() else {
        panic!("exactly one covering Pair plan should be selected");
    };
    assert_eq!(plan.name, "satisfies::Pair");
    assert_eq!(plan.rows.len(), 2);
    assert!(plan.covers_schema());
    assert_eq!(
        facts
            .plan_by_identity(plan.identity_fingerprint())
            .map(|selected| selected.name.as_str()),
        Some("satisfies::Pair")
    );
    let root_plan = selected_external_root_provider_plan_id(facts, "Pair")
        .expect("external-root bridge must consume the retained Pair selection");
    assert_eq!(root_plan.normalized_identity(), plan.identity_fingerprint());
    assert!(
        selected_external_root_provider_plan_id(facts, "Missing")
            .expect_err("an unselected root slot must fail closed")
            .0
            .contains("no retained selected provider plan")
    );

    let _ = std::fs::remove_dir_all(&project);
}
