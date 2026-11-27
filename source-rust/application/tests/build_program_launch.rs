//! The installer and the reviewed Build authenticate the same program demand.
//! A returned Build declaration makes the real process exit observable.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use package_compilation::build_execution::{
    BuildProgramBinding, BuildProgramInvocation, RestrictedBuildGrants, RestrictedBuildOperation,
    RestrictedBuildRequest,
};
use package_compilation::declarations::BuildDeclaration;
use platform_custody::temporary_directory::TemporaryDirectory;
use project_build::{PlanningRequest, PlanningSources, interpret, prepare};
use std::path::{Path, PathBuf};
use std::process::Command;

struct Accepted(RestrictedBuildRequest);

impl RestrictedBuildGrants for Accepted {
    fn admits(
        &mut self,
        _: &package_compilation::build_execution::RestrictedBuildSite,
        requested: &RestrictedBuildRequest,
    ) -> Result<bool, Vec<diagnostics::Diagnostic>> {
        Ok(requested == &self.0)
    }
}

const ROOT_MARKERS: &str = r#"data Packer {}
data Other {}
machine unused(arguments: @[@[u8]]) -> |program|::RunResult blocks {
    block |program|::run::<Other>(arguments)
}
"#;

fn authored_launcher(program: &str, declarations: &str) -> String {
    let body = r#"machine build(builder: @mutable BuildPlan) blocks {
    let arguments: [@[u8]; 3] = ["", "two words", "last"];
    let outcome: |program|::RunResult = block |program|::run::<PROGRAM_TYPE>(@arguments[..]);
    transition outcome {
        |program|::RunResult::Exited { status } -> exited(builder, status),
        _ -> failed(builder)
    }
    machine exited(builder: @mutable BuildPlan, status: u32) {
        transition status {
            47 -> success(builder),
            _ -> failed(builder)
        }
    }
    machine success(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "launched" }; }
    machine failed(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "launch_failed" }; }
}"#;
    format!(
        "use {{omega}}/build/program.omg;\n{declarations}\n{}",
        body.replace("PROGRAM_TYPE", program)
    )
}

