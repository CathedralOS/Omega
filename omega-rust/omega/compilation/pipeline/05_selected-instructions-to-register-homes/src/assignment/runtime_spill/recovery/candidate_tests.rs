use super::{FailedDomain, candidate_position};
use crate::RegisterHomeError;
use register_model::RegisterClassId;
use selected_instructions::{
    DistinctUseDefTie, EdgeRegisterTransfer, FunctionLiveRanges, LiveRangePoint, LivenessPosition,
    SelectedBlockId, SelectedInstructionId, VirtualInterference, VirtualRegisterId,
};
use semantic_vocabulary::{EdgeId, MachineId};

fn pressure(function: usize, register: u32) -> RegisterHomeError {
    RegisterHomeError::NoCompatibleHome { function, register }
}

#[test]
fn pressure_ranking_filters_other_functions_and_preserves_equal_score_order() {
    let roster = [
        (1, VirtualRegisterId(9)),
        (2, VirtualRegisterId(3)),
        (2, VirtualRegisterId(9)),
    ];
    assert_eq!(
        candidate_position(
            &pressure(2, 9),
            &roster,
            |(function, _)| *function == 2,
            |_| 0
        ),
        Some(1)
    );
    assert_eq!(
        candidate_position(
            &pressure(2, 9),
            &roster[..2],
            |(function, _)| *function == 2,
            |_| 0
        ),
        Some(1)
    );
    // The reported leader gains no preference over a longer-lived blocker.
    assert_eq!(
        candidate_position(
            &pressure(2, 9),
            &roster,
            |(function, _)| *function == 2,
            |(_, register)| (1, if register.0 == 3 { 100 } else { 1 })
        ),
        Some(1)
    );
}

#[test]
fn incompatible_interval_homes_choose_an_admitted_recovery_candidate() {
    let roster = [(2, VirtualRegisterId(9))];
    assert_eq!(
        candidate_position(
            &RegisterHomeError::NoCommonCandidate {
                function: 2,
                register: 9
            },
            &roster,
            |_| true,
            |_| 0
        ),
        Some(0)
    );
    assert_eq!(
        candidate_position(
            &RegisterHomeError::UnknownOrIncompatibleView {
                function: 2,
                register: 9,
                view: 0
            },
            &roster,
            |_| true,
            |_| 0
        ),
        None
    );
}

#[test]
fn empty_common_domain_can_only_rewrite_the_failed_value() {
    let failure = RegisterHomeError::NoCommonCandidate {
        function: 2,
        register: 9,
    };
    let roster = [
        (2, VirtualRegisterId(7)),
        (1, VirtualRegisterId(9)),
        (2, VirtualRegisterId(9)),
    ];
    assert_eq!(
        candidate_position(&failure, &roster, |_| true, |_| 100),
        Some(2)
    );
    assert_eq!(
        candidate_position(&failure, &roster[..2], |_| true, |_| 100),
        None
    );
    let ranges = split_ranges(&[(9, 10)], &[], &[(9, 7)]);
    let mut domain = FailedDomain::in_function(2, &ranges, VirtualRegisterId(9));
    domain.members = vec![VirtualRegisterId(9)];
    domain.intrinsic = true;
    assert_eq!(domain.relief(2, VirtualRegisterId(9)), 1);
    assert_eq!(domain.relief(2, VirtualRegisterId(7)), 0);
    assert_eq!(domain.relief(2, VirtualRegisterId(10)), 0);
}

fn tie(use_register: u32, def_register: u32) -> DistinctUseDefTie {
    DistinctUseDefTie {
        block: SelectedBlockId(0),
        position: LivenessPosition(use_register),
        instruction: SelectedInstructionId(use_register),
        use_operand: 0,
        use_virtual_register: VirtualRegisterId(use_register),
        use_point: LiveRangePoint(use_register),
        def_operand: 1,
        def_virtual_register: VirtualRegisterId(def_register),
        def_point: LiveRangePoint(def_register),
        class: RegisterClassId(0),
    }
}

fn transfer(argument: u32, parameter: u32) -> EdgeRegisterTransfer {
    EdgeRegisterTransfer {
        source: SelectedBlockId(0),
        target: SelectedBlockId(1),
        psi_edge: EdgeId::new(u64::from(argument) + 1).unwrap(),
        argument: VirtualRegisterId(argument),
        parameter: VirtualRegisterId(parameter),
        class: RegisterClassId(0),
    }
}

