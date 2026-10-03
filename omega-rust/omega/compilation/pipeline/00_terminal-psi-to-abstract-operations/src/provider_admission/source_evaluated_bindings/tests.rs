use super::{
    PreparedSourceEvaluatedBindings, SourceEvaluatedImportSettlement,
    prepare_source_evaluated_bindings, validate_evaluated_import_rows,
};
use crate::provider_admission::{NativeBoundaryRealization, NativeProviderSettlement};
use effects::provider_plan::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest, EvaluatedBindingReceipt, EvaluatedBindingUsage,
    EvaluatedForeignImport, ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod,
    ServiceSchema,
};
use std::borrow::Cow;
use std::collections::BTreeSet;

fn external_import_row(plan: &ProviderPlan) -> calling_conventions::ExternalBindingRow {
    let row = &plan.rows[0];
    let ProviderBinding::Import { evaluated } = &row.binding else {
        panic!("fixture must select an evaluated import");
    };
    calling_conventions::ExternalBindingRow {
        target_name: "windows_x86_64".into(),
        trait_name: plan.schema.trait_name.clone(),
        method: row.method.clone(),
        requirement_identity: row.requirement_identity.clone(),
        table_type: plan.provider_type.clone(),
        boundary_entry_plan: None,
        binding: calling_conventions::ExternalBindingKind::Import {
            locator: evaluated.locator().clone(),
        },
    }
}

#[test]
fn retained_roster_accepts_exact_and_target_inherited_imports() {
    let mut plans = [import_plan("first"), import_plan("second")];
    plans[1].target.clear();
    let mut rows = vec![
        external_import_row(&plans[1]),
        external_import_row(&plans[0]),
    ];
    let mut unrelated = rows[0].clone();
    unrelated.requirement_identity = "other".into();
    unrelated.binding = calling_conventions::ExternalBindingKind::Syscall { number: 1 };
    rows.push(unrelated);
    assert_eq!(
        validate_evaluated_import_rows(target::TargetProfile::WindowsX64, &plans, &rows),
        Ok(())
    );
    assert_eq!(
        validate_evaluated_import_rows(target::TargetProfile::WindowsX64, &[], &[]),
        Ok(())
    );
}

#[test]
fn retained_roster_rejects_each_external_coordinate_drift() {
    let plan = import_plan("first");
    for coordinate in 0..5 {
        let mut row = external_import_row(&plan);
        let field = match coordinate {
            0 => &mut row.target_name,
            1 => &mut row.trait_name,
            2 => &mut row.method,
            3 => &mut row.requirement_identity,
            _ => &mut row.table_type,
        };
        field.push_str("::substituted");
        assert_eq!(
            validate_evaluated_import_rows(
                target::TargetProfile::WindowsX64,
                std::slice::from_ref(&plan),
                &[row]
            ),
            Err(
                "Terminal native proposal selected import does not rejoin one exact external-binding row"
            ),
        );
    }
}

#[test]
fn retained_roster_rejects_missing_duplicate_reused_and_unmatched_imports() {
    let plan = import_plan("first");
    let row = external_import_row(&plan);
    let plans = std::slice::from_ref(&plan);
    let target = target::TargetProfile::WindowsX64;
    let no_exact_row = Err(
        "Terminal native proposal selected import does not rejoin one exact external-binding row",
    );
    assert_eq!(
        validate_evaluated_import_rows(target, plans, &[]),
        no_exact_row
    );
    assert_eq!(
        validate_evaluated_import_rows(target, plans, &[row.clone(), row.clone()]),
        no_exact_row
    );
    assert_eq!(
        validate_evaluated_import_rows(target, &[plan.clone(), plan], std::slice::from_ref(&row)),
        Err("Terminal native proposal external import was reused by multiple selected rows"),
    );
    assert_eq!(
        validate_evaluated_import_rows(target, &[], &[row]),
        Err("Terminal native proposal retains an unmatched evaluated external import row"),
    );
}

