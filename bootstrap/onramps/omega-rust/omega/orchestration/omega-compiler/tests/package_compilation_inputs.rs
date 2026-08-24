use omega_compiler::{
    CompileOptions, PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
    compile_to_checked_with_packages, compile_with_packages,
};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-package-inputs-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create temporary package compilation tree");
        Self(path)
    }

    fn package(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).expect("create package directory");
        path
    }

    fn write(path: impl AsRef<Path>, source: &str) {
        fs::write(path, source).expect("write Omega package test source");
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
}

#[test]
fn reconciled_bindings_ignore_build_dependency_discovery() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let admitted = tree.package("admitted");
    let malicious = tree.package("malicious");

    TempTree::write(
        root.join("main.omg"),
        "use dep::values;\nconst RESULT: u32 = 42;\n",
    );
    TempTree::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) {\n    builder.depend_as(\"dep\", Source::Path { location: \"../malicious\" });\n}\n",
    );
    TempTree::write(admitted.join("values.omg"), "const ANSWER: u32 = 42;\n");
    TempTree::write(malicious.join("values.omg"), "this is not Omega source\n");

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), root.clone()),
            PackageSourceBinding::new(identity(2), admitted),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("reconciled bindings should validate");

    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("trusted package binding should be the only dependency authority");
}

#[test]
fn canonical_build_dependency_vocabulary_typechecks() {
    let tree = TempTree::new();
    let root = tree.package("root");

    TempTree::write(root.join("main.omg"), "const RESULT: u32 = 42;\n");
    TempTree::write(
        root.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.depend(Source::Path { location: "../ordinary" });
    builder.depend(Source::Git {
        repository: "https://github.com/CathedralOS/arithmetic-kernels.git",
        revision: "0123456789abcdef"
    });
    builder.depend_as(
        "arithmetic_kernels",
        Source::Path { location: "../colliding" }
    );
}
"#,
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), root.clone())],
        Vec::new(),
    )
    .expect("root-only package graph should validate");

    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("canonical dependency vocabulary should typecheck");
}

#[test]
fn aliases_are_requester_local_and_dependency_imports_are_package_local() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");

    TempTree::write(
        root.join("main.omg"),
        "use shared::root_value;\nconst RESULT: u32 = 42;\n",
    );
    TempTree::write(
        middle.join("root_value.omg"),
        "use shared::leaf_value;\nuse local_value;\nconst ROOT_VALUE: u32 = 42;\n",
    );
    TempTree::write(
        middle.join("local_value.omg"),
        "const LOCAL_VALUE: u32 = 1;\n",
    );
    TempTree::write(leaf.join("leaf_value.omg"), "const LEAF_VALUE: u32 = 41;\n");

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), root.clone()),
            PackageSourceBinding::new(identity(2), middle),
            PackageSourceBinding::new(identity(3), leaf),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "shared", identity(2)),
            PackageDependencyBinding::new(identity(2), "shared", identity(3)),
        ],
    )
    .expect("requester-local aliases should validate");

    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("requester-local and package-local imports should compile");
}

#[test]
fn dependency_provider_plan_retains_exact_dependency_package_provenance() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");

    TempTree::write(root.join("main.omg"), "use dep::provider;\n");
    TempTree::write(
        dependency.join("provider.omg"),
        r#"boundary trait Pair { machine first(); }
data Provider { }
machine Provider::first() satisfies Pair::first via Binding::VtableSlot(1);
"#,
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), root.clone()),
            PackageSourceBinding::new(identity(2), dependency),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("reconciled provider graph should validate");

    let checked = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("dependency provider should check");
    let [plan] = checked.selected_provider_plans().plans() else {
        panic!("one selected dependency provider plan")
    };
    assert_eq!(plan.origin_package_identity, Some(identity(2)));
    assert_eq!(plan.provider_type_package_identity, Some(identity(2)));
    assert_eq!(plan.schema.trait_package_identity, Some(identity(2)));
    assert_eq!(
        plan.schema.methods[0].requirement_owner_package_identity,
        Some(identity(2))
    );
    assert_eq!(plan.origin_package, "");
}

#[test]
fn dependency_build_files_cannot_join_the_program() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");
    TempTree::write(root.join("main.omg"), "use dep::build;\n");
    TempTree::write(
        dependency.join("build.omg"),
        "machine build(builder: &mut Build) { }\n",
    );

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), root.clone()),
            PackageSourceBinding::new(identity(2), dependency),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("reconciled bindings should validate");

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("dependency build file import must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("may not load dependency build file")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[cfg(unix)]
#[test]
fn dependency_import_symlink_escape_rejects() {
    use std::os::unix::fs::symlink;

    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");
    let outside = tree.package("outside");
    TempTree::write(root.join("main.omg"), "use dep::escape;\n");
    TempTree::write(outside.join("secret.omg"), "const SECRET: u32 = 42;\n");
    symlink(outside.join("secret.omg"), dependency.join("escape.omg"))
        .expect("create escaping import symlink");

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), root.clone()),
            PackageSourceBinding::new(identity(2), dependency),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("reconciled bindings should validate");

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("symlink import escape must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("escapes expected source root")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[cfg(unix)]
#[test]
fn root_build_companion_symlink_escape_rejects_before_loading() {
    use std::os::unix::fs::symlink;

    let tree = TempTree::new();
    let root = tree.package("root");
    let outside = tree.package("outside");
    TempTree::write(root.join("main.omg"), "const RESULT: u32 = 42;\n");
    TempTree::write(
        outside.join("hostile-build.omg"),
        "machine build(builder: &mut Build) { }\n",
    );
    symlink(outside.join("hostile-build.omg"), root.join("build.omg"))
        .expect("create escaping build companion symlink");

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), root.clone())],
        Vec::new(),
    )
    .expect("root package input should validate");

    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("root build companion escape must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("escapes every reconciled source root")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn native_package_entrypoint_uses_the_same_reconciled_binding_mode() {
    let Some(target_name) = host_target_name() else {
        return;
    };
    let tree = TempTree::new();
    let root = tree.package("root");
    let admitted = tree.package("admitted");
    let malicious = tree.package("malicious");

    TempTree::write(
        root.join("main.omg"),
        r#"use dep::values;
boundary trait Console { machine exit_process(return_code: i32); }
data Main { console: Console; }
machine Main::main(&mut self) {
    transition ANSWER == 42 { true -> yes() _ -> no() }
    state yes(&mut self) { self.console.exit_process(0); }
    state no(&mut self) { self.console.exit_process(1); }
}
"#,
    );
    TempTree::write(
        root.join("build.omg"),
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) {
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
    builder.depend_as("dep", Source::Path { location: "../malicious" });
}
"#,
    );
    TempTree::write(admitted.join("values.omg"), "const ANSWER: u32 = 42;\n");
    TempTree::write(malicious.join("values.omg"), "this is not Omega source\n");

    let inputs = PackageCompilationInputs::new(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), root.clone()),
            PackageSourceBinding::new(identity(2), admitted),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("reconciled package graph should validate");

    compile_with_packages(
        CompileOptions {
            root_path: root.join("main.omg"),
            build_dir: Some(tree.0.join("build-output")),
            target_name: Some(target_name.to_owned()),
            write_output: false,
        },
        inputs,
    )
    .expect("native package compilation should use reconciled imports only");
}

fn host_target_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows_x64"),
        ("linux", "x86_64") => Some("linux_x64"),
        ("linux", "aarch64") => Some("linux_arm64"),
        ("macos", "aarch64") => Some("macos_arm64"),
        _ => None,
    }
}
