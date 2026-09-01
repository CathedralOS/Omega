use super::*;
use omega_abstract_operations::{AbstractFunction, AbstractOperation, AbstractOperationPlan};
use omega_effects::{
    provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema},
    CompilerIntrinsicExecutionIdentity, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use psi_core::{BoundaryMachineId, MachineId};

const ADAPTER_REQUIREMENT: &str = "test::Adapter::run()";
const LEAF_REQUIREMENT: &str = "test::Console::exit()";

fn service_method(requirement: &str) -> ServiceMethod {
    let (owner, name) = requirement
        .rsplit_once("::")
        .expect("fixture requirement has an owner");
    ServiceMethod {
        name: name.trim_end_matches("()").to_owned(),
        requirement_owner: owner.to_owned(),
        requirement_owner_package_identity: None,
        requirement_identity: requirement.to_owned(),
        parameter_count: 0,
        parameter_type_identities: Vec::new(),
        entry_claims: Vec::new(),
        has_result: false,
        result_type_identity: None,
        result_claims: Vec::new(),
        service_reach: vec![owner.to_owned()],
        synchronous_invocations: Vec::new(),
        may_suspend: false,
        may_block: false,
        terminates_guarantee: false,
        termination_premises: Vec::new(),
        calling_plan_report_fingerprint: None,
        calling_plan_commitment: None,
    }
}

fn selected_plan(
    name: &str,
    provider_type: &str,
    requirement: &str,
    binding: ProviderBinding,
) -> ProviderPlan {
    let method = service_method(requirement);
    ProviderPlan {
        name: name.to_owned(),
        provider_type: provider_type.to_owned(),
        provider_type_package_identity: None,
        target: "linux_x86_64".to_owned(),
        schema: ServiceSchema {
            trait_name: method.requirement_owner.clone(),
            trait_package_identity: None,
            methods: vec![method.clone()],
        },
        rows: vec![ProviderPlanRow {
            method: method.name,
            requirement_identity: requirement.to_owned(),
            requirement_lifetime_partition: Vec::new(),
            binding,
        }],
        origin_package_identity: None,
        origin_package: "test".to_owned(),
    }
}