#[test]
fn retained_roster_rejects_target_binding_kind_and_locator_substitution() {
    let plan = import_plan("first");
    let row = external_import_row(&plan);
    let target = target::TargetProfile::WindowsX64;
    let wrong_target =
        Err("Terminal native proposal selected import disagrees with its target profile");
    let mut wrong_plan_target = plan.clone();
    wrong_plan_target.target = "linux_x86_64".into();
    assert_eq!(
        validate_evaluated_import_rows(target, &[wrong_plan_target], std::slice::from_ref(&row)),
        wrong_target
    );
    let mut inherited = plan.clone();
    inherited.target.clear();
    assert_eq!(
        validate_evaluated_import_rows(
            target::TargetProfile::MacosArm64,
            &[inherited],
            std::slice::from_ref(&row)
        ),
        wrong_target
    );

    let mut substituted = row.clone();
    substituted.binding = calling_conventions::ExternalBindingKind::Syscall { number: 1 };
    assert_eq!(
        validate_evaluated_import_rows(target, std::slice::from_ref(&plan), &[substituted]),
        Err("Terminal native proposal selected import rejoined a non-evaluated external binding"),
    );
    let mut substituted = row;
    substituted.binding = external_import_row(&import_plan("other")).binding;
    assert_eq!(
        validate_evaluated_import_rows(target, &[plan], &[substituted]),
        Err("Terminal native proposal external import substituted the selected evaluated locator"),
    );
}

fn import_plan(requirement: &str) -> ProviderPlan {
    let locator = effects::normalize_foreign_locator(
        effects::ForeignLocatorCandidate::PeByName {
            library: b"example.dll".to_vec(),
            export: requirement.as_bytes().to_vec(),
        },
        target::TargetProfile::WindowsX64,
    )
    .expect("normalized fixture locator");
    let receipt = EvaluatedBindingReceipt::from_evaluation(
        None,
        format!("fixture::{requirement}"),
        EvaluatedBindingProducerClosureDigest::from_bytes([1; 32]).unwrap(),
        1,
        EvaluatedBindingUsage::from_evaluator(7, 1, 10, 1000, 0, 0, 4, 12, 3, 0).unwrap(),
        EvaluatedBindingEvaluationDigest::from_bytes([2; 32]).unwrap(),
        1,
        EvaluatedBindingMaterializationDigest::from_bytes([3; 32]).unwrap(),
        locator.identity_digest(),
    )
    .expect("retained fixture evaluation");
    ProviderPlan {
        name: format!("fixture::{requirement}"),
        provider_type: "ForeignProvider".to_owned(),
        provider_type_package_identity: None,
        target: "windows_x86_64".to_owned(),
        schema: ServiceSchema {
            trait_name: requirement.to_owned(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                name: requirement.to_owned(),
                requirement_owner: requirement.to_owned(),
                requirement_owner_package_identity: None,
                requirement_identity: requirement.to_owned(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: Vec::new(),
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            }],
        },
        rows: vec![ProviderPlanRow {
            method: requirement.to_owned(),
            requirement_identity: requirement.to_owned(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::Import {
                evaluated: EvaluatedForeignImport::from_retained_evidence(locator, receipt)
                    .expect("matching evaluation and locator"),
            },
        }],
        origin_package_identity: None,
        origin_package: "fixture".to_owned(),
    }
}

fn stack(settlement: &NativeProviderSettlement<'_>) -> task_plans::AdmittedSameStackContribution {
    let NativeBoundaryRealization::NormalizedForeignCall(stack) = settlement.realization else {
        panic!("prepared imports must be normalized foreign calls");
    };
    stack.clone()
}

fn supplied<'evidence>(
    settlement: &NativeProviderSettlement<'evidence>,
) -> SourceEvaluatedImportSettlement<'evidence> {
    let NativeBoundaryRealization::NormalizedForeignCall(stack) = settlement.realization else {
        panic!("prepared imports must be normalized foreign calls");
    };
    SourceEvaluatedImportSettlement::new(settlement.provider_execution, stack)
}

