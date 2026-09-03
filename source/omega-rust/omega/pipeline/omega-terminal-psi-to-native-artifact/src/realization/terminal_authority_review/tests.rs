use super::*;
use omega_abstract_operations::{AbstractFunction, AbstractOperation, AbstractOperationPlan};
use omega_effects::{
    CheckedPhysicalTerminalMechanismIdentity, CheckedSyscallArgumentContractIdentity,
    CompilerIntrinsicExecutionIdentity, PortableFilesystemAuthorityFacet,
    ServiceTerminalAuthorityPermission, SyscallTerminalMechanismIdentity, TerminalAuthorityClass,
    TerminalAuthorityDisposition, TerminalMechanismIdentity,
    provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema},
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
        result: psi_terminal::BoundaryMachineResult::Unit,
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
            result: omega_abstract_operations::AbstractBoundaryResult::Unit,
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

fn port_write_function(machine: u32, ports: &[u16]) -> AbstractFunction {
    let service = psi_core::ServiceId::new(1).unwrap();
    let mut function = function(machine, &[]);
    function.published_service_ceiling = vec![service];
    for (index, port) in ports.iter().enumerate() {
        function.operations.insert(
            index,
            AbstractOperation::PortWrite {
                psi_operation: psi_core::OperationId::new(
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

fn installed_port_write_candidate(
    boundary: u32,
    requirement: &str,
    provider: &str,
    candidate_identity: &str,
    machine: u32,
) -> psi_terminal::ProviderCandidateConformance {
    let mut candidate =
        installed_candidate(boundary, requirement, provider, candidate_identity, machine);
    candidate.refinement.realized_service_ceiling = vec![psi_core::ServiceId::new(1).unwrap()];
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
        mechanism: CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32.into(),
    }
}

fn filesystem_foreign_fixture() -> (
    omega_effects::provider_plan::EvaluatedForeignImport,
    TerminalMechanismIdentity,
) {
    let locator = omega_target::normalize_foreign_locator(
        omega_target::ForeignLocatorCandidate::ElfVersioned {
            object: b"libfixture.so".to_vec(),
            symbol: b"filesystem_operation".to_vec(),
            version: b"FIXTURE_1".to_vec(),
        },
        omega_target::TargetProfile::LinuxX64,
    )
    .expect("fixture locator normalizes");
    let usage = omega_effects::provider_plan::EvaluatedBindingUsage::from_evaluator(
        1, 1, 1, 1, 0, 0, 1, 1, 1, 0,
    )
    .expect("fixture usage");
    let receipt = omega_effects::provider_plan::EvaluatedBindingReceipt::from_evaluation(
        None,
        "test::filesystem_binding".to_owned(),
        omega_effects::provider_plan::EvaluatedBindingProducerClosureDigest::from_bytes([31; 32])
            .unwrap(),
        1,
        usage,
        omega_effects::provider_plan::EvaluatedBindingEvaluationDigest::from_bytes([32; 32])
            .unwrap(),
        1,
        omega_effects::provider_plan::EvaluatedBindingMaterializationDigest::from_bytes([33; 32])
            .unwrap(),
        locator.identity_digest(),
    )
    .expect("fixture receipt");
    let evaluated = omega_effects::provider_plan::EvaluatedForeignImport::from_retained_evidence(
        locator.clone(),
        receipt,
    )
    .expect("fixture receipt matches locator");
    let mechanism =
        omega_effects::NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
            &locator,
            omega_effects::provider_plan::BoundaryCallingPlanCommitment::from_digest([34; 32]),
        )
        .into();
    (evaluated, mechanism)
}

fn checked_port_write_mechanism(
    target: omega_target::TargetProfile,
    port: u16,
) -> TerminalMechanismIdentity {
    CheckedPhysicalTerminalMechanismIdentity::port_write(target, port).into()
}

fn checked_port_write_policy(
    target: omega_target::TargetProfile,
    port: u16,
) -> TerminalAuthorityPolicy {
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
        omega_target::TargetProfile::LinuxX64,
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
    assert!(
        review_terminal_authority_closure(
            [7; 32],
            omega_target::TargetProfile::LinuxX64,
            &plan,
            &selected,
            &physical,
            &denied,
            &[intrinsic_mechanism(1)],
            &[],
        )
        .expect_err("exercised class exceeds empty permission")
        .contains("exceeds")
    );
    assert!(
            review_terminal_authority_closure(
                [7; 32],
                omega_target::TargetProfile::LinuxX64,
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
            omega_target::TargetProfile::LinuxX64,
            &plan,
            &selected,
            &physical,
            &permitted,
            &[AdmittedTerminalMechanism {
                boundary: BoundaryMachineId::new(1).unwrap(),
                mechanism,
            }],
            &[],
        )
        .expect("the exact selected row covers its explicitly supplied filesystem facet");
        assert_eq!(receipt.leaves()[0].exercised(), &disposition);
        assert_eq!(receipt.leaves()[0].permitted(), &disposition);
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
        omega_target::TargetProfile::LinuxX64,
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
        omega_target::TargetProfile::LinuxX64,
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
                omega_target::TargetProfile::LinuxX64,
                &plan,
                &selected,
                &super::super::terminal_authority_policy::current_terminal_authority_policy(),
                &super::super::terminal_authority_permission_policy::current_terminal_authority_permission_policy(),
                &[],
                &[],
            )
            .expect_err("root physical operation has no selected provider permission")
            .contains("no selected provider requirement custody")
        );
}

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
        super::super::terminal_authority_policy::current_terminal_authority_policy(),
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
    let physical = super::super::terminal_authority_policy::terminal_authority_policy_with_rows(
        [0x03f8, 0x0080]
            .into_iter()
            .map(|port| {
                super::super::terminal_authority_policy::TerminalAuthorityPolicyRow::new(
                    checked_port_write_mechanism(omega_target::TargetProfile::LinuxX64, port),
                    TerminalAuthorityDisposition::from_classes([TerminalAuthorityClass::PortIo]),
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
                omega_target::TargetProfile::LinuxX64,
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
            omega_target::TargetProfile::LinuxX64,
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
        omega_target::TargetProfile::LinuxX64,
        &syscall_plan,
        &syscall_selected,
        &physical,
        &permitted,
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
            omega_target::TargetProfile::LinuxX64,
            &syscall_plan,
            &substituted,
            &physical,
            &permitted,
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
            omega_target::TargetProfile::LinuxX64,
            &syscall_plan,
            &syscall_selected,
            &wrong_policy,
            &permitted,
            &admitted,
            &[],
        )
        .expect_err("checked syscall contract substitution rejects")
        .contains("does not classify")
    );

    assert!(
        super::super::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            omega_target::TargetProfile::WindowsX64,
            1,
            &syscall_plan,
            syscall_boundary,
        )
        .expect_err("unsupported syscall target rejects")
        .contains("does not support target")
    );
}
