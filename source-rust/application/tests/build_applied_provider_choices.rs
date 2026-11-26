/*

End-to-end CLI check that a package may choose a provider whose slot and
implementation are applied to compile-time arguments, and hand that choice to
the program that depends on it. Building the executable for Linux checks that
the target has the method the consumer calls. The same package also names a
different provider application for macOS.

`Reading<const Tag: u64>` is one requirement per tag, and an implementation
`FirstReading<T, const Tag: u64>` satisfies `Reading<Tag>` for whatever tag it
is applied to. So a choice has to say which application it means:
`Build::select_provider::<Reading<Tag>, FirstReading<[Holder<u64>; 2], Tag>>`,
written once in a helper generic in `Tag` and called with `(8u64 + 9u64)`,
chooses `FirstReading<[Holder<u64>; 2], 17>` for `Reading<17>` and for no
other tag.

The consumer calls the chosen method at tag 17. The controls change the
requested tag, one argument of the package’s Build, or the provider’s result
at a different tag. The same public result bound applies to every tag.

*/

#[path = "support/project.rs"]
mod project;

const PACKAGE: &str = "public boundary trait Reading<const Tag: u64> {
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
    check_tag(name, choice, PACKAGE, 17)
}

fn check_tag(name: &str, choice: &str, package: &str, tag: u64) -> (bool, String) {
    let build = format!(
        "use main.omg as app;
machine build(builder: @mutable Build) {{
    Build::package(builder, \"reading\");
    configure::<(8u64 + 9u64)>(builder);
}}
machine configure<const Tag: u64>(builder: @mutable Build) {{
    {choice}
}}
"
    );
    let main = format!(
        "use {{reading}}/main.omg as provider;
machine main() reaches |provider|::Reading<{tag}u64> crashes none {{
    let result: u64 = |provider|::Reading::<{tag}u64>::total();
}}
"
    );
    check_package(name, &build, &[("main.omg", package)], &main)
}

fn check_package(name: &str, build: &str, files: &[(&str, &str)], main: &str) -> (bool, String) {
    let root = project::stage(
        name,
        &[
            ("reading/build.omg", build),
            ("program/main.omg", main),
            (
                "program/build.omg",
                "use main.omg as app;
use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
    Build::application(builder, \"reader\");
    let registration: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: \"../reading\" }));
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
}
",
            ),
        ],
    );
    project::write(&root.path().join("reading"), files);
    project::status(&project::omega(
        &root.path().join("program"),
        &["--target", "linux_x86_64", "build.omg"],
    ))
}

#[test]
fn a_package_chooses_an_applied_provider_and_its_consumer_receives_the_choice() {
    let (compiled, stderr) = check(
        "chosen",
        "Build::select_provider::<|app|::Reading<Tag>, |app|::FirstReading<[|app|::Holder<u64>; 2], Tag>>(builder, linux_x86_64);
        Build::select_provider::<|app|::Reading<Tag>, |app|::FirstReading<[|app|::Holder<u32>; 2], Tag>>(builder, macos_arm64);",
    );
    assert!(compiled, "{stderr}");
}

#[test]
fn a_consumer_cannot_use_the_choice_for_a_different_tag() {
    let (compiled, stderr) = check_tag(
        "unselected_tag",
        "Build::select_provider::<|app|::Reading<Tag>, |app|::FirstReading<[|app|::Holder<u64>; 2], Tag>>(builder, linux_x86_64);",
        PACKAGE,
        29,
    );
    assert!(!compiled, "{stderr}");
    assert!(
        stderr.contains("Reading::total") && stderr.contains("has no provider"),
        "{stderr}"
    );
}

#[test]
fn the_implementation_owes_its_result_bound_at_unselected_tags() {
    let package = PACKAGE.replace("_ -> 1129u64", "_ -> 0u64");
    let (compiled, stderr) = check_tag(
        "unselected_branch",
        "Build::select_provider::<|app|::Reading<Tag>, |app|::FirstReading<[|app|::Holder<u64>; 2], Tag>>(builder, linux_x86_64);",
        &package,
        17,
    );
    assert!(!compiled, "{stderr}");
    assert!(
        stderr.contains("cannot prove ensures contract") && stderr.contains("total"),
        "{stderr}"
    );
}

#[test]
fn a_provider_applied_to_another_tag_than_its_slot_is_rejected() {
    let (compiled, stderr) = check(
        "crossed",
        "Build::select_provider::<|app|::Reading<Tag>, |app|::FirstReading<[|app|::Holder<u64>; 2], 29u64>>(builder, linux_x86_64);",
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
        "Build::select_provider::<|app|::Reading<Tag, Tag>, |app|::FirstReading<[|app|::Holder<u64>; 2], Tag>>(builder, linux_x86_64);",
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
        "Build::select_provider::<|app|::Reading<17u32>, |app|::FirstReading<[|app|::Holder<u64>; 2], Tag>>(builder, linux_x86_64);",
    );
    assert!(!compiled);
    assert!(
        stderr.contains("the value parameter `Tag` of type `u64` the literal `17`"),
        "{stderr}"
    );
}

#[test]
fn a_package_selects_its_slot_and_provider_from_separate_files() {
    let (compiled, stderr) = check_package(
        "file_operands",
        r#"use slot.omg;
use provider.omg;
use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
    Build::package(builder, "reading");
    let kit_registration: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "kit" }));
    Build::select_provider::<|slot|::Reading, |provider|::Selected>(builder, linux_x86_64);
}"#,
        &[
            ("main.omg", ""),
            (
                "slot.omg",
                "public boundary trait Reading { machine total() -> u64 reaches none crashes none; }",
            ),
            (
                "provider.omg",
                "use slot.omg; use {kit}/math.omg as math; Selected satisfies |slot|::Reading { machine total() -> u64 { |math|::answer() } }",
            ),
            (
                "kit/build.omg",
                r#"machine build(builder: @mutable Build) { Build::package(builder, "kit"); }"#,
            ),
            ("kit/math.omg", "public machine answer() -> u64 { 42 }"),
        ],
        "machine main() { }\n",
    );
    assert!(compiled, "{stderr}");
}

#[test]
fn a_dependency_retains_the_domain_used_in_each_provider_choice() {
    let (compiled, report) = check_package(
        "qualified_inputs",
        r#"use main.omg as app;
machine build(builder: @mutable Build) {
    Build::package(builder, "reading");
    Build::select_provider::<|app|::Reading<|app|::Tag in |app|::Tag::Left>, |app|::First>(builder, linux_x86_64);
    Build::select_provider::<|app|::Reading<|app|::Tag in |app|::Tag::Right>, |app|::Second>(builder, linux_x86_64);
}"#,
        &[(
            "main.omg",
            r#"public data Tag {}
public domain Tag::Left;
public domain Tag::Right;
public boundary trait Reading<T> { machine value() -> u32 reaches none crashes none; }
First satisfies Reading<Tag in Tag::Left> { machine value() -> u32 { 29u32 } }
Second satisfies Reading<Tag in Tag::Right> { machine value() -> u32 { 37u32 } }
public machine right() -> u32 { Reading::<Tag in Tag::Right>::value() }"#,
        )],
        r#"use {reading}/main.omg as provider;
machine main() crashes Trap {
    transition |provider|::right() == 37u32 { true -> done(), _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}"#,
    );
    assert!(compiled, "{report}");
}
