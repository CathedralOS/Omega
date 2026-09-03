use super::{
    ADAPTER_REQUIREMENT, abstract_plan, boundary, checked_port_write_mechanism,
    checked_port_write_policy, function, installed_port_write_candidate, permission_policy,
    port_write_function, selected_plan,
};
use crate::realization::terminal_authority_review::review_terminal_authority_closure;
use omega_effects::{
    SelectedProviderPlanFacts, TerminalAuthorityClass, TerminalAuthorityDisposition,
    provider_plan::ProviderBinding,
};

#[test]
fn checked_adapter_port_write_requires_exact_physical_and_service_policy() {
    let adapter = selected_plan(
        "adapter",
        "AdapterProvider",
        ADAPTER_REQUIREMENT,
        ProviderBinding::CheckedAdapter {
            machine_identity: "AdapterProvider::run".to_owned(),
            machine_package_identity: None,
        },
    );
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![adapter.clone()])
        .expect("selected checked adapter");
    let candidate = installed_port_write_candidate(
        1,
        ADAPTER_REQUIREMENT,
        "AdapterProvider",
        "AdapterProvider::run",
        2,
    );
    let plan = abstract_plan(
        vec![boundary(1, ADAPTER_REQUIREMENT)],
        vec![candidate.clone()],
        vec![function(1, &[1]), port_write_function(2, &[0x03f8])],
    );
    let physical = checked_port_write_policy(omega_target::TargetProfile::LinuxX64, 0x03f8);
    let permitted = permission_policy(
        &[&adapter],
        TerminalAuthorityDisposition::from_classes([TerminalAuthorityClass::PortIo]),
    );
    let receipt = review_terminal_authority_closure(
        [13; 32],
        omega_target::TargetProfile::LinuxX64,
        &plan,
        &selected,
        &physical,
        &permitted,
        &[],
        std::slice::from_ref(&candidate),
    )
    .expect("selected checked adapter retains one exact PortWrite leaf");
    assert_eq!(receipt.leaves().len(), 1);
    assert_eq!(
        receipt.leaves()[0].requirement_identity(),
        ADAPTER_REQUIREMENT
    );
    assert_eq!(
        receipt.leaves()[0].mechanism(),
        checked_port_write_mechanism(omega_target::TargetProfile::LinuxX64, 0x03f8),
    );
    assert_eq!(
        receipt.leaves()[0].exercised().classes(),
        &[TerminalAuthorityClass::PortIo],
    );

    for wrong_policy in [
        crate::realization::terminal_authority_policy::current_terminal_authority_policy(),
        checked_port_write_policy(omega_target::TargetProfile::LinuxX64, 0x0080),
        checked_port_write_policy(omega_target::TargetProfile::WindowsX64, 0x03f8),
    ] {
        assert!(
            review_terminal_authority_closure(
                [13; 32],
                omega_target::TargetProfile::LinuxX64,
                &plan,
                &selected,
                &wrong_policy,
                &permitted,
                &[],
                std::slice::from_ref(&candidate),
            )
            .expect_err("missing, wrong-port, or wrong-profile physical row rejects")
            .contains("does not classify")
        );
    }

    let denied = permission_policy(&[&adapter], TerminalAuthorityDisposition::from_classes([]));
    assert!(
        review_terminal_authority_closure(
            [13; 32],
            omega_target::TargetProfile::LinuxX64,
            &plan,
            &selected,
            &physical,
            &denied,
            &[],
            &[candidate],
        )
        .expect_err("PortIo cannot exceed the selected requirement permission")
        .contains("exceeds")
    );
}

#[test]
fn checked_adapter_port_write_rejects_service_target_and_plural_mechanism_drift() {
    let adapter = selected_plan(
        "adapter",
        "AdapterProvider",
        ADAPTER_REQUIREMENT,
        ProviderBinding::CheckedAdapter {
            machine_identity: "AdapterProvider::run".to_owned(),
            machine_package_identity: None,
        },
    );
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![adapter.clone()])
        .expect("selected checked adapter");
    let candidate = installed_port_write_candidate(
        1,
        ADAPTER_REQUIREMENT,
        "AdapterProvider",
        "AdapterProvider::run",
        2,
    );
    let permitted = permission_policy(
        &[&adapter],
        TerminalAuthorityDisposition::from_classes([TerminalAuthorityClass::PortIo]),
    );
    let physical =
        crate::realization::terminal_authority_policy::terminal_authority_policy_with_rows(
            [0x03f8, 0x0080]
                .into_iter()
                .map(|port| {
                    crate::realization::terminal_authority_policy::TerminalAuthorityPolicyRow::new(
                        checked_port_write_mechanism(omega_target::TargetProfile::LinuxX64, port),
                        TerminalAuthorityDisposition::from_classes([
                            TerminalAuthorityClass::PortIo,
                        ]),
                    )
                })
                .collect(),
        )
        .unwrap();

    let plural = abstract_plan(
        vec![boundary(1, ADAPTER_REQUIREMENT)],
        vec![candidate.clone()],
        vec![function(1, &[1]), port_write_function(2, &[0x03f8, 0x0080])],
    );
    assert!(
        review_terminal_authority_closure(
            [14; 32],
            omega_target::TargetProfile::LinuxX64,
            &plural,
            &selected,
            &physical,
            &permitted,
            &[],
            std::slice::from_ref(&candidate),
        )
        .expect_err("one requirement cannot smuggle two distinct checked mechanisms")
        .contains("repeats")
    );

    let mut missing_service = port_write_function(2, &[0x03f8]);
    missing_service.published_service_ceiling.clear();
    let missing_service = abstract_plan(
        vec![boundary(1, ADAPTER_REQUIREMENT)],
        vec![candidate.clone()],
        vec![function(1, &[1]), missing_service],
    );
    assert!(
        review_terminal_authority_closure(
            [14; 32],
            omega_target::TargetProfile::LinuxX64,
            &missing_service,
            &selected,
            &physical,
            &permitted,
            &[],
            std::slice::from_ref(&candidate),
        )
        .expect_err("operation outside the checked service ceiling rejects")
        .contains("outside its verified service ceiling")
    );

    let one = abstract_plan(
        vec![boundary(1, ADAPTER_REQUIREMENT)],
        vec![candidate.clone()],
        vec![function(1, &[1]), port_write_function(2, &[0x03f8])],
    );
    assert!(
        review_terminal_authority_closure(
            [14; 32],
            omega_target::TargetProfile::LinuxArm64,
            &one,
            &selected,
            &physical,
            &permitted,
            &[],
            std::slice::from_ref(&candidate),
        )
        .expect_err("PortWrite remains fenced on a non-x86 target")
        .contains("selected target")
    );

    let mut arm_adapter = adapter;
    arm_adapter.target = "linux_arm64".to_owned();
    let arm_selected = SelectedProviderPlanFacts::from_selected_plans(vec![arm_adapter.clone()])
        .expect("selected AArch64 checked adapter");
    let arm_permitted = permission_policy(
        &[&arm_adapter],
        TerminalAuthorityDisposition::from_classes([TerminalAuthorityClass::PortIo]),
    );
    let arm_physical = checked_port_write_policy(omega_target::TargetProfile::LinuxArm64, 0x03f8);
    assert!(
        review_terminal_authority_closure(
            [14; 32],
            omega_target::TargetProfile::LinuxArm64,
            &one,
            &arm_selected,
            &arm_physical,
            &arm_permitted,
            &[],
            &[candidate],
        )
        .expect_err("checked PortWrite remains fenced on AArch64")
        .contains("uses x86 PortWrite on non-x86 target")
    );
}
