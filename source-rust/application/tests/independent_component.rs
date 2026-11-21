//! A root Build selects an entryless dependency as a separate native supplier.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use platform_custody::temporary_directory::TemporaryDirectory;
use std::{fs, process::Command};

#[test]
fn native_loads_the_separately_compiled_supplier() {
    let scratch = TemporaryDirectory::create("entryless-component", false).unwrap();
    let root = scratch.path();
    let package = root.join("supplier");
    let consumer = root.join("application");
    let output = root.join("out");
    fs::create_dir_all(&package).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    fs::write(
        package.join("build.omg"),
        r#"machine build(builder: @mut Build) { Build::package(builder, "numbers"); }"#,
    )
    .unwrap();
    fs::write(
        package.join("main.omg"),
        r#"
public boundary trait Reading {
    machine value(input: u64) -> u64 reaches none crashes none;
    machine unused() -> u64 reaches none crashes none;
}
public ConstantReading satisfies Reading {
    machine value(input: u64) -> u64 { input }
    machine unused() -> u64 { 11u64 }
}
"#,
    )
    .unwrap();
    fs::write(consumer.join("build.omg"), r#"
machine build(builder: @mut Build) {
    Build::application(builder, "number_application");
    let dependency: DependencyRegistration = Build::dependency(builder, dependency\from_source(Source::Path { location: "../supplier" }));
    Build::select_provider::<numbers\Reading, numbers\ConstantReading>(builder, linux_x86_64, CompositionMode::Independent);
    Build::bind(builder, linux_x86_64\ProgramEntry, main);
}
"#).unwrap();
    fs::write(
        consumer.join("main.omg"),
        r#"
use numbers\Reading;
machine main() reaches Reading crashes Trap {
    let number: u64 = Reading::value(73u64);
    transition number == 73u64 { true -> done(), false -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#,
    )
    .unwrap();
    let review = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args([
            "packages",
            "update",
            "--offline",
            "--target",
            "linux_x86_64",
            "--dangerously-accept-all",
            "--project",
        ])
        .arg(&consumer)
        .output()
        .unwrap();
    assert!(
        review.status.success() && consumer.join("omega.lock").is_file(),
        "the receiver must be able to review its selected supplier: {} {}",
        String::from_utf8_lossy(&review.stdout),
        String::from_utf8_lossy(&review.stderr)
    );
    let build = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--target", "linux_x86_64", "--build-dir"])
        .arg(&output)
        .arg(consumer.join("build.omg"))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let components: Vec<_> = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "so"))
        .collect();
    let [component] = components.as_slice() else {
        panic!("expected one published supplier alongside the executable: {components:?}");
    };
    assert!(component.with_extension("so.component").is_file());
    let executable = output.join("omega-program");
    let run = || {
        Command::new(&executable)
            .env_remove("LD_LIBRARY_PATH")
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let missing = component.with_extension("withheld");
    fs::rename(component, &missing).unwrap();
    let refused = run();
    assert!(
        !refused.status.success(),
        "the consumer ran with its required supplier missing"
    );
    assert!(
        String::from_utf8_lossy(&refused.stderr)
            .contains(component.file_name().unwrap().to_str().unwrap())
    );
    fs::rename(missing, component).unwrap();
}
