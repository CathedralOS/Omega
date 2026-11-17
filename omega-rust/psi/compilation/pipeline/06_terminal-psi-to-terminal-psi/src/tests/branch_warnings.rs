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
        let warnings = &result.machines.get(id(1)).unwrap().impossible;
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
        let (_, result) = run(vec![guarded_program(32, contradictory, impossible_entry)]);
        let warnings = &result.machines.get(id(1)).unwrap().impossible;
        assert!(
            warnings.is_empty(),
            "contradictory={contradictory}, impossible_entry={impossible_entry}: {warnings:?}"
        );
    }
}
