//! Executed selection, receiver authority, and product declaration scope.

use super::{TempProject, application_build};
use compiler::{CheckedCompileRequest, compile_to_checked};

fn project(build: &str) -> TempProject {
    let project = TempProject::with_main("use contract; use left; use right;", build);
    std::fs::write(
        project.0.join("contract.omg"),
        "module contract; pub boundary trait Reader { machine read() -> i32; }",
    )
    .unwrap();
    for (module, result) in [("left", 11), ("right", 37)] {
        std::fs::write(project.0.join(format!("{module}.omg")), format!(
            "module {module}; use contract::Reader; pub data Provider {{}} machine Provider::read() -> i32 satisfies Reader::read {{ {result} }}"
        )).unwrap();
    }
    project
}

fn check(build: &str) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    let project = project(build);
    compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("macos_arm64"),
    ))
}

fn assert_selected(checked: &compiler::CheckedCompilation, expected: &str) {
    let selected = checked
        .selected_provider_provenance()
        .iter()
        .filter(|plan| plan.plan.schema.trait_name == "contract::Reader")
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].plan.provider_type, expected);
    assert!(matches!(&selected[0].selected_by,
        provider_planning::ProviderSelectionProvenance::BuildOverride(rows) if rows.len() == 1));
}

#[test]
fn reached_helper_selects_and_unreached_helper_does_not() {
    let build = format!("{}\n
        machine choose(builder: &mut Build) {{ builder.select_provider<contract::Reader, left::Provider>(); }}
        machine unused(builder: &mut Build) {{ builder.select_provider<contract::Reader, right::Provider>(); }}",
        application_build("choose(builder);"));
    let checked = check(&build).expect("executed helper selection");
    assert_selected(&checked, "left::Provider");
    let selected = checked
        .selected_provider_provenance()
        .iter()
        .find(|plan| plan.plan.schema.trait_name == "contract::Reader")
        .unwrap();
    let provider_planning::ProviderSelectionProvenance::BuildOverride(rows) = &selected.selected_by
    else {
        unreachable!()
    };
    let choosing_machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == rows[0].selecting_machine)
        .unwrap();
    assert_eq!(choosing_machine.name.as_str(), "choose");
    assert!(
        build_evaluation::validate_executed_provider_selection_declaration(
            &checked.typed,
            &rows[0]
        )
    );
    let mut wrong_owner = rows[0].clone();
    wrong_owner.selecting_machine = checked.selected_build_machine_symbol().unwrap();
    assert!(
        !build_evaluation::validate_executed_provider_selection_declaration(
            &checked.typed,
            &wrong_owner
        )
    );
}

#[test]
fn expression_position_is_an_executed_selection() {
    let checked = check(&application_build(
        "(builder.select_provider<contract::Reader, right::Provider>());",
    ))
    .expect("selection used as an expression");
    assert_selected(&checked, "right::Provider");
}

#[test]
fn returned_build_reborrow_keeps_selection_authority() {
    let build = format!(
        "{}\n machine retain(builder: &mut Build) -> &mut Build {{ transition {{ _ -> (builder) }} }}",
        application_build("retain(builder).select_provider<contract::Reader, left::Provider>();")
    );
    let checked = check(&build).expect("returned original Build reborrow");
    assert_selected(&checked, "left::Provider");
}

#[test]
fn unexecuted_conflicting_selection_does_not_override() {
    let checked = check(&application_build(
        "builder.select_provider<contract::Reader, left::Provider>();
         transition false { true -> skipped(builder) false -> done() }
         state skipped(builder: &mut Build) { builder.select_provider<contract::Reader, right::Provider>(); }
         state done() {}",
    ))
    .expect("only reached selection applies");
    assert_selected(&checked, "left::Provider");
}

#[test]
fn reached_conflicting_selections_reject() {
    let errors = check(&application_build(
        "builder.select_provider<contract::Reader, left::Provider>();
         builder.select_provider<contract::Reader, right::Provider>();",
    ))
    .expect_err("one slot cannot have two selected providers");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("two provider types")),
        "{errors:?}"
    );
}

#[test]
fn fabricated_build_has_no_selection_authority() {
    let errors = check(&application_build(
        "let mut fabricated: Build = Build {};
         (&mut fabricated).select_provider<contract::Reader, left::Provider>();",
    ))
    .expect_err("new records do not inherit Build authority");
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("source cannot construct the compiler-owned Build activation")),
        "{errors:?}"
    );
}

#[test]
fn resolved_lookalike_does_not_receive_product_scope() {
    let build = format!(
        "{}\n data Fake {{}} machine Fake::select_provider<T [copy], U [copy]>(&mut self) {{}}",
        application_build(
            "let mut fake: Fake = Fake {}; fake.select_provider<contract::Reader, left::Provider>();"
        )
    );
    let errors = check(&build).expect_err("ordinary generics have no product lookup");
    assert!(!errors.is_empty());
}

