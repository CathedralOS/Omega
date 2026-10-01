use super::rejoin_system_call;
use calling_conventions::{
    CallSignature, CallingPolicy, EntryControl, ExternalBindingKind, ExternalBindingRow,
    ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};
use target::TargetProfile;

fn fixture(
    profile: TargetProfile,
    number: i64,
    signature: CallSignature,
) -> (ProviderPlan, ExternalBindingRow) {
    let plan = ProviderPlan {
        name: "selected::kernel".into(),
        provider_type: "KernelEntry".into(),
        provider_type_package_identity: None,
        target: profile.target_name().into(),
        schema: ServiceSchema {
            trait_name: "KernelBoundary".into(),
            trait_package_identity: None,
            // This unit tests physical settlement, not source checking. The
            // Terminal signature remains the downstream consumer's obligation.
            methods: Vec::new(),
        },
        rows: vec![ProviderPlanRow {
            method: "invoke".into(),
            requirement_identity: "KernelBoundary::invoke".into(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::Syscall { number },
        }],
        origin_package_identity: None,
        origin_package: "fixture".into(),
    };
    let policy = CallingPolicy::native_syscall_for_target(profile.native_target()).unwrap();
    let entry = evaluate_ordinary_boundary_entry_plan(policy, &signature).unwrap();
    let external = ExternalBindingRow {
        target_name: plan.target.clone(),
        trait_name: plan.schema.trait_name.clone(),
        method: plan.rows[0].method.clone(),
        requirement_identity: plan.rows[0].requirement_identity.clone(),
        table_type: plan.provider_type.clone(),
        boundary_entry_plan: Some(entry.plan().clone()),
        binding: ExternalBindingKind::Syscall { number },
    };
    (plan, external)
}

#[test]
fn retains_exact_kernel_number_target_and_abi_without_import_stack_custody() {
    for profile in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        // Mixed widths and an integer result challenge a count-only recipe;
        // there is no dependency on Clock or on one syscall number.
        let signature = CallSignature {
            parameters: vec![
                ValueShape::integer(8, 8),
                ValueShape::integer(4, 4),
                ValueShape::integer(2, 2),
                ValueShape::integer(8, 8),
            ],
            result: Some(ValueShape::integer(8, 8)),
        };
        let (plan, external) = fixture(profile, 211, signature);
        let admitted = rejoin_system_call(
            &plan,
            std::slice::from_ref(&external),
            "KernelBoundary::invoke",
            profile.native_target(),
        )
        .unwrap();
        let target_operations::NormalizedForeignCallMechanism::SystemCall(kernel) =
            &admitted.mechanism
        else {
            panic!("expected kernel binding");
        };
        assert_eq!(kernel.number, 211);
        assert_eq!(kernel.target, profile);
        assert_eq!(
            kernel.provider_plan_commitment,
            *plan.identity_digest().as_bytes()
        );
        assert_eq!(kernel.requirement_identity, "KernelBoundary::invoke");
        assert_eq!(
            Some(admitted.boundary_entry_plan),
            external.boundary_entry_plan
        );
    }
}

#[test]
fn rejects_substituted_kernel_binding_coordinates_and_incomplete_rosters() {
    let profile = TargetProfile::LinuxX64;
    let (plan, external) = fixture(
        profile,
        7,
        CallSignature {
            parameters: Vec::new(),
            result: None,
        },
    );
    let reject = |plan: &ProviderPlan, rows: &[ExternalBindingRow]| {
        assert!(
            rejoin_system_call(
                plan,
                rows,
                "KernelBoundary::invoke",
                profile.native_target()
            )
            .is_err()
        );
    };
    reject(&plan, &[]);
    reject(&plan, &[external.clone(), external.clone()]);
    let mut duplicate = plan.clone();
    duplicate.rows.push(duplicate.rows[0].clone());
    reject(&duplicate, std::slice::from_ref(&external));
    for coordinate in 0..7 {
        let mut changed = external.clone();
        match coordinate {
            0 => changed.binding = ExternalBindingKind::Syscall { number: 35 },
            1 => changed.target_name = "linux_arm64".into(),
            2 => changed.trait_name.push_str("Other"),
            3 => changed.method.push_str("Other"),
            4 => changed.requirement_identity.push_str("Other"),
            5 => changed.table_type.push_str("Other"),
            _ => changed.boundary_entry_plan = None,
        }
        reject(&plan, &[changed]);
    }
    let mut changed = plan.clone();
    changed.rows[0].binding = ProviderBinding::Syscall { number: -1 };
    reject(&changed, std::slice::from_ref(&external));
    assert!(
        rejoin_system_call(
            &plan,
            &[external],
            "KernelBoundary::invoke",
            TargetProfile::LinuxArm64.native_target()
        )
        .is_err()
    );
}

#[test]
fn rejects_call_return_policy_and_corrupted_supervisor_plan() {
    let profile = TargetProfile::LinuxX64;
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(4, 4)],
        result: Some(ValueShape::integer(8, 8)),
    };
    let (plan, external) = fixture(profile, 7, signature.clone());
    for coordinate in 0..4 {
        let mut changed = external.clone();
        let entry = changed.boundary_entry_plan.as_mut().unwrap();
        match coordinate {
            0 => {
                *entry =
                    evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                        .unwrap()
                        .plan()
                        .clone()
            }
            1 => entry.call.entry_control = EntryControl::CallReturn,
            2 => entry.call.parameters[0].locations.clear(),
            _ => entry.call.ordinary_clobbers = Default::default(),
        }
        assert!(
            rejoin_system_call(
                &plan,
                &[changed],
                "KernelBoundary::invoke",
                profile.native_target()
            )
            .is_err()
        );
    }
}
