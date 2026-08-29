use omega_build_declarations::BuildDeclarationError;
use omega_compiler::{compile_to_checked, compile_to_checked_with_packages};
use omega_package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempProject(PathBuf);

impl TempProject {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-checked-build-machine-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create temporary Omega project");
        Self(path)
    }

    fn write(&self, name: &str, source: &str) {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create temporary Omega source directory");
        }
        fs::write(path, source).expect("write temporary Omega source");
    }

    fn main(&self) -> PathBuf {
        self.0.join("main.omg")
    }

    fn root(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn absent_build_machine_retains_no_symbol_or_standalone_package_identity() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");

    let checked = compile_to_checked(&project.main(), None).expect("program should check");

    assert_eq!(checked.selected_build_machine_symbol(), None);
    assert_eq!(checked.package_identity(), None);
}

#[test]
fn present_build_machine_retains_its_exact_checked_symbol() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.application(\"checked-build-symbol\"); }\n",
    );

    let checked = compile_to_checked(&project.main(), None).expect("build program should check");
    let build = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("checked build machine");

    assert_eq!(checked.selected_build_machine_symbol(), Some(build.symbol));
}

#[test]
fn free_build_root_composes_an_ordinary_free_helper_contract() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"use omega::language::std::console;

machine configure(builder: &mut Build)
reaches Console
{
    builder.freestanding = false;
}

machine build(builder: &mut Build)
reaches Console
{
    builder.application("free-helper-composition");
    configure(builder);
}
"#,
    );

    let checked = compile_to_checked(&project.main(), None)
        .expect("the canonical root must compose an ordinary free helper contract");
    assert!(checked.selected_build_machine_symbol().is_some());
}

#[test]
fn selected_free_build_without_a_project_role_rejects_with_shared_diagnostic() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write("build.omg", "machine build(builder: &mut Build) { }\n");

    let diagnostics = compile_to_checked(&project.main(), None)
        .expect_err("a selected free build root must declare its project role");
    let missing_kind = BuildDeclarationError::MissingBuildDeclaration.to_string();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.ends_with(&missing_kind)),
        "unexpected diagnostics: {diagnostics:#?}",
    );
}

#[test]
fn selected_scoped_build_rejects_with_directed_migration_diagnostic() {
    let project = TempProject::new();
    project.write("main.omg", "data Owner { }\nconst ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        "machine Owner::build(&mut self, builder: &mut Build) { }\n",
    );

    let diagnostics = compile_to_checked(&project.main(), None)
        .expect_err("a selected scoped build root must never receive build authority");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("free `machine build(builder: &mut Build)` entry")
                && diagnostic
                    .message
                    .contains("`Owner::build` is an ordinary scoped machine")
        }),
        "unexpected diagnostics: {diagnostics:#?}",
    );
}

#[test]
fn imported_file_named_build_is_not_a_project_build_root() {
    let project = TempProject::new();
    project.write(
        "main.omg",
        "use nested::build;\ndata Helper { }\nconst ANSWER: u32 = 42;\n",
    );
    project.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.application(\"imported-build-source\"); }\n",
    );
    project.write(
        "nested/build.omg",
        "machine Helper::build(&mut self, builder: &mut Build) { }\n",
    );

    let checked = compile_to_checked(&project.main(), None)
        .expect("an imported build.omg must remain ordinary program source");
    let root_build = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("checked project build machine");

    assert_eq!(
        checked.selected_build_machine_symbol(),
        Some(root_build.symbol)
    );
}

#[test]
fn exact_build_source_receives_toolchain_build_while_program_build_remains_ordinary() {
    let project = TempProject::new();
    project.write(
        "main.omg",
        "data Build { marker: i32 in Wrapping; }\ndata Main { local: Build; }\n",
    );
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("source-scoped-build-vocabulary");
}

"#,
    );

    let checked = compile_to_checked(&project.main(), None)
        .expect("program and toolchain Build declarations must occupy exact source contexts");
    let builds = checked
        .typed
        .data_definitions()
        .iter()
        .filter(|definition| definition.name.as_str() == "Build")
        .collect::<Vec<_>>();
    assert_eq!(builds.len(), 2);
    let toolchain_build = builds
        .iter()
        .copied()
        .find(|definition| {
            checked
                .typed
                .symbols
                .symbol_source_span(definition.symbol)
                .and_then(|span| checked.typed.symbols.source_file(span))
                .is_some_and(|file| file.path == Path::new("<build-prelude>"))
        })
        .expect("exact toolchain Build declaration");
    let program_build = builds
        .iter()
        .copied()
        .find(|definition| definition.symbol != toolchain_build.symbol)
        .expect("ordinary program Build declaration");

    let build_machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("project build machine");
    let build_parameter = checked
        .typed
        .state_parameters(&checked.typed.machine_states(build_machine)[0])
        .first()
        .expect("builder parameter");
    let psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. } = checked
        .typed
        .type_reference_table
        .type_reference(build_parameter.type_reference)
    else {
        panic!("builder must remain a reference");
    };
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        checked.typed.type_reference_table.type_reference(*referee)
    else {
        panic!("builder referee must remain nominal");
    };
    assert_eq!(*symbol, toolchain_build.symbol);

    let main = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Main")
        .expect("Main declaration");
    let psi_typed_trees::data::DataMember::Field(local) = &checked.typed.data_members(main)[0]
    else {
        panic!("Main.local must remain a field");
    };
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } = checked
        .typed
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        panic!("Main.local must retain a nominal type");
    };
    assert_eq!(*symbol, program_build.symbol);
}

#[test]
fn source_scoped_toolchain_binding_does_not_hide_ordinary_duplicates() {
    let project = TempProject::new();
    project.write(
        "main.omg",
        "use other;\ndata Build { first: i32 in Wrapping; }\n",
    );
    project.write("other.omg", "data Build { second: i32 in Wrapping; }\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("duplicate-program-builds");
}
"#,
    );

    let diagnostics = compile_to_checked(&project.main(), None)
        .expect_err("two ordinary program Build declarations must still conflict");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "duplicate data `Build`"),
        "unexpected diagnostics: {diagnostics:#?}",
    );
}

#[test]
fn package_aware_checked_compilation_retains_the_reconciled_root_identity() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    let root_identity = PackageKeyIdentity::from_digest([7; 32]).expect("nonzero package identity");
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![PackageSourceBinding::new(
            root_identity,
            "checked-build-identity",
            project.root().to_owned(),
        )],
        Vec::new(),
    )
    .expect("root package inputs should validate");

    let checked = compile_to_checked_with_packages(&project.main(), None, inputs)
        .expect("package-aware program should check");

    assert_eq!(checked.package_identity(), Some(root_identity));
    assert_eq!(checked.selected_build_machine_symbol(), None);
}

#[test]
fn a_scoped_build_cannot_compete_with_the_free_selected_entry() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"data Helper { }
machine build(builder: &mut Build) { builder.application("free-build-wins"); }
machine Helper::build(&mut self, builder: &mut Build) { }
"#,
    );

    let diagnostics = compile_to_checked(&project.main(), None)
        .expect_err("a scoped build in selected build.omg must reject before selection");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`Helper::build` is an ordinary scoped machine")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}
