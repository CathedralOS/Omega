use super::{candidate_position, member_relief, split_domain_pressure};
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
fn failed_identity_in_another_function_does_not_gain_priority() {
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
        Some(2)
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
}

#[test]
fn nonpressure_failures_do_not_choose_a_runtime_spill() {
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
        None
    );
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
    assert!(split_domain_pressure(
        &ranges,
        VirtualRegisterId(0),
        VirtualRegisterId(7)
    ));
    assert!(!split_domain_pressure(
        &ranges,
        VirtualRegisterId(0),
        VirtualRegisterId(5)
    ));
}

#[test]
fn relief_counts_membership_alongside_covered_siblings() {
    // A member covers itself; an outsider covering the whole transitive chain
    // {0, 1, 2} scores higher than any single member's own split.
    let ranges = split_ranges(&[(0, 1)], &[(1, 2)], &[(0, 9), (1, 9), (2, 9)]);
    assert_eq!(
        member_relief(&ranges, VirtualRegisterId(0), VirtualRegisterId(9)),
        3
    );
    assert_eq!(
        member_relief(&ranges, VirtualRegisterId(0), VirtualRegisterId(1)),
        1
    );
    assert_eq!(
        member_relief(&ranges, VirtualRegisterId(0), VirtualRegisterId(2)),
        1
    );
}
