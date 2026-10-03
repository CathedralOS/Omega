//! Private scheduling control: artifact admission is exercised by the external
//! structural_nominal_returns test; this test observes suspended frame custody.
use super::*;
use crate::AcceptTerminalEffects;
use crate::execution::ExecutableMachine;
use crate::structural_identities::LocalStructuralIdentities;
use semantic_vocabulary::{BlockId, EdgeId, MachineId};
use terminal_psi::{Block, StructuralTypeDeclaration};

fn place(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}
fn machine(value: u64) -> MachineId {
    MachineId::new(value).unwrap()
}
fn block(value: u64) -> BlockId {
    BlockId::new(value).unwrap()
}

fn execution() -> TerminalExecution {
    let structural_type = StructuralTypeId::new(1).unwrap();
    let machines = (1..=3)
        .map(|identity| {
            let block = block(identity);
            (
                machine(identity),
                ExecutableMachine {
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    structural_places: Vec::new(),
                    entry_claims: Vec::new(),
                    content_entry_claims: Vec::new(),
                    result: TerminalMachineResult::Unit,
                    entry: block,
                    blocks: BTreeMap::from([(
                        block,
                        Block {
                            id: block,
                            parameters: Vec::new(),
                            structural_parameters: Vec::new(),
                            erased_scalar_formals: Vec::new(),
                            erased_proof_formals: Vec::new(),
                            operations: Vec::new(),
                            terminator: Terminator::ReturnUnit {
                                edge: EdgeId::new(identity).unwrap(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                    )]),
                },
            )
        })
        .collect();
    TerminalExecution {
        program: std::sync::Arc::new(crate::program::AdmittedTerminalProgram::for_test(
            machines,
            BTreeMap::from([(
                structural_type,
                StructuralTypeDeclaration {
                    id: structural_type,
                    identity: "Token".into(),
                    shape: StructuralTypeShape::Record { fields: Vec::new() },
                },
            )]),
        )),
        structural_cases: BTreeMap::new(),
        dynamic_parameters: BTreeMap::new(),
        provider_installation: BTreeMap::new(),
        assumed_bindings: Vec::new(),
        values: BTreeMap::new(),
        structural_values: (1..=3)
            .map(|identity| {
                (
                    place(identity),
                    TerminalStructuralValue {
                        opaque_identity: identity + 100,
                        structural_type,
                        qualifications: Vec::new(),
                        path: Vec::new(),
                    },
                )
            })
            .collect(),
        structural_primitive_storage: BTreeMap::new(),
        reference_referents: BTreeMap::new(),
        structural_element_view_fields: BTreeMap::new(),
        structural_primitive_entry_places: BTreeMap::new(),
        local_structural_identities: LocalStructuralIdentities::with_reserved_identities([
            101, 102, 103,
        ]),
        structural_scalar_fields: BTreeMap::new(),
        structural_byte_sequence_fields: BTreeMap::new(),
        structural_byte_storage: BTreeMap::new(),
        scalar_case_values: BTreeMap::new(),
        scalar_array_values: BTreeMap::new(),
        element_view_values: BTreeMap::new(),
        byte_sequence_values: BTreeMap::new(),
        live_affine_frontier: (1..=3)
            .map(|identity| StructuralAffineDiscard {
                place: place(identity),
                path: Vec::new(),
                structural_type,
            })
            .collect(),
        live_claims: BTreeMap::new(),
        placed_view_occurrences: BTreeMap::new(),
        current_machine: machine(1),
        current: block(1),
        next_operation: 0,
        call_stack: Vec::new(),
        result: None,
        crash: None,
        effects: Vec::new(),
    }
}

#[test]
fn nominal_cleanup_suspends_before_later_disposals() {
    let mut execution = execution();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let nominal = |root, target| {
        TerminalAffineCleanupAction::InvokeNominal(NominalAffineCleanup {
            place: place(root),
            structural_type,
            cleanup_machine: machine(target),
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        })
    };
    let result = TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true));
    assert!(matches!(
        execution
            .continue_cleanup_actions(
                vec![
                    nominal(3, 3),
                    TerminalAffineCleanupAction::DiscardRoot(place(2)),
                    nominal(1, 2)
                ]
                .into_iter(),
                CompletedReturn::plain(result.clone()),
            )
            .unwrap(),
        TerminatorFlow::Continue
    ));
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for _ in 0..2 {
        assert!(matches!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        let suspended = execution.call_stack.last().unwrap();
        assert_eq!(
            suspended
                .structural_values
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![place(1), place(2)],
            "later roots remain live while the first hook is suspended"
        );
        assert_eq!(suspended.live_affine_frontier.len(), 3);
    }
    meter.replenish(1).unwrap();
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.current_machine, machine(2));
    let suspended = execution.call_stack.last().unwrap();
    assert!(suspended.structural_values.is_empty());
    assert_eq!(
        suspended
            .live_affine_frontier
            .iter()
            .map(|entry| entry.place)
            .collect::<Vec<_>>(),
        vec![place(1)],
        "the middle discard commits only after the first hook returns"
    );
    meter.replenish(1).unwrap();
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(result)
    );
    assert!(execution.call_stack.is_empty());
    assert!(execution.live_affine_frontier.is_empty());
    assert_eq!(
        meter.usage().total_units(),
        2,
        "each hook return is paid exactly once"
    );
}