#[test]
fn an_exact_accepted_binding_runs_the_authored_build_launcher() {
    package_compilation::toolchain_library::configure(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source/library"),
    )
    .unwrap();
    let temp = TemporaryDirectory::create("checked-build-program", false).unwrap();
    let app = temp.path().join("app");
    std::fs::create_dir(&app).unwrap();
    let entry = app.join("build.omg");
    std::fs::write(&entry, authored_launcher("Packer", ROOT_MARKERS)).unwrap();
    let profile = target::TargetProfile::host_if_supported().unwrap();
    let source =
        package_manager::operations::begin_local_project_preparation(&entry, &[profile], true)
            .unwrap()
            .acquire_root()
            .unwrap();

    let image = build_guest(temp.path());
    let public = PathBuf::from(env!("CARGO_BIN_EXE_omega"));
    let invocation = |selector: &str| BuildProgramInvocation {
        allow: true,
        inherited: Vec::new(),
        reviewed_root_name: Some("board".into()),
        worker: Some(public.clone()),
        bindings: vec![BuildProgramBinding::new(selector.to_owned(), image.clone()).unwrap()],
    };
    let prepare_source = |programs, accepted: Option<RestrictedBuildRequest>| {
        prepare(
            PlanningRequest {
                root: &entry,
                sources: PlanningSources::Captured(source.source_capture()),
                execution: package_compilation::BuildExecutionInputs {
                    profile: Some(profile),
                    programs,
                    restricted_grants: accepted.map(|request| {
                        Box::new(Accepted(request)) as Box<dyn RestrictedBuildGrants>
                    }),
                    ..Default::default()
                },
                artifact_only: false,
            },
            Default::default(),
            Default::default(),
        )
    };
    let refusal = |programs| {
        prepare_source(programs, None)
            .err()
            .expect("unauthorized Build reached execution")
    };
    for (grants, expected) in [
        (BuildProgramInvocation::default(), "ceiling"),
        (invocation("Other"), "lacks an executable binding"),
    ] {
        let diagnostics = refusal(grants).into_diagnostics();
        assert!(
            diagnostics
                .iter()
                .any(|d| d.message.contains(expected) && d.message.contains("Packer")),
            "expected {expected:?}, got {diagnostics:?}"
        );
    }

    let build_execution::CompileFailure::BuildConsentRequired { requests, .. } =
        refusal(invocation("Packer"))
    else {
        panic!("an exact invoker grant must reach the package consent boundary");
    };
    let [request] = requests.as_slice() else {
        panic!("unexpected requests {requests:?}")
    };
    assert!(
        matches!(request.operation(), RestrictedBuildOperation::ProgramLaunch(row) if row.declaration == "Packer")
    );
    let approved = prepare_source(invocation("board::Packer"), Some(request.clone())).unwrap();
    let result = interpret(approved, &mut ()).unwrap();
    let Some(BuildDeclaration::Package(package)) = result.returned().declaration() else {
        panic!("the Build did not return a package")
    };
    assert_eq!(package.name.as_str(), "launched");

    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source/library");
    std::fs::write(
        app.join("main.omg"),
        "public machine answer() -> u32 { 47u32 }",
    )
    .unwrap();
    let package = |command: &str, binding: &Path, arguments: &[&str]| {
        Command::new(&public)
            .current_dir(&app)
            .args([
                "packages",
                command,
                "--allow",
                "ProgramLaunch",
                "--program",
                "Packer",
            ])
            .arg(binding)
            .args(arguments)
            .env("OMEGA_LIBRARY_ROOT", &library)
            .output()
            .unwrap()
    };
    let show = |output: &std::process::Output| {
        format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    };
    require_undelegated_child_refusal(temp.path(), &public, &image, &library);
    let targets = [
        target::TargetProfile::LinuxX64,
        target::TargetProfile::LinuxArm64,
    ];
    let pending = package(
        "update",
        &image,
        &[
            "--target",
            targets[0].target_name(),
            "--target",
            targets[1].target_name(),
        ],
    );
    assert_eq!(pending.status.code(), Some(3), "{}", show(&pending));
    let lock = app.join("omega.lock");
    assert!(!lock.exists());
    let decision = app.join("build/package-manager/review-build.txt");
    let draft = std::fs::read_to_string(&decision).unwrap();
    assert!(
        draft.contains("Packer")
            && draft.contains("program-owner requesting-source")
            && draft.contains("timeout-ms")
            && draft.contains("decision build"),
        "{draft}"
    );
    std::fs::remove_file(&decision).unwrap();
    let recovered = package("update", &image, &["--resume"]);
    assert_eq!(recovered.status.code(), Some(3), "{}", show(&recovered));
    assert_eq!(std::fs::read_to_string(&decision).unwrap(), draft);
    assert!(!app.join("omega.lock").exists());

    std::fs::write(&decision, draft.replace(" pending\n", " reject\n")).unwrap();
    let rejected = package("update", &image, &["--resume"]);
    assert_eq!(rejected.status.code(), Some(3), "{}", show(&rejected));
    assert!(!lock.exists());

    // An edit to a displayed finding cannot be interpreted as a decision.
    std::fs::write(
        &decision,
        draft
            .replace("Packer", "Other")
            .replace(" pending\n", " accept\n"),
    )
    .unwrap();
    let forged = package("update", &image, &["--resume"]);
    assert_ne!(forged.status.code(), Some(0));
    assert!(
        show(&forged).contains("findings changed"),
        "{}",
        show(&forged)
    );
    std::fs::write(&decision, draft.replace(" pending\n", " accept\n")).unwrap();
    let product = package("update", &image, &["--resume"]);
    assert_eq!(product.status.code(), Some(3), "{}", show(&product));
    assert!(
        !lock.exists(),
        "Build acceptance published an unchecked Product"
    );
    for target in targets {
        let target_doc = app
            .join("build/package-manager")
            .join(format!("review-{}.txt", target.target_name()));
        let graph = std::fs::read_to_string(&target_doc).unwrap();
        assert!(
            graph.contains("launched") && !graph.contains("launch_failed"),
            "{graph}"
        );
        std::fs::write(&target_doc, graph.replace(" pending\n", " accept\n")).unwrap();
    }
    let published = package("update", &image, &["--resume"]);
    assert!(published.status.success(), "{}", show(&published));
    let locked = std::fs::read(&lock).unwrap();
    let audit = package("audit", &image, &[]);
    assert!(audit.status.success(), "{}", show(&audit));
    for target in targets {
        assert!(
            show(&audit).contains(&format!(
                "product target {} execution {}",
                target.target_name(),
                profile.target_name()
            )),
            "{}",
            show(&audit)
        );
    }
    assert_eq!(std::fs::read(&lock).unwrap(), locked);

    let compile = || {
        Command::new(&public)
            .args(["build", "--allow", "ProgramLaunch", "--program", "Packer"])
            .arg(&image)
            .arg(&entry)
            .env("OMEGA_LIBRARY_ROOT", &library)
            .output()
            .unwrap()
    };
    let unchanged = compile();
    assert!(unchanged.status.success(), "{}", show(&unchanged));
    assert_eq!(std::fs::read(&lock).unwrap(), locked);

    // The same nominal selector and locator may now resolve to a different
    // image. Portable acceptance covers the approved bytes.
    {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&image)
            .unwrap()
            .write_all(&[0])
            .unwrap();
    }
    let changed_image = prepare_source(invocation("Packer"), Some(request.clone()))
        .err()
        .expect("a changed executable reused the previous acceptance");
    assert!(matches!(
        changed_image,
        build_execution::CompileFailure::BuildConsentRequired { .. }
    ));

    // The normal public entry still consumes package acceptance. A root
    // invocation cannot manufacture it by passing the reviewed executable.
    let cli = compile();
    assert!(
        !cli.status.success(),
        "ordinary compilation invented consent"
    );
    let error = String::from_utf8_lossy(&cli.stderr);
    assert!(
        error.contains("onsent") || error.contains("rogramLaunch"),
        "{error}"
    );
    assert_eq!(std::fs::read(&lock).unwrap(), locked);
}

