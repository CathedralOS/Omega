use super::*;

fn transfer(source: u32, destination: u32, projected: bool) -> ParameterTransfer {
    ParameterTransfer {
        destination: SymbolHandle::from_arena_index(destination),
        source: Some(ProgressSubject {
            root: SymbolHandle::from_arena_index(source),
            projections: if projected {
                vec![SymbolHandle::from_arena_index(100)]
            } else {
                Vec::new()
            },
        }),
    }
}

#[test]
fn projected_self_edge_grows_but_identity_self_edge_does_not() {
    for projected in [false, true] {
        let transfers = [transfer(1, 1, projected)];
        assert_eq!(grows_on_cycle(&transfers, &transfers[0]), projected);
    }
}

#[test]
fn growth_can_return_through_multiple_identity_transfers() {
    let transfers = [
        transfer(1, 2, true),
        transfer(2, 3, false),
        transfer(3, 1, false),
    ];
    assert!(grows_on_cycle(&transfers, &transfers[0]));
    assert!(!grows_on_cycle(&transfers, &transfers[1]));
    assert!(!grows_on_cycle(&transfers, &transfers[2]));
}

#[test]
fn projected_input_to_an_identity_cycle_is_not_itself_cyclic() {
    let transfers = [
        transfer(1, 2, true),
        transfer(2, 3, false),
        transfer(3, 2, false),
    ];
    assert!(
        transfers
            .iter()
            .all(|transfer| !grows_on_cycle(&transfers, transfer))
    );
}

#[test]
fn a_finite_projection_chain_does_not_gain_a_cycle_from_another_parameter() {
    let transfers = [
        transfer(1, 2, true),
        transfer(2, 3, true),
        transfer(4, 4, true),
    ];
    assert!(!grows_on_cycle(&transfers, &transfers[0]));
    assert!(!grows_on_cycle(&transfers, &transfers[1]));
    assert!(grows_on_cycle(&transfers, &transfers[2]));
}

#[test]
fn an_unresolved_argument_is_not_a_projection_edge() {
    let transfers = [ParameterTransfer {
        destination: SymbolHandle::from_arena_index(1),
        source: None,
    }];
    assert!(!grows_on_cycle(&transfers, &transfers[0]));
}
