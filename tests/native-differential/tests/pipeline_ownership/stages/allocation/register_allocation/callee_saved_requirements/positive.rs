use crate::tests::*;

use super::fixture::{call_homes, ordinary_homes, preserving_call_homes, stage, wide_budget};

#[test]
fn scalar_calls_report_exact_callee_saved_writes_and_replay_deterministically() {
    for (target, abi) in [
        (
            NativeTarget::linux_x64(),
            FrameAbiPreservationConvention::SystemVAMD64,
        ),
        (
            NativeTarget::windows_x64(),
            FrameAbiPreservationConvention::MicrosoftX64,
        ),
        (
            NativeTarget::uefi_x64(),
            FrameAbiPreservationConvention::MicrosoftX64,
        ),
        (
            NativeTarget::linux_arm64(),
            FrameAbiPreservationConvention::Aapcs64,
        ),
        (
            NativeTarget::macos_arm64(),
            FrameAbiPreservationConvention::DarwinAapcs64,
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
fn general_scalar_calls_report_exact_callee_saved_writes_and_replay_deterministically() {
    for (target, abi) in [
        (
            NativeTarget::linux_x64(),
            FrameAbiPreservationConvention::SystemVAMD64,
        ),
        (
            NativeTarget::windows_x64(),
            FrameAbiPreservationConvention::MicrosoftX64,
        ),
        (
            NativeTarget::uefi_x64(),
            FrameAbiPreservationConvention::MicrosoftX64,
        ),
        (
            NativeTarget::linux_arm64(),
            FrameAbiPreservationConvention::Aapcs64,
        ),
        (
            NativeTarget::macos_arm64(),
            FrameAbiPreservationConvention::DarwinAapcs64,
        ),
    ] {
        let source = preserving_call_homes(target);
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
            CalleeSavedModificationWitness::OperandDefinition { .. }
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
fn preservation_rosters_follow_the_selected_convention_on_call_fixtures() {
    // The reported roster is the selected convention's own set, not a shared
    // constant: Microsoft x64 adds rdi/rsi and xmm6-15 to the System-V callee
    // set, while Darwin shares the AAPCS64 callee set but moves x18 from
    // caller-saved into the fixed platform reservation.
    let staged = |target| {
        let source = call_homes(target);
        let environment = source
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment()
            .clone();
        (stage(&source, wide_budget()).unwrap(), environment)
    };
    let (system_v, system_v_environment) = staged(NativeTarget::linux_x64());
    let (microsoft, microsoft_environment) = staged(NativeTarget::windows_x64());
    let (uefi, uefi_environment) = staged(NativeTarget::uefi_x64());
    let (aapcs, aapcs_environment) = staged(NativeTarget::linux_arm64());
    let (darwin, darwin_environment) = staged(NativeTarget::macos_arm64());

    for (validated, environment) in [
        (&system_v, &system_v_environment),
        (&microsoft, &microsoft_environment),
        (&uefi, &uefi_environment),
        (&aapcs, &aapcs_environment),
        (&darwin, &darwin_environment),
    ] {
        let selected = selected_abi_preservation(environment).unwrap();
        assert_eq!(
            validated.plan().callee_saved_units,
            selected.convention.callee_saved
        );
    }

    let callee_set = |validated: &ValidatedAllocatedCalleeSavedRequirements| {
        validated
            .plan()
            .callee_saved_units
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
    };
    let view_units = |environment: &ValidatedTargetRegisterEnvironment, name: &str| {
        environment
            .physical()
            .model()
            .view_named(name)
            .unwrap()
            .units
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
    };

    let system_v_callee = callee_set(&system_v);
    let microsoft_callee = callee_set(&microsoft);
    assert!(system_v_callee.is_subset(&microsoft_callee));
    assert!(system_v_callee.len() < microsoft_callee.len());
    assert_eq!(callee_set(&uefi), microsoft_callee);
    for name in ["rsi", "rdi"] {
        let units = view_units(&microsoft_environment, name);
        assert!(units.is_subset(&microsoft_callee));
        assert!(!units.is_subset(&system_v_callee));
    }

    assert_eq!(callee_set(&aapcs), callee_set(&darwin));
    assert_ne!(aapcs.receipt().abi(), darwin.receipt().abi());
    let x18 = view_units(&darwin_environment, "x18");
    let aapcs_convention = selected_abi_preservation(&aapcs_environment)
        .unwrap()
        .convention;
    let darwin_convention = selected_abi_preservation(&darwin_environment)
        .unwrap()
        .convention;
    assert!(
        x18.iter()
            .all(|unit| aapcs_convention.caller_saved.contains(unit))
    );
    assert!(
        x18.iter()
            .all(|unit| darwin_convention.fixed.contains(unit))
    );
    assert!(
        x18.iter()
            .all(|unit| !darwin_convention.caller_saved.contains(unit))
    );
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
