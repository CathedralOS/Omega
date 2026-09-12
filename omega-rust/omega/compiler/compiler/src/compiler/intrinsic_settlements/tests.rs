use super::*;
use effects::provider_plan::{ProviderBinding, ProviderPlanRow};
use provider_planning::plans::{
    ProviderPlanProvenance, ProviderSchemaDeclaration, ProviderSelectionProvenance,
};
use target_operations::CompilerBuiltinExecution;

fn selected_plan(
    name: &str,
    rows: &[(&str, Option<CompilerIntrinsicExecutionIdentity>)],
) -> (ProviderPlan, SelectedProviderReviewProvenance) {
    let plan = ProviderPlan {
        name: name.to_owned(),
        rows: rows
            .iter()
            .map(|(requirement, _)| ProviderPlanRow {
                method: "operation".to_owned(),
                requirement_identity: (*requirement).to_owned(),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::CompilerIntrinsic {
                    machine: "realization".to_owned(),
                },
            })
            .collect(),
        ..ProviderPlan::default()
    };
    let retained = SelectedProviderReviewProvenance {
        plan: plan.clone(),
        provider: ProviderPlanProvenance {
            schema: ProviderSchemaDeclaration::BoundaryTrait(symbols::SymbolHandle::invalid()),
            provider_type: None,
            row_requirements: Vec::new(),
            row_realizations: Vec::new(),
            row_target_machine_origins: Vec::new(),
        },
        selected_by: ProviderSelectionProvenance::UniqueCoveringCandidate,
        row_compiler_intrinsic_executions: rows.iter().map(|(_, execution)| *execution).collect(),
    };
    (plan, retained)
}

fn demands(identities: &[&str]) -> BTreeSet<String> {
    identities
        .iter()
        .map(|identity| (*identity).to_owned())
        .collect()
}

#[test]
fn reordered_rows_preserve_lexical_proposals_and_exact_plan_associations() {
    use CompilerIntrinsicExecutionIdentity::{
        HostedExitProcessI32, HostedReadByte, HostedWriteByteI32,
    };
    let (first, first_retained) = selected_plan(
        "first",
        &[
            ("zeta", Some(HostedExitProcessI32)),
            ("alpha", Some(HostedWriteByteI32)),
            ("unused", None),
        ],
    );
    let (mut second, mut second_retained) = selected_plan(
        "second",
        &[("beta", Some(HostedReadByte)), ("ordinary", None)],
    );
    second.rows[1].binding = ProviderBinding::Syscall { number: 0 };
    second_retained.plan = second.clone();
    let demanded = demands(&["zeta", "ordinary", "beta", "missing", "alpha"]);
    for reverse_rows in [false, true] {
        let mut plans = [first.clone(), second.clone()];
        let mut provenance = [first_retained.clone(), second_retained.clone()];
        if reverse_rows {
            for (plan, retained) in plans.iter_mut().zip(&mut provenance) {
                plan.rows.reverse();
                retained.plan.rows.reverse();
                retained.row_compiler_intrinsic_executions.reverse();
            }
        }
        let proposals =
            derive_selected_intrinsic_settlement_proposals(&plans, &provenance, &demanded)
                .expect("sorted join retains original row execution and plan index");
        let actual = proposals
            .iter()
            .map(|proposal| {
                (
                    proposal.requirement_identity.as_str(),
                    proposal.plan_index,
                    proposal.execution,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            [
                ("alpha", 0, CompilerBuiltinExecution::HostedWriteByteI32),
                ("beta", 1, CompilerBuiltinExecution::HostedReadByte),
                ("zeta", 0, CompilerBuiltinExecution::HostedExitProcessI32),
            ]
        );
    }
}

#[test]
fn duplicate_missing_and_unsupported_executions_keep_lexical_diagnostics() {
    use CompilerIntrinsicExecutionIdentity::{HostedReadByte, NamedFloatNegation};
    let (first, first_retained) = selected_plan(
        "first",
        &[
            (
                "zeta",
                Some(NamedFloatNegation(numerics::literals::FloatFormat::F32)),
            ),
            ("beta", None),
            ("alpha", Some(HostedReadByte)),
            ("alpha", None),
        ],
    );
    let (second, second_retained) = selected_plan("second", &[("alpha", Some(HostedReadByte))]);
    let diagnostics = derive_selected_intrinsic_settlement_proposals(
        &[first, second],
        &[first_retained, second_retained],
        &demands(&["zeta", "missing", "alpha", "beta"]),
    )
    .expect_err("duplicates must not choose a row or hide later diagnostics");
    let messages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        [
            "Terminal boundary `alpha` resolves to 3 selected compiler-intrinsic rows",
            "selected compiler intrinsic `first` for Terminal boundary `beta` has no closed native catalog identity",
            "selected compiler intrinsic `first` for Terminal boundary `zeta` has no native boundary realization",
        ]
    );
}

#[test]
fn undemanded_plans_still_require_complete_aligned_provenance() {
    let (plan, retained) = selected_plan("unused", &[("unused", None)]);
    for demanded in [BTreeSet::new(), demands(&["unmatched"])] {
        let diagnostics = derive_selected_intrinsic_settlement_proposals(
            std::slice::from_ref(&plan),
            &[],
            &demanded,
        )
        .expect_err("missing retained plan rejects even without demand");
        assert!(diagnostics[0].message.contains("plans are not aligned"));
        for corrupt_plan in [false, true] {
            let mut malformed = retained.clone();
            if corrupt_plan {
                malformed.plan.rows[0].requirement_identity = "substituted".to_owned();
            } else {
                malformed.row_compiler_intrinsic_executions.clear();
            }
            let diagnostics = derive_selected_intrinsic_settlement_proposals(
                std::slice::from_ref(&plan),
                &[malformed],
                &demanded,
            )
            .expect_err("all retained plan and execution alignments are checked");
            assert_eq!(
                diagnostics[0].message,
                "selected compiler-intrinsic plan `unused` has misaligned retained settlement provenance"
            );
        }
    }
}

#[test]
fn undemanded_duplicates_and_absent_intrinsic_rows_produce_no_proposals() {
    let (plan, retained) = selected_plan("unused", &[("unused", None), ("unused", None)]);
    for demanded in [BTreeSet::new(), demands(&["before", "zz_after"])] {
        assert!(
            derive_selected_intrinsic_settlement_proposals(
                std::slice::from_ref(&plan),
                std::slice::from_ref(&retained),
                &demanded,
            )
            .expect("undemanded rows grant no proposal")
            .is_empty()
        );
    }
}
