/*

End-to-end CLI check that a Build helper may be given compile-time values
(`omega --check`).

A Build often keeps its choices in a helper and tells the helper what to do
through values known while compiling: `configure<true, (8u64 + 9u64)>(builder)`
passes a Boolean and a number written as a calculation. The helper is a
generic machine, so the compiler makes the one machine that application
selects, with `true` and `17` in place of the parameters, and the Build runs
that machine.

The first program does this and makes a provider choice inside the helper; it
must compile, and the choice must count: a second, different choice for the
same slot and target is then a conflict. The twins change one thing each. A
helper that requires `Tag == 17u64` and is given `(8u64 + 8u64)` is rejected
with the value the calculation has, `16`. A calculation that mixes two integer
types has no single value and stays refused, never accepted.

*/

use std::process::Command;

const PRODUCT: &str = "boundary trait Service { machine value() -> u64 reaches none crashes none; }
Provider satisfies Service { machine value() -> u64 { 7 } }
Other satisfies Service { machine value() -> u64 { 8 } }
machine main() reaches Service { let seen: u64 = Service::value(); }
";

const HELPER: &str = "machine configure<const Flag: bool, const Tag: u64>(builder: &mut Build)
requires Tag == 17u64
{
    Build::select_provider<Service, Provider>(builder, linux_x86_64);
}
";

/// Checks a staged project whose Build runs `body` after naming the
/// application, for the one target the Build binds an entry on.
fn check(name: &str, body: &str) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-static-arguments-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staging directory");
    std::fs::write(root.join("main.omg"), PRODUCT).expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) {{
    Build::application(builder, \"configured\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
{body}}}
{HELPER}"
        ),
    )
    .expect("staged Build");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--check", "main.omg", "--target", "linux_x86_64"])
        .current_dir(&root)
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
fn a_build_helper_is_given_a_boolean_and_a_calculated_number() {
    // The same number written four ways meets the helper's requirement.
    for (name, tag) in [
        ("typed", "(8u64 + 9u64)"),
        ("literal", "17u64"),
        ("anonymous", "(20 - 3)"),
        ("landed", "(8u64 + 9)"),
    ] {
        let (compiled, stderr) = check(name, &format!("    configure<true, {tag}>(builder);\n"));
        assert!(compiled, "{tag}: {stderr}");
    }
}

#[test]
fn the_choice_a_configured_helper_makes_is_a_choice_of_the_build() {
    let (compiled, stderr) = check(
        "counted",
        "    configure<false, (8u64 + 9u64)>(builder);
    Build::select_provider<Service, Other>(builder, linux_x86_64);
",
    );
    assert!(!compiled);
    assert!(
        stderr.contains("conflicting providers on target `linux_x86_64`")
            && stderr.contains("`Provider` and `Other`"),
        "{stderr}"
    );
}

#[test]
fn a_calculated_number_is_held_to_the_helper_s_requirement_by_its_value() {
    let (compiled, stderr) = check("short", "    configure<true, (8u64 + 8u64)>(builder);\n");
    assert!(!compiled);
    assert!(
        stderr.contains("cannot prove requires contract for call configure")
            && stderr.contains("16 == 17"),
        "{stderr}"
    );
}

#[test]
fn a_calculation_with_no_single_value_is_refused_and_never_accepted() {
    for (name, argument) in [
        // Two integer types: no carrier the calculation lands in.
        ("mixed", "(8u64 + 9u32)"),
        // The sum does not fit the type its operands state.
        ("wide", "(18446744073709551615u64 + 1u64)"),
    ] {
        let (compiled, stderr) = check(
            name,
            &format!("    configure<true, {argument}>(builder);\n"),
        );
        assert!(!compiled, "{argument}");
        assert!(stderr.contains("not supported yet"), "{argument}: {stderr}");
    }
}