fn split_ranges(
    ties: &[(u32, u32)],
    transfers: &[(u32, u32)],
    interference: &[(u32, u32)],
) -> FunctionLiveRanges {
    FunctionLiveRanges {
        machine: MachineId::new(1).unwrap(),
        block_domains: Vec::new(),
        virtual_registers: Vec::new(),
        tied_pairs: ties
            .iter()
            .map(|(used, defined)| tie(*used, *defined))
            .collect(),
        edge_transfers: transfers
            .iter()
            .map(|(argument, parameter)| transfer(*argument, *parameter))
            .collect(),
        copy_affinities: Vec::new(),
        early_clobbers: Vec::new(),
        architectural_units: Vec::new(),
        interference: interference
            .iter()
            .map(|(lower, higher)| VirtualInterference {
                lower: VirtualRegisterId(*lower),
                higher: VirtualRegisterId(*higher),
            })
            .collect(),
    }
}

#[test]
fn pressure_coalesces_across_tied_split_points() {
    // The domain {0, 1} is one live range split at the tie: member fragments
    // are disjoint, so a victim overlapping only the sibling fragment still
    // blocks the shared home even though it never touches the leader.
    let ranges = split_ranges(&[(0, 1)], &[], &[(1, 7)]);
    let domain = FailedDomain::in_function(0, &ranges, VirtualRegisterId(0));
    assert!(domain.relief(0, VirtualRegisterId(7)) > 0);
    assert_eq!(domain.relief(0, VirtualRegisterId(5)), 0);
    // A register of another function never relieves this domain.
    assert_eq!(domain.relief(1, VirtualRegisterId(7)), 0);

    // The interfering fragment may be a retired reload. Its untried edge
    // destination still owns the same home and must remain a pressure victim.
    let ranges = split_ranges(&[(0, 1), (7, 8)], &[(8, 9)], &[(1, 7), (1, 8)]);
    let domain = FailedDomain::in_function(0, &ranges, VirtualRegisterId(0));
    assert_eq!(domain.relief(0, VirtualRegisterId(9)), 1);
    assert_eq!(domain.relief(0, VirtualRegisterId(7)), 1);
    assert_eq!(domain.relief(0, VirtualRegisterId(5)), 0);
}

#[test]
fn direct_pressure_precedes_longer_peers_without_excluding_them() {
    use selected_instructions::{LiveRangeFragment, VirtualLiveRange};
    let mut ranges = split_ranges(&[], &[(8, 9)], &[(0, 7), (0, 8)]);
    for (register, length) in [(7, 2), (9, 200), (10, 2)] {
        ranges.virtual_registers.push(VirtualLiveRange {
            virtual_register: VirtualRegisterId(register),
            class: RegisterClassId(0),
            occurrences: Vec::new(),
            fixed_constraints: Vec::new(),
            fragments: vec![LiveRangeFragment {
                block: SelectedBlockId(0),
                start: LiveRangePoint(0),
                end: LiveRangePoint(length),
            }],
            edge_connectors: Vec::new(),
        });
    }
    ranges.interference.push(VirtualInterference {
        lower: VirtualRegisterId(0),
        higher: VirtualRegisterId(10),
    });
    let domain = FailedDomain::in_function(0, &ranges, VirtualRegisterId(0));
    let roster = [
        (0, VirtualRegisterId(9)),
        (0, VirtualRegisterId(7)),
        (0, VirtualRegisterId(10)),
    ];
    let select = |roster: &[(usize, VirtualRegisterId)]| {
        candidate_position(
            &pressure(0, 0),
            roster,
            |(function, register)| domain.relief(*function, *register) > 0,
            |(function, register)| domain.spill_priority(*function, *register),
        )
    };
    assert_eq!(select(&roster), Some(1));
    assert_eq!(select(&roster[..1]), Some(0));

    // Union direction is not minimum identity. Grouping uses the root's
    // original roster position, including an unsorted roster.
    let ranges = split_ranges(&[(9, 1), (1, 7)], &[(4, 2)], &[]);
    let components = crate::assignment::same_home::SameHomeComponents::from_ranges(&ranges);
    let roster = [
        VirtualRegisterId(7),
        VirtualRegisterId(4),
        VirtualRegisterId(9),
        VirtualRegisterId(1),
        VirtualRegisterId(2),
    ];
    let positions: Vec<_> = roster
        .iter()
        .map(|member| {
            roster
                .iter()
                .position(|candidate| *candidate == components.leader(*member))
        })
        .collect();
    assert_eq!(positions, [Some(2), Some(1), Some(2), Some(2), Some(1)]);
}

#[test]
fn relief_counts_membership_alongside_covered_siblings() {
    // A member covers itself; an outsider covering the whole transitive chain
    // {0, 1, 2} scores higher than any single member's own split.
    let ranges = split_ranges(&[(0, 1)], &[(1, 2)], &[(0, 9), (1, 9), (2, 9)]);
    let domain = FailedDomain::in_function(0, &ranges, VirtualRegisterId(0));
    assert_eq!(domain.relief(0, VirtualRegisterId(9)), 3);
    assert_eq!(domain.relief(0, VirtualRegisterId(1)), 1);
    assert_eq!(domain.relief(0, VirtualRegisterId(2)), 1);
}
