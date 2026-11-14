/*

End-to-end CLI check of a const argument written as a machine call, in a
Build (`omega --check`).

A type can take a number known while compiling, `Buffer<4>`, and the number
may be computed: `Buffer<capacity()>` asks the compiler to run `capacity()`
and apply `Buffer` at its value. generics.md, "Transparent type aliases":
"Closed argument expressions may execute ordinary admitted compile-time code.
A computed const value retains its declared carrier and canonical value".

Each program here writes such an argument in its Build and is wrong in one
way, so the compiler must reach the call and say what is wrong with it: a
call returning another integer type than the parameter declares, a call
whose value is negative where the value is an array length, and a call of a
machine that reaches a runtime service and so has no compile-time value.
The accepted case, where the value becomes the literal of the application,
is pinned in `build-time-evaluation` (`a_const_argument_written_as_a_call_`
`becomes_the_literal_of_its_value`): a Build that declares generic data is
still refused afterwards for the declaration itself.

*/

use std::process::Command;

const PRODUCT: &str = "machine main() { let marker: u8 = 0; }\n";

/// Checks a staged project whose Build declares `declarations` and holds a
/// local of the type `held`.
fn check(name: &str, declarations: &str, held: &str) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-const-argument-calls-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staging directory");
    std::fs::write(root.join("main.omg"), PRODUCT).expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        format!(
            "{declarations}
machine build(builder: &mut Build) {{
    Build::application(builder, \"computed\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    let held: {held};
}}
"
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
fn a_call_returning_another_integer_type_is_rejected_for_its_type() {
    let (compiled, stderr) = check(
        "narrow",
        "data Buffer<const Capacity: u64> { slots: [u8; Capacity]; }
machine narrow() -> u8 { 4 }",
        "Buffer<narrow()>",
    );
    assert!(!compiled);
    assert!(
        stderr.contains(
            "const argument for `Capacity` of `Buffer` must be a `u64`, found a call that \
             returns a `u8`"
        ),
        "{stderr}"
    );
}

#[test]
fn a_negative_value_is_no_array_length() {
    let (compiled, stderr) = check(
        "negative",
        "data Signed<const Capacity: i64> { slots: [u8; Capacity]; }
machine below() -> i64 { 0i64 - 1i64 }",
        "Signed<below()>",
    );
    assert!(!compiled);
    assert!(
        stderr.contains("const argument for `Capacity` of `Signed`")
            && stderr.contains("an applied array length is not a closed nonnegative integer"),
        "{stderr}"
    );
}

#[test]
fn a_call_that_reaches_a_runtime_service_has_no_compile_time_value() {
    let (compiled, stderr) = check(
        "service",
        "data Buffer<const Capacity: u64> { slots: [u8; Capacity]; }
boundary trait Clock { machine now() -> u64 reaches none crashes none; }
machine observed() -> u64 reaches Clock { Clock::now() }",
        "Buffer<observed()>",
    );
    assert!(!compiled);
    assert!(
        stderr.contains("const argument for `Capacity` of `Buffer`"),
        "{stderr}"
    );
}
