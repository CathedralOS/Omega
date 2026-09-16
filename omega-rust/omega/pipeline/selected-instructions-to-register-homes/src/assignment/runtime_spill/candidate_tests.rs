use super::{candidate_position, split_domain_pressure};
use crate::RegisterHomeError;
use register_model::RegisterClassId;
use selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use semantic_vocabulary::{EdgeId, MachineId};

fn pressure(function: usize, register: u32) -> RegisterHomeError {
    RegisterHomeError::NoCompatibleHome { function, register }
}

#[test]
fn failed_original_register_precedes_lower_numbered_interfering_values() {
    let roster = [(2, VirtualRegisterId(3)), (2, VirtualRegisterId(9))];
    assert_eq!(
        candidate_position(&pressure(2, 9), &roster, |_| true),
        Some(1)
    );
}

#[test]
fn rejected_failed_register_restores_original_order_and_is_not_retried() {
    let mut roster = vec![
        (2, VirtualRegisterId(3)),
        (2, VirtualRegisterId(5)),
        (2, VirtualRegisterId(9)),
    ];
    let failure = pressure(2, 9);
    let selected = candidate_position(&failure, &roster, |_| true).unwrap();
    assert_eq!(roster.remove(selected), (2, VirtualRegisterId(9)));
    // This is the existing removal-before-admission path for unsupported values.
    let selected = candidate_position(&failure, &roster, |_| true).unwrap();
    assert_eq!(roster.remove(selected), (2, VirtualRegisterId(3)));
    assert_eq!(candidate_position(&failure, &roster, |_| true), Some(0));
    assert_eq!(roster[0], (2, VirtualRegisterId(5)));
}

#[test]
fn failed_identity_in_another_function_does_not_gain_priority() {
    let roster = [
        (1, VirtualRegisterId(9)),
        (2, VirtualRegisterId(3)),
        (2, VirtualRegisterId(9)),
    ];
    assert_eq!(
        candidate_position(&pressure(2, 9), &roster, |(function, _)| *function == 2),
        Some(2)
    );
    assert_eq!(
        candidate_position(&pressure(2, 9), &roster[..2], |(function, _)| *function
            == 2),
        Some(1)
    );
}

#[test]
fn absent_or_generated_failed_value_uses_only_overlapping_original_candidates() {
    let roster = [
        (2, VirtualRegisterId(3)),
        (2, VirtualRegisterId(5)),
        (2, VirtualRegisterId(9)),
    ];
    // A generated reload or spill address never enlarges the original roster.
    assert_eq!(
        candidate_position(&pressure(2, 100), &roster, |(_, register)| *register
            == VirtualRegisterId(5)),
        Some(1)
    );
    assert_eq!(
        candidate_position(&pressure(2, 100), &roster, |_| false),
        None
    );
    assert_eq!(candidate_position(&pressure(2, 100), &[], |_| true), None);
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
            |_| true
        ),
        None
    );
}

fn tie(use_register: u32, def_register: u32) -> crate::DistinctUseDefTie {
    crate::DistinctUseDefTie {
        block: SelectedBlockId(0),
        position: crate::LivenessPosition(use_register),
        instruction: SelectedInstructionId(use_register),
        use_operand: 0,
        use_virtual_register: VirtualRegisterId(use_register),
        use_point: crate::LiveRangePoint(use_register),
        def_operand: 1,
        def_virtual_register: VirtualRegisterId(def_register),
        def_point: crate::LiveRangePoint(def_register),
        class: RegisterClassId(0),
    }
}

fn transfer(argument: u32, parameter: u32) -> crate::EdgeRegisterTransfer {
    crate::EdgeRegisterTransfer {
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
) -> crate::FunctionLiveRanges {
    crate::FunctionLiveRanges {
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
            .map(|(lower, higher)| crate::VirtualInterference {
                lower: VirtualRegisterId(*lower),
                higher: VirtualRegisterId(*higher),
            })
            .collect(),
    }
}

#[test]
fn pressure_relief_still_covers_the_named_leader_fragment() {
    // An isolated failure with no ties keeps the original interference rule.
    let ranges = split_ranges(&[], &[], &[(0, 7)]);
    assert!(split_domain_pressure(
        &ranges,
        VirtualRegisterId(0),
        VirtualRegisterId(0)
    ));
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
fn pressure_coalesces_across_transitive_and_edge_transfer_split_points() {
    // Ties and edge transfers chain into one domain: {0, 1} by tie, {1, 2} by
    // edge transfer. Interference anywhere in the split range is pressure.
    let ranges = split_ranges(&[(0, 1)], &[(1, 2)], &[(2, 9)]);
    assert!(split_domain_pressure(
        &ranges,
        VirtualRegisterId(0),
        VirtualRegisterId(9)
    ));
}

#[test]
fn a_member_fragment_is_itself_a_pressure_victim() {
    // Splitting a member's own range dissolves the shared-home requirement.
    let ranges = split_ranges(&[(0, 1)], &[], &[]);
    assert!(split_domain_pressure(
        &ranges,
        VirtualRegisterId(0),
        VirtualRegisterId(1)
    ));
    assert!(split_domain_pressure(
        &ranges,
        VirtualRegisterId(1),
        VirtualRegisterId(0)
    ));
}
