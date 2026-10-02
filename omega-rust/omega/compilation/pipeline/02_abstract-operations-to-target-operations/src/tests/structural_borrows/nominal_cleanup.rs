//! A checked named cleanup borrows the original by-value parameter home.
use super::source_plan;
use abstract_operations::AbstractOperation;
use target::NativeTarget;
use target_operations::{CallSiteOwner, TargetUnitOperation};
use terminal_psi::{StructuralAccess, TerminalAffineCleanupAction};

#[test]
fn named_nominal_cleanup_borrows_the_owned_parameter_on_its_exact_edge() {
    let source = source_plan(
        r#"
            data Token { ready: bool; }
            machine Token::drop(subject: &mut Token) { subject.ready = false; }
            data Main {}
            machine Main::run(subject: Token) {}
        "#,
    );
    let (caller, edge, cleanup) = source
        .functions
        .iter()
        .find_map(|function| {
            function.operations.iter().find_map(|operation| {
                let AbstractOperation::ReturnUnit {
                    psi_edge,
                    cleanup_actions,
                } = operation
                else {
                    return None;
                };
                let [TerminalAffineCleanupAction::InvokeNominal(cleanup)] =
                    cleanup_actions.as_slice()
                else {
                    return None;
                };
                Some((function.machine, *psi_edge, cleanup))
            })
        })
        .expect("source-produced nominal cleanup");
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("owned arrival lends its existing home");
        crate::validate_abstract_to_target_translation(&source, native, &target).unwrap();
        let function = target
            .functions
            .iter()
            .find(|function| function.machine == caller)
            .unwrap();
        let row = function
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|row| matches!(row, TargetUnitOperation::CleanupCall { .. }))
            .unwrap();
        let TargetUnitOperation::CleanupCall {
            owner,
            callee,
            arguments,
            call_plan,
        } = row
        else {
            unreachable!()
        };
        assert_eq!(
            *owner,
            CallSiteOwner::CleanupAction {
                edge,
                action_ordinal: 0
            }
        );
        assert_eq!(*callee, cleanup.cleanup_machine);
        assert_eq!(arguments.len(), 1);
        assert_eq!(arguments[0].place, cleanup.place);
        assert_eq!(arguments[0].access, StructuralAccess::MutableBorrow);
        assert!(arguments[0].path.is_empty());
        assert_eq!(call_plan.parameters.len(), 1);
        assert!(call_plan.result.is_none());
        // This stage's receipt checks function and ABI rosters. Exact call
        // action/order refusals belong to the independent legalization replay.
    }
}
