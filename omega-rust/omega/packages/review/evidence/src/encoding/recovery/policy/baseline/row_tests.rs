//! Complete row projection over the same typed fixture as baseline recovery.
use super::tests::fixture;
use super::*;
mod rich;

fn rows(policy: &PackagePolicyBaseline) -> (Vec<PackagePolicyRow>, PackagePolicyRowUsage) {
    policy
        .canonical_rows_with_limits(PackagePolicyRowLimits::default())
        .unwrap()
}

#[test]
fn complete_rows_are_named_lossless_and_leave_existing_baseline_bytes_unchanged() {
    let value = fixture();
    let binary = value.canonical_bytes().unwrap();
    let text = value.canonical_text().unwrap();
    let (projected, usage) = rows(&value);
    assert_eq!(usage.rows(), projected.len());
    assert!(
        projected
            .windows(2)
            .all(|pair| (pair[0].kind(), pair[0].key_bytes())
                < (pair[1].kind(), pair[1].key_bytes()))
    );
    assert!(projected.iter().all(|row| {
        row.canonical_text()
            .starts_with("omega_package_policy_row_text 1\n")
    }));
    let constant = projected
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::PublicConst)
        .unwrap();
    assert!(
        constant
            .canonical_text()
            .contains("canonical_value_encoding")
    );
    assert!(constant.canonical_text().contains("42"));
    assert!(!constant.initial_requires_decision());
    assert!(constant.update_requires_decision());
    let header = projected
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::Header)
        .unwrap();
    assert!(!header.update_requires_decision());
    assert!(!header.audit_recommended_on_change());
    assert_eq!(value.canonical_bytes().unwrap(), binary);
    assert_eq!(value.canonical_text().unwrap(), text);
    assert_eq!(
        rows(
            &PackagePolicyBaseline::recover_canonical(
                &binary,
                PackagePolicyRecoveryLimits::default()
            )
            .unwrap()
        ),
        (projected, usage)
    );
}

#[test]
fn aggregate_requested_storage_and_work_have_exact_boundaries() {
    let value = fixture();
    let (projected, usage) = rows(&value);
    let exact_bytes = projected.len() * std::mem::size_of::<PackagePolicyRow>()
        + projected
            .iter()
            .map(|row| {
                row.key_bytes().len() + row.canonical_bytes().len() + row.canonical_text().len()
            })
            .sum::<usize>();
    assert_eq!(usage.owned_bytes(), exact_bytes);
    let limits = PackagePolicyRowLimits {
        maximum_rows: usage.rows(),
        maximum_owned_bytes: usage.owned_bytes(),
        maximum_sequence_elements: usage.sequence_elements(),
        maximum_key_bytes: projected
            .iter()
            .map(|row| row.key_bytes().len())
            .max()
            .unwrap(),
        maximum_canonical_bytes: projected
            .iter()
            .map(|row| row.canonical_bytes().len())
            .max()
            .unwrap(),
        maximum_text_bytes: projected
            .iter()
            .map(|row| row.canonical_text().len())
            .max()
            .unwrap(),
        ..PackagePolicyRowLimits::default()
    };
    assert_eq!(
        value.canonical_rows_with_limits(limits).unwrap(),
        (projected, usage)
    );
    for axis in 0..6 {
        let mut short = limits;
        match axis {
            0 => short.maximum_rows -= 1,
            1 => short.maximum_owned_bytes -= 1,
            2 => short.maximum_sequence_elements -= 1,
            3 => short.maximum_key_bytes -= 1,
            4 => short.maximum_canonical_bytes -= 1,
            _ => short.maximum_text_bytes -= 1,
        }
        assert!(
            value.canonical_rows_with_limits(short).is_err(),
            "axis {axis}"
        );
    }
    assert!(
        value
            .canonical_rows_with_limits(PackagePolicyRowLimits {
                maximum_depth: 0,
                ..limits
            })
            .is_err()
    );
}

#[test]
fn constant_value_change_keeps_coordinate_and_changes_complete_row() {
    let value = fixture();
    let mut changed = value.clone();
    changed.public_api.consts[0].canonical_value_encoding = "43".to_owned();
    let original = rows(&value).0;
    let candidate = rows(&changed).0;
    let differences = original
        .iter()
        .zip(&candidate)
        .filter(|(left, right)| left != right)
        .collect::<Vec<_>>();
    assert_eq!(differences.len(), 1);
    let (left, right) = differences[0];
    assert_eq!(left.kind(), PackagePolicyRowKind::PublicConst);
    assert_eq!(left.key_bytes(), right.key_bytes());
    assert_ne!(left.canonical_bytes(), right.canonical_bytes());
    assert_ne!(left.canonical_text(), right.canonical_text());
}

#[test]
fn duplicate_exact_coordinates_reject_even_when_values_differ() {
    let mut value = fixture();
    let mut duplicate = value.public_api.consts[0].clone();
    duplicate.canonical_value_encoding = "43".to_owned();
    value.public_api.consts.push(duplicate);
    assert!(
        value
            .canonical_rows_with_limits(PackagePolicyRowLimits::default())
            .is_err()
    );
}

