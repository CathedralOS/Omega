//! Checked direct-syscall mechanism derivation during provider settlement.

use super::*;
use crate::realization::providers::settlements::validate_source_evaluated_import_coverage;

const REQUIREMENT: &str = "omega::test::Foreign::leaf()";

fn syscall_plan(profile: target::TargetProfile, number: i64) -> effects::SelectedProviderPlanFacts {
    let mut plan = import_plan(b"unused", profile);
    plan.rows[0].binding = ProviderBinding::Syscall { number };
    effects::SelectedProviderPlanFacts::from_selected_plans(vec![plan])
        .expect("one exact syscall plan")
}

fn abstract_plan() -> abstract_operations::AbstractOperationPlan {
    let machine = semantic_vocabulary::MachineId::new(850).unwrap();
    let boundary = semantic_vocabulary::BoundaryMachineId::new(850).unwrap();
    abstract_operations::AbstractOperationPlan {
        psi: terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0x85; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![terminal_psi::BoundaryMachineDeclaration {
            id: boundary,
            identity: REQUIREMENT.into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        functions: vec![abstract_operations::AbstractFunction {
            machine,
            attachment: None,
            entry: semantic_vocabulary::BlockId::new(850).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: abstract_operations::AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![abstract_operations::AbstractBlockEntry {
                block: semantic_vocabulary::BlockId::new(850).unwrap(),
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                abstract_operations::AbstractOperation::BoundaryCall {
                    psi_operation: semantic_vocabulary::OperationId::new(850).unwrap(),
                    result: abstract_operations::AbstractBoundaryResult::Unit,
                    boundary,
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                abstract_operations::AbstractOperation::ReturnUnit {
                    psi_edge: semantic_vocabulary::EdgeId::new(850).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    }
}

fn add_unqualified_structural_parameter(plan: &mut abstract_operations::AbstractOperationPlan) {
    let structural_type = semantic_vocabulary::StructuralTypeId::new(851).unwrap();
    let place = semantic_vocabulary::PlaceId::new(851).unwrap();
    plan.structural_types
        .push(terminal_psi::StructuralTypeDeclaration {
            id: structural_type,
            identity: "omega::test::Payload".into(),
            shape: terminal_psi::StructuralTypeShape::PrimitiveScalar(
                semantic_vocabulary::ScalarType::Boolean,
            ),
        });
    plan.boundary_machines[0].structural_parameters =
        vec![terminal_psi::StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            access: terminal_psi::StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }];
    let abstract_operations::AbstractOperation::BoundaryCall {
        structural_arguments,
        ..
    } = &mut plan.functions[0].operations[0]
    else {
        panic!("fixture retains its boundary call")
    };
    structural_arguments.push(terminal_psi::StructuralArgument {
        place,
        path: Vec::new(),
        access: terminal_psi::StructuralAccess::SharedBorrow,
    });
}

fn external(
    profile: target::TargetProfile,
    number: i64,
) -> calling_conventions::ExternalBindingRow {
    calling_conventions::ExternalBindingRow {
        target_name: profile.target_name().into(),
        trait_name: "omega::test::Foreign".into(),
        method: "leaf".into(),
        requirement_identity: REQUIREMENT.into(),
        table_type: String::new(),
        boundary_entry_plan: None,
        binding: calling_conventions::ExternalBindingKind::Syscall { number },
    }
}

#[test]
fn conservative_contract_commits_scalar_carriers_and_rejects_occurrence_drift() {
    let profile = target::TargetProfile::LinuxX64;
    let mut plan = abstract_plan();
    let boundary = plan.boundary_machines[0].id;
    let empty =
        crate::realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect("empty verified signature has an exact conservative contract");

    plan.boundary_machines[0].scalar_parameters = vec![semantic_vocabulary::ScalarType::Boolean];
    assert!(
        crate::realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect_err("declaration/call arity drift rejects")
        .contains("does not match")
    );

    let value = semantic_vocabulary::ValueId::new(851).unwrap();
    plan.functions[0].operations.insert(
        0,
        abstract_operations::AbstractOperation::BooleanConstant {
            psi_operation: semantic_vocabulary::OperationId::new(851).unwrap(),
            result: value,
            value: true,
        },
    );
    let abstract_operations::AbstractOperation::BoundaryCall { arguments, .. } =
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
fn conservative_contract_rejects_root_structural_qualifications_without_stable_domains() {
    let profile = target::TargetProfile::LinuxX64;
    let mut plan = abstract_plan();
    add_unqualified_structural_parameter(&mut plan);
    plan.boundary_machines[0].structural_parameters[0]
        .qualifications
        .push(semantic_vocabulary::StructuralDomainId::new(851).unwrap());
    let boundary = plan.boundary_machines[0].id;

    assert!(
        crate::realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect_err("module-local root qualification IDs cannot enter stable policy identity")
        .contains("does not yet support root structural qualifications")
    );
}

#[test]
fn conservative_contract_rejects_projected_qualifications_without_stable_domains() {
    let profile = target::TargetProfile::LinuxX64;
    let mut plan = abstract_plan();
    add_unqualified_structural_parameter(&mut plan);
    plan.boundary_machines[0].structural_parameters[0]
        .projected_qualifications
        .push(terminal_psi::StructuralPathQualification {
            path: vec![terminal_psi::StructuralPathSegment::Field("value".into())],
            domain: semantic_vocabulary::StructuralDomainId::new(851).unwrap(),
        });
    let boundary = plan.boundary_machines[0].id;

    assert!(
        crate::realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect_err("module-local projected qualification IDs cannot enter stable policy identity")
        .contains("does not yet support projected structural qualifications")
    );
}

#[test]
fn conservative_contract_rejects_boundary_requirements_without_stable_domains() {
    let profile = target::TargetProfile::LinuxX64;
    let mut plan = abstract_plan();
    add_unqualified_structural_parameter(&mut plan);
    plan.boundary_machines[0]
        .requires
        .push(terminal_psi::StructuralDomainRequirement {
            argument_index: 0,
            domain: semantic_vocabulary::StructuralDomainId::new(851).unwrap(),
        });
    let boundary = plan.boundary_machines[0].id;

    assert!(
        crate::realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect_err("module-local boundary requirement IDs cannot enter stable policy identity")
        .contains("does not yet support boundary structural-domain requirements")
    );
}

#[test]
fn settlement_derives_the_exact_checked_syscall_mechanism_and_rejects_substitution() {
    let profile = target::TargetProfile::LinuxX64;
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
            effects::TerminalAuthorityDisposition::from_classes([]),
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
            effects::TerminalAuthorityDisposition::from_classes([]),
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
    wrong_number.binding = calling_conventions::ExternalBindingKind::Syscall { number: 2 };
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
    wrong_target.target_name = target::TargetProfile::LinuxArm64.target_name().into();
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
