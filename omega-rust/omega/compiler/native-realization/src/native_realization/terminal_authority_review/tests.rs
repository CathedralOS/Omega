use super::{
    AdmittedTerminalMechanism, SelectedProviderPlanFacts, TerminalAuthorityPermissionPolicy,
    TerminalAuthorityPolicy, review_terminal_authority_closure,
};
use abstract_operations::{AbstractFunction, AbstractOperation, AbstractOperationPlan};
use effects::{
    CheckedPhysicalTerminalMechanismIdentity, CheckedSyscallArgumentContractIdentity,
    CompilerIntrinsicExecutionIdentity, PortableFilesystemAuthorityFacet,
    ServiceTerminalAuthorityPermission, SyscallTerminalMechanismIdentity, TerminalAuthorityClass,
    TerminalAuthorityDisposition, TerminalMechanismIdentity,
    provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema},
};
use semantic_vocabulary::{BoundaryMachineId, MachineId};

const ADAPTER_REQUIREMENT: &str = "test::Adapter::run()";
const LEAF_REQUIREMENT: &str = "test::Console::exit()";

#[path = "tests/checked_physical_adapter.rs"]
mod checked_physical_adapter;

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

fn boundary(id: u32, requirement: &str) -> terminal_psi::BoundaryMachineDeclaration {
    terminal_psi::BoundaryMachineDeclaration {
        fixed_service_reach: Vec::new(),
        id: semantic_vocabulary::BoundaryMachineId::new(u64::from(id)).unwrap(),
        identity: requirement.to_owned(),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
        crash_routes: Vec::new(),
    }
}

fn function(machine: u32, boundary_ids: &[u32]) -> AbstractFunction {
    let machine = MachineId::new(u64::from(machine)).unwrap();
    let block = semantic_vocabulary::BlockId::new(machine.get()).unwrap();
    let mut operations = boundary_ids
        .iter()
        .enumerate()
        .map(|(index, boundary)| AbstractOperation::BoundaryCall {
            psi_operation: semantic_vocabulary::OperationId::new(
                machine.get().saturating_mul(10) + index as u64 + 1,
            )
            .unwrap(),
            result: abstract_operations::AbstractBoundaryResult::Unit,
            boundary: BoundaryMachineId::new(u64::from(*boundary)).unwrap(),
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        })
        .collect::<Vec<_>>();
    operations.push(AbstractOperation::ReturnUnit {
        psi_edge: semantic_vocabulary::EdgeId::new(machine.get().saturating_mul(10) + 9).unwrap(),
        cleanup_actions: Vec::new(),
    });
    AbstractFunction {
        machine,
        attachment: None,
        entry: block,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: abstract_operations::AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![abstract_operations::AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block,
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations,
    }
}

fn port_write_function(machine: u32, ports: &[u16]) -> AbstractFunction {
    let service = semantic_vocabulary::ServiceId::new(1).unwrap();
    let mut function = function(machine, &[]);
    function.published_service_ceiling = vec![service];
    for (index, port) in ports.iter().enumerate() {
        function.operations.insert(
            index,
            AbstractOperation::PortWrite {
                psi_operation: semantic_vocabulary::OperationId::new(
                    function.machine.get().saturating_mul(10) + index as u64 + 1,
                )
                .unwrap(),
                service,
                port: *port,
                value: index as u8,
            },
        );
    }
    function
}

fn abstract_plan(
    boundaries: Vec<terminal_psi::BoundaryMachineDeclaration>,
    provider_candidates: Vec<terminal_psi::ProviderCandidateConformance>,
    functions: Vec<AbstractFunction>,
) -> AbstractOperationPlan {
    AbstractOperationPlan {
        psi: terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([41; 32]),
        },
        entry: MachineId::new(1).unwrap(),
        structural_types: Vec::new().into(),
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
) -> terminal_psi::ProviderCandidateConformance {
    terminal_psi::ProviderCandidateConformance {
        boundary: BoundaryMachineId::new(u64::from(boundary)).unwrap(),
        requirement_identity: requirement.to_owned(),
        provider_identity: provider.to_owned(),
        candidate_identity: candidate_identity.to_owned(),
        candidate: MachineId::new(u64::from(machine)).unwrap(),
        signature: terminal_psi::ProviderSignature {
            parameters: Vec::new(),
        },
        refinement: terminal_psi::ProviderRefinement {
            positional_parameters: Vec::new(),
            required_domains: Vec::new(),
            realized_service_ceiling: Vec::new(),
        },
    }
}

fn installed_port_write_candidate(
    boundary: u32,
    requirement: &str,
    provider: &str,
    candidate_identity: &str,
    machine: u32,
) -> terminal_psi::ProviderCandidateConformance {
    let mut candidate =
        installed_candidate(boundary, requirement, provider, candidate_identity, machine);
    candidate.refinement.realized_service_ceiling =
        vec![semantic_vocabulary::ServiceId::new(1).unwrap()];
    candidate
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
        mechanism: CompilerIntrinsicExecutionIdentity::HostedExitProcessI32.into(),
    }
}

