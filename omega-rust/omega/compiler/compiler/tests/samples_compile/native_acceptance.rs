//! Runtime fixture acceptance is explicit, target-bound and separate from checking.
use super::*;

fn byte_identity(
    checked: &compiler::CheckedCompilation,
) -> Option<effects::CompilerIntrinsicExecutionIdentity> {
    let (plan, retained) = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .zip(checked.selected_provider_provenance())
        .find(|(plan, _)| plan.schema.trait_name == "Console")
        .unwrap();
    let row = plan
        .rows
        .iter()
        .position(|row| row.method == "write_byte")
        .unwrap();
    retained.row_compiler_intrinsic_executions[row]
}

#[test]
fn native_sample_console_acceptance_binds_the_exact_selected_target() {
    let root = repo_root().join("samples/cli/basics/cli_mvp/main.omg");
    for target in ["linux_x86_64", "linux_arm64", "macos_arm64"] {
        let unaccepted =
            compile_to_checked_with_packages(&root, Some(target), sample_package_inputs(&root))
                .unwrap();
        assert_eq!(byte_identity(&unaccepted), None);
        let accepted = sample_native_package_inputs(&root, Some(target)).unwrap();
        assert_eq!(accepted.accepted_semantic_bindings().count(), 1);
        let checked = compile_to_checked_with_packages(&root, Some(target), accepted).unwrap();
        assert_eq!(
            byte_identity(&checked),
            Some(effects::CompilerIntrinsicExecutionIdentity::HostedWriteByteI32)
        );
    }
    let linux_binding = sample_native_package_inputs(&root, Some("linux_arm64")).unwrap();
    assert!(
        compile_to_checked_with_packages(&root, Some("macos_arm64"), linux_binding).is_err(),
        "acceptance of another selected target must not authorize this plan"
    );
}

#[test]
fn native_sample_without_standard_library_does_not_gain_console_acceptance() {
    let root = repo_root().join("samples/uefi/uefi_hello/main.omg");
    assert_eq!(
        sample_native_package_inputs(&root, Some("uefi_x86_64")).unwrap(),
        sample_package_inputs(&root)
    );
}
