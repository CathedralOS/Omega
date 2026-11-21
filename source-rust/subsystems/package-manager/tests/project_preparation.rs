//! Exercise package and provider selection through the public project API.
use build_declarations::DependencyPurpose;
use package_manager::project::open_product;
use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::path::Path;
use target::TargetProfile;

fn configure() {
    static LIBRARY: std::sync::Once = std::sync::Once::new();
    LIBRARY.call_once(|| {
        let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../source/library");
        package_compilation::toolchain_library::configure(&library).unwrap();
    });
}

fn write_package(root: &Path, build: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(root.join("build.omg"), build).unwrap();
    fs::write(root.join("main.omg"), "machine main() {}").unwrap();
}

#[test]
fn build_dependencies_use_the_host_and_product_dependencies_use_the_requested_target() {
    configure();
    let temp = TemporaryDirectory::create("project-targets", false).unwrap();
    let client = temp.path().join("client");
    write_package(
        &client,
        r#"machine build(builder: @mut Build) {
        Build::application(builder, "client");
        let build_dep: DependencyDescription = dependency\from_source(Source::Path { location: "library" });
        let build_registration: DependencyRegistration = Build::build_dependency(builder, build_dep);
        let product_dep: DependencyDescription = dependency\from_source(Source::Path { location: "library" });
        let product_registration: DependencyRegistration = Build::dependency(builder, product_dep);
        Build::bind(builder, windows_x86_64\ProgramEntry, main);
        Build::bind(builder, linux_x86_64\ProgramEntry, main);
        Build::bind(builder, macos_arm64\ProgramEntry, main);
        Build::bind(builder, linux_arm64\ProgramEntry, main);
    }"#,
    );
    write_package(
        &client.join("library"),
        r#"machine build(builder: @mut Build) { Build::package(builder, "shared"); }"#,
    );
    let host = TargetProfile::host_if_supported().unwrap();
    let deployment = if host == TargetProfile::LinuxArm64 {
        TargetProfile::LinuxX64
    } else {
        TargetProfile::LinuxArm64
    };
    let product = open_product(
        &client.join("build.omg"),
        deployment,
        true,
        &temp.path().join("out"),
        None,
    )
    .unwrap();
    let inputs = product.package_inputs().unwrap();
    let handoffs: Vec<_> = inputs
        .dependency_build_handoffs()
        .map(|row| (row.package(), row.purpose(), row.target()))
        .collect();
    assert_eq!(handoffs.len(), 2);
    assert_eq!(handoffs[0].0, handoffs[1].0);
    assert!(handoffs.contains(&(handoffs[0].0, DependencyPurpose::Build, host)));
    assert!(handoffs.contains(&(handoffs[0].0, DependencyPurpose::Product, deployment)));
}

#[test]
fn provider_lookup_refuses_an_ambiguous_build_dependency() {
    configure();
    let temp = TemporaryDirectory::create("project-provider", false).unwrap();
    let client = temp.path().join("client");
    write_package(
        &client,
        r#"use omega\language\core\ProgramImage;
    machine build(builder: @mut Build) suspends {
        Build::application(builder, "client");
        let mut first: DependencyDescription = dependency\from_source(Source::Path { location: "one" });
        dependency\with_alias(@mut first, "backing");
        let first_registration: DependencyRegistration = Build::build_dependency(builder, first);
        let second: DependencyDescription = dependency\from_source(Source::Path { location: "two" });
        let second_registration: DependencyRegistration = Build::build_dependency(builder, second);
        suspend Build::await_provider::<ProgramImage, backing\TestImage>(builder);
    }"#,
    );
    for (directory, name) in [("one", "first_provider"), ("two", "backing")] {
        write_package(
            &client.join(directory),
            &format!(
                "machine build(builder: @mut Build) {{ Build::package(builder, \"{name}\"); }}"
            ),
        );
    }
    let result = open_product(
        &client.join("build.omg"),
        TargetProfile::host_if_supported().unwrap(),
        true,
        &temp.path().join("out"),
        None,
    );
    let Err(errors) = result else {
        panic!("two issued dependencies share the provider alias")
    };
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("Build dependency qualifier `backing` is ambiguous")),
        "{errors:?}"
    );
}

#[cfg(unix)]
#[test]
fn selecting_a_workspace_member_reuses_its_build() {
    use artifacts::compile_timings::counters::source_build_execution_count;
    use std::process::Command;

    // Give only this test process the SSH route and an isolated execution counter.
    const REMOTE: &str = "OMEGA_PROJECT_TEST_WORKSPACE";
    if let Some(remote) = std::env::var_os(REMOTE) {
        configure();
        let directory = Path::new(&remote).parent().unwrap();
        let before = source_build_execution_count();
        let product = open_product(
            &directory.join("client/build.omg"),
            TargetProfile::host_if_supported().unwrap(),
            false,
            &directory.join("out"),
            None,
        )
        .unwrap();
        assert_eq!(
            product
                .package_inputs()
                .unwrap()
                .dependency_build_handoffs()
                .count(),
            1
        );
        assert_eq!(
            source_build_execution_count() - before,
            3,
            "client, workspace, and selected member each execute once"
        );
        return;
    }

    let temp = TemporaryDirectory::create("project-workspace", false).unwrap();
    let remote = temp.path().join("workspace");
    fs::create_dir(&remote).unwrap();
    fs::write(
        remote.join("build.omg"),
        r#"machine build(builder: @mut Build) { Build::member(builder, "member"); }"#,
    )
    .unwrap();
    write_package(
        &remote.join("member"),
        r#"machine build(builder: @mut Build) { Build::package(builder, "member"); }"#,
    );
    for args in [
        &["init", "--quiet", "-b", "fixture"][..],
        &["add", "."],
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "workspace",
        ],
    ] {
        let result = Command::new("git")
            .args(args)
            .current_dir(&remote)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    write_package(
        &temp.path().join("client"),
        r#"machine build(builder: @mut Build) {
        Build::application(builder, "client");
        let member: DependencyDescription = dependency\from_source(Source::Git {
            repository: "ssh://git@omega-fixture.invalid/workspace.git", revision: "fixture",
            selection: PackageSelection::Named { package: "member" }
        });
        let registration: DependencyRegistration = Build::dependency(builder, member);
        Build::bind(builder, windows_x86_64\ProgramEntry, main);
        Build::bind(builder, linux_x86_64\ProgramEntry, main);
        Build::bind(builder, macos_arm64\ProgramEntry, main);
        Build::bind(builder, linux_arm64\ProgramEntry, main);
    }"#,
    );
    let result = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "selecting_a_workspace_member_reuses_its_build",
            "--nocapture",
        ])
        .env(REMOTE, &remote)
        .env("GIT_SSH_VARIANT", "simple")
        .env(
            "GIT_SSH_COMMAND",
            "sh -c 'exec git upload-pack \"$OMEGA_PROJECT_TEST_WORKSPACE\"'",
        )
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