fn filesystem_foreign_fixture() -> (
    effects::provider_plan::EvaluatedForeignImport,
    TerminalMechanismIdentity,
) {
    let (evaluated, mechanism) = foreign_import_fixture(
        target::ForeignLocatorCandidate::ElfVersioned {
            object: b"libfixture.so".to_vec(),
            symbol: b"filesystem_operation".to_vec(),
            version: b"FIXTURE_1".to_vec(),
        },
        target::TargetProfile::LinuxX64,
    );
    (evaluated, mechanism.into())
}

/// One evaluated import binding and its unconstrained normalized-foreign
/// mechanism, whose admitted calling plan is the whole contract.
fn foreign_import_fixture(
    candidate: target::ForeignLocatorCandidate,
    target: target::TargetProfile,
) -> (
    effects::provider_plan::EvaluatedForeignImport,
    effects::NormalizedForeignTerminalMechanismIdentity,
) {
    let locator =
        target::normalize_foreign_locator(candidate, target).expect("fixture locator normalizes");
    let usage =
        effects::provider_plan::EvaluatedBindingUsage::from_evaluator(1, 1, 1, 1, 0, 0, 1, 1, 1, 0)
            .expect("fixture usage");
    let receipt = effects::provider_plan::EvaluatedBindingReceipt::from_evaluation(
        None,
        "test::filesystem_binding".to_owned(),
        effects::provider_plan::EvaluatedBindingProducerClosureDigest::from_bytes([31; 32])
            .unwrap(),
        1,
        usage,
        effects::provider_plan::EvaluatedBindingEvaluationDigest::from_bytes([32; 32]).unwrap(),
        1,
        effects::provider_plan::EvaluatedBindingMaterializationDigest::from_bytes([33; 32])
            .unwrap(),
        locator.identity_digest(),
    )
    .expect("fixture receipt");
    let evaluated = effects::provider_plan::EvaluatedForeignImport::from_retained_evidence(
        locator.clone(),
        receipt,
    )
    .expect("fixture receipt matches locator");
    let mechanism = effects::NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
        &locator,
        effects::provider_plan::BoundaryCallingPlanCommitment::from_digest([34; 32]),
    );
    (evaluated, mechanism)
}

fn checked_port_write_mechanism(
    target: target::TargetProfile,
    port: u16,
) -> TerminalMechanismIdentity {
    CheckedPhysicalTerminalMechanismIdentity::port_write(target, port).into()
}

fn checked_port_write_policy(target: target::TargetProfile, port: u16) -> TerminalAuthorityPolicy {
    super::super::terminal_authority_policy::terminal_authority_policy_with_rows(vec![
        super::super::terminal_authority_policy::TerminalAuthorityPolicyRow::new(
            checked_port_write_mechanism(target, port),
            TerminalAuthorityDisposition::from_classes([TerminalAuthorityClass::PortIo]),
        ),
    ])
    .expect("one exact checked PortWrite row")
}

