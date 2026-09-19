//! Direct root operands select product declarations under the binding author's
//! package scope, then retain that exact symbol through executable publication.

use super::{
    TempProject, application_build, foreign_helper_inputs, foreign_product_inputs, package_identity,
};
use build_declarations::DependencyPurpose;
use compiler::{
    CheckedCompileRequest, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
    compile_to_checked,
};
use package_compilation::{
    BuildDeclarationKind, PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use std::fs;

fn dependency_with_entry(source: &str) -> TempProject {
    let dependency = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"entry-library\"); }",
    );
    fs::write(dependency.0.join("setup.omg"), source).expect("dependency source");
    dependency
}

fn dual_purpose_inputs(
    project: &TempProject,
    dependency: &TempProject,
) -> PackageCompilationInputs {
    PackageCompilationInputs::new(
        package_identity(1),
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(package_identity(1), "root-binding-owner", project.0.clone()),
            PackageSourceBinding::new(package_identity(2), "entry-library", dependency.0.clone()),
        ],
        [DependencyPurpose::Build, DependencyPurpose::Product]
            .into_iter()
            .map(|purpose| {
                PackageDependencyBinding::for_purpose(
                    package_identity(1),
                    "support",
                    package_identity(2),
                    purpose,
                )
            })
            .collect(),
    )
    .expect("separate build and product edges to the entry library")
}

#[test]
fn direct_dependency_root_binding_keeps_its_exact_symbol_through_terminal_production() {
    let dependency = dependency_with_entry("module setup; pub machine launch() { }");
    let project = TempProject::with_main(
        "use support::setup; machine launch() { crash Trap; }",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); builder.roots.bind(windows_x86_64::ProgramEntry, support::setup::launch); }",
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(dual_purpose_inputs(&project, &dependency));
    let checked = compile_to_checked(request).expect("qualified dependency root selection");
    let selected_symbol = checked
        .selected_program_entry()
        .expect("entry")
        .source_signature()
        .machine_symbol();
    assert_eq!(
        checked
            .symbols
            .symbol_product_package_identity(selected_symbol),
        Some(package_identity(2))
    );
    let report = compiler::retained_terminal_report_from_checked_package(
        project.main(),
        checked,
        proof_admission::AdmissionProfile::default(),
    )
    .expect("qualified dependency entry reaches Terminal production");
    let artifact = report
        .into_retained_terminal_artifact()
        .expect("retained Terminal product");
    artifact
        .validate()
        .expect("independent Terminal verification");
    assert_eq!(
        artifact
            .native_realization_proposal()
            .expect("retained entry proposal")
            .checked_program_entry()
            .source_machine_symbol(),
        selected_symbol
    );
    assert_eq!(
        artifact
            .native_realization_proposal()
            .expect("retained entry proposal")
            .checked_program_entry()
            .source_machine_name(),
        "setup::launch"
    );
}

#[test]
fn exact_module_root_path_does_not_collide_with_an_attached_machine_shorthand() {
    let declarations = TempProject::with_main(
        "use Entry; use other;",
        &application_build("builder.roots.bind(windows_x86_64::ProgramEntry, Entry::launch);"),
    );
    fs::write(
        declarations.0.join("Entry.omg"),
        "module Entry; pub machine launch() { }",
    )
    .expect("exact module entry");
    fs::write(
        declarations.0.join("other.omg"),
        "module other; pub data Entry { } pub machine Entry::launch(&mut self) { }",
    )
    .expect("attached machine with the same relative name");
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &declarations.main(),
        Some("windows_x86_64"),
    ))
    .expect("exact module path is not an attached-machine shorthand");
    let selected = checked
        .selected_program_entry()
        .expect("entry")
        .source_signature()
        .machine_symbol();
    assert_eq!(
        checked.symbols.display_path(selected, "::"),
        "Entry::launch"
    );

    let project = TempProject::with_main(
        "use support::Entry; use support::other;",
        &application_build(
            "builder.roots.bind(windows_x86_64::ProgramEntry, support::Entry::launch);",
        ),
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_product_inputs(&project, &declarations));
    let checked = compile_to_checked(request).expect("exact dependency path is not a shorthand");
    let selected = checked
        .selected_program_entry()
        .expect("entry")
        .source_signature()
        .machine_symbol();
    assert_eq!(
        checked.symbols.display_path(selected, "::"),
        "Entry::launch"
    );
    assert_eq!(
        checked.symbols.symbol_product_package_identity(selected),
        Some(package_identity(2))
    );
}

