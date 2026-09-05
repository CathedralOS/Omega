use omega_compiler::compile_to_checked;
use omega_optimization_core::OptimizationReportRequest;
use psi_checked_interpreter::interpret_entry;

use super::support::{
    HOSTED_NATIVE_TARGETS, diagnostic_snapshots, fail_canary, interpreter_canary,
};

#[test]
fn source_acceptance_diagnostics_and_interpreter_are_stable_on_every_target() {
    let pass = interpreter_canary();
    let fail = fail_canary();
    let expected_message = std::fs::read_to_string(fail.join("expected.txt"))
        .expect("failure canary owns an expected diagnostic")
        .trim()
        .to_owned();
    let expected_diagnostics = [format!("Error|{expected_message}|none")];

    for target in HOSTED_NATIVE_TARGETS {
        let checked = compile_to_checked(&pass.join("main.omg"), Some(target)).unwrap_or_else(
            |diagnostics| {
                panic!(
                    "no-selection source acceptance changed for {target}: {:#?}",
                    diagnostic_snapshots(&diagnostics)
                )
            },
        );
        assert!(checked.optimization_selections().is_empty());
        assert_eq!(
            checked.optimization_report_request(),
            OptimizationReportRequest::Suppressed
        );
        assert_eq!(
            checked.optimization_selection_identity(),
            checked.optimization_selections().identity()
        );

        let interpreted = interpret_entry(&checked, "Main::main", &[]);
        assert_eq!(interpreted.exit_code, 70, "{target}");
        assert_eq!(interpreted.stdout, b"ABC\n", "{target}");
        assert!(interpreted.stderr.is_empty(), "{target}");
        assert_eq!(interpreted.error, None, "{target}");

        let diagnostics = compile_to_checked(&fail.join("main.omg"), Some(target))
            .expect_err("the no-selection rejection canary must remain rejected");
        assert_eq!(
            diagnostic_snapshots(&diagnostics),
            expected_diagnostics,
            "{target}"
        );
    }
}