#[test]
fn retained_danger_and_slack_remain_audit_relevant_without_blanket_api_decisions() {
    let (projected, _) = rows(&fixture());
    let danger = projected
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::DangerousCapability)
        .unwrap();
    assert!(danger.initial_requires_decision());
    assert!(danger.update_requires_decision());
    assert!(danger.audit_recommended_when_present());
    let slack = projected
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::DangerousSlack)
        .unwrap();
    assert!(!slack.initial_requires_decision());
    assert!(!slack.update_requires_decision());
    assert!(slack.audit_recommended_when_present());
    let representation = projected
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::RepresentationTarget)
        .unwrap();
    assert!(!representation.initial_requires_decision());
    assert!(!representation.update_requires_decision());
    assert!(!representation.audit_recommended_when_present());
    assert!(!representation.audit_recommended_on_change());
}

#[test]
fn both_semantic_exposures_are_retained_with_nonordinal_keys() {
    let mut value = fixture();
    let mut second = value.semantic_dependencies[0].clone();
    second.exposure = PackageReviewSemanticDependencyExposure::PublicInterface;
    value.semantic_dependencies.push(second);
    value.semantic_dependencies.sort();
    let projected = rows(&value).0;
    let dependencies = projected
        .iter()
        .filter(|row| row.kind() == PackagePolicyRowKind::SemanticDependency)
        .collect::<Vec<_>>();
    assert_eq!(dependencies.len(), 2);
    assert_ne!(dependencies[0].key_bytes(), dependencies[1].key_bytes());
    let key = dependencies[0].key_bytes().to_vec();
    value.public_api.consts.clear();
    assert!(rows(&value).0.iter().any(|row| row.kind()
        == PackagePolicyRowKind::SemanticDependency
        && row.key_bytes() == key));
}

#[test]
fn empty_context_and_initial_ordinary_api_do_not_manufacture_findings() {
    let mut value = fixture();
    value.callables.callables.clear();
    value.dangerous_capabilities.clear();
    value.slack_uses.clear();
    value.semantic_dependencies.clear();
    value.canonical_bytes().unwrap();
    assert!(
        rows(&value)
            .0
            .iter()
            .all(|row| !row.initial_requires_decision()
                && !row.audit_recommended_when_present()
                && !row.audit_recommended_on_change())
    );
    value.public_api.consts.clear();
    assert_eq!(rows(&value).0.len(), 3);
}

#[test]
fn initial_assumptions_use_supply_not_visibility_or_ordinary_ensures() {
    let mut value = fixture();
    value.callables.callables[0]
        .contracts
        .push(PackageReviewCallableContract {
            kind: PackageReviewContractKind::Ensures,
            result_case: None,
            binding: None,
            evidence_lane_position: None,
            fact: PackageReviewContractFact::Expression(PackageReviewContractExpression::Boolean(
                true,
            )),
        });
    value.canonical_bytes().unwrap();
    let projected = rows(&value).0;
    let ordinary = projected
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::Callable)
        .unwrap();
    assert!(!ordinary.initial_requires_decision());
    assert!(!ordinary.audit_recommended_on_change());
    value.slack_uses.clear();
    let callable = &mut value.callables.callables[0];
    callable.supply = PackageReviewCallableSupply::AdmissionClaim;
    callable.role = PackagePolicyCallableRole::PrivateAssumption;
    callable.checked_service_reach = PackageReviewCheckedServiceReach::NoCheckedBody;
    callable.capability_flows.clear();
    callable.reachable_capability_flows.clear();
    callable.checked_termination = PackagePolicyTermination::NoGuarantee;
    callable.mutation.paths.clear();
    value.canonical_bytes().unwrap();
    let projected = rows(&value).0;
    let assumption = projected
        .iter()
        .find(|row| row.kind() == PackagePolicyRowKind::Callable)
        .unwrap();
    assert!(assumption.initial_requires_decision());
    assert!(assumption.audit_recommended_on_change());
    assert!(!assumption.audit_recommended_when_present());
    value.callables.callables[0].role = PackagePolicyCallableRole::Boundary;
    value.canonical_bytes().unwrap();
    assert!(
        rows(&value)
            .0
            .iter()
            .find(|row| row.kind() == PackagePolicyRowKind::Callable)
            .unwrap()
            .initial_requires_decision()
    );
}

#[test]
fn recursive_contract_meaning_and_depth_remain_under_one_row_budget() {
    let mut value = fixture();
    let mut requirement = PackageReviewBooleanExpression::Constant(true);
    for _ in 0..16 {
        requirement = PackageReviewBooleanExpression::Not(Box::new(requirement));
    }
    value.callables.callables[0]
        .checked_crash
        .structural_runtime_requirements = Some(vec![requirement]);
    value.canonical_bytes().unwrap();
    let (original, usage) = rows(&value);
    assert!(
        value
            .canonical_rows_with_limits(PackagePolicyRowLimits {
                maximum_depth: 8,
                ..PackagePolicyRowLimits::default()
            })
            .is_err()
    );
    value.callables.callables[0]
        .checked_crash
        .structural_runtime_requirements =
        Some(vec![PackageReviewBooleanExpression::Constant(false)]);
    let (changed, simpler) = rows(&value);
    assert!(usage.sequence_elements() > simpler.sequence_elements());
    assert_ne!(original, changed);
}