#[test]
fn direct_root_preserves_attached_names_in_a_named_product_module() {
    let project = TempProject::with_main(
        "module app; data Application { } machine Application::start(&mut self) { }",
        &application_build("builder.roots.bind(windows_x86_64::ProgramEntry, Application::start);"),
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect("the root product module retains its attached entry spelling");
    let selected = checked
        .selected_program_entry()
        .expect("entry")
        .source_signature()
        .machine_symbol();
    assert_eq!(
        checked.symbols.display_path(selected, "::"),
        "app::Application::start"
    );
}

#[test]
fn direct_root_uses_the_product_target_of_a_shared_dependency_alias() {
    let build_dependency = dependency_with_entry("module setup; pub machine launch() { }");
    let product_dependency = dependency_with_entry("module setup; pub machine launch() { }");
    let project = TempProject::with_main(
        "use support::setup;",
        "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); builder.roots.bind(windows_x86_64::ProgramEntry, support::setup::launch); }",
    );
    let inputs = PackageCompilationInputs::new(
        package_identity(1),
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(package_identity(1), "root-binding-owner", project.0.clone()),
            PackageSourceBinding::new(
                package_identity(2),
                "build-library",
                build_dependency.0.clone(),
            ),
            PackageSourceBinding::new(
                package_identity(3),
                "product-library",
                product_dependency.0.clone(),
            ),
        ],
        vec![
            PackageDependencyBinding::for_purpose(
                package_identity(1),
                "support",
                package_identity(2),
                DependencyPurpose::Build,
            ),
            PackageDependencyBinding::new(package_identity(1), "support", package_identity(3)),
        ],
    )
    .expect("independent alias targets");
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(inputs);
    let checked = compile_to_checked(request).expect("select the product alias target");
    let selected = checked
        .selected_program_entry()
        .expect("entry")
        .source_signature()
        .machine_symbol();
    assert_eq!(
        checked.symbols.symbol_product_package_identity(selected),
        Some(package_identity(3))
    );
}

