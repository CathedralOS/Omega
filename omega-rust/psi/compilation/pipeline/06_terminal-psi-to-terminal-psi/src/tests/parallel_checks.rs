use super::*;
use crate::ProofSearchActivity;
use execution_admission::ExecutionBudget;

fn cohort(valid: bool) -> typed_trees_to_terminal_psi::Elaboration {
    let entry = Machine {
        requires: vec![at_least(1, u128::from(valid)), at_most(1, 100)],
        ..machine(
            vec![u32_type()],
            vec![returns(call(2, vec![slot(0, u32_type())]))],
        )
    };
    let helper = machine(
        vec![u32_type()],
        vec![returns(strict(
            Strict::Subtract,
            slot(0, u32_type()),
            unsigned(1),
        ))],
    );
    let mut machines = vec![entry, helper];
    machines.extend((0..30).map(|_| add_of_formals(vec![at_most(1, 100), at_most(2, 100)])));
    let mut elaboration = lower(machines).unwrap();
    elaboration.private_inputs = vec![id(2)];
    elaboration.local_results = vec![id(2)];
    elaboration
}

#[test]
fn parallel_checks_preserve_inference_proofs_and_refused_obligations() {
    for valid in [true, false] {
        let input = cohort(valid);
        let mut expected = None;
        for workers in [1, 2, 4, 8] {
            let budget = ExecutionBudget::with_workers(workers).unwrap();
            budget.run(|| {
                let mut elaboration = input.clone();
                let mut activity = ProofSearchActivity::default();
                let result = discharge(&mut elaboration, &mut activity).unwrap();
                let mut refused = 0;
                for (machine, checked) in result.machines.iter() {
                    for (obligation, verdict) in checked.verdicts.iter() {
                        match verdict {
                            Ok(Discharged::Certificate(evidence)) => {
                                assert_kernel_accepted(
                                    &elaboration,
                                    machine.get(),
                                    obligation,
                                    evidence,
                                );
                            }
                            Err(_) => refused += 1,
                            _ => {}
                        }
                    }
                }
                assert_eq!(refused == 0, valid);
                // Includes inferred contracts, proof trees, knowledge IDs,
                // source correspondence and ordered positive/negative verdicts.
                let found = format!("{elaboration:?}{result:?}");
                if let Some(expected) = &expected {
                    assert_eq!(&found, expected);
                } else {
                    expected = Some(found);
                }
            });
        }
    }
}

#[test]
fn parallel_checks_select_the_lowest_structurally_failing_machine() {
    let mut settled = cohort(true);
    let mut activity = ProofSearchActivity::default();
    discharge(&mut settled, &mut activity).unwrap();
    let mut input = settled;
    // Both machines fail before proof search. Higher identities may finish
    // first, but cannot replace the refusal the sequential walk selected.
    for (machine, tables) in input.tables.machines.iter_mut() {
        if matches!(machine.get(), 2 | 17) {
            assert!(!tables.knowledge.is_empty());
        } else {
            tables.knowledge = IdTable::default();
        }
    }
    for workers in [1, 2, 4, 8] {
        let budget = ExecutionBudget::with_workers(workers).unwrap();
        for _ in 0..4 {
            let mut activity = ProofSearchActivity::default();
            let error = budget
                .run(|| discharge(&mut input.clone(), &mut activity))
                .unwrap_err();
            assert_eq!(error.machine, id(2));
            assert_eq!(error.detail, "knowledge rows already present");
        }
    }
}
