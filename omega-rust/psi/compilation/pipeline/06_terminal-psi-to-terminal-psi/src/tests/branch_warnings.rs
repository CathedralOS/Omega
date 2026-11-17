//! Warning explanations consume accepted premise provenance, not another
//! series of proof searches. These tests use the real lowered/checker tables.

use super::*;
use crate::branch_warnings::Analysis;
use crate::graph::Graph;

fn guarded_program(unrelated: usize, contradictory: bool, impossible_entry: bool) -> Machine {
    let mut requires = vec![at_most(1, 5)];
    requires.extend((0..unrelated).map(|clause| at_most(2, 100 + clause as u128)));
    if impossible_entry {
        requires.push(Proposition::Falsehood);
    }
    Machine {
        requires,
        ..machine(
            vec![u32_type(), u32_type()],
            vec![
                guarded(
                    compare(
                        if contradictory {
                            Strict::Greater
                        } else {
                            Strict::Less
                        },
                        slot(0, u32_type()),
                        // Below three and at least three both occur under
                        // x <= 5; below ten would make the fallback dead.
                        unsigned(if contradictory { 10 } else { 3 }),
                    ),
                    unsigned(1),
                ),
                returns(unsigned(0)),
            ],
        )
    }
}

#[test]
fn warning_explanations_use_one_refutation_per_point_not_one_per_premise_or_arm() {
    for unrelated in [0, 32, 128] {
        let (elaboration, result) = run(vec![guarded_program(unrelated, true, false)]);
        let mut activity = crate::ProofSearchActivity::default();
        let tables = elaboration.tables.machines.get(id(1)).unwrap();
        let lowered = &elaboration.module.machines[0];
        let branches = &elaboration.sources.get(id(1)).unwrap().branches;
        let deferred = &result.machines.get(id(1)).unwrap().branch_warnings;
        let warnings = deferred.materialize(lowered, tables, branches, &mut activity);
        let searches = activity.started();
        assert!(
            searches > 0,
            "diagnostics must perform the real warning search"
        );
        assert_eq!(
            deferred.materialize(lowered, tables, branches, &mut activity),
            warnings
        );
        assert_eq!(
            activity.started(),
            searches,
            "repeated diagnostics redo no search"
        );
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0].premises,
            [Justification::EntryRequirement { clause: 0 }]
        );
        let tables = elaboration.tables.machines.get(id(1)).unwrap();
        let lowered = &elaboration.module.machines[0];
        let graph = Graph::new(id(1), lowered, tables).unwrap();
        let context = PropositionContext::from_value_types(
            tables
                .values
                .iter()
                .map(|(value, row)| (value, row.scalar_type)),
        )
        .unwrap();
        let branch = elaboration.sources.get(id(1)).unwrap().branches[0];
        let mut analysis = Analysis::new(&context, &graph, tables, lowered, id(1));
        let repeated = analysis.run(
            &vec![branch; 64],
            &mut crate::ProofSearchActivity::default(),
        );
        assert_eq!(repeated.len(), 64);
        assert_eq!(analysis.searches, 2, "unrelated premises: {unrelated}");
        assert_eq!(analysis.query_count(), 2);
        for warning in repeated {
            assert_eq!(warning.premises, warnings[0].premises);
        }
    }
}

#[test]
fn consistent_arms_and_arrivals_already_impossible_before_the_branch_stay_quiet() {
    for (contradictory, impossible_entry) in [(false, false), (true, true)] {
        let (elaboration, result) = run(vec![guarded_program(32, contradictory, impossible_entry)]);
        let warnings = result
            .machines
            .get(id(1))
            .unwrap()
            .branch_warnings
            .materialize(
                &elaboration.module.machines[0],
                elaboration.tables.machines.get(id(1)).unwrap(),
                &elaboration.sources.get(id(1)).unwrap().branches,
                &mut crate::ProofSearchActivity::default(),
            );
        assert!(
            warnings.is_empty(),
            "contradictory={contradictory}, impossible_entry={impossible_entry}: {warnings:?}"
        );
    }
}

#[test]
fn mandatory_checking_leaves_warning_search_to_accepted_diagnostics() {
    let mut searches = Vec::new();
    for fails_guarantee in [true, false] {
        let lower_program = |without_warning_sites| {
            let mut machine = guarded_program(0, true, false);
            if fails_guarantee {
                machine.ensures.push(Proposition::Falsehood);
            }
            let mut input = lower(vec![machine]).unwrap();
            if without_warning_sites {
                // Keep the operations and contracts identical; remove only
                // the source coordinates used to explain optional warnings.
                input.sources.get_mut(id(1)).unwrap().branches.clear();
            }
            input
        };
        let mut activity = crate::ProofSearchActivity::default();
        let verdict = crate::verdict::judge_owing_with_activity(
            Ok(lower_program(false)),
            crate::verdict::AppliedContracts::Undecided,
            &mut activity,
        )
        .unwrap();
        let mut mandatory = crate::ProofSearchActivity::default();
        let control = crate::verdict::judge_owing_with_activity(
            Ok(lower_program(true)),
            crate::verdict::AppliedContracts::Undecided,
            &mut mandatory,
        )
        .unwrap();
        searches.push((fails_guarantee, activity.started(), mandatory.started()));
        if fails_guarantee {
            for rejected in [verdict, control] {
                let RouteVerdict::Rejected { unproved, .. } = rejected else {
                    panic!("the false guarantee must be rejected");
                };
                assert!(!unproved.is_empty(), "the guarantee must remain unproved");
            }
        } else {
            let RouteVerdict::Accepted(accepted) = verdict else {
                panic!("the guarded program must be accepted");
            };
            assert!(matches!(control, RouteVerdict::Accepted(_)));
            let diagnostics = accepted.advisories();
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].message.contains("unreachable branch:"));
            assert!(diagnostics[0].message.contains("an entry requirement"));
            assert_eq!(accepted.advisories(), diagnostics);
        }
    }
    // Check both semantic outcomes before comparing the optional work.
    for (fails_guarantee, actual, mandatory) in searches {
        assert_eq!(
            actual, mandatory,
            "mandatory checking searched warning coordinates: fails_guarantee={fails_guarantee}"
        );
    }
}
