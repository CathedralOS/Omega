/*

End-to-end CLI check that a Build can call code from a build dependency it
prepared (`omega --check`).

A `build.omg` registers a build dependency, waits for it, and then calls it:

    use kit;

    machine build(builder: &mut Build)
    suspends
    {
        Build::application(builder, "staged");
        builder.freestanding = true;
        ... registered = Build::build_dependency(builder, request aliased "kit");

        suspend Build::prepare_dependency(builder, registered);
        let seven: i32 = kit::v();
        ...
    }

The statements after the wait use a package that does not exist for the Build
until the wait is over. `wiki/spec/build/scoped_execution.md` ("Checked Build
stages") says how that is checked: "Execute only a completely checked and
admitted prefix", and "After preparation, the compiler resolves and checks
the dependent continuation before it executes." So the Build is checked and
run in stages, one per top-level wait. The first stage is everything up to
and including the wait. Once the dependency is prepared, the rest is
resolved with `kit` selectable, checked, and run against the same Build: the
same `builder`, the same registrations, nothing run twice.

The first program does exactly that, and binds its entry only if `kit::v()`
returned what the dependency declares (7) and `builder.freestanding`, set
before the wait, is still set after it. Its other arm declares a second
project role, which a Build is refused for. So the program compiles only if
the call went through the dependency, the setting was carried, and the
statements before the wait ran once.

The others are the same project with a line changed:

- the dependency returns 8: the other arm is taken and refused, so the value
  really came from the dependency;
- a statement after the wait uses a local bound before it: refused as not
  supported yet, since only the entry's parameters are carried across a wait;
- the call is written before the wait: `kit` names nothing there, and the
  Build is rejected before any of it runs;
- the statements after the wait call a dependency registered for the Product
  (`lib`): a Product dependency is not selectable from a Build;
- they call a machine of `kit` that is not public: not selectable either.

*/

use std::process::Command;

/// `omega --check` on a project whose Build registers `dep/` as the build
/// dependency `kit`. `before` runs before the Build waits for `kit`, `after`
/// runs after it, and `value` is what the dependency's `v()` returns.
fn check(name: &str, before: &str, after: &str, value: i32) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-staged-dependency-calls-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("dep")).expect("staging directory");
    std::fs::write(root.join("main.omg"), "machine main() { }\n").expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        format!(
            "use kit;

machine build(builder: &mut Build)
suspends
{{
    Build::application(builder, \"staged\");
    builder.freestanding = true;
    let mut kit_request: DependencyDescription = dependency::from_source(Source::Path {{ location: \"dep\" }});
    dependency::with_alias(&mut kit_request, \"kit\");
    let kit_registered: DependencyRegistration = Build::build_dependency(builder, kit_request);
{before}
    suspend Build::prepare_dependency(builder, kit_registered);
{after}
    machine carried(builder: &mut Build) {{
        Build::bind(builder, linux_x86_64::ProgramEntry, main);
    }}
    machine wrong(builder: &mut Build) {{
        Build::application(builder, \"second_role\");
    }}
}}
"
        ),
    )
    .expect("staged Build");
    std::fs::write(
        root.join("dep/build.omg"),
        "machine build(builder: &mut Build) {
    Build::package(builder, \"kit_package\");
}
",
    )
    .expect("staged dependency Build");
    std::fs::write(
        root.join("dep/main.omg"),
        format!(
            "public machine v() -> i32 {{ {value} }}
machine hidden() -> i32 {{ 7 }}
"
        ),
    )
    .expect("staged dependency source");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("--check")
        .arg("main.omg")
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

/// The statements after the wait: call `callee`, and bind the entry only if
/// it returned 7 and the setting made before the wait is still set.
fn after(callee: &str) -> String {
    format!(
        "    let seven: i32 = {callee}();
    transition seven == 7 && builder.freestanding {{
        true -> carried(builder)
        _ -> wrong(builder)
    }}"
    )
}

#[test]
fn a_build_calls_the_dependency_it_prepared_and_keeps_what_it_did_before() {
    let (compiled, stderr) = check("called", "", &after("kit::v"), 7);
    assert!(compiled, "{stderr}");
}

#[test]
fn the_value_after_the_wait_is_the_one_the_dependency_returns() {
    let (compiled, stderr) = check("other-value", "", &after("kit::v"), 8);
    assert!(!compiled);
    assert!(stderr.contains("project role"), "{stderr}");
}

#[test]
fn a_local_bound_before_the_wait_is_not_carried_across_it() {
    let (compiled, stderr) = check(
        "captured-local",
        "    let before: i32 = 7;",
        "    let seven: i32 = before;
    transition seven == 7 {
        true -> carried(builder)
        _ -> wrong(builder)
    }",
        7,
    );
    assert!(!compiled);
    assert!(
        stderr.contains("not supported yet") && stderr.contains("`before`"),
        "{stderr}"
    );
}

#[test]
fn a_call_written_before_the_wait_names_nothing() {
    let (compiled, stderr) = check(
        "used-before",
        "    let early: i32 = kit::v();",
        &after("kit::v"),
        7,
    );
    assert!(!compiled);
    assert!(
        stderr.contains("names a value nothing the program declares"),
        "{stderr}"
    );
}

#[test]
fn a_product_dependency_is_not_selectable_from_the_build() {
    let (compiled, stderr) = check(
        "product-dependency",
        "    let mut lib_request: DependencyDescription = dependency::from_source(Source::Path { location: \"dep\" });
    dependency::with_alias(&mut lib_request, \"lib\");
    let lib_registered: DependencyRegistration = Build::dependency(builder, lib_request);",
        &after("lib::v"),
        7,
    );
    assert!(!compiled);
    assert!(
        stderr.contains("names a value nothing the program declares"),
        "{stderr}"
    );
}

#[test]
fn a_private_machine_of_the_dependency_is_not_selectable() {
    let (compiled, _) = check("private", "", &after("kit::hidden"), 7);
    assert!(!compiled);
}
