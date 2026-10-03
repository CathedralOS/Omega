//! Source-free structural completion retains its payload across ordered hooks
//! and each fuel boundary, using the same canonical artifact admission as calls.
use super::{edge_id, ordered_empty_nominal_affine_module, place_id, structural_type_id};
use proof_admission::AdmissionProfile;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter};
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalStructuralInputs, TerminalStructuralResult, TerminalStructuralValue,
};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachineResult, Terminator,
};
use terminal_verifier::ProofBundle;

#[test]
fn structural_return_keeps_result_across_reverse_order_hooks_and_fuel_resumption() {
    let mut module = ordered_empty_nominal_affine_module(false);
    let result_type = structural_type_id(3);
    module.structural_types.push(StructuralTypeDeclaration {
        id: result_type,
        identity: "Result".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let caller = &mut module.machines[0];
    caller
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(3),
            position: 2,
            is_self: false,
            structural_type: result_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(3),
        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
            position: 2,
            is_self: false,
        },
    });
    caller.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(4),
        kind: semantic_vocabulary::StructuralPlaceKind::Result,
    });
    caller.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: place_id(4),
        structural_type: result_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    let Terminator::ReturnUnitNominalAffine { edge, cleanups } =
        caller.blocks[0].terminator.clone()
    else {
        panic!("ordered fixture has nominal cleanup");
    };
    caller.blocks[0].terminator = Terminator::ReturnStructural {
        edge,
        source: place_id(3),
        returned_claims: Vec::new(),
        cleanup_actions: cleanups
            .into_iter()
            .map(TerminalAffineCleanupAction::InvokeNominal)
            .collect(),
    };
    let arguments = [1, 2, 3].map(|index| TerminalStructuralValue {
        opaque_identity: 70 + index,
        structural_type: structural_type_id(index),
        qualifications: Vec::new(),
        path: Vec::new(),
    });
    let mut execution = TerminalExecution::start_artifact(
        &encode_module(&module).expect("structural cleanup encodes"),
        &encode_proof_section(&module, &ProofBundle::default()).expect("proof section encodes"),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
    )
    .expect("structural cleanup verifies without source");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for expected in [edge_id(1), edge_id(3), edge_id(2)] {
        assert!(
            matches!(execution.resume(&mut meter, &mut AcceptTerminalEffects).unwrap(), TerminalExecutionStatus::SponsorExhausted(FuelExhaustion { site: FuelChargeSite::Edge(edge), .. }) if edge == expected)
        );
        if expected == edge_id(1) {
            assert_eq!(
                execution.live_affine_frontier().count(),
                2,
                "unpaid return preserves both owned cleanup roots"
            );
        }
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
            TerminalStructuralResult {
                value: arguments[2].clone(),
                claims: Vec::new()
            }
        ))
    );
}
