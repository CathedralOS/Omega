use crate::support::*;

fn evidence_call_fixture(
    target: &str,
    callee_binding: &str,
    caller_binding: &str,
) -> (TempPackage, CheckedCompilation) {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        &format!(
            r#"pub trait Evidence {{}}
pub proposition carries(value: i32) evidence Evidence;

pub machine observes(value: i32) -> bool
requires {callee_binding}: carries(value)
{{ true }}

pub machine inspect(value: i32)
requires {caller_binding}: carries(value)
requires observes(value; {caller_binding})
{{ }}
"#,
        ),
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("checked language should accept an evidence-bearing public contract call");
    (package, checked)
}

fn evidence_call_expression(
    review: &CheckedPackageReviewProjection,
) -> &PackageReviewContractExpression {
    let inspect = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "inspect")
        .expect("public inspect callable");
    inspect
        .contracts()
        .iter()
        .find_map(|contract| match contract.fact() {
            PackageReviewContractFact::Expression(
                expression @ PackageReviewContractExpression::Call { .. },
            ) => Some(expression),
            _ => None,
        })
        .expect("evidence-bearing call contract")
}

fn assert_tamper_rejected(
    target: &str,
    tamper: impl FnOnce(&mut CheckedCompilation),
    expected: &str,
) {
    let (_package, mut checked) = evidence_call_fixture(target, "required", "incoming");
    tamper(&mut checked);
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("tampered evidence-call custody must fail package review");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected `{expected}` in {diagnostics:#?}",
    );
}

#[test]
fn public_contract_call_projects_exact_evidence_binding() {
    let Some(target) = host_target_name() else {
        return;
    };
    let (_package, checked) = evidence_call_fixture(target, "required", "incoming");
    let review = project_checked_package_review(&checked)
        .expect("checked evidence-bearing public contract call should project");
    let PackageReviewContractExpression::Call {
        target,
        evidence_arguments,
        ..
    } = evidence_call_expression(&review)
    else {
        unreachable!()
    };
    let PackageReviewContractCallTarget::Nominal(target) = target else {
        panic!("evidence call should retain its nominal target")
    };
    assert_eq!(target.path(), "observes::entry");
    let [binding] = evidence_arguments.as_slice() else {
        panic!("one exact evidence binding")
    };
    assert_eq!(binding.lane_position(), 0);
    assert_eq!(binding.source().owner().path(), "inspect");
    assert_eq!(binding.source().kind(), PackageReviewContractKind::Requires);
    assert_eq!(binding.source().lane_position(), 0);
    assert_eq!(binding.parameter().owner().path(), "observes");
    assert_eq!(
        binding.parameter().kind(),
        PackageReviewContractKind::Requires
    );
    assert_eq!(binding.parameter().lane_position(), 0);
}

#[test]
fn evidence_call_identity_ignores_local_binding_renames() {
    let Some(target) = host_target_name() else {
        return;
    };
    let (_original_package, original) = evidence_call_fixture(target, "required", "incoming");
    let (_renamed_package, renamed) = evidence_call_fixture(target, "witness_", "evidence");
    let original = project_checked_package_review(&original)
        .expect("original evidence call should project")
        .canonical_review_bytes()
        .expect("original canonical review bytes");
    let renamed = project_checked_package_review(&renamed)
        .expect("renamed evidence call should project")
        .canonical_review_bytes()
        .expect("renamed canonical review bytes");
    assert_eq!(original, renamed);
}

#[test]
fn evidence_call_projection_rejects_missing_occurrence_custody() {
    let Some(target) = host_target_name() else {
        return;
    };
    assert_tamper_rejected(
        target,
        |checked| {
            checked
                .facts
                .proof
                .contract_expression_evidence_calls
                .clear()
        },
        "has 0 exact checked occurrence rows; expected one",
    );
}

#[test]
fn evidence_call_projection_rejects_duplicate_occurrence_custody() {
    let Some(target) = host_target_name() else {
        return;
    };
    assert_tamper_rejected(
        target,
        |checked| {
            let duplicate = checked.facts.proof.contract_expression_evidence_calls[0].clone();
            checked
                .facts
                .proof
                .contract_expression_evidence_calls
                .push(duplicate);
        },
        "has 2 exact checked occurrence rows; expected one",
    );
}

#[test]
fn evidence_call_projection_rejects_redirected_target_coordinate() {
    let Some(target) = host_target_name() else {
        return;
    };
    assert_tamper_rejected(
        target,
        |checked| {
            let row = &mut checked.facts.proof.contract_expression_evidence_calls[0];
            row.target_state_symbol = row.target_machine_symbol;
        },
        "disagrees with its exact checked target or lane arity",
    );
}

#[test]
fn evidence_call_projection_rejects_redirected_term_binding() {
    let Some(target) = host_target_name() else {
        return;
    };
    assert_tamper_rejected(
        target,
        |checked| {
            let binding = &mut checked.facts.proof.contract_expression_evidence_calls[0]
                .evidence_arguments[0];
            binding.parameter = binding.source;
        },
        "changed checked parameter binding at lane 0",
    );
}

#[test]
fn evidence_call_projection_rejects_changed_lane() {
    let Some(target) = host_target_name() else {
        return;
    };
    assert_tamper_rejected(
        target,
        |checked| {
            checked.facts.proof.contract_expression_evidence_calls[0].evidence_arguments[0]
                .lane_position = 1;
        },
        "changed checked lane position 0",
    );
}

#[test]
fn evidence_call_projection_rejects_argument_drift_after_checking() {
    let Some(target) = host_target_name() else {
        return;
    };
    assert_tamper_rejected(
        target,
        |checked| {
            let expression = checked.facts.proof.contract_expression_evidence_calls[0].expression;
            let typed_trees::expression::ExpressionNode::Call(call) =
                checked.expression_table.expression(expression).clone()
            else {
                panic!("checked evidence occurrence must rejoin a call")
            };
            let argument = checked.expression_table.expression_handles(call.arguments)[0];
            *checked.typed.expression_table.expression_mut(argument) =
                typed_trees::expression::ExpressionNode::Integer(
                    numerics::literals::IntegerLiteral::from_value(41),
                );
        },
        "changed checked binding at lane 0",
    );
}