#[test]
fn resolved_lookalike_executes_as_an_ordinary_generic_method() {
    let build = format!(
        "{}\n data Fake {{}} machine Fake::select_provider<T [copy], U [copy]>(&mut self) {{}}",
        application_build(
            "let mut fake: Fake = Fake {}; fake.select_provider<u8, u8>();
             builder.select_provider<contract::Reader, left::Provider>();"
        )
    );
    let checked = check(&build).expect("ordinary same-named method remains callable");
    assert_selected(&checked, "left::Provider");
}

#[test]
fn computed_composition_mode_is_evaluated() {
    let checked = check(&application_build(
        "let mode: CompositionMode = CompositionMode::Fused;
         builder.select_provider<contract::Reader, left::Provider>(mode);",
    ))
    .expect("mode is an ordinary evaluated value");
    assert_selected(&checked, "left::Provider");
}

fn provider_dependency() -> TempProject {
    let dependency = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"provider-library\"); }",
    );
    std::fs::write(
        dependency.0.join("setup.omg"),
        "module setup;
         pub boundary trait Reader { machine read() -> i32; }
         pub data Provider {} pub data Other {}
         machine Provider::read() -> i32 satisfies Reader::read { 11 }
         machine Other::read() -> i32 satisfies Reader::read { 37 }",
    )
    .unwrap();
    dependency
}

#[test]
fn product_dependency_selection_retains_its_exact_package() {
    let dependency = provider_dependency();
    let project = TempProject::with_main(
        "use support::setup;",
        &application_build(
            "builder.select_provider<support::setup::Reader, support::setup::Provider>();",
        ),
    );
    let inputs = super::foreign_product_inputs(&project, &dependency);
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&project.main(), Some("macos_arm64"))
    })
    .expect("static product operands do not require a host import");
    let plan = checked
        .selected_provider_provenance()
        .iter()
        .find(|plan| plan.plan.schema.trait_name == "setup::Reader")
        .unwrap();
    assert_eq!(plan.plan.provider_type, "setup::Provider");
    let provider = plan.provider.provider_type.unwrap();
    assert_eq!(
        checked
            .typed
            .symbols
            .symbol_product_package_identity(provider),
        Some(super::fixture_package_identity(2))
    );
}

#[test]
fn shared_alias_selects_product_instance_not_build_instance() {
    let host = provider_dependency();
    let product = provider_dependency();
    let project = TempProject::with_main(
        "use support::setup;",
        &format!(
            "use support::setup; {}",
            application_build(
                "builder.select_provider<support::setup::Reader, support::setup::Provider>();"
            )
        ),
    );
    let inputs = package_compilation::PackageCompilationInputs::new(
        super::fixture_package_identity(1),
        package_compilation::BuildDeclarationKind::Application,
        vec![
            package_compilation::PackageSourceBinding::new(
                super::fixture_package_identity(1),
                "owner",
                project.0.clone(),
            ),
            package_compilation::PackageSourceBinding::new(
                super::fixture_package_identity(2),
                "host",
                host.0.clone(),
            ),
            package_compilation::PackageSourceBinding::new(
                super::fixture_package_identity(3),
                "product",
                product.0.clone(),
            ),
        ],
        vec![
            package_compilation::PackageDependencyBinding::for_purpose(
                super::fixture_package_identity(1),
                "support",
                super::fixture_package_identity(2),
                build_declarations::DependencyPurpose::Build,
            ),
            package_compilation::PackageDependencyBinding::new(
                super::fixture_package_identity(1),
                "support",
                super::fixture_package_identity(3),
            ),
        ],
    )
    .unwrap();
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&project.main(), Some("macos_arm64"))
    })
    .expect("same alias in two contexts retains the product identity");
    let provider = checked
        .selected_provider_provenance()
        .iter()
        .find(|plan| plan.plan.schema.trait_name == "setup::Reader")
        .unwrap()
        .provider
        .provider_type
        .unwrap();
    assert_eq!(
        checked
            .typed
            .symbols
            .symbol_product_package_identity(provider),
        Some(super::fixture_package_identity(3))
    );
}

#[test]
fn build_only_dependency_is_not_a_product_selection_scope() {
    let dependency = provider_dependency();
    let project = TempProject::with_main(
        "const VALUE: u8 = 1;",
        &format!(
            "use support::setup; {}",
            application_build(
                "builder.select_provider<support::setup::Reader, support::setup::Provider>();"
            )
        ),
    );
    let errors = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(super::foreign_helper_inputs(&project, &dependency)),
        ..CheckedCompileRequest::new(&project.main(), Some("macos_arm64"))
    })
    .expect_err("Build imports grant no product selection");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("provider selection")
                && error.message.contains("does not resolve")),
        "{errors:?}"
    );
}