#[test]
fn intrinsic_leaf_requires_exact_service_permission() {
    let leaf = selected_plan(
        "leaf",
        "LeafProvider",
        LEAF_REQUIREMENT,
        ProviderBinding::CompilerIntrinsic {
            machine: "test::hosted_exit_process_i32".to_owned(),
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
        target::TargetProfile::LinuxX64,
        &plan,
        &selected,
        &physical,
        Some(&permitted),
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
    assert!(
        review_terminal_authority_closure(
            [7; 32],
            target::TargetProfile::LinuxX64,
            &plan,
            &selected,
            &physical,
            Some(&denied),
            &[intrinsic_mechanism(1)],
            &[],
        )
        .expect_err("exercised class exceeds empty permission")
        .contains("exceeds")
    );
    assert!(
            review_terminal_authority_closure(
                [7; 32],
                target::TargetProfile::LinuxX64,
                &plan,
                &selected,
                &physical,
                Some(&super::super::terminal_authority_permission_policy::current_terminal_authority_permission_policy()),
                &[intrinsic_mechanism(1)],
                &[],
            )
            .expect_err("missing exact permission rejects")
            .contains("no exact row")
        );
}

#[test]
fn absent_permission_policy_records_no_receiver_admission_claim() {
    let leaf = selected_plan(
        "leaf",
        "LeafProvider",
        LEAF_REQUIREMENT,
        ProviderBinding::CompilerIntrinsic {
            machine: "test::hosted_exit_process_i32".to_owned(),
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

    // `None` is not deny-all: physical classification and exercised authority
    // still review normally, but no leaf carries an adjudicated permission.
    let unclaimed = review_terminal_authority_closure(
        [7; 32],
        target::TargetProfile::LinuxX64,
        &plan,
        &selected,
        &physical,
        None,
        &[intrinsic_mechanism(1)],
        &[],
    )
    .expect("ordinary production reviews without a receiving policy");
    assert_eq!(unclaimed.leaves().len(), 1);
    assert_eq!(
        unclaimed.leaves()[0].exercised().classes(),
        &[TerminalAuthorityClass::ProcessTermination]
    );
    assert_eq!(unclaimed.leaves()[0].permitted(), None);
    assert_eq!(unclaimed.permission_policy(), None);
    unclaimed
        .validate()
        .expect("unclaimed receipt replays its canonical identity");

    // `None` is also not allow-all: it cannot alias any explicit admission.
    let disposition =
        TerminalAuthorityDisposition::from_classes([TerminalAuthorityClass::ProcessTermination]);
    let permitted = permission_policy(&[&leaf], disposition.clone());
    let claimed = review_terminal_authority_closure(
        [7; 32],
        target::TargetProfile::LinuxX64,
        &plan,
        &selected,
        &physical,
        Some(&permitted),
        &[intrinsic_mechanism(1)],
        &[],
    )
    .expect("explicit admission reviews the same closure");
    assert_eq!(claimed.leaves()[0].permitted(), Some(&disposition));
    assert_ne!(
        unclaimed.identity(),
        claimed.identity(),
        "absent and present receiver policies mint distinct review identities",
    );
}

#[test]
fn portable_filesystem_facets_cover_their_exact_selected_closure_rows() {
    const REQUIREMENT: &str = "test::FilesystemHost::operation()";
    let (evaluated, mechanism) = filesystem_foreign_fixture();
    let leaf = selected_plan(
        "filesystem-leaf",
        "FilesystemProvider",
        REQUIREMENT,
        ProviderBinding::Import { evaluated },
    );
    let selected =
        SelectedProviderPlanFacts::from_selected_plans(vec![leaf.clone()]).expect("selected leaf");
    let plan = abstract_plan(
        vec![boundary(1, REQUIREMENT)],
        Vec::new(),
        vec![function(1, &[1])],
    );

    for facet in PortableFilesystemAuthorityFacet::ALL {
        let disposition = TerminalAuthorityDisposition::from_filesystem_facets([facet]);
        let physical =
            super::super::terminal_authority_policy::terminal_authority_policy_with_rows(vec![
                super::super::terminal_authority_policy::TerminalAuthorityPolicyRow::new(
                    mechanism,
                    disposition.clone(),
                ),
            ])
            .expect("one exact normalized-foreign policy row");
        let permitted = super::super::terminal_authority_permission_policy::terminal_authority_permission_policy_with_rows(vec![
            ServiceTerminalAuthorityPermission::for_filesystem_facets(
                leaf.schema.identity_digest(),
                REQUIREMENT,
                [facet],
            ),
        ])
        .expect("exact filesystem permission row");
        let receipt = review_terminal_authority_closure(
            [29; 32],
            target::TargetProfile::LinuxX64,
            &plan,
            &selected,
            &physical,
            Some(&permitted),
            &[AdmittedTerminalMechanism {
                boundary: BoundaryMachineId::new(1).unwrap(),
                mechanism,
            }],
            &[],
        )
        .expect("the exact selected row covers its explicitly supplied filesystem facet");
        assert_eq!(receipt.leaves()[0].exercised(), &disposition);
        assert_eq!(receipt.leaves()[0].permitted(), Some(&disposition));
    }
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
            machine: "test::hosted_exit_process_i32".to_owned(),
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
        target::TargetProfile::LinuxX64,
        &plan,
        &selected,
        &physical,
        Some(&permitted),
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
            machine: "test::hosted_exit_process_i32".to_owned(),
        },
    );
    let selected =
        SelectedProviderPlanFacts::from_selected_plans(vec![leaf.clone()]).expect("selected leaf");
    let mut entry = function(1, &[]);
    entry.operations.insert(
        0,
        AbstractOperation::CallUnit {
            psi_operation: semantic_vocabulary::OperationId::new(1).unwrap(),
            callee: MachineId::new(2).unwrap(),
            arguments: Vec::new(),
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
        target::TargetProfile::LinuxX64,
        &plan,
        &selected,
        &physical,
        Some(&permitted),
        &[intrinsic_mechanism(1)],
        &[],
    )
    .expect("internal call reaches exact terminal leaf");
    assert_eq!(receipt.leaves().len(), 1);
    assert_eq!(receipt.leaves()[0].requirement_identity(), LEAF_REQUIREMENT);
}

#[test]
fn root_checked_physical_operation_has_no_provider_permission_context() {
    let plan = abstract_plan(
        Vec::new(),
        Vec::new(),
        vec![port_write_function(1, &[0x80])],
    );
    let selected =
        SelectedProviderPlanFacts::from_selected_plans(Vec::new()).expect("empty selected closure");
    assert!(
            review_terminal_authority_closure(
                [12; 32],
                target::TargetProfile::LinuxX64,
                &plan,
                &selected,
                &super::super::terminal_authority_policy::current_terminal_authority_policy(),
                Some(&super::super::terminal_authority_permission_policy::current_terminal_authority_permission_policy()),
                &[],
                &[],
            )
            .expect_err("root physical operation has no selected provider permission")
            .contains("no selected provider requirement custody")
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
                target::TargetProfile::LinuxX64,
                &plan,
                &selected,
                &super::super::terminal_authority_policy::current_terminal_authority_policy(),
                Some(&super::super::terminal_authority_permission_policy::current_terminal_authority_permission_policy()),
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
    let syscall_selected = SelectedProviderPlanFacts::from_selected_plans(vec![syscall.clone()])
        .expect("exact syscall role remains selectable data");
    let syscall_plan = abstract_plan(
        vec![boundary(1, LEAF_REQUIREMENT)],
        Vec::new(),
        vec![function(1, &[1])],
    );
    let syscall_boundary = BoundaryMachineId::new(1).unwrap();
    let mechanism =
        super::super::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            target::TargetProfile::LinuxX64,
            1,
            &syscall_plan,
            syscall_boundary,
        )
        .expect("verified zero-argument syscall has a conservative checked contract");
    let physical =
        super::super::terminal_authority_policy::terminal_authority_policy_with_rows(vec![
            super::super::terminal_authority_policy::TerminalAuthorityPolicyRow::new(
                mechanism,
                TerminalAuthorityDisposition::from_classes([]),
            ),
        ])
        .expect("exact syscall policy row");
    let permitted = permission_policy(&[&syscall], TerminalAuthorityDisposition::from_classes([]));
    let admitted = [AdmittedTerminalMechanism {
        boundary: syscall_boundary,
        mechanism,
    }];
    let receipt = review_terminal_authority_closure(
        [10; 32],
        target::TargetProfile::LinuxX64,
        &syscall_plan,
        &syscall_selected,
        &physical,
        Some(&permitted),
        &admitted,
        &[],
    )
    .expect("checked syscall identity reaches exact closure review");
    assert_eq!(receipt.leaves()[0].mechanism(), mechanism);

    let mut substituted = syscall.clone();
    substituted.rows[0].binding = ProviderBinding::Syscall { number: 2 };
    let substituted = SelectedProviderPlanFacts::from_selected_plans(vec![substituted])
        .expect("substituted syscall plan remains ordinary data");
    assert!(
        review_terminal_authority_closure(
            [10; 32],
            target::TargetProfile::LinuxX64,
            &syscall_plan,
            &substituted,
            &physical,
            Some(&permitted),
            &admitted,
            &[],
        )
        .expect_err("syscall number substitution rejects")
        .contains("substituted its selected binding role")
    );

    let TerminalMechanismIdentity::Syscall(exact_syscall) = mechanism else {
        unreachable!()
    };
    let wrong_contract: TerminalMechanismIdentity = SyscallTerminalMechanismIdentity::new(
        exact_syscall.target(),
        exact_syscall.number(),
        CheckedSyscallArgumentContractIdentity::from_digest([99; 32]),
    )
    .into();
    let wrong_policy =
        super::super::terminal_authority_policy::terminal_authority_policy_with_rows(vec![
            super::super::terminal_authority_policy::TerminalAuthorityPolicyRow::new(
                wrong_contract,
                TerminalAuthorityDisposition::from_classes([]),
            ),
        ])
        .expect("substituted contract policy remains ordinary data");
    assert!(
        review_terminal_authority_closure(
            [10; 32],
            target::TargetProfile::LinuxX64,
            &syscall_plan,
            &syscall_selected,
            &wrong_policy,
            Some(&permitted),
            &admitted,
            &[],
        )
        .expect_err("checked syscall contract substitution rejects")
        .contains("does not classify")
    );

    assert!(
        super::super::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            target::TargetProfile::WindowsX64,
            1,
            &syscall_plan,
            syscall_boundary,
        )
        .expect_err("unsupported syscall target rejects")
        .contains("does not support target")
    );
}

#[test]
fn filesystem_cohort_closure_admits_settled_rows_and_refuses_generic_release() {
    use super::super::{
        filesystem_host_permission_row, filesystem_mechanism_row,
        terminal_authority_permission_policy::terminal_authority_permission_policy_with_rows,
        terminal_authority_policy::terminal_authority_policy_with_rows,
    };

    let requirements: [(&str, i64, u8); 4] = [
        ("test::FilesystemHost::read()", 0, 7),
        ("test::FilesystemHost::sync()", 74, 9),
        ("test::FilesystemHost::read_link()", 89, 8),
        ("test::FilesystemHost::close()", 3, 4),
    ];
    let methods = requirements
        .iter()
        .map(|(requirement, ..)| service_method(requirement))
        .collect::<Vec<_>>();
    let plan_for = |rows: &[usize]| ProviderPlan {
        name: "filesystem".to_owned(),
        provider_type: "FilesystemProvider".to_owned(),
        provider_type_package_identity: None,
        target: "linux_x86_64".to_owned(),
        schema: ServiceSchema {
            trait_name: "test::FilesystemHost".to_owned(),
            trait_package_identity: None,
            methods: methods.clone(),
        },
        rows: rows
            .iter()
            .map(|index| ProviderPlanRow {
                method: methods[*index].name.clone(),
                requirement_identity: methods[*index].requirement_identity.clone(),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::Syscall {
                    number: requirements[*index].1,
                },
            })
            .collect(),
        origin_package_identity: None,
        origin_package: "test".to_owned(),
    };
    let provider_plan = plan_for(&[0, 1, 2, 3]);
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![provider_plan.clone()])
        .expect("selected filesystem");
    let boundaries = requirements
        .iter()
        .enumerate()
        .map(|(index, (requirement, ..))| boundary(index as u32 + 1, requirement))
        .collect::<Vec<_>>();
    let mechanisms = requirements
        .iter()
        .enumerate()
        .map(|(index, (_, number, contract))| AdmittedTerminalMechanism {
            boundary: BoundaryMachineId::new(index as u64 + 1).unwrap(),
            mechanism: SyscallTerminalMechanismIdentity::new(
                target::TargetProfile::LinuxX64,
                *number as u32,
                CheckedSyscallArgumentContractIdentity::from_digest([*contract; 32]),
            )
            .into(),
        })
        .collect::<Vec<_>>();
    // The settled cohorts earn exact mechanism rows; a generic close mechanism
    // earns none under the settled control/lifecycle policy.
    let physical = terminal_authority_policy_with_rows(
        methods[..3]
            .iter()
            .zip(&mechanisms[..3])
            .map(|(method, admitted)| {
                filesystem_mechanism_row(admitted.mechanism, method)
                    .expect("settled cohort emits one exact mechanism row")
            })
            .collect(),
    )
    .expect("exact cohort classification rows");
    assert_eq!(
        filesystem_mechanism_row(mechanisms[3].mechanism, &methods[3]),
        Err(super::super::UnsettledFilesystemRequirement::OrdinaryReleaseContract),
        "generic close cannot inherit the occurrence-specific release contract"
    );
    // The consumer permission table covers all four requirements; the close
    // cohort's explicit empty row retains service reach and review identity.
    let permitted = terminal_authority_permission_policy_with_rows(
        methods
            .iter()
            .map(|method| {
                filesystem_host_permission_row(provider_plan.schema.identity_digest(), method)
                    .expect("every canonical requirement has a justified permission")
            })
            .collect(),
    )
    .expect("exact filesystem permission table");

    let error = review_terminal_authority_closure(
        [31; 32],
        target::TargetProfile::LinuxX64,
        &abstract_plan(
            boundaries.clone(),
            Vec::new(),
            vec![function(1, &[1, 2, 3, 4])],
        ),
        &selected,
        &physical,
        Some(&permitted),
        &mechanisms,
        &[],
    )
    .expect_err("the unclassified release mechanism stays fail-closed");
    assert!(error.contains("does not classify"), "{error}");
    assert!(error.contains("close"), "{error}");

    let receipt = review_terminal_authority_closure(
        [31; 32],
        target::TargetProfile::LinuxX64,
        &abstract_plan(boundaries, Vec::new(), vec![function(1, &[1, 2, 3])]),
        &selected,
        &physical,
        Some(&permitted),
        &mechanisms[..3],
        &[],
    )
    .expect("settled cohorts admit their exact rows");
    assert_eq!(receipt.leaves().len(), 3);
    let leaf = receipt
        .leaves()
        .iter()
        .find(|leaf| leaf.requirement_identity() == "test::FilesystemHost::sync()")
        .expect("the explicit empty leaf retains its exact review identity");
    assert!(leaf.exercised().is_authority_class_empty());
    assert!(
        leaf.permitted()
            .expect("adjudicated leaf")
            .is_authority_class_empty()
    );
    let leaf = receipt
        .leaves()
        .iter()
        .find(|leaf| leaf.requirement_identity() == "test::FilesystemHost::read_link()")
        .expect("read_link retains its exact leaf");
    assert_eq!(
        leaf.exercised().classes(),
        &[TerminalAuthorityClass::FilesystemMetadataQuery]
    );
    receipt.validate().expect("canonical receipt replays");
}

#[test]
fn filesystem_cohort_closure_admits_constrained_release_occurrence() {
    use super::super::{
        filesystem_host_permission_row, filesystem_mechanism_row,
        filesystem_ordinary_release_contract, filesystem_release_mechanism_row,
        terminal_authority_permission_policy::terminal_authority_permission_policy_with_rows,
        terminal_authority_policy::terminal_authority_policy_with_rows,
    };

    let requirements: [(&str, i64, u8); 4] = [
        ("test::FilesystemHost::read()", 0, 7),
        ("test::FilesystemHost::sync()", 74, 9),
        ("test::FilesystemHost::read_link()", 89, 8),
        ("test::FilesystemHost::close()", 3, 4),
    ];
    let methods = requirements
        .iter()
        .map(|(requirement, ..)| service_method(requirement))
        .collect::<Vec<_>>();
    let provider_plan = ProviderPlan {
        name: "filesystem".to_owned(),
        provider_type: "FilesystemProvider".to_owned(),
        provider_type_package_identity: None,
        target: "linux_x86_64".to_owned(),
        schema: ServiceSchema {
            trait_name: "test::FilesystemHost".to_owned(),
            trait_package_identity: None,
            methods: methods.clone(),
        },
        rows: requirements
            .iter()
            .zip(&methods)
            .map(|((_, number, _), method)| ProviderPlanRow {
                method: method.name.clone(),
                requirement_identity: method.requirement_identity.clone(),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::Syscall { number: *number },
            })
            .collect(),
        origin_package_identity: None,
        origin_package: "test".to_owned(),
    };
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![provider_plan.clone()])
        .expect("selected filesystem");
    let boundaries = requirements
        .iter()
        .enumerate()
        .map(|(index, (requirement, ..))| boundary(index as u32 + 1, requirement))
        .collect::<Vec<_>>();
    // The proved constrained close occurrence binds the retained release
    // contract into its mechanism key; the unconstrained sibling keeps an
    // ordinary contract and is a different key, not a competing row.
    let release_contract = filesystem_ordinary_release_contract([9; 32]);
    let mechanism_for = |index: usize| AdmittedTerminalMechanism {
        boundary: BoundaryMachineId::new(index as u64 + 1).unwrap(),
        mechanism: SyscallTerminalMechanismIdentity::new(
            target::TargetProfile::LinuxX64,
            requirements[index].1 as u32,
            if index == 3 {
                release_contract.checked_argument_contract()
            } else {
                CheckedSyscallArgumentContractIdentity::from_digest([requirements[index].2; 32])
            },
        )
        .into(),
    };
    let mechanisms = (0..4).map(mechanism_for).collect::<Vec<_>>();
    let physical = terminal_authority_policy_with_rows(
        methods[..3]
            .iter()
            .zip(&mechanisms[..3])
            .map(|(method, admitted)| {
                filesystem_mechanism_row(admitted.mechanism, method)
                    .expect("settled cohort emits one exact mechanism row")
            })
            .chain(std::iter::once(
                filesystem_release_mechanism_row(
                    mechanisms[3].mechanism,
                    &methods[3],
                    release_contract,
                )
                .expect("the constrained close earns its evidence-bound empty row"),
            ))
            .collect(),
    )
    .expect("exact cohort classification rows including the release row");
    let permitted = terminal_authority_permission_policy_with_rows(
        methods
            .iter()
            .map(|method| {
                filesystem_host_permission_row(provider_plan.schema.identity_digest(), method)
                    .expect("every canonical requirement has a justified permission")
            })
            .collect(),
    )
    .expect("exact filesystem permission table");

    let receipt = review_terminal_authority_closure(
        [31; 32],
        target::TargetProfile::LinuxX64,
        &abstract_plan(
            boundaries.clone(),
            Vec::new(),
            vec![function(1, &[1, 2, 3, 4])],
        ),
        &selected,
        &physical,
        Some(&permitted),
        &mechanisms,
        &[],
    )
    .expect("the constrained close occurrence admits under its bound contract");
    assert_eq!(receipt.leaves().len(), 4);
    let leaf = receipt
        .leaves()
        .iter()
        .find(|leaf| leaf.requirement_identity() == "test::FilesystemHost::close()")
        .expect("the release leaf retains its exact review identity");
    assert!(leaf.exercised().is_authority_class_empty());
    assert!(
        leaf.permitted()
            .expect("adjudicated leaf")
            .is_authority_class_empty()
    );
    receipt.validate().expect("canonical receipt replays");

    // An unconstrained close mechanism under the same number does not inherit
    // the bound row: its key differs and classification stays fail-closed.
    let mut substituted = mechanisms.clone();
    substituted[3].mechanism = SyscallTerminalMechanismIdentity::new(
        target::TargetProfile::LinuxX64,
        3,
        CheckedSyscallArgumentContractIdentity::from_digest([4; 32]),
    )
    .into();
    let error = review_terminal_authority_closure(
        [31; 32],
        target::TargetProfile::LinuxX64,
        &abstract_plan(boundaries, Vec::new(), vec![function(1, &[1, 2, 3, 4])]),
        &selected,
        &physical,
        Some(&permitted),
        &substituted,
        &[],
    )
    .expect_err("an unconstrained close does not inherit the bound row");
    assert!(error.contains("does not classify"), "{error}");
    assert!(error.contains("close"), "{error}");
}

