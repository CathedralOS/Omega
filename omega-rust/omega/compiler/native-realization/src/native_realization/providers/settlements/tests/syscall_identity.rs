//! Import coverage joins and checked direct-syscall mechanism derivation.
use super::{ProviderBinding, import_plan};
use crate::native_realization::providers::settlements::validate_source_evaluated_import_coverage;

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
        structural_types: Vec::new().into(),
        boundary_machines: vec![terminal_psi::BoundaryMachineDeclaration {
            fixed_service_reach: Vec::new(),
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
            crash_routes: Vec::new(),
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
                structural_parameters: Vec::new(),
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
        .make_mut()
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
fn coverage_rejects_missing_and_duplicate_external_rows_before_policy_lookup() {
    let profile = target::TargetProfile::LinuxX64;
    let plan = abstract_plan();
    let selected = syscall_plan(profile, 1);
    let policy = crate::native_realization::current_terminal_authority_policy();
    let external = external(profile, 1);
    for (rows, count) in [(Vec::new(), 0), (vec![external.clone(), external], 2)] {
        let error = validate_source_evaluated_import_coverage(
            &plan,
            &selected,
            &policy,
            profile.native_target(),
            &rows,
            &[],
            &[],
        )
        .expect_err("external multiplicity is independently checked");
        assert_eq!(
            error[0].message,
            format!(
                "demanded syscall `{REQUIREMENT}` resolves to {count} retained external binding rows"
            )
        );
    }
}

#[test]
fn coverage_counts_uncalled_boundary_aliases_and_preserves_error_precedence() {
    let profile = target::TargetProfile::LinuxX64;
    let mut plan = abstract_plan();
    let mut alias = plan.boundary_machines[0].clone();
    alias.id = semantic_vocabulary::BoundaryMachineId::new(851).unwrap();
    plan.boundary_machines.push(alias);
    let selected = syscall_plan(profile, 1);
    let policy = crate::native_realization::current_terminal_authority_policy();
    for (number, expected) in [
        (2, "substituted its normalized syscall number"),
        (1, "resolves to 2 Terminal boundaries"),
    ] {
        let error = validate_source_evaluated_import_coverage(
            &plan,
            &selected,
            &policy,
            profile.native_target(),
            &[external(profile, number)],
            &[],
            &[],
        )
        .expect_err("uncalled declarations still participate in exact boundary identity");
        assert_eq!(
            error[0].message,
            format!("demanded syscall `{REQUIREMENT}` {expected}")
        );
    }
}

fn callback_occurrence_row(
    operation: semantic_vocabulary::OperationId,
) -> abstract_operations_to_target_operations::AdmittedNativeCallbackArgument {
    let shape = calling_conventions::ValueShape::integer(8, 8);
    let registrar = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        calling_conventions::CallingPolicy::native_for_target(target::NativeTarget::linux_x64()),
        &calling_conventions::CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap()
    .plan()
    .clone();
    abstract_operations_to_target_operations::AdmittedNativeCallbackArgument {
        terminal_operation: operation,
        placement_index: 0,
        callback_function: function_identity::MachineFunctionIdentity::callback_thunk(
            function_identity::StateKey {
                machine: symbols::SymbolHandle::from_parts(1, 1),
                state: symbols::SymbolHandle::from_parts(2, 1),
                segment_index: 0,
            },
            0,
        )
        .unwrap(),
        application: calling_conventions::NativeParameterApplication {
            parameter: calling_conventions::NativeParameterId::new(1).unwrap(),
            native_ordinal: 0,
            shape,
            placement: registrar.call.parameters[0].clone(),
        },
        registrar_boundary_entry_plan: registrar,
        registrar_context: calling_conventions::CallbackMaterializationContext::default(),
        registrar_application_commitment: [1; 32],
    }
}

#[test]
fn coverage_callback_join_counts_duplicate_operations_and_keeps_callback_order() {
    let mut plan = abstract_plan();
    let operation = semantic_vocabulary::OperationId::new(850).unwrap();
    let missing = semantic_vocabulary::OperationId::new(849).unwrap();
    let selected = effects::SelectedProviderPlanFacts::default();
    let policy = crate::native_realization::current_terminal_authority_policy();
    let callback = callback_occurrence_row(operation);
    validate_source_evaluated_import_coverage(
        &plan,
        &selected,
        &policy,
        target::NativeTarget::linux_x64(),
        &[],
        &[],
        std::slice::from_ref(&callback),
    )
    .expect("one occurrence joins; this does not claim callback publication admission");
    let duplicate = plan.functions[0].operations[0].clone();
    plan.functions[0].operations.push(duplicate);
    let absent = callback_occurrence_row(missing);
    for (callbacks, rejected, count) in [
        (vec![callback.clone(), absent.clone()], operation, 2),
        (vec![absent, callback], missing, 0),
    ] {
        let error = validate_source_evaluated_import_coverage(
            &plan,
            &selected,
            &policy,
            target::NativeTarget::linux_x64(),
            &[],
            &[],
            &callbacks,
        )
        .expect_err("callback rows retain input-order diagnostics despite sorted lookup keys");
        assert_eq!(
            error[0].message,
            format!(
                "native callback operation {} resolves to {count} abstract boundary calls during source-import coverage",
                rejected.get(),
            )
        );
    }
}

#[test]
fn import_coverage_preserves_callback_multiplicity_and_registrar_plan_rejection() {
    let profile = target::TargetProfile::LinuxX64;
    let plan = abstract_plan();
    let selected = effects::SelectedProviderPlanFacts::from_selected_plans(vec![import_plan(
        b"leaf", profile,
    )])
    .unwrap();
    let policy = crate::native_realization::current_terminal_authority_policy();
    let callback = callback_occurrence_row(semantic_vocabulary::OperationId::new(850).unwrap());
    let ProviderBinding::Import { evaluated } = &selected.plans()[0].rows[0].binding else {
        panic!("fixture retains a normalized import")
    };
    let mut external = external(profile, 1);
    external.binding = calling_conventions::ExternalBindingKind::Import {
        locator: evaluated.locator().clone(),
    };
    external.boundary_entry_plan = Some(callback.registrar_boundary_entry_plan.clone());
    let mut mismatched = callback.clone();
    mismatched
        .registrar_boundary_entry_plan
        .call
        .parameters
        .clear();
    for callbacks in [vec![callback.clone(), callback], vec![mismatched]] {
        let error = validate_source_evaluated_import_coverage(
            &plan,
            &selected,
            &policy,
            profile.native_target(),
            std::slice::from_ref(&external),
            &[],
            &callbacks,
        )
        .expect_err("one matching registrar plan remains mandatory after occurrence joins");
        assert_eq!(
            error[0].message,
            format!(
                "demanded normalized import `{REQUIREMENT}` has an invalid admitted implementation contract: retained implementation contract rejoins {} exact native callbacks with no unique matching registrar plan",
                callbacks.len(),
            )
        );
    }
}

#[test]
fn uncalled_selected_import_needs_no_settlement_but_cannot_hide_an_orphan_callback() {
    let profile = target::TargetProfile::LinuxX64;
    let mut plan = abstract_plan();
    plan.functions[0].operations.remove(0);
    let selected = effects::SelectedProviderPlanFacts::from_selected_plans(vec![import_plan(
        b"leaf", profile,
    )])
    .unwrap();
    let policy = crate::native_realization::current_terminal_authority_policy();
    let admitted = validate_source_evaluated_import_coverage(
        &plan,
        &selected,
        &policy,
        profile.native_target(),
        &[],
        &[],
        &[],
    )
    .expect("unreachable selection retains identity without execution inputs")
    .0;
    assert!(admitted.is_empty());
    let callback = callback_occurrence_row(semantic_vocabulary::OperationId::new(850).unwrap());
    let error = validate_source_evaluated_import_coverage(
        &plan,
        &selected,
        &policy,
        profile.native_target(),
        &[],
        &[],
        &[callback],
    )
    .expect_err("empty demand cannot hide an orphan callback occurrence");
    assert_eq!(
        error[0].message,
        "native callback operation 850 resolves to 0 abstract boundary calls during source-import coverage"
    );
}

#[test]
fn conservative_contract_commits_scalar_carriers_and_rejects_occurrence_drift() {
    let profile = target::TargetProfile::LinuxX64;
    let mut plan = abstract_plan();
    let boundary = plan.boundary_machines[0].id;
    let empty =
        crate::native_realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect("empty verified signature has an exact conservative contract");

    plan.boundary_machines[0].scalar_parameters = vec![semantic_vocabulary::ScalarType::Boolean];
    assert!(
        crate::native_realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
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
        crate::native_realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
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
        crate::native_realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
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
        crate::native_realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
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
        crate::native_realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
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
        crate::native_realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect("verified boundary supplies the conservative checked contract");
    let policy = crate::native_realization::terminal_authority_policy_with_rows(vec![
        crate::native_realization::TerminalAuthorityPolicyRow::new(
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
    .expect("provider settlement derives the checked syscall identity")
    .0;
    assert_eq!(
        admitted,
        vec![
            crate::native_realization::providers::AdmittedTerminalMechanism {
                boundary,
                mechanism,
            }
        ]
    );
    let selected_plan = &selected.plans()[0];
    let permission =
        crate::native_realization::terminal_authority_permission_policy_with_rows(vec![
            crate::native_realization::TerminalAuthorityPermissionPolicyRow::new(
                selected_plan.schema.identity_digest(),
                REQUIREMENT,
                effects::TerminalAuthorityDisposition::from_classes([]),
            ),
        ])
        .expect("exact syscall requirement permission");
    let receipt =
        crate::native_realization::terminal_authority_review::review_terminal_authority_closure(
            [0x86; 32],
            profile,
            &plan,
            &selected,
            &policy,
            Some(&permission),
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
        &crate::native_realization::current_terminal_authority_policy(),
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

/// A selected `close_handle` syscall plan: the same requirement the rest of
/// this file demands, but served by a canonical `FilesystemHost` release
/// cohort method so occurrence-bound keys may classify.
fn filesystem_release_syscall_plan(
    profile: target::TargetProfile,
    number: i64,
) -> effects::SelectedProviderPlanFacts {
    let mut plan = import_plan(b"unused", profile);
    plan.schema.trait_name = "omega::test::FilesystemHost".into();
    plan.schema.methods[0].name = "close_handle".into();
    plan.rows[0].method = "close_handle".into();
    plan.rows[0].binding = ProviderBinding::Syscall { number };
    effects::SelectedProviderPlanFacts::from_selected_plans(vec![plan])
        .expect("one exact filesystem release syscall plan")
}

fn retained_release_contracts(
    occurrences: &[(&[u8], u64)],
) -> Vec<crate::native_realization::FilesystemOrdinaryReleaseContract> {
    let record = build_evaluation::capture_verified_build_filesystem_replay_record(
        &build_evaluation::test_support::replayable_native_handle_query_chain_summary(occurrences),
        build_evaluation::BuildFilesystemReplayRecordLimits::default(),
    )
    .expect("retained query-release chains encode")
    .expect("verified query-release chains retain replay custody");
    crate::native_realization::filesystem_native_handle_query_release_contracts(
        &record,
        build_evaluation::BuildFilesystemReplayRecordLimits::default(),
    )
    .expect("retained occurrences realize their contracts")
}

/// The build replay's retained release contracts mint bound mechanism keys for
/// occurrences of a different execution. Settlement must never consult them to
/// narrow a demanded program mechanism — the program's own checked-flow
/// derivation, rejoined per call site, is the only admissible narrowing.
#[test]
fn settlement_never_narrows_a_release_cohort_from_build_replay_evidence() {
    let profile = target::TargetProfile::LinuxX64;
    let target = profile.native_target();
    let plan = abstract_plan();
    let boundary = plan.boundary_machines[0].id;
    let selected = filesystem_release_syscall_plan(profile, 1);
    let conservative =
        crate::native_realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect("verified boundary supplies the conservative checked contract");
    // Contracts derived from this compile's own build replay mint bound keys
    // for a different execution's occurrences — no amount of that evidence may
    // classify the program's demanded mechanism, even when the receiving
    // policy rows every bound key.
    let contracts = retained_release_contracts(&[(b"pkg/main.omg", 7), (b"pkg/lib.omg", 8)]);
    let [first, second] = contracts.as_slice() else {
        panic!("two retained occurrences derive two contracts")
    };
    let first_bound =
        crate::native_realization::filesystem_release_bound_mechanism(conservative, *first)
            .expect("a syscall accepts the checked release coordinate");
    let second_bound =
        crate::native_realization::filesystem_release_bound_mechanism(conservative, *second)
            .expect("a syscall accepts the checked release coordinate");
    assert_ne!(first_bound, second_bound);
    let bound_only = crate::native_realization::terminal_authority_policy_with_rows(vec![
        crate::native_realization::TerminalAuthorityPolicyRow::new(
            first_bound,
            effects::TerminalAuthorityDisposition::from_filesystem_facets([]),
        ),
        crate::native_realization::TerminalAuthorityPolicyRow::new(
            second_bound,
            effects::TerminalAuthorityDisposition::from_filesystem_facets([]),
        ),
    ])
    .expect("exact release-bound policy rows");
    let external = external(profile, 1);

    let error = validate_source_evaluated_import_coverage(
        &plan,
        &selected,
        &bound_only,
        target,
        std::slice::from_ref(&external),
        &[],
        &[],
    )
    .expect_err("retained build-replay evidence cannot classify a program mechanism");
    assert!(
        error[0]
            .message
            .contains("does not classify syscall mechanism")
    );

    // The release-cohort demand classifies only under the conservative key the
    // receiving policy itself rows — the bound coordinate never substitutes.
    let policy = crate::native_realization::terminal_authority_policy_with_rows(vec![
        crate::native_realization::TerminalAuthorityPolicyRow::new(
            first_bound,
            effects::TerminalAuthorityDisposition::from_filesystem_facets([]),
        ),
        crate::native_realization::TerminalAuthorityPolicyRow::new(
            conservative,
            effects::TerminalAuthorityDisposition::from_classes([]),
        ),
    ])
    .expect("conservative and bound rows coexist");
    let admitted = validate_source_evaluated_import_coverage(
        &plan,
        &selected,
        &policy,
        target,
        std::slice::from_ref(&external),
        &[],
        &[],
    )
    .expect("the receiving policy's own conservative key admits the demand")
    .0;
    assert_eq!(
        admitted,
        vec![
            crate::native_realization::providers::AdmittedTerminalMechanism {
                boundary,
                mechanism: conservative,
            }
        ],
        "settlement admits the conservative key, never a build-replay bound coordinate"
    );
}

#[test]
fn settlement_never_binds_release_contracts_into_non_release_cohorts() {
    let profile = target::TargetProfile::LinuxX64;
    let target = profile.native_target();
    let plan = abstract_plan();
    let boundary = plan.boundary_machines[0].id;
    // `leaf` is not a `FilesystemHost` ordinary-release cohort method, so no
    // retained contract may narrow its mechanism key.
    let selected = syscall_plan(profile, 1);
    let conservative =
        crate::native_realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect("verified boundary supplies the conservative checked contract");
    let contracts = retained_release_contracts(&[(b"pkg/main.omg", 7)]);
    let [contract] = contracts.as_slice() else {
        panic!("one retained occurrence derives one contract")
    };
    let bound =
        crate::native_realization::filesystem_release_bound_mechanism(conservative, *contract)
            .expect("a syscall accepts the checked release coordinate");
    // Even when the receiving policy rows the bound key, a non-release
    // requirement must classify under its own exact mechanism only.
    let policy = crate::native_realization::terminal_authority_policy_with_rows(vec![
        crate::native_realization::TerminalAuthorityPolicyRow::new(
            bound,
            effects::TerminalAuthorityDisposition::from_filesystem_facets([]),
        ),
        crate::native_realization::TerminalAuthorityPolicyRow::new(
            conservative,
            effects::TerminalAuthorityDisposition::from_classes([]),
        ),
    ])
    .expect("exact policy rows");
    let admitted = validate_source_evaluated_import_coverage(
        &plan,
        &selected,
        &policy,
        target,
        &[external(profile, 1)],
        &[],
        &[],
    )
    .expect("the conservative key still admits a non-release cohort")
    .0;
    assert_eq!(
        admitted[0].mechanism, conservative,
        "a non-release cohort never carries a release-contract coordinate"
    );
}

/// A selected `set_len` syscall plan served by a canonical `FilesystemHost`
/// facet-cohort method: the toolchain-settled cohort emits the exact
/// classification row for its mechanism, so the receiving policy need not
/// spell it.
fn filesystem_cohort_syscall_plan(
    profile: target::TargetProfile,
    number: i64,
) -> effects::SelectedProviderPlanFacts {
    let mut plan = import_plan(b"unused", profile);
    plan.schema.trait_name = "omega::test::FilesystemHost".into();
    plan.schema.methods[0].name = "set_len".into();
    plan.rows[0].method = "set_len".into();
    plan.rows[0].binding = ProviderBinding::Syscall { number };
    effects::SelectedProviderPlanFacts::from_selected_plans(vec![plan])
        .expect("one exact filesystem cohort syscall plan")
}

fn merge_cohort_rows(
    supplied: &crate::native_realization::TerminalAuthorityPolicy,
    cohort_rows: Vec<crate::native_realization::TerminalAuthorityPolicyRow>,
) -> Result<
    crate::native_realization::TerminalAuthorityPolicy,
    crate::native_realization::TerminalAuthorityPolicyBuildError,
> {
    let mut rows = supplied.explicit_rows().to_vec();
    for row in cohort_rows {
        if !rows.contains(&row) {
            rows.push(row);
        }
    }
    crate::native_realization::terminal_authority_policy_with_rows(rows)
}

#[test]
fn settled_filesystem_cohort_emits_and_forged_substitutions_reject() {
    let profile = target::TargetProfile::LinuxX64;
    let target = profile.native_target();
    let plan = abstract_plan();
    let boundary = plan.boundary_machines[0].id;
    let selected = filesystem_cohort_syscall_plan(profile, 1);
    let mechanism =
        crate::native_realization::terminal_authority_policy::conservative_syscall_terminal_mechanism(
            profile, 1, &plan, boundary,
        )
        .expect("verified boundary supplies the conservative checked contract");
    let external_row = external(profile, 1);
    let minted = crate::native_realization::terminal_authority_policy::filesystem_mechanism_row(
        mechanism,
        &selected.plans()[0].schema.methods[0],
    )
    .expect("`set_len` is a settled facet cohort member");

    // With no receiving-policy input the cohort row classifies the leaf.
    let (admitted, cohort_rows) = validate_source_evaluated_import_coverage(
        &plan,
        &selected,
        &crate::native_realization::current_terminal_authority_policy(),
        target,
        std::slice::from_ref(&external_row),
        &[],
        &[],
    )
    .expect("the toolchain-settled cohort classifies the demanded leaf");
    assert_eq!(
        admitted,
        vec![
            crate::native_realization::providers::AdmittedTerminalMechanism {
                boundary,
                mechanism,
            }
        ]
    );
    assert_eq!(cohort_rows, vec![minted.clone()]);

    // A supplied row identical to the minted row confirms rather than
    // duplicates: the merged effective policy holds exactly one row.
    let confirming =
        crate::native_realization::terminal_authority_policy_with_rows(vec![minted.clone()])
            .expect("an identical supplied row still builds a policy");
    let (_, cohort_rows) = validate_source_evaluated_import_coverage(
        &plan,
        &selected,
        &confirming,
        target,
        std::slice::from_ref(&external_row),
        &[],
        &[],
    )
    .expect("a confirming supplied row admits the cohort leaf");
    let effective = merge_cohort_rows(&confirming, cohort_rows)
        .expect("identical supplied and minted rows merge without duplication");
    assert_eq!(effective.explicit_rows().len(), 1);

    // A supplied row classifying the same mechanism under a different
    // disposition is a forged classification: coverage admits the leaf under
    // the caller's row, but the merge into the minted set rejects the
    // substitution as a duplicate mechanism key.
    let forged = crate::native_realization::terminal_authority_policy_with_rows(vec![
        crate::native_realization::TerminalAuthorityPolicyRow::new(
            mechanism,
            effects::TerminalAuthorityDisposition::from_classes([]),
        ),
    ])
    .expect("a forged row builds an exact supplied policy");
    let (_, cohort_rows) = validate_source_evaluated_import_coverage(
        &plan,
        &selected,
        &forged,
        target,
        std::slice::from_ref(&external_row),
        &[],
        &[],
    )
    .expect("the supplied row classifies the leaf before the merge");
    let error = merge_cohort_rows(&forged, cohort_rows)
        .expect_err("a substituted cohort classification is a forged receiver row");
    assert!(matches!(
        error,
        crate::native_realization::TerminalAuthorityPolicyBuildError::DuplicateMechanism(
            duplicate
        ) if duplicate == mechanism
    ));

    // A method outside every settled cohort mints nothing and still demands a
    // receiving row.
    let mut unknown_plan = import_plan(b"unused", profile);
    unknown_plan.schema.methods[0].name = "unbound_host_leaf".into();
    unknown_plan.rows[0].method = "unbound_host_leaf".into();
    unknown_plan.rows[0].binding = ProviderBinding::Syscall { number: 1 };
    let unknown_selected =
        effects::SelectedProviderPlanFacts::from_selected_plans(vec![unknown_plan])
            .expect("one exact unknown-method syscall plan");
    let error = validate_source_evaluated_import_coverage(
        &plan,
        &unknown_selected,
        &crate::native_realization::current_terminal_authority_policy(),
        target,
        &[external(profile, 1)],
        &[],
        &[],
    )
    .expect_err("unknown methods keep the fail-closed classification");
    assert!(
        error[0]
            .message
            .contains("does not classify syscall mechanism")
    );
}
