use super::*;
use provider_planning::plans::CompilerIntrinsicExecutionIdentity;

#[test]
fn hosted_byte_catalog_retains_exact_target_and_provider_custody() {
    let root = pass_canary(fixture_roster::RUNTIME_ADAPTER_FORWARDING_EXIT).join("main.omg");
    for target in ["linux_x86_64", "linux_arm64", "macos_arm64"] {
        let checked = compile_to_checked(&root, Some(target))
            .expect("the target-selected Console provider checks");
        let (plan, retained) = checked
            .selected_provider_plans()
            .plans()
            .iter()
            .zip(checked.selected_provider_provenance())
            .find(|(plan, _)| plan.schema.trait_name == "Console")
            .expect("one selected Console plan");
        assert_eq!(plan.target, target);
        let byte_row = plan
            .rows
            .iter()
            .position(|row| row.method == "write_byte")
            .unwrap();
        assert_eq!(
            retained.row_compiler_intrinsic_executions[byte_row],
            Some(CompilerIntrinsicExecutionIdentity::HostedWriteByteI32),
            "{target}"
        );

        // A matching symbol or signature without the accepted package binding
        // must not manufacture the retained compiler-owned realization.
        for requested_target in [None, Some(target), Some("windows_x86_64")] {
            let projected =
                selected_dispatch::derive_selected_compiler_intrinsic_execution_identity_for_row(
                    &checked,
                    plan,
                    retained.provider.schema,
                    &plan.rows[byte_row],
                    retained.provider.row_requirements[byte_row],
                    retained.provider.row_realizations[byte_row],
                    requested_target,
                )
                .unwrap();
            assert_eq!(
                projected,
                Some(selected_dispatch::SelectedCompilerIntrinsicExecutionIdentity::Unsupported)
            );
        }
        if target == "macos_arm64" {
            for method in ["exit_process", "read_byte", "read_line"] {
                let row = plan
                    .rows
                    .iter()
                    .position(|row| row.method == method)
                    .unwrap();
                assert_eq!(
                    retained.row_compiler_intrinsic_executions[row], None,
                    "byte output does not admit the separate {method} catalog role"
                );
            }
        }
    }
}