fn build_guest(root: &Path) -> PathBuf {
    compile_guest(
        root,
        r#"#include <string.h>
int main(int count, char **args) {
    return count == 4 && !strcmp(args[1], "") && !strcmp(args[2], "two words")
        && !strcmp(args[3], "last") ? 47 : 9;
}"#,
    )
}

fn compile_guest(root: &Path, program: &str) -> PathBuf {
    let guest = root.join("guest.c");
    std::fs::write(&guest, program).unwrap();
    let image = root.join("program");
    assert!(
        Command::new("cc")
            .args(["-static", "-O2"])
            .arg(&guest)
            .arg("-o")
            .arg(&image)
            .status()
            .unwrap()
            .success()
    );

    image
}

fn require_undelegated_child_refusal(root: &Path, cli: &Path, image: &Path, library: &Path) {
    let parent = root.join("parent");
    let child = root.join("child");
    for dir in [&parent, &child] {
        std::fs::create_dir(dir).unwrap();
        std::fs::write(
            dir.join("main.omg"),
            "public machine answer() -> u32 { 10u32 }",
        )
        .unwrap();
    }
    std::fs::write(parent.join("build.omg"), r#"use {omega}/build/dependency.omg;
data Packer {}
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Package { name: "parent" };
    _ = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "../child" }));
}"#).unwrap();
    std::fs::write(
        child.join("build.omg"),
        r#"use {omega}/build/program.omg;
data Packer {}
machine build(builder: @mutable BuildPlan) blocks {
    let args: [@[u8]; 1] = ["child"];
    _ = block |program|::run::<Packer>(@args[..]);
    builder.product = BuildProduct::Package { name: "child" };
}"#,
    )
    .unwrap();
    let result = Command::new(cli)
        .current_dir(parent)
        .env("OMEGA_LIBRARY_ROOT", library)
        .args(["build", "--allow", "ProgramLaunch", "--program", "Packer"])
        .arg(image)
        .arg("build.omg")
        .output()
        .unwrap();
    let message = String::from_utf8_lossy(&result.stderr);
    assert!(
        !result.status.success() && message.contains("Packer") && message.contains("ceiling"),
        "{message}"
    );
}