fn rejection(
    result: Result<PreparedSourceEvaluatedBindings<'_>, Vec<diagnostics::Diagnostic>>,
    expected: &str,
) {
    let diagnostics = result.expect_err("invalid import evidence must reject");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains(expected), "{diagnostics:?}");
}

#[test]
fn derives_only_demanded_imports_with_exact_plan_and_stack_custody() {
    let mut intrinsic = import_plan("builtin");
    intrinsic.rows[0].binding = ProviderBinding::CompilerIntrinsic {
        machine: "fixture::builtin".to_owned(),
    };
    let plans = [
        import_plan("second"),
        import_plan("unused"),
        intrinsic,
        import_plan("first"),
    ];
    let demanded = BTreeSet::from([
        "first".to_owned(),
        "second".to_owned(),
        "builtin".to_owned(),
    ]);
    let prepared = prepare_source_evaluated_bindings(&plans, &demanded, &[]).unwrap();
    let settlements = prepared.settlements();
    let repeated = prepare_source_evaluated_bindings(&plans, &demanded, &[]).unwrap();
    let repeated = repeated.settlements();
    assert_eq!(settlements.len(), 2);
    for ((settlement, repeated), plan) in settlements
        .iter()
        .zip(repeated.iter())
        .zip([&plans[0], &plans[3]])
    {
        assert!(std::ptr::eq(settlement.provider_plan, plan));
        let execution = settlement.provider_execution;
        let contribution = stack(settlement);
        assert_eq!(
            execution.requirement_identity(),
            plan.rows[0].requirement_identity
        );
        assert_eq!(
            execution.provider_plan_report_identity(),
            plan.report_fingerprint()
        );
        assert_eq!(
            execution.normalized_root_report_identity(),
            plan.report_fingerprint()
        );
        assert_eq!(
            contribution.provider_plan_commitment().as_bytes(),
            *plan.identity_digest().as_bytes()
        );
        assert_eq!(
            contribution.requirement_identity(),
            execution.requirement_identity()
        );
        assert_eq!(contribution.bytes(), 256 * 1024);
        assert_eq!(contribution.alignment(), 16);
        assert_eq!(contribution, stack(repeated));
        assert_ne!(execution.provider_execution_report_identity(), 0);
        assert_eq!(
            execution.provider_execution_report_identity(),
            repeated
                .provider_execution
                .provider_execution_report_identity()
        );
        assert_eq!(
            execution.provider_execution_report_fingerprint(),
            repeated
                .provider_execution
                .provider_execution_report_fingerprint()
        );
        assert_eq!(
            execution.boundary_contract_report_fingerprint(),
            repeated
                .provider_execution
                .boundary_contract_report_fingerprint()
        );
    }
}

#[test]
fn supplied_imports_preserve_evidence_and_caller_order() {
    let plans = [import_plan("first"), import_plan("second")];
    let demanded = BTreeSet::from(["first".to_owned(), "second".to_owned()]);
    let derived = prepare_source_evaluated_bindings(&plans, &demanded, &[]).unwrap();
    let derived = derived.settlements();
    let imports = [supplied(&derived[1]), supplied(&derived[0])];
    let prepared = prepare_source_evaluated_bindings(&plans, &demanded, &imports).unwrap();
    let settlements = prepared.settlements();
    assert!(matches!(settlements, Cow::Borrowed(_)));
    assert_eq!(settlements.len(), 2);
    for (settlement, import) in settlements.iter().zip(imports) {
        assert!(std::ptr::eq(
            settlement.provider_execution,
            import.provider_execution()
        ));
        let NativeBoundaryRealization::NormalizedForeignCall(stack) = settlement.realization else {
            panic!("normalized import expected");
        };
        assert!(std::ptr::eq(stack, import.same_stack()));
    }
    assert!(std::ptr::eq(settlements[0].provider_plan, &plans[1]));
    assert!(std::ptr::eq(settlements[1].provider_plan, &plans[0]));
}

