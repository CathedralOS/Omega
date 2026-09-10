use super::{compute_function, fixtures, validate};
use register_model::{RegisterClassId, RegisterOperandAccess, RegisterViewId};
use selected_instructions::{
    EdgeRegisterTransfer, LiveRangeEdgeConnector, LiveRangeFragment, LiveRangePoint,
    LivenessPosition, SelectedBlockId, SelectedInstructionId, VirtualFixedConstraint,
    VirtualFixedConstraintSite, VirtualOccurrence, VirtualRegisterId,
};
use semantic_vocabulary::EdgeId;

#[test]
fn empty_physical_values_have_no_home_and_replay_requires_exact_remaining_roster() {
    let physical = fixtures::physical();
    let mut legality = fixtures::legality(&[(0, 1), (2, 3)]);
    legality.virtual_registers[1].points.clear();
    let ranges = fixtures::ranges(2, &[]);
    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(homes.assignments.len(), 1);
    assert_eq!(homes.assignments[0].virtual_register, VirtualRegisterId(0));
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );
    validate::validate_function(0, &homes, &legality, &ranges, &physical).unwrap();

    let mut extra = homes.clone();
    let mut dead_home = extra.assignments[0];
    dead_home.virtual_register = VirtualRegisterId(1);
    extra.assignments.push(dead_home);
    assert!(validate::validate_function(0, &extra, &legality, &ranges, &physical).is_err());
    let mut missing = homes;
    missing.assignments.clear();
    assert!(validate::validate_function(0, &missing, &legality, &ranges, &physical).is_err());

    legality.virtual_registers[0].points.clear();
    let empty = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert!(empty.assignments.is_empty());
    validate::validate_function(0, &empty, &legality, &ranges, &physical).unwrap();
}

#[test]
fn empty_legality_cannot_discard_any_remaining_physical_requirement() {
    let physical = fixtures::physical();
    for mutation in 0..13 {
        let mut legality = fixtures::legality(&[(0, 1), (2, 3)]);
        legality.virtual_registers[1].points.clear();
        let mut ranges = fixtures::ranges(2, &[]);
        match mutation {
            0 => ranges.virtual_registers[1]
                .occurrences
                .push(VirtualOccurrence {
                    position: LivenessPosition(1),
                    point: LiveRangePoint(2),
                    instruction: SelectedInstructionId(1),
                    operand: 0,
                    access: RegisterOperandAccess::Use,
                }),
            1 => ranges.virtual_registers[1]
                .fragments
                .push(LiveRangeFragment {
                    block: SelectedBlockId(0),
                    start: LiveRangePoint(2),
                    end: LiveRangePoint(3),
                }),
            2 => ranges.virtual_registers[1]
                .edge_connectors
                .push(LiveRangeEdgeConnector {
                    source: SelectedBlockId(0),
                    terminator: SelectedInstructionId(1),
                    polarity_ordinal: 0,
                    psi_edge: EdgeId::new(1).unwrap(),
                    target: SelectedBlockId(1),
                }),
            3 => ranges.virtual_registers[1]
                .fixed_constraints
                .push(VirtualFixedConstraint {
                    site: VirtualFixedConstraintSite::Entry,
                    view: RegisterViewId(0),
                }),
            4 => legality.virtual_registers[1].early_clobber_points.push(
                crate::VirtualEarlyClobberPointLegality {
                    block: SelectedBlockId(0),
                    position: LivenessPosition(1),
                    instruction: SelectedInstructionId(1),
                    operand: 0,
                    point: LiveRangePoint(2),
                    candidates: vec![RegisterViewId(0)],
                },
            ),
            5 => legality.virtual_registers[1].entry_transitions.push(
                register_homes::EntryFixedViewTransition {
                    from_view: RegisterViewId(0),
                    to_site: VirtualFixedConstraintSite::Entry,
                    to_view: RegisterViewId(1),
                },
            ),
            6 => ranges.tied_pairs = fixtures::tied_ranges(&[]).tied_pairs,
            7 => ranges.edge_transfers.push(EdgeRegisterTransfer {
                source: SelectedBlockId(0),
                target: SelectedBlockId(1),
                psi_edge: EdgeId::new(1).unwrap(),
                argument: VirtualRegisterId(0),
                parameter: VirtualRegisterId(1),
                class: RegisterClassId(0),
            }),
            8 => {
                let mut early = fixtures::early_clobber_ranges().early_clobbers.remove(0);
                early.def_virtual_register = VirtualRegisterId(1);
                early.uses.clear();
                ranges.early_clobbers.push(early);
            }
            9 => ranges.interference = fixtures::ranges(2, &[(0, 1)]).interference,
            10 => ranges.virtual_registers[1].virtual_register = VirtualRegisterId(2),
            11 => ranges.virtual_registers[1].class = RegisterClassId(1),
            12 => {
                let mut early = fixtures::early_clobber_ranges().early_clobbers.remove(0);
                early.def_virtual_register = VirtualRegisterId(0);
                ranges.early_clobbers.push(early);
            }
            _ => unreachable!(),
        }
        assert!(
            compute_function(0, &legality, &ranges, &physical).is_err(),
            "producer accepted physical requirement mutation {mutation}"
        );
        assert!(
            validate::replay_function(0, &legality, &ranges, &physical).is_err(),
            "replay accepted physical requirement mutation {mutation}"
        );
    }
}
