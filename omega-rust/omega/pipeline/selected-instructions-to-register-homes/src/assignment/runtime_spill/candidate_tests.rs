use super::{candidate_position, candidates, member_relief, split_domain_pressure};
use crate::RegisterHomeError;
use legalized_operations::LegalizedStructuralContract;
use optimization_unit::ValueDefinitionSite;
use register_environment::baseline_target_register_environment;
use register_model::RegisterClassId;
use selected_instructions::{
    SelectedBlockId, SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, PlaceId, ScalarType, ValueId,
};
use target::NativeTarget;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn pressure(function: usize, register: u32) -> RegisterHomeError {
    RegisterHomeError::NoCompatibleHome { function, register }
}

/// The finite roster covers every register the representation's own
/// entry-liveness replay recognizes — scalar and structural parameters plus
/// the hidden aggregate-result destination — alongside instruction results,
/// block parameters, and instruction-defined structural registers: field
/// observations and transport registers join by origin so a pressured
/// structural plan can spill them, while generated spill origins never do.
#[test]
fn roster_covers_every_entry_live_in_origin() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let copy = environment.constraint(keys.copy_i64).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let class = copy.operands[0].class;
    let view = environment
        .physical()
        .model()
        .classes
        .iter()
        .find(|row| row.id == class)
        .and_then(|row| row.views.first())
        .copied()
        .expect("the scalar class always declares a view");
    let place = PlaceId::new(1).unwrap();
    let structural_type = semantic_vocabulary::StructuralTypeId::new(1).unwrap();
    let register = |id: u32,
                    origin: VirtualRegisterOrigin,
                    site: Option<ValueDefinitionSite>,
                    pinned: bool| VirtualRegister {
        id: VirtualRegisterId(id),
        scalar_type,
        class,
        origin,
        definition_site: site,
        entry_fixed_view: pinned.then_some(view),
    };
    let machine = MachineId::new(1).unwrap();
    let function = SelectedFunction {
        machine,
        attachment: None,
        provenance: Default::default(),
        structural: Some(LegalizedStructuralContract {
            result: Some(terminal_psi::StructuralResultDeclaration {
                place,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                reference_sources: Vec::new(),
            }),
            structural_types: Vec::new().into(),
            parameters: vec![legalized_operations::LegalizedCallUnitParameter {
                semantic: terminal_psi::StructuralParameterDeclaration {
                    place,
                    position: 0,
                    is_self: false,
                    structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
                target: target_operations::TargetStructuralParameter {
                    place,
                    structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                    projected_qualifications: Vec::new(),
                    shape: calling_conventions::ValueShape::borrowed_reference(8, 8),
                    placement: calling_conventions::ValuePlacement {
                        shape: calling_conventions::ValueShape::borrowed_reference(8, 8),
                        locations: Vec::new(),
                    },
                },
            }],
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
        }),
        local_storage_slots: Vec::new(),
        outgoing_arguments: Vec::new(),
        calls: Vec::new(),
        normalized_foreign_calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
        entry_block: SelectedBlockId(0),
        virtual_registers: vec![
            register(
                0,
                VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(1).unwrap(),
                    parameter_index: 0,
                },
                Some(ValueDefinitionSite::FunctionParameter(0)),
                true,
            ),
            register(
                1,
                VirtualRegisterOrigin::StructuralParameter {
                    place,
                    parameter_index: 0,
                },
                None,
                true,
            ),
            register(
                2,
                VirtualRegisterOrigin::AbiTransport {
                    instruction: SelectedInstructionId(0),
                    place,
                    byte_offset: 0,
                },
                None,
                true,
            ),
            // An instruction-made transport register is no entry live-in, but
            // joins the roster as an ordinary structural victim.
            register(
                3,
                VirtualRegisterOrigin::AbiTransport {
                    instruction: SelectedInstructionId(4),
                    place,
                    byte_offset: 0,
                },
                None,
                false,
            ),
            register(
                4,
                VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(4),
                    source_value: ValueId::new(2).unwrap(),
                },
                Some(ValueDefinitionSite::FunctionParameter(0)),
                false,
            ),
            register(
                5,
                VirtualRegisterOrigin::BlockParameter {
                    source_value: ValueId::new(3).unwrap(),
                    block: SelectedBlockId(1),
                    parameter_index: 0,
                },
                Some(ValueDefinitionSite::BlockParameter {
                    block: semantic_vocabulary::BlockId::new(3).unwrap(),
                    position: 0,
                }),
                false,
            ),
            // Generated spill origins never become candidates.
            register(
                6,
                VirtualRegisterOrigin::SpillAddress {
                    instruction: SelectedInstructionId(4),
                    register: VirtualRegisterId(0),
                },
                None,
                false,
            ),
            register(
                7,
                VirtualRegisterOrigin::StructuralObservation {
                    instruction: SelectedInstructionId(4),
                    place,
                    byte_offset: 0,
                },
                None,
                false,
            ),
        ],
        blocks: Vec::new(),
    };
    let plan = SelectedInstructionPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
        },
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        target,
        entry: machine,
        functions: vec![function].into(),
    };
    assert_eq!(
        candidates(&plan),
        vec![
            (0, VirtualRegisterId(0)),
            (0, VirtualRegisterId(1)),
            (0, VirtualRegisterId(2)),
            (0, VirtualRegisterId(3)),
            (0, VirtualRegisterId(4)),
            (0, VirtualRegisterId(5)),
            (0, VirtualRegisterId(7)),
        ]
    );
}

