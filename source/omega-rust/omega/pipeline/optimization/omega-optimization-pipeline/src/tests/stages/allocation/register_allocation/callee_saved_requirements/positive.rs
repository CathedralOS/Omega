use crate::tests::*;

use super::fixture::{call_homes, ordinary_homes, stage, wide_budget};

#[test]
fn scalar_calls_report_exact_callee_saved_writes_and_replay_deterministically() {
    for (target, abi) in [
        (
            NativeTarget::linux_x64(),
            FrameAbiPreservationConvention::SystemVAMD64,
        ),
        (
            NativeTarget::linux_arm64(),
            FrameAbiPreservationConvention::Aapcs64,
        ),
    ] {
        let source = call_homes(target);
        let first = stage(&source, wide_budget()).unwrap();
        let repeated = stage(&source, wide_budget()).unwrap();
        assert_eq!(first, repeated);
        assert_eq!(first.receipt().selected(), source.custody().selected());
        assert_eq!(first.receipt().homes(), source.custody().homes());
        assert_eq!(
            first.receipt().post_allocation_manifest(),
            source.custody().post_allocation_manifest()
        );
        assert_eq!(first.receipt().target(), target);
        assert_eq!(first.receipt().abi(), abi);
        assert_eq!(
            first.receipt().identity(),
            allocated_callee_saved_requirement_identity(first.plan())
        );
        assert!(first.receipt().modified_function_count() > 0);
        assert!(first.receipt().modified_unit_count() > 0);
        assert!(first.receipt().witness_count() > 0);

        let witnesses = first
            .plan()
            .functions
            .iter()
            .flat_map(|function| &function.modified_units)
            .flat_map(|requirement| &requirement.witnesses)
            .collect::<Vec<_>>();
        assert!(witnesses.iter().any(|witness| matches!(
            witness,
            CalleeSavedModificationWitness::OperandDefinition {
                virtual_register: VirtualRegisterId(5),
                ..
            }
        )));
        assert!(!witnesses.iter().any(|witness| matches!(
            witness,
            CalleeSavedModificationWitness::ImplicitClobber { .. }
        )));
        assert!(
            first
                .plan()
                .functions
                .iter()
                .flat_map(|function| &function.modified_units)
                .all(|requirement| first.plan().callee_saved_units.contains(&requirement.unit))
        );

        let replayed =
            validate_allocated_callee_saved_requirements(&source, first.plan().clone()).unwrap();
        assert_eq!(replayed, first);
    }
}

#[test]
fn five_native_targets_retain_exact_empty_requirements_when_no_preserved_home_is_written() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = ordinary_homes(target);
        let requirements = stage(&source, wide_budget()).unwrap();
        assert_eq!(requirements.receipt().target(), target);
        assert_eq!(requirements.receipt().modified_function_count(), 0);
        assert_eq!(requirements.receipt().modified_unit_count(), 0);
        assert_eq!(requirements.receipt().witness_count(), 0);
        assert!(
            requirements
                .plan()
                .functions
                .iter()
                .all(|function| function.modified_units.is_empty())
        );
    }
}