#[test]
fn filesystem_release_occurrence_review_binds_the_retained_record() {
    use super::super::terminal_authority_policy::filesystem_native_handle_query_release_contracts;
    use super::super::{
        filesystem_host_permission_row, filesystem_release_mechanism_row,
        terminal_authority_permission_policy::terminal_authority_permission_policy_with_rows,
        terminal_authority_policy::terminal_authority_policy_with_rows,
    };

    let limits = build_evaluation::BuildFilesystemReplayRecordLimits::default();
    let record = build_evaluation::capture_verified_build_filesystem_replay_record(
        &build_evaluation::test_support::replayable_native_handle_query_chain_summary(&[(
            b"pkg/main.omg",
            7,
        )]),
        limits,
    )
    .expect("the retained query-release chain encodes")
    .expect("the verified query-release chain keeps replay custody");
    let contracts = filesystem_native_handle_query_release_contracts(&record, limits)
        .expect("the retained occurrence realizes its release contract");
    let [contract] = contracts.as_slice() else {
        panic!("one retained occurrence derives exactly one release contract")
    };

    let requirement = "test::FilesystemHost::close_handle()";
    let method = service_method(requirement);
    let provider_plan = ProviderPlan {
        name: "filesystem".to_owned(),
        provider_type: "FilesystemProvider".to_owned(),
        provider_type_package_identity: None,
        target: "linux_x86_64".to_owned(),
        schema: ServiceSchema {
            trait_name: "test::FilesystemHost".to_owned(),
            trait_package_identity: None,
            methods: vec![method.clone()],
        },
        rows: vec![ProviderPlanRow {
            method: method.name.clone(),
            requirement_identity: method.requirement_identity.clone(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::Syscall { number: 3 },
        }],
        origin_package_identity: None,
        origin_package: "test".to_owned(),
    };
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![provider_plan.clone()])
        .expect("selected filesystem close");
    let mechanisms = vec![AdmittedTerminalMechanism {
        boundary: BoundaryMachineId::new(1).unwrap(),
        mechanism: SyscallTerminalMechanismIdentity::new(
            target::TargetProfile::LinuxX64,
            3,
            contract.checked_argument_contract(),
        )
        .into(),
    }];
    let physical = terminal_authority_policy_with_rows(vec![
        filesystem_release_mechanism_row(mechanisms[0].mechanism, &method, *contract)
            .expect("the retained occurrence earns its evidence-bound empty row"),
    ])
    .expect("exact release policy");
    let permitted = terminal_authority_permission_policy_with_rows(vec![
        filesystem_host_permission_row(provider_plan.schema.identity_digest(), &method)
            .expect("the canonical release requirement has a justified permission"),
    ])
    .expect("exact filesystem permission table");

    let receipt = review_terminal_authority_closure(
        [31; 32],
        target::TargetProfile::LinuxX64,
        &abstract_plan(
            vec![boundary(1, requirement)],
            Vec::new(),
            vec![function(1, &[1])],
        ),
        &selected,
        &physical,
        Some(&permitted),
        &mechanisms,
        &[],
    )
    .expect("the constrained close leaf admits under the retained record's contract");
    let [leaf] = receipt.leaves() else {
        panic!("one requirement admits exactly one leaf")
    };
    assert_eq!(leaf.requirement_identity(), requirement);
    assert!(leaf.exercised().is_authority_class_empty());
    assert!(
        leaf.permitted()
            .expect("adjudicated leaf")
            .is_authority_class_empty()
    );
    receipt.validate().expect("canonical receipt replays");

    // A mechanism minted from a different record's occurrence is a stale or
    // substituted proof for this review: it stays unclassified.
    let foreign_record = build_evaluation::capture_verified_build_filesystem_replay_record(
        &build_evaluation::test_support::replayable_native_handle_query_chain_summary(&[(
            b"pkg/other.omg",
            7,
        )]),
        limits,
    )
    .expect("the second chain encodes")
    .expect("the second verified chain keeps custody");
    let foreign_contracts =
        filesystem_native_handle_query_release_contracts(&foreign_record, limits)
            .expect("the second retained occurrence realizes its contract");
    let [foreign_contract] = foreign_contracts.as_slice() else {
        panic!("the second occurrence derives one contract")
    };
    assert_ne!(contract, foreign_contract);
    let substituted = vec![AdmittedTerminalMechanism {
        boundary: BoundaryMachineId::new(1).unwrap(),
        mechanism: SyscallTerminalMechanismIdentity::new(
            target::TargetProfile::LinuxX64,
            3,
            foreign_contract.checked_argument_contract(),
        )
        .into(),
    }];
    let error = review_terminal_authority_closure(
        [31; 32],
        target::TargetProfile::LinuxX64,
        &abstract_plan(
            vec![boundary(1, requirement)],
            Vec::new(),
            vec![function(1, &[1])],
        ),
        &selected,
        &physical,
        Some(&permitted),
        &substituted,
        &[],
    )
    .expect_err("another record's occurrence does not inherit this row");
    assert!(error.contains("does not classify"), "{error}");
    assert!(error.contains("close_handle"), "{error}");
}

