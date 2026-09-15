use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, compile, compile_reviewed_repository_fixture,
    compile_rooted_canary_for_native_host, executable_name, fs, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn runtime_numeric_conversion_surface_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::NUMERIC_CONVERSION_SURFACE);
    let build_dir =
        std::env::temp_dir().join(format!("omega-numeric-conversion-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("unsigned numeric conversion surface should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("unsigned numeric conversion surface should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected exact/wrapping/saturating/trapping/widening unsigned conversions to agree; \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_i64_to_u64_exact_guard_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_I64_TO_U64_EXACT_GUARD_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-i64-u64-exact-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("guarded dynamic i64-to-u64 exact conversion should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("guarded i64-to-u64 canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("guarded dynamic i64-to-u64 exact conversion should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected guarded dynamic i64-to-u64 exact conversion to preserve 32; got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_numeric_signed_conversion_surface_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::NUMERIC_SIGNED_CONVERSION_SURFACE);
    let build_dir =
        std::env::temp_dir().join(format!("omega-numeric-signed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("signed numeric conversion surface should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("signed numeric conversion surface should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed exact/wrapping/saturating/trapping/widening conversions to agree; \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn numeric_trapping_conversion_overflow_aborts() {
    let canary = pass_canary(fixture_roster::NUMERIC_TRAPPING_CONVERSION_OVERFLOW);
    let build_dir = std::env::temp_dir().join(format!("omega-numeric-trap-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("trapping numeric conversion should compile from its authored root");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("trapping numeric conversion should run");
    assert!(
        !output.status.success() && output.status.code() != Some(7),
        "expected out-of-range narrowing to trap before returning; got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("trapping numeric conversion should reach checked trees");
    let outcome = interpret(&checked, &[]);
    assert!(
        outcome
            .error
            .as_deref()
            .is_some_and(|reason| reason.contains("arithmetic overflow in Trapping domain")),
        "interpreter must report the same conversion trap, got {:?}",
        outcome.error
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_numeric_cross_signed_conversion_surface_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::NUMERIC_CROSS_SIGNED_CONVERSION_SURFACE);
    let build_dir =
        std::env::temp_dir().join(format!("omega-numeric-cross-signed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("cross-signed numeric conversion surface should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("cross-signed numeric conversion surface should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both signedness directions and all explicit policies to agree; \
         got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("cross-signed surface should reach checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter must agree on cross-signed conversions, got {:?}",
        outcome.error
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn unsigned_to_signed_saturation_checks_and_interprets_boundary_values() {
    let directory =
        std::env::temp_dir().join(format!("omega-u64-i64-saturation-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("create saturation fixture");
    for value in [
        0,
        1,
        5_000_000_000,
        i64::MAX as u64 - 1,
        i64::MAX as u64,
        i64::MAX as u64 + 1,
        u64::MAX,
    ] {
        let expected = value.min(i64::MAX as u64);
        fs::write(
            directory.join("main.omg"),
            format!(
                r#"
use omega::language::core::numeric_conversion;
data Main {{}}
machine Main::main(&mut self) -> i32 {{
    let source: u64 = {value};
    let converted: i64 = narrow_u64_to_i64_saturating(source);
    transition converted == {expected} {{
        true -> (70)
        _ -> (71)
    }}
}}
"#
            ),
        )
        .expect("write saturation fixture");
        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &directory.join("main.omg"),
            None,
        ))
        .unwrap_or_else(|diagnostics| panic!("{value}: {diagnostics:#?}"));
        let outcome = interpret(&checked, &[]);
        assert!(outcome.error.is_none(), "{value}: {:?}", outcome.error);
        assert_eq!(
            outcome.exit_code, 70,
            "{value} should saturate to {expected}"
        );
    }
    fs::remove_dir_all(directory).expect("remove saturation fixture");
}

#[test]
fn numeric_cross_signed_trapping_conversions_abort() {
    for &(name, label) in fixture_roster::CROSS_SIGNED_TRAP_PASS_CANARIES {
        let canary = pass_canary(name);
        let build_dir = std::env::temp_dir().join(format!(
            "omega-numeric-cross-trap-{}-{}",
            label.replace(' ', "-"),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);

        compile_rooted_canary_for_native_host(&canary, build_dir.clone()).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "{label} trapping conversion should compile:\n{}",
                    diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            },
        );

        let output = Command::new(build_dir.join(executable_name()))
            .output()
            .unwrap_or_else(|error| panic!("{label} trapping conversion should run: {error}"));
        assert!(
            !output.status.success() && output.status.code() != Some(7),
            "{label} must trap before returning; got {:?}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );

        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &canary.join("main.omg"),
            None,
        ))
        .unwrap_or_else(|_| panic!("{label} should reach checked trees"));
        let outcome = interpret(&checked, &[]);
        assert!(
            outcome
                .error
                .as_deref()
                .is_some_and(|reason| reason.contains("arithmetic overflow in Trapping domain")),
            "interpreter must report the same {label} trap, got {:?}",
            outcome.error
        );

        let _ = fs::remove_dir_all(&build_dir);
    }
}
