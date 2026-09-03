//! Checked direct-syscall mechanism derivation during provider settlement.

use super::*;
use crate::realization::providers::settlements::validate_source_evaluated_import_coverage;

const REQUIREMENT: &str = "omega::test::Foreign::leaf()";

fn syscall_plan(
    profile: omega_target::TargetProfile,
    number: i64,
) -> omega_effects::SelectedProviderPlanFacts {
    let mut plan = import_plan(b"unused", profile);
    plan.rows[0].binding = ProviderBinding::Syscall { number };
    omega_effects::SelectedProviderPlanFacts::from_selected_plans(vec![plan])
        .expect("one exact syscall plan")
}

fn abstract_plan() -> omega_abstract_operations::AbstractOperationPlan {
    let machine = psi_core::MachineId::new(850).unwrap();
    let boundary = psi_core::BoundaryMachineId::new(850).unwrap();
    omega_abstract_operations::AbstractOperationPlan {
        psi: psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
            program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x85; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
            id: boundary,
            identity: REQUIREMENT.into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        functions: vec![omega_abstract_operations::AbstractFunction {
            machine,
            attachment: None,
            entry: psi_core::BlockId::new(850).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: omega_abstract_operations::AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![omega_abstract_operations::AbstractBlockEntry {
                block: psi_core::BlockId::new(850).unwrap(),
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                omega_abstract_operations::AbstractOperation::BoundaryCall {
                    psi_operation: psi_core::OperationId::new(850).unwrap(),
                    result: None,
                    boundary,
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                omega_abstract_operations::AbstractOperation::ReturnUnit {
                    psi_edge: psi_core::EdgeId::new(850).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

fn external(
    profile: omega_target::TargetProfile,
    number: i64,
) -> omega_calling_conventions::ExternalBindingRow {
    omega_calling_conventions::ExternalBindingRow {
        target_name: profile.target_name().into(),
        trait_name: "omega::test::Foreign".into(),
        method: "leaf".into(),
        requirement_identity: REQUIREMENT.into(),
        table_type: String::new(),
        boundary_entry_plan: None,
        binding: omega_calling_conventions::ExternalBindingKind::Syscall { number },
    }
}

#[test]
fn conservative_contract_commits_scalar_carriers_and_rejects_occurrence_drift() {
    let profile = omega_target::TargetProfile::LinuxX64;
    let mut plan = abstract_plan();
    let boundary = plan.boundary_machines[0].id;
    let empty =
        crate::realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect("empty verified signature has an exact conservative contract");

    plan.boundary_machines[0].scalar_parameters = vec![psi_core::ScalarType::Boolean];
    assert!(
        crate::realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect_err("declaration/call arity drift rejects")
        .contains("does not match")
    );

    let value = psi_core::ValueId::new(851).unwrap();
    plan.functions[0].operations.insert(
        0,
        omega_abstract_operations::AbstractOperation::BooleanConstant {
            psi_operation: psi_core::OperationId::new(851).unwrap(),
            result: value,
            value: true,
        },
    );
    let omega_abstract_operations::AbstractOperation::BoundaryCall { arguments, .. } =
        &mut plan.functions[0].operations[1]
    else {
        panic!("fixture retains its boundary call")
    };
    arguments.push(value);
    let boolean =
        crate::realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect("matched boolean signature has an exact conservative contract");
    assert_ne!(empty, boolean);
}

#[test]
fn settlement_derives_the_exact_checked_syscall_mechanism_and_rejects_substitution() {
    let profile = omega_target::TargetProfile::LinuxX64;
    let target = profile.native_target();
    let plan = abstract_plan();
    let boundary = plan.boundary_machines[0].id;
    let selected = syscall_plan(profile, 1);
    let mechanism =
        crate::realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect("verified boundary supplies the conservative checked contract");
    let policy = crate::realization::terminal_authority_policy_with_rows(vec![
        crate::realization::TerminalAuthorityPolicyRow::new(
            mechanism,
            omega_effects::TerminalAuthorityDisposition::from_classes([]),
        ),
    ])
    .expect("exact syscall policy row");
    let external = external(profile, 1);

    let admitted = validate_source_evaluated_import_coverage(
        &plan,
        &selected,
        &policy,
        target,
        std::slice::from_ref(&external),
        &[],
        &[],
    )
    .expect("provider settlement derives the checked syscall identity");
    assert_eq!(
        admitted,
        vec![crate::realization::providers::AdmittedTerminalMechanism {
            boundary,
            mechanism,
        }]
    );
    let selected_plan = &selected.plans()[0];
    let permission = crate::realization::terminal_authority_permission_policy_with_rows(vec![
        crate::realization::TerminalAuthorityPermissionPolicyRow::new(
            selected_plan.schema.identity_digest(),
            REQUIREMENT,
            omega_effects::TerminalAuthorityDisposition::from_classes([]),
        ),
    ])
    .expect("exact syscall requirement permission");
    let receipt = crate::realization::terminal_authority_review::review_terminal_authority_closure(
        [0x86; 32],
        profile,
        &plan,
        &selected,
        &policy,
        &permission,
        &admitted,
        &[],
    )
    .expect("closure review consumes the mechanism derived by provider settlement");
    assert_eq!(receipt.leaves()[0].mechanism(), mechanism);

    let mut wrong_number = external.clone();
    wrong_number.binding = omega_calling_conventions::ExternalBindingKind::Syscall { number: 2 };
    let number_error = validate_source_evaluated_import_coverage(
        &plan,
        &selected,
        &policy,
        target,
        &[wrong_number],
        &[],
        &[],
    )
    .expect_err("retained external number substitution rejects");
    assert!(
        number_error[0]
            .message
            .contains("substituted its normalized syscall number")
    );

    let mut wrong_target = external.clone();
    wrong_target.target_name = omega_target::TargetProfile::LinuxArm64.target_name().into();
    let target_error = validate_source_evaluated_import_coverage(
        &plan,
        &selected,
        &policy,
        target,
        &[wrong_target],
        &[],
        &[],
    )
    .expect_err("retained external target substitution rejects");
    assert!(
        target_error[0]
            .message
            .contains("substituted its retained external target profile")
    );

    let absent_policy = validate_source_evaluated_import_coverage(
        &plan,
        &selected,
        &crate::realization::current_terminal_authority_policy(),
        target,
        &[external],
        &[],
        &[],
    )
    .expect_err("an absent exact syscall policy row rejects");
    assert!(
        absent_policy[0]
            .message
            .contains("does not classify syscall mechanism")
    );
}
