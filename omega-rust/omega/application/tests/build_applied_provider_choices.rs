/*

End-to-end CLI check that a package may choose a provider whose slot and
implementation are applied to compile-time arguments, and hand that choice to
the program that depends on it (`omega --check`).

`Reading<const Tag: u64>` is one requirement per tag, and an implementation
`FirstReading<T, const Tag: u64>` satisfies `Reading<Tag>` for whatever tag it
is applied to. So a choice has to say which application it means:
`Build::select_provider<Reading<Tag>, FirstReading<[Holder<u64>; 2], Tag>>`,
written once in a helper generic in `Tag` and called with `(8u64 + 9u64)`,
chooses `FirstReading<[Holder<u64>; 2], 17>` for `Reading<17>` and for no
other tag.

The first test stages that package and a program depending on it; both Builds
must complete and the program must compile. The twins change one thing each
in the package's Build and must be rejected there: a provider applied to a
different tag than the slot, an application with one argument too many, and a
tag written as a literal of another integer type.

*/

use std::process::Command;

const PACKAGE: &str = "module main;
public boundary trait Reading<const Tag: u64> {
    machine total() -> u64 reaches none crashes none ensures result >= 1000u64;
}
FirstReading<T, const Tag: u64> satisfies Reading<Tag> {
    machine total() -> u64 ensures result >= 1000u64 { match Tag { 17u64 -> 1100u64, _ -> 1129u64 } }
}
data Holder<T> { value: T; }
";

/// Checks a staged program that depends on a staged package whose Build
/// helper, generic in `Tag` and called at `(8u64 + 9u64)`, runs `choice`.
fn check(name: &str, choice: &str) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-applied-provider-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let package = root.join("reading");
    let program = root.join("program");
    std::fs::create_dir_all(&package).expect("staged package directory");
    std::fs::create_dir_all(&program).expect("staged program directory");
    std::fs::write(package.join("main.omg"), PACKAGE).expect("staged package");
    std::fs::write(
        package.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) {{
    Build::package(builder, \"reading\");
    configure<(8u64 + 9u64)>(builder);
}}
machine configure<const Tag: u64>(builder: &mut Build) {{
    {choice}
}}
"
        ),
    )
    .expect("staged package Build");
    std::fs::write(program.join("main.omg"), "machine main() { }\n").expect("staged program");
    std::fs::write(
        program.join("build.omg"),
        "machine build(builder: &mut Build) {
    Build::application(builder, \"reader\");
    let registration: DependencyRegistration = Build::dependency(builder, dependency::from_source(Source::Path { location: \"../reading\" }));
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
}
",
    )
    .expect("staged program Build");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--check", "main.omg", "--target", "linux_x86_64"])
        .current_dir(&program)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn a_package_chooses_an_applied_provider_and_its_consumer_receives_the_choice() {
    let (compiled, stderr) = check(
        "chosen",
        "Build::select_provider<Reading<Tag>, FirstReading<[Holder<u64>; 2], Tag>>(builder, linux_x86_64);",
    );
    assert!(compiled, "{stderr}");
}

#[test]
fn a_provider_applied_to_another_tag_than_its_slot_is_rejected() {
    let (compiled, stderr) = check(
        "crossed",
        "Build::select_provider<Reading<Tag>, FirstReading<[Holder<u64>; 2], 29u64>>(builder, linux_x86_64);",
    );
    assert!(!compiled);
    assert!(
        stderr.contains(
            "provider `FirstReading` does not satisfy the selected application of `Reading`"
        ),
        "{stderr}"
    );
}

#[test]
fn an_application_with_the_wrong_number_of_arguments_is_rejected() {
    let (compiled, stderr) = check(
        "excess",
        "Build::select_provider<Reading<Tag, Tag>, FirstReading<[Holder<u64>; 2], Tag>>(builder, linux_x86_64);",
    );
    assert!(!compiled);
    assert!(
        stderr.contains("`Reading` takes 1 static argument(s) and this provider choice gives it 2"),
        "{stderr}"
    );
}

#[test]
fn a_tag_stated_in_another_integer_type_is_rejected() {
    let (compiled, stderr) = check(
        "narrow",
        "Build::select_provider<Reading<17u32>, FirstReading<[Holder<u64>; 2], Tag>>(builder, linux_x86_64);",
    );
    assert!(!compiled);
    assert!(
        stderr.contains("the value parameter `Tag` of type `u64` the literal `17`"),
        "{stderr}"
    );
}