#[test]
fn failed_original_register_precedes_lower_numbered_interfering_values() {
    let roster = [(2, VirtualRegisterId(3)), (2, VirtualRegisterId(9))];
    assert_eq!(
        candidate_position(&pressure(2, 9), &roster, |_| true, |_| 0),
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
    let selected = candidate_position(&failure, &roster, |_| true, |_| 0).unwrap();
    assert_eq!(roster.remove(selected), (2, VirtualRegisterId(9)));
    // This is the existing removal-before-admission path for unsupported values.
    let selected = candidate_position(&failure, &roster, |_| true, |_| 0).unwrap();
    assert_eq!(roster.remove(selected), (2, VirtualRegisterId(3)));
    assert_eq!(
        candidate_position(&failure, &roster, |_| true, |_| 0),
        Some(0)
    );
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
fn absent_or_generated_failed_value_uses_only_overlapping_original_candidates() {
    let roster = [
        (2, VirtualRegisterId(3)),
        (2, VirtualRegisterId(5)),
        (2, VirtualRegisterId(9)),
    ];
    // A generated reload or spill address never enlarges the original roster.
    assert_eq!(
        candidate_position(
            &pressure(2, 100),
            &roster,
            |(_, register)| *register == VirtualRegisterId(5),
            |_| 0
        ),
        Some(1)
    );
    assert_eq!(
        candidate_position(&pressure(2, 100), &roster, |_| false, |_| 0),
        None
    );
    assert_eq!(
        candidate_position(&pressure(2, 100), &[], |_| true, |_| 0),
        None
    );
}

#[test]
fn a_victim_covering_more_split_fragments_relieves_first() {
    // The failed leader is absent, so the choice falls to relief: register 8
    // blocks two member fragments while earlier-listed register 3 covers one.
    let roster = [
        (2, VirtualRegisterId(3)),
        (2, VirtualRegisterId(8)),
        (2, VirtualRegisterId(5)),
    ];
    let relief = |(_, register): &(usize, VirtualRegisterId)| match *register {
        VirtualRegisterId(3) => 1,
        VirtualRegisterId(8) => 2,
        _ => 1,
    };
    assert_eq!(
        candidate_position(&pressure(2, 9), &roster, |_| true, relief),
        Some(1)
    );
}

#[test]
fn equal_relief_keeps_roster_order() {
    let roster = [
        (2, VirtualRegisterId(3)),
        (2, VirtualRegisterId(8)),
        (2, VirtualRegisterId(5)),
    ];
    assert_eq!(
        candidate_position(
            &pressure(2, 9),
            &roster,
            |(_, register)| *register != VirtualRegisterId(3),
            |_| 0
        ),
        Some(1)
    );
    assert_eq!(
        candidate_position(&pressure(2, 9), &roster, |_| true, |_| 7),
        Some(0)
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

#[test]
fn relief_counts_the_member_fragments_a_victim_covers() {
    // Domain {0, 1} split at a tie: victim 8 interferes with both fragments,
    // victim 7 touches only the sibling, and victim 5 covers nothing.
    let ranges = split_ranges(&[(0, 1)], &[], &[(0, 7), (0, 8), (1, 8)]);
    assert_eq!(
        member_relief(&ranges, VirtualRegisterId(0), VirtualRegisterId(8)),
        2
    );
    assert_eq!(
        member_relief(&ranges, VirtualRegisterId(0), VirtualRegisterId(7)),
        1
    );
    assert_eq!(
        member_relief(&ranges, VirtualRegisterId(0), VirtualRegisterId(5)),
        0
    );
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

#[test]
fn an_untied_domain_scores_one_for_the_leader_and_its_interferers() {
    let ranges = split_ranges(&[], &[], &[(0, 7)]);
    assert_eq!(
        member_relief(&ranges, VirtualRegisterId(0), VirtualRegisterId(0)),
        1
    );
    assert_eq!(
        member_relief(&ranges, VirtualRegisterId(0), VirtualRegisterId(7)),
        1
    );
    assert_eq!(
        member_relief(&ranges, VirtualRegisterId(0), VirtualRegisterId(4)),
        0
    );
}