#[test]
fn a_reviewed_program_crosses_only_the_registering_dependency_edge() {
    let temp = TemporaryDirectory::create("delegated-build-program", false).unwrap();
    let parent = temp.path().join("parent");
    let child = temp.path().join("child");
    for directory in [&parent, &child] {
        std::fs::create_dir(directory).unwrap();
        std::fs::write(
            directory.join("main.omg"),
            "public machine answer() -> u32 { 47u32 }",
        )
        .unwrap();
    }
    std::fs::write(
        parent.join("build.omg"),
        r#"use {omega}/build/program.omg;
machine build(builder: @mutable BuildPlan) suspends {
    let child: DependencyRegistration = Build::dependency(builder,
        Source::Path { location: "../child" },
        ProgramLaunch::Inherit::<|program|::RunResult>());
    suspend Build::prepare_dependency(builder, child);
    let needed: RequiredOutput = BuildOutput::require(@mutable builder.output, "receipt.txt");
    let receipt_file: BuildPath = BuildOutput::resolve(@builder.output, "receipt.txt");
    let result: OutputCompletion = BuildOutput::complete(@mutable builder.output, needed, receipt_file);
    transition result {
        OutputCompletion::Sealed { receipt } -> installed(builder),
        OutputCompletion::Retry { obligation, file } -> failed(builder, obligation)
    }
    machine installed(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "parent" }; }
    machine failed(builder: @mutable BuildPlan, obligation: RequiredOutput) {
        BuildOutput::fail(@mutable builder.output, obligation, "delegated writer did not write the parent's output");
    }
}"#,
    )
    .unwrap();
    std::fs::write(
        child.join("build.omg"),
        authored_launcher("|program|::RunResult", ""),
    )
    .unwrap();
    let image = compile_guest(
        temp.path(),
        r#"#include <stdio.h>
#include <string.h>
int main(int count, char **args) {
    char source[4096] = {0};
    FILE *input = fopen("/build.omg", "rb");
    if (!input || !fread(source, 1, sizeof(source)-1, input) || fclose(input) != 0) return 5;
    if (!strstr(source, "ProgramLaunch::Inherit")) return 6;
    FILE *output = fopen("/receipt.txt", "wb");
    if (!output || fputs("forwarded", output) < 0 || fclose(output) != 0) return 7;
    return count == 4 && !strcmp(args[1], "") && !strcmp(args[2], "two words")
        && !strcmp(args[3], "last") ? 47 : 9;
}"#,
    );
    let cli = Path::new(env!("CARGO_BIN_EXE_omega"));
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source/library");
    let run = |args: &[&str]| {
        let output = Command::new(cli)
            .current_dir(&parent)
            .env("OMEGA_LIBRARY_ROOT", &library)
            .args(args)
            .args(["--allow", "ProgramLaunch", "--program", "RunResult"])
            .arg(&image)
            .args([
                "--program-read",
                "build.omg",
                "--program-write",
                "receipt.txt",
            ])
            .output()
            .unwrap();
        let report = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        (output.status.code(), report)
    };
    let vote = |path: &Path| {
        let source = std::fs::read_to_string(path).unwrap();
        assert!(source.contains(" pending\n"), "{source}");
        std::fs::write(path, source.replace(" pending\n", " accept\n")).unwrap();
        source
    };
    let phase = parent.join("build/package-manager/review-build.txt");
    let lock = parent.join("omega.lock");
    let (status, report) = run(&["packages", "update"]);
    assert_eq!(status, Some(3), "{report}");
    assert!(vote(&phase).contains("root Build entry"));
    let (status, report) = run(&["packages", "update", "--resume"]);
    assert_eq!(status, Some(3), "{report}");
    let child_review = vote(&phase);
    assert!(
        child_review.contains("dependency") && child_review.contains("program-owner toolchain"),
        "{child_review}"
    );
    assert!(!lock.exists());
    let (status, report) = run(&["packages", "update", "--resume"]);
    assert_eq!(status, Some(3), "{report}");
    let target = target::TargetProfile::host_if_supported().unwrap();
    let graph = vote(&parent.join(format!(
        "build/package-manager/review-{}.txt",
        target.target_name()
    )));
    assert!(
        graph.contains("launched") && !graph.contains("launch_failed"),
        "{graph}"
    );
    let (status, report) = run(&["packages", "update", "--resume"]);
    assert_eq!(status, Some(0), "{report}");
    let (status, report) = run(&["build", "build.omg"]);
    assert_eq!(status, Some(0), "{report}");
    let published = report
        .lines()
        .find_map(|line| line.strip_prefix("published completed build outputs to "))
        .unwrap_or_else(|| panic!("parent did not publish its sealed output: {report}"));
    assert_eq!(
        std::fs::read(Path::new(published).join("files/receipt.txt")).unwrap(),
        b"forwarded"
    );

    // The same acquired child is already settled when the second registration
    // asks for it. Its stronger first edge cannot lend the second its program.
    let accepted_lock = std::fs::read(&lock).unwrap();
    let build = parent.join("build.omg");
    let source = std::fs::read_to_string(&build).unwrap();
    let second_edge = r#"suspend Build::prepare_dependency(builder, child);
    let mutable other: DependencyDescription = |dependency|::from_source(Source::Path { location: "../child" });
    |dependency|::with_alias(@mutable other, "second");
    let sibling: DependencyRegistration = Build::dependency(builder, other);
    suspend Build::prepare_dependency(builder, sibling);"#;
    std::fs::write(
        &build,
        format!(
            "use {{omega}}/build/dependency.omg;\n{}",
            source.replace(
                "suspend Build::prepare_dependency(builder, child);",
                second_edge
            )
        ),
    )
    .unwrap();
    let (mut status, mut report) = run(&["packages", "update"]);
    for _ in 0..2 {
        if status != Some(3) {
            break;
        }
        vote(&phase);
        (status, report) = run(&["packages", "update", "--resume"]);
    }
    assert_eq!(status, Some(1), "{report}");
    assert!(
        report.contains("registration lacks the reviewed ProgramLaunch binding")
            && report.contains("RunResult"),
        "{report}"
    );
    assert_eq!(std::fs::read(lock).unwrap(), accepted_lock);
}