#[test]
fn direct_dependency_root_binding_rejects_private_or_unauthorized_declarations() {
    for (entry_source, path) in [
        (
            "module setup; machine launch() { }",
            "support::setup::launch",
        ),
        (
            "module setup; pub machine launch() { }",
            "unknown::setup::launch",
        ),
    ] {
        let dependency = dependency_with_entry(entry_source);
        let project = TempProject::with_main(
            "use support::setup;",
            &application_build(&format!(
                "builder.roots.bind(windows_x86_64::ProgramEntry, {path});"
            )),
        );
        let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
        request.package_inputs = Some(foreign_product_inputs(&project, &dependency));
        let Err(diagnostics) = compile_to_checked(request) else {
            panic!("unauthorized direct root bound: {path}");
        };
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("not a product declaration visible")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn direct_dependency_root_binding_does_not_borrow_build_scope_nameability() {
    let dependency = dependency_with_entry("module setup; pub machine launch() { }");
    fs::write(
        dependency.0.join("product.omg"),
        "module product; pub data ProductOnly { }",
    )
    .expect("separate product source");
    // Even an authorized product edge cannot select setup::launch when only
    // the build checked instance of setup.omg exists in the supplied frontier.
    for has_product_edge in [false, true] {
        let main = if has_product_edge {
            "use support::product;"
        } else {
            "const ANSWER: u8 = 0;"
        };
        let project = TempProject::with_main(
            main,
            "use support::setup; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); builder.roots.bind(windows_x86_64::ProgramEntry, support::setup::launch); }",
        );
        let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
        request.package_inputs = Some(if has_product_edge {
            dual_purpose_inputs(&project, &dependency)
        } else {
            foreign_helper_inputs(&project, &dependency)
        });
        let Err(diagnostics) = compile_to_checked(request) else {
            panic!("build instance selected with product edge {has_product_edge}");
        };
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("not a product declaration visible")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn direct_dependency_root_binding_preserves_entry_signature_validation() {
    let dependency = dependency_with_entry("module setup; pub machine launch(argument: u64) { }");
    let project = TempProject::with_main(
        "use support::setup;",
        &application_build(
            "builder.roots.bind(windows_x86_64::ProgramEntry, support::setup::launch);",
        ),
    );
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(foreign_product_inputs(&project, &dependency));
    let Err(diagnostics) = compile_to_checked(request) else {
        panic!("a qualified path cannot admit an incompatible entry signature");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("ProgramEntry` exposes no arrival parameters")
            && diagnostic.message.contains("support::setup::launch")),
        "{diagnostics:?}"
    );
}

#[test]
fn foreign_helper_cannot_select_through_its_callers_product_alias() {
    let dependency = dependency_with_entry("module setup; pub machine launch() { }");
    let helper = TempProject::new(
        "machine build(builder: &mut Build) { builder.package(\"build-helper\"); }",
    );
    fs::write(helper.0.join("configure.omg"),
        "module configure; pub machine configure(builder: &mut Build) { builder.roots.bind(windows_x86_64::ProgramEntry, runtime::setup::launch); }",
    ).expect("foreign helper source");
    let project = TempProject::with_main(
        "use runtime::setup;",
        "use support::configure; machine build(builder: &mut Build) { builder.application(\"root-binding-owner\"); configure::configure(builder); }",
    );
    let inputs = PackageCompilationInputs::new(
        package_identity(1),
        BuildDeclarationKind::Application,
        vec![
            PackageSourceBinding::new(package_identity(1), "root-binding-owner", project.0.clone()),
            PackageSourceBinding::new(package_identity(2), "build-helper", helper.0.clone()),
            PackageSourceBinding::new(package_identity(3), "entry-library", dependency.0.clone()),
        ],
        vec![
            PackageDependencyBinding::for_purpose(
                package_identity(1),
                "support",
                package_identity(2),
                DependencyPurpose::Build,
            ),
            PackageDependencyBinding::new(package_identity(1), "runtime", package_identity(3)),
        ],
    )
    .expect("caller-only product dependency");
    let mut request = CheckedCompileRequest::new(&project.main(), Some("windows_x86_64"));
    request.package_inputs = Some(inputs);
    let diagnostics = compile_to_checked(request)
        .expect_err("borrowing Build cannot lend the caller's product aliases");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("not a product declaration visible")),
        "{diagnostics:?}"
    );
}

#[test]
fn direct_module_root_binding_selects_the_owners_private_product_declaration() {
    let project = TempProject::with_main(
        "use entry; machine launch() { crash Trap; }",
        &application_build("builder.roots.bind(windows_x86_64::ProgramEntry, entry::launch);"),
    );
    fs::write(
        project.0.join("entry.omg"),
        "module entry; machine launch() { }",
    )
    .expect("private product module");
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &project.main(),
        Some("windows_x86_64"),
    ))
    .expect("qualified own-package entry binds without executing either declaration");
    let entry_symbol = checked
        .selected_program_entry()
        .expect("entry")
        .source_signature()
        .machine_symbol();
    assert_eq!(
        checked.symbols.display_path(entry_symbol, "::"),
        "entry::launch"
    );
}

#[test]
fn direct_dependency_root_binding_publishes_and_executes_on_the_matching_host() {
    let Some(profile) = target::TargetProfile::host_if_supported() else {
        eprintln!("SKIP: native root binding requires a supported hosted target");
        return;
    };
    let dependency = dependency_with_entry("module setup; pub machine launch() { }");
    let project = TempProject::with_main(
        "use support::setup; machine launch() { crash Trap; }",
        &application_build(&format!(
            "builder.roots.bind({}::ProgramEntry, support::setup::launch);",
            profile.root_slot_owner_name()
        )),
    );
    let report = compile(
        CompileRequest::new(CompileOptions {
            root_path: project.main(),
            build_dir: None,
            target_name: Some(profile.target_name().into()),
        })
        .with_package_inputs(foreign_product_inputs(&project, &dependency))
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("qualified dependency entry reaches native production");
    let published = report
        .publish_retained_native_artifact(&project.0.join("out"))
        .expect("native publication");
    let executable = published
        .checked_native_executable_path()
        .expect("checked executable receipt");
    let output = std::process::Command::new(executable)
        .output()
        .expect("execute selected entry");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}