#[test]
fn supplied_roster_rejects_missing_duplicate_unselected_and_substituted_evidence() {
    let plans = [import_plan("first"), import_plan("second")];
    let demanded = BTreeSet::from(["first".to_owned(), "second".to_owned()]);
    let derived = prepare_source_evaluated_bindings(&plans, &demanded, &[]).unwrap();
    let derived = derived.settlements();
    let imports = [supplied(&derived[0]), supplied(&derived[1])];
    rejection(
        prepare_source_evaluated_bindings(&plans, &demanded, &imports[..1]),
        "demanded import `second` has no supplied execution and stack custody",
    );
    rejection(
        prepare_source_evaluated_bindings(&plans, &demanded, &[imports[0], imports[0]]),
        "requirement `first` was supplied more than once",
    );
    rejection(
        prepare_source_evaluated_bindings(&plans, &BTreeSet::from(["first".to_owned()]), &imports),
        "requirement `second` is not a demanded selected import",
    );
    let mut changed = plans.clone();
    changed[0].name.push_str("::substituted");
    rejection(
        prepare_source_evaluated_bindings(&changed, &demanded, &imports),
        "execution evidence for `first` names a different provider-plan report coordinate",
    );
}

#[test]
fn demanded_import_must_have_one_selected_row_before_any_evidence_is_derived() {
    let plan = import_plan("first");
    let demanded = BTreeSet::from(["first".to_owned()]);
    rejection(
        prepare_source_evaluated_bindings(&[plan.clone(), plan.clone()], &demanded, &[]),
        "resolves to 2 selected import plans",
    );
    let mut duplicate = plan.clone();
    duplicate.rows.push(plan.rows[0].clone());
    rejection(
        prepare_source_evaluated_bindings(std::slice::from_ref(&duplicate), &demanded, &[]),
        "resolves to 2 selected import plans",
    );
    let empty =
        prepare_source_evaluated_bindings(std::slice::from_ref(&duplicate), &BTreeSet::new(), &[])
            .expect("undemanded rows settle nothing");
    assert!(empty.settlements().is_empty());
}

#[test]
fn demanded_syscalls_share_preparation_without_fabricating_import_reserves() {
    let mut syscall = import_plan("kernel");
    syscall.target = "linux_x86_64".into();
    syscall.rows[0].binding = ProviderBinding::Syscall { number: 7 };
    let plans = [
        import_plan("foreign"),
        syscall.clone(),
        import_plan("unused"),
    ];
    let demanded = BTreeSet::from(["foreign".to_owned(), "kernel".to_owned()]);
    let derived = prepare_source_evaluated_bindings(&plans, &demanded, &[]).unwrap();
    let settlements = derived.settlements();
    assert_eq!(settlements.len(), 2);
    assert_eq!(stack(&settlements[0]).bytes(), 256 * 1024);
    assert!(matches!(
        settlements[1].realization,
        NativeBoundaryRealization::SystemCall
    ));
    assert!(std::ptr::eq(settlements[1].provider_plan, &plans[1]));
    assert_eq!(
        settlements[1].provider_execution.requirement_identity(),
        "kernel"
    );
    let imported = [supplied(&settlements[0])];
    let external = prepare_source_evaluated_bindings(&plans, &demanded, &imported).unwrap();
    let external = external.settlements();
    assert!(std::ptr::eq(
        external[0].provider_execution,
        settlements[0].provider_execution
    ));
    assert!(matches!(
        external[1].realization,
        NativeBoundaryRealization::SystemCall
    ));
    assert_eq!(
        external[1]
            .provider_execution
            .provider_execution_report_fingerprint(),
        settlements[1]
            .provider_execution
            .provider_execution_report_fingerprint()
    );
    assert!(
        prepare_source_evaluated_bindings(&[syscall.clone(), syscall], &demanded, &[]).is_err()
    );
}
