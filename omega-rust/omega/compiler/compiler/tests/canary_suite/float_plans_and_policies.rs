//! Fixtures shared by the float plan and policy canaries.

#[path = "float_plans_and_policies/directed_float_selections.rs"]
mod directed_float_selections;
#[path = "../fixture_rosters/float_plans_and_policies.rs"]
pub(super) mod fixture_roster;
#[path = "float_match_interpreter.rs"]
mod float_match_interpreter;
#[path = "float_plans_and_policies/float_provider_identities.rs"]
mod float_provider_identities;
#[path = "float_plans_and_policies/named_float_rewrites.rs"]
mod named_float_rewrites;
#[path = "float_plans_and_policies/requirement_intrinsics.rs"]
mod requirement_intrinsics;

fn optional_intrinsic_diagnostic_label(
    checked: &compiler::CheckedCompilation,
    plan: &effects::provider_plan::ProviderPlan,
) -> Option<String> {
    let mut operators = checked.typed.operators().iter().filter(|operator| {
        typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator)
            == plan.schema.trait_name
    });
    if let Some(operator) = operators.next() {
        assert!(
            operators.next().is_none(),
            "selected intrinsic plan must resolve one exact boundary operator"
        );
        return provider_planning::compiler_intrinsic_diagnostic_label(&checked.typed, operator);
    }
    // A top-level `boundary requirement` is its own species: its plan schema
    // is the requirement's machine path and the label comes from the same
    // requirement view the compiler-intrinsic bridge keys on.
    let mut requirements = checked.typed.machines().iter().filter(|machine| {
        machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            && machine.name.as_str() == plan.schema.trait_name
    });
    let requirement = requirements.next()?;
    assert!(
        requirements.next().is_none(),
        "selected intrinsic plan must resolve one exact top-level boundary requirement"
    );
    let view =
        provider_planning::IntrinsicRequirement::from_requirement(&checked.typed, requirement)?;
    provider_planning::compiler_intrinsic_diagnostic_label_for(&checked.typed, &view)
}

/// Every retained named use stamped with a selected plan, whichever species
/// spelled it: the operator use facts and the direct requirement use facts.
fn stamped_named_uses(
    checked: &compiler::CheckedCompilation,
) -> Vec<(typed_trees::expression::ExpressionHandle, u64)> {
    checked
        .facts
        .operators
        .named_uses()
        .map(|operator_use| {
            (
                operator_use.expression,
                operator_use.provider_plan_report_fingerprint,
            )
        })
        .chain(
            checked
                .facts
                .operators
                .named_requirement_uses()
                .map(|requirement_use| {
                    (
                        requirement_use.expression,
                        requirement_use.provider_plan_report_fingerprint,
                    )
                }),
        )
        .filter(|(_, fingerprint)| *fingerprint != 0)
        .collect()
}

fn selected_intrinsic_diagnostic_label(
    checked: &compiler::CheckedCompilation,
    plan: &effects::provider_plan::ProviderPlan,
) -> String {
    optional_intrinsic_diagnostic_label(checked, plan)
        .expect("selected float intrinsic must have a structured diagnostic label")
}

// Binding-site operator selection (chapter 8): the signature `requires`
// statically selects the domain-owned `+` meaning, and checked evidence records
// that choice without consulting flow facts.

pub(super) fn retained_float_differential_result_identity(
    suite_id: &str,
    target: &str,
    coverage: &[&str],
    selected_intrinsics: &std::collections::BTreeSet<String>,
    selected_plan_identities: &[u64],
    outcome: &checked_interpreter::InterpretOutcome,
    output: &std::process::Output,
    cross_targets: &[&str],
) -> u64 {
    fn retain(hash: &mut u64, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            *hash ^= u64::from(byte);
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    let mut result_identity = 0xcbf29ce484222325_u64;
    retain(&mut result_identity, suite_id.as_bytes());
    retain(&mut result_identity, target.as_bytes());
    for category in coverage {
        retain(&mut result_identity, category.as_bytes());
    }
    for intrinsic in selected_intrinsics {
        retain(&mut result_identity, intrinsic.as_bytes());
    }
    for identity in selected_plan_identities {
        retain(&mut result_identity, &identity.to_le_bytes());
    }
    retain(&mut result_identity, &outcome.exit_code.to_le_bytes());
    retain(&mut result_identity, &outcome.stdout);
    retain(&mut result_identity, &outcome.stderr);
    retain(
        &mut result_identity,
        &output.status.code().unwrap_or_default().to_le_bytes(),
    );
    retain(&mut result_identity, &output.stdout);
    retain(&mut result_identity, &output.stderr);
    for target in cross_targets {
        retain(
            &mut result_identity,
            format!("{target}:cross-compile-passed").as_bytes(),
        );
    }
    result_identity
}

fn retained_float_policy_differential_result_identity(
    suite_id: &str,
    target: &str,
    coverage: &[&str],
    selected_evidence: &std::collections::BTreeSet<String>,
    selected_plan_identities: &[u64],
    observations: &[(
        &str,
        &checked_interpreter::InterpretOutcome,
        &std::process::Output,
    )],
    cross_builds: &std::collections::BTreeSet<String>,
) -> u64 {
    fn retain(hash: &mut u64, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            *hash ^= u64::from(byte);
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    let mut result_identity = 0xcbf29ce484222325_u64;
    retain(&mut result_identity, suite_id.as_bytes());
    retain(&mut result_identity, target.as_bytes());
    for category in coverage {
        retain(&mut result_identity, category.as_bytes());
    }
    for evidence in selected_evidence {
        retain(&mut result_identity, evidence.as_bytes());
    }
    for identity in selected_plan_identities {
        retain(&mut result_identity, &identity.to_le_bytes());
    }
    for (label, outcome, output) in observations {
        retain(&mut result_identity, label.as_bytes());
        retain(&mut result_identity, &outcome.exit_code.to_le_bytes());
        retain(&mut result_identity, &outcome.stdout);
        retain(&mut result_identity, &outcome.stderr);
        match &outcome.error {
            Some(error) => {
                retain(&mut result_identity, b"interpreter-error");
                retain(&mut result_identity, error.as_bytes());
            }
            None => retain(&mut result_identity, b"interpreter-success"),
        }
        retain(
            &mut result_identity,
            format!("{:?}", output.status).as_bytes(),
        );
        retain(&mut result_identity, &output.stdout);
        retain(&mut result_identity, &output.stderr);
    }
    for cross_build in cross_builds {
        retain(
            &mut result_identity,
            format!("{cross_build}:cross-compile-passed").as_bytes(),
        );
    }
    result_identity
}