#[test]
fn foreign_release_occurrence_review_binds_the_retained_record() {
    use super::super::terminal_authority_policy::filesystem_native_handle_query_release_contracts;
    use super::super::{
        filesystem_host_permission_row, filesystem_mechanism_row, filesystem_release_mechanism_row,
        terminal_authority_permission_policy::terminal_authority_permission_policy_with_rows,
        terminal_authority_policy::{
            UnsettledFilesystemRequirement, terminal_authority_policy_with_rows,
        },
    };

    let limits = build_evaluation::BuildFilesystemReplayRecordLimits::default();
    let record = build_evaluation::capture_verified_build_filesystem_replay_record(
        &build_evaluation::test_support::replayable_native_handle_query_chain_summary(&[(
            b"pkg/main.omg",
            7,
        )]),
        limits,
    )
    .expect("the retained query-release chain encodes")
    .expect("the verified query-release chain keeps replay custody");
    let contracts = filesystem_native_handle_query_release_contracts(&record, limits)
        .expect("the retained occurrence realizes its release contract");
    let [contract] = contracts.as_slice() else {
        panic!("one retained occurrence derives exactly one release contract")
    };

    // The Windows realization of `close_handle` is a `kernel32!CloseHandle`
    // import, not a direct syscall: the constrained occurrence keeps the
    // admitted calling plan and carries the retained contract as its checked
    // coordinate.
    let requirement = "test::FilesystemHost::close_handle()";
    let method = service_method(requirement);
    let (evaluated, unconstrained) = foreign_import_fixture(
        target::ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"CloseHandle".to_vec(),
        },
        target::TargetProfile::WindowsX64,
    );
    let bound: TerminalMechanismIdentity = unconstrained
        .with_checked_argument_contract(contract.checked_argument_contract())
        .into();
    let provider_plan = ProviderPlan {
        name: "filesystem".to_owned(),
        provider_type: "FilesystemProvider".to_owned(),
        provider_type_package_identity: None,
        target: "windows_x86_64".to_owned(),
        schema: ServiceSchema {
            trait_name: "test::FilesystemHost".to_owned(),
            trait_package_identity: None,
            methods: vec![method.clone()],
        },
        rows: vec![ProviderPlanRow {
            method: method.name.clone(),
            requirement_identity: method.requirement_identity.clone(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::Import { evaluated },
        }],
        origin_package_identity: None,
        origin_package: "test".to_owned(),
    };
    let selected = SelectedProviderPlanFacts::from_selected_plans(vec![provider_plan.clone()])
        .expect("selected filesystem close_handle import");
    let mechanisms = vec![AdmittedTerminalMechanism {
        boundary: BoundaryMachineId::new(1).unwrap(),
        mechanism: bound,
    }];
    let physical = terminal_authority_policy_with_rows(vec![
        filesystem_release_mechanism_row(bound, &method, *contract)
            .expect("the retained occurrence earns its evidence-bound empty row"),
    ])
    .expect("exact release policy");
    assert_eq!(
        filesystem_mechanism_row(bound, &method),
        Err(UnsettledFilesystemRequirement::OrdinaryReleaseContract),
        "the generic emitter never classifies a release cohort"
    );
    let permitted = terminal_authority_permission_policy_with_rows(vec![
        filesystem_host_permission_row(provider_plan.schema.identity_digest(), &method)
            .expect("the canonical release requirement has a justified permission"),
    ])
    .expect("exact filesystem permission table");

    let plan = abstract_plan(
        vec![boundary(1, requirement)],
        Vec::new(),
        vec![function(1, &[1])],
    );
    let receipt = review_terminal_authority_closure(
        [31; 32],
        target::TargetProfile::WindowsX64,
        &plan,
        &selected,
        &physical,
        Some(&permitted),
        &mechanisms,
        &[],
    )
    .expect("the constrained close_handle import admits under the retained record's contract");
    let [leaf] = receipt.leaves() else {
        panic!("one requirement admits exactly one leaf")
    };
    assert_eq!(leaf.requirement_identity(), requirement);
    assert_eq!(leaf.mechanism(), bound);
    assert!(leaf.exercised().is_authority_class_empty());
    assert!(
        leaf.permitted()
            .expect("adjudicated leaf")
            .is_authority_class_empty()
    );
    receipt.validate().expect("canonical receipt replays");

    // The unconstrained import of the same symbol under the same admitted
    // plan does not inherit the bound row: the admitted plan alone is not
    // narrowing evidence, so classification stays fail-closed.
    let substituted = vec![AdmittedTerminalMechanism {
        boundary: BoundaryMachineId::new(1).unwrap(),
        mechanism: unconstrained.into(),
    }];
    let error = review_terminal_authority_closure(
        [31; 32],
        target::TargetProfile::WindowsX64,
        &plan,
        &selected,
        &physical,
        Some(&permitted),
        &substituted,
        &[],
    )
    .expect_err("the unconstrained import does not inherit the bound row");
    assert!(error.contains("does not classify"), "{error}");
    assert!(error.contains("close_handle"), "{error}");
}