fn boundary(id: u32, requirement: &str) -> psi_terminal::BoundaryMachineDeclaration {
    psi_terminal::BoundaryMachineDeclaration {
        id: psi_core::BoundaryMachineId::new(u64::from(id)).unwrap(),
        identity: requirement.to_owned(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: None,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    }
}

fn function(machine: u32, boundary_ids: &[u32]) -> AbstractFunction {
    let machine = MachineId::new(u64::from(machine)).unwrap();
    let block = psi_core::BlockId::new(machine.get()).unwrap();
    let mut operations = boundary_ids
        .iter()
        .enumerate()
        .map(|(index, boundary)| AbstractOperation::BoundaryCall {
            psi_operation: psi_core::OperationId::new(
                machine.get().saturating_mul(10) + index as u64 + 1,
            )
            .unwrap(),
            result: None,
            boundary: BoundaryMachineId::new(u64::from(*boundary)).unwrap(),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        })
        .collect::<Vec<_>>();
    operations.push(AbstractOperation::ReturnUnit {
        psi_edge: psi_core::EdgeId::new(machine.get().saturating_mul(10) + 9).unwrap(),
        cleanup_actions: Vec::new(),
    });
    AbstractFunction {
        machine,
        attachment: None,
        entry: block,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: omega_abstract_operations::AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![omega_abstract_operations::AbstractBlockEntry {
            block,
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations,
    }
}

fn abstract_plan(
    boundaries: Vec<psi_terminal::BoundaryMachineDeclaration>,
    provider_candidates: Vec<psi_terminal::ProviderCandidateConformance>,
    functions: Vec<AbstractFunction>,
) -> AbstractOperationPlan {
    AbstractOperationPlan {
        psi: psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
            program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([41; 32]),
        },
        entry: MachineId::new(1).unwrap(),
        structural_types: Vec::new(),
        boundary_machines: boundaries,
        provider_candidates,
        functions,
    }
}

fn installed_candidate(
    boundary: u32,
    requirement: &str,
    provider: &str,
    candidate_identity: &str,
    machine: u32,
) -> psi_terminal::ProviderCandidateConformance {
    psi_terminal::ProviderCandidateConformance {
        boundary: BoundaryMachineId::new(u64::from(boundary)).unwrap(),
        requirement_identity: requirement.to_owned(),
        provider_identity: provider.to_owned(),
        candidate_identity: candidate_identity.to_owned(),
        candidate: MachineId::new(u64::from(machine)).unwrap(),
        signature: psi_terminal::ProviderUnitSignature {
            parameters: Vec::new(),
        },
        refinement: psi_terminal::ProviderUnitRefinement {
            positional_parameters: Vec::new(),
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    }
}

fn permission_policy(
    plans: &[&ProviderPlan],
    permitted: TerminalAuthorityDisposition,
) -> TerminalAuthorityPermissionPolicy {
    super::super::terminal_authority_permission_policy::terminal_authority_permission_policy_with_rows(
            plans
                .iter()
                .map(|plan| {
                    super::super::terminal_authority_permission_policy::TerminalAuthorityPermissionPolicyRow::new(
                        plan.schema.identity_digest(),
                        plan.rows[0].requirement_identity.clone(),
                        permitted.clone(),
                    )
                })
                .collect(),
        )
        .expect("exact fixture permissions")
}

fn intrinsic_mechanism(boundary: u32) -> AdmittedTerminalMechanism {
    AdmittedTerminalMechanism {
        boundary: BoundaryMachineId::new(u64::from(boundary)).unwrap(),
        mechanism: CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32.into(),
    }
}

#[test]
fn intrinsic_leaf_requires_exact_service_permission() {
    let leaf = selected_plan(
        "leaf",
        "LeafProvider",
        LEAF_REQUIREMENT,
        ProviderBinding::CompilerIntrinsic {
            machine: "test::linux_exit_group_i32".to_owned(),
        },
    );
    let selected =
        SelectedProviderPlanFacts::from_selected_plans(vec![leaf.clone()]).expect("selected leaf");
    let plan = abstract_plan(
        vec![boundary(1, LEAF_REQUIREMENT)],
        Vec::new(),
        vec![function(1, &[1])],
    );
    let physical = super::super::terminal_authority_policy::current_terminal_authority_policy();
    let permitted = permission_policy(
        &[&leaf],
        TerminalAuthorityDisposition::from_classes([TerminalAuthorityClass::ProcessTermination]),
    );
    let receipt = review_terminal_authority_closure(
        [7; 32],
        omega_target::NativeTarget::linux_x64(),
        &plan,
        &selected,
        &physical,
        &permitted,
        &[intrinsic_mechanism(1)],
        &[],
    )
    .expect("contained intrinsic closure");
    assert_eq!(receipt.leaves().len(), 1);
    assert_eq!(receipt.leaves()[0].requirement_identity(), LEAF_REQUIREMENT);
    assert_eq!(
        receipt.leaves()[0].exercised().classes(),
        &[TerminalAuthorityClass::ProcessTermination]
    );

    let denied = permission_policy(&[&leaf], TerminalAuthorityDisposition::from_classes([]));
    assert!(review_terminal_authority_closure(
        [7; 32],
        omega_target::NativeTarget::linux_x64(),
        &plan,
        &selected,
        &physical,
        &denied,
        &[intrinsic_mechanism(1)],
        &[],
    )
    .expect_err("exercised class exceeds empty permission")
    .contains("exceeds"));
    assert!(
            review_terminal_authority_closure(
                [7; 32],
                omega_target::NativeTarget::linux_x64(),
                &plan,
                &selected,
                &physical,
                &super::super::terminal_authority_permission_policy::current_terminal_authority_permission_policy(),
                &[intrinsic_mechanism(1)],
                &[],
            )
            .expect_err("missing exact permission rejects")
            .contains("no exact row")
        );
}

#[test]
fn checked_adapter_expands_to_selected_terminal_leaf() {
    let adapter = selected_plan(
        "adapter",
        "AdapterProvider",
        ADAPTER_REQUIREMENT,
        ProviderBinding::CheckedAdapter {
            machine_identity: "AdapterProvider::run".to_owned(),
            machine_package_identity: None,
        },
    );
    let leaf = selected_plan(
        "leaf",
        "LeafProvider",
        LEAF_REQUIREMENT,
        ProviderBinding::CompilerIntrinsic {
            machine: "test::linux_exit_group_i32".to_owned(),
        },
    );
    let selected =
        SelectedProviderPlanFacts::from_selected_plans(vec![adapter.clone(), leaf.clone()])
            .expect("selected adapter and leaf");
    let candidate = installed_candidate(
        1,
        ADAPTER_REQUIREMENT,
        "AdapterProvider",
        "AdapterProvider::run",
        2,
    );
    let plan = abstract_plan(
        vec![
            boundary(1, ADAPTER_REQUIREMENT),
            boundary(2, LEAF_REQUIREMENT),
        ],
        vec![candidate.clone()],
        vec![function(1, &[1]), function(2, &[2])],
    );
    let physical = super::super::terminal_authority_policy::current_terminal_authority_policy();
    let permitted = permission_policy(
        &[&leaf],
        TerminalAuthorityDisposition::from_classes([TerminalAuthorityClass::ProcessTermination]),
    );
    let receipt = review_terminal_authority_closure(
        [8; 32],
        omega_target::NativeTarget::linux_x64(),
        &plan,
        &selected,
        &physical,
        &permitted,
        &[intrinsic_mechanism(2)],
        &[candidate],
    )
    .expect("checked adapter closure reaches exact leaf");
    assert_eq!(receipt.leaves().len(), 1);
    assert_eq!(receipt.leaves()[0].requirement_identity(), LEAF_REQUIREMENT);
}

#[test]
fn internal_call_edges_are_part_of_the_reviewed_closure() {
    let leaf = selected_plan(
        "leaf",
        "LeafProvider",
        LEAF_REQUIREMENT,
        ProviderBinding::CompilerIntrinsic {
            machine: "test::linux_exit_group_i32".to_owned(),
        },
    );
    let selected =
        SelectedProviderPlanFacts::from_selected_plans(vec![leaf.clone()]).expect("selected leaf");
    let mut entry = function(1, &[]);
    entry.operations.insert(
        0,
        AbstractOperation::CallUnit {
            psi_operation: psi_core::OperationId::new(1).unwrap(),
            callee: MachineId::new(2).unwrap(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    );
    let plan = abstract_plan(
        vec![boundary(1, LEAF_REQUIREMENT)],
        Vec::new(),
        vec![entry, function(2, &[1])],
    );
    let physical = super::super::terminal_authority_policy::current_terminal_authority_policy();
    let permitted = permission_policy(
        &[&leaf],
        TerminalAuthorityDisposition::from_classes([TerminalAuthorityClass::ProcessTermination]),
    );
    let receipt = review_terminal_authority_closure(
        [11; 32],
        omega_target::NativeTarget::linux_x64(),
        &plan,
        &selected,
        &physical,
        &permitted,
        &[intrinsic_mechanism(1)],
        &[],
    )
    .expect("internal call reaches exact terminal leaf");
    assert_eq!(receipt.leaves().len(), 1);
    assert_eq!(receipt.leaves()[0].requirement_identity(), LEAF_REQUIREMENT);
}

#[test]
fn checked_physical_operations_without_a_role_fail_closed() {
    let mut entry = function(1, &[]);
    entry.operations.insert(
        0,
        AbstractOperation::PortWrite {
            psi_operation: psi_core::OperationId::new(1).unwrap(),
            service: psi_core::ServiceId::new(1).unwrap(),
            port: 0x80,
            value: 0,
        },
    );
    let plan = abstract_plan(Vec::new(), Vec::new(), vec![entry]);
    let selected =
        SelectedProviderPlanFacts::from_selected_plans(Vec::new()).expect("empty selected closure");
    assert!(
            review_terminal_authority_closure(
                [12; 32],
                omega_target::NativeTarget::linux_x64(),
                &plan,
                &selected,
                &super::super::terminal_authority_policy::current_terminal_authority_policy(),
                &super::super::terminal_authority_permission_policy::current_terminal_authority_permission_policy(),
                &[],
                &[],
            )
            .expect_err("unsupported checked physical role rejects")
            .contains("checked physical terminal operation unsupported")
        );
}

#[test]
fn checked_adapter_cycles_and_unsupported_roles_fail_closed() {
    let first = selected_plan(
        "first",
        "FirstProvider",
        ADAPTER_REQUIREMENT,
        ProviderBinding::CheckedAdapter {
            machine_identity: "FirstProvider::run".to_owned(),
            machine_package_identity: None,
        },
    );
    let second_requirement = "test::Second::run()";
    let second = selected_plan(
        "second",
        "SecondProvider",
        second_requirement,
        ProviderBinding::CheckedAdapter {
            machine_identity: "SecondProvider::run".to_owned(),
            machine_package_identity: None,
        },
    );
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![first, second])
        .expect("selected cycle plans remain ordinary data");
    let first_candidate = installed_candidate(
        1,
        ADAPTER_REQUIREMENT,
        "FirstProvider",
        "FirstProvider::run",
        2,
    );
    let second_candidate = installed_candidate(
        2,
        second_requirement,
        "SecondProvider",
        "SecondProvider::run",
        3,
    );
    let plan = abstract_plan(
        vec![
            boundary(1, ADAPTER_REQUIREMENT),
            boundary(2, second_requirement),
        ],
        vec![first_candidate.clone(), second_candidate.clone()],
        vec![function(1, &[1]), function(2, &[2]), function(3, &[1])],
    );
    assert!(
            review_terminal_authority_closure(
                [9; 32],
                omega_target::NativeTarget::linux_x64(),
                &plan,
                &selected,
                &super::super::terminal_authority_policy::current_terminal_authority_policy(),
                &super::super::terminal_authority_permission_policy::current_terminal_authority_permission_policy(),
                &[],
                &[first_candidate, second_candidate],
            )
            .expect_err("checked-provider cycle rejects")
            .contains("cycle")
        );

    let syscall = selected_plan(
        "syscall",
        "SyscallProvider",
        LEAF_REQUIREMENT,
        ProviderBinding::Syscall { number: 1 },
    );
    let syscall_selected = SelectedProviderPlanFacts::from_selected_plans(vec![syscall])
        .expect("unsupported role remains selectable data");
    let syscall_plan = abstract_plan(
        vec![boundary(1, LEAF_REQUIREMENT)],
        Vec::new(),
        vec![function(1, &[1])],
    );
    assert!(
            review_terminal_authority_closure(
                [10; 32],
                omega_target::NativeTarget::linux_x64(),
                &syscall_plan,
                &syscall_selected,
                &super::super::terminal_authority_policy::current_terminal_authority_policy(),
                &super::super::terminal_authority_permission_policy::current_terminal_authority_permission_policy(),
                &[],
                &[],
            )
            .expect_err("unsupported syscall role rejects")
            .contains("unsupported syscall")
        );
}
