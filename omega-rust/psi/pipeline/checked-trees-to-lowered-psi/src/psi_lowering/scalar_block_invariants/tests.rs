use super::*;
use semantic_vocabulary::{BlockId, EdgeId, MachineId};
use terminal_psi::ScalarBlockInvariant;

fn seed(header: u64, obligation: u64) -> ScalarBlockInvariant {
    ScalarBlockInvariant {
        machine: MachineId::new(1).unwrap(),
        header: BlockId::new(header).unwrap(),
        predicate: Proposition::Truth,
        arrivals: vec![ScalarBlockInvariantArrival {
            edge: EdgeId::new(header).unwrap(),
            obligation: ObligationId::new(obligation).unwrap(),
        }],
    }
}

#[test]
fn rejected_expansion_restores_all_seed_rows_and_arrival_identities_once() {
    let original = vec![seed(2, 7), seed(3, 8)];
    let mut seeds = Some(original.clone());
    let mut expanded = original.clone();
    expanded[0].predicate = Proposition::Falsehood;
    expanded.remove(1);
    expanded.push(seed(4, 9));
    assert!(restore_seeds(&mut expanded, &mut seeds));
    assert_eq!(expanded, original);
    assert!(seeds.is_none());
    // The next proof/drop round may reject an original seed. Restoration must
    // not resurrect it or restart discovery indefinitely.
    expanded.remove(0);
    assert!(!restore_seeds(&mut expanded, &mut seeds));
    assert_eq!(expanded, original[1..]);
}

#[test]
fn unchanged_seed_roster_is_not_a_new_retry() {
    let mut original = vec![seed(2, 7)];
    let mut seeds = Some(original.clone());
    assert!(!restore_seeds(&mut original, &mut seeds));
    assert!(seeds.is_none());
}
