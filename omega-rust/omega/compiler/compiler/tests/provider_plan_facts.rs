use compiler::CheckedCompileRequest;
use compiler::compile_to_checked;
use provider_planning::plans::selected_external_root_provider_plan_id;

#[test]
fn checked_progress_entry_retains_selected_syscall_binding_without_table_projection() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../../tests/omega/pass/progress/provider_receiver_progress_installation/main.omg",
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&source, Some("linux_x64")))
        .expect("selected progress-bearing source entry should check");
    assert_eq!(checked.selected_program_entry_machine(), Some("Main::main"));
    let [plan] = checked.selected_provider_plans().plans() else {
        panic!("one selected Scheduler provider");
    };
    let syscall = plan
        .rows
        .iter()
        .find(|row| {
            matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::Syscall { .. }
            )
        })
        .expect("selected syscall row");
    assert!(plan.rows.iter().any(|row| matches!(
        row.binding,
        effects::provider_plan::ProviderBinding::VtableField { .. }
    )));

    // Native realization needs the exact selected syscall, but the grant's
    // table route has its own consumer and must not demand a host calling plan.
    let [external] = checked.external_binding_rows() else {
        panic!("only the selected syscall crosses the native external handoff");
    };
    assert_eq!(external.requirement_identity, syscall.requirement_identity);
    assert_eq!(external.trait_name, plan.schema.trait_name);
    assert_eq!(external.table_type, plan.provider_type);
    assert_eq!(external.method, syscall.method);
    assert_eq!(external.target_name, "linux_x86_64");
    assert_eq!(
        external.binding,
        calling_conventions::ExternalBindingKind::Syscall { number: 1 }
    );
    let entry = external.boundary_entry_plan.as_ref().expect("syscall ABI");
    let [parameter] = entry.call.parameters.as_slice() else {
        panic!("only the output value crosses the ABI after fused receiver erasure");
    };
    assert_eq!(
        parameter.shape,
        calling_conventions::ValueShape::integer(8, 8)
    );
    assert!(entry.call.result.is_none());

    // Projection is not provider installation: the original progress demand
    // remains pending and bound to this same selected requirement and plan.
    let manifest = checked.component_progress().expect("build-bound progress");
    let [demand] = manifest.pending() else {
        panic!("one unresolved installation premise");
    };
    assert_eq!(demand.requirement_identity, external.requirement_identity);
    assert_eq!(
        demand.provider_plan_report_identity,
        plan.report_fingerprint()
    );
    assert_eq!(demand.profile_identity, "Scheduler::WeakFair");
    assert_eq!(demand.establishment_routes.len(), 1);
}

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
    satisfies Pair::first via Binding::DllImport("omega-test", "pair_first");
machine second_leaf(code: i32) -> i32
    satisfies Pair::second via Binding::DllImport("omega-test", "pair_second");

data Main { }
machine Main::main(&mut self) { }
"#,
    )
    .expect("write test program");

    let checked = compile_to_checked(CheckedCompileRequest::new(&source, None))
        .expect("provider program should check");
    let facts = checked.selected_provider_plans();
    let [plan] = facts.plans() else {
        panic!("exactly one covering Pair plan should be selected");
    };
    assert_eq!(plan.name, "satisfies::Pair");
    assert_eq!(plan.rows.len(), 2);
    assert!(plan.covers_schema());
    assert_eq!(
        facts
            .plan_by_report_fingerprint(plan.report_fingerprint())
            .map(|selected| selected.name.as_str()),
        Some("satisfies::Pair")
    );
    let root_plan = selected_external_root_provider_plan_id(facts, "Pair")
        .expect("external-root bridge must consume the retained Pair selection");
    assert_eq!(root_plan.normalized_identity(), plan.report_fingerprint());
    assert!(
        selected_external_root_provider_plan_id(facts, "Missing")
            .expect_err("an unselected root slot must fail closed")
            .0
            .contains("no retained selected provider plan")
    );

    let _ = std::fs::remove_dir_all(&project);
}
