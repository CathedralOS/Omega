//! Attached Unit closure and transfer regression families.

use super::{
    LoweringError, PermissionClaimIdentity, hard_root_checked_fixture, lower_machine, unit_claim_at,
};
use crate::TerminalMachineSelection;
use crate::proofs::operation_proofs::finalize_operation_proofs;
use crate::terminal_identities::claim_id;
use checked_trees::{
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypeShape,
};
use language_semantics::Multiplicity;
use terminal_psi::{StructuralMultiplicity, StructuralPathSegment};
#[test]
fn array_call_numeric_requirements_use_completed_argument_facts() {
    let checked = crate::front_end::checked_program(
        "machine read() -> [u8; 1] { bounded(7u8 + 2u8) }
         machine bounded(value: u8 [0..=9]) -> [u8; 1] { [value + 1u8] }",
    );
    let entry = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "read")
        .unwrap()
        .symbol;
    let mut closure = crate::unit::attached_unit::lower_unit_closure(
        &checked,
        &crate::unit::attached_unit::UnitClosureRequest::unit(&checked, entry, &[entry]),
    )
    .unwrap();
    finalize_operation_proofs(&mut closure.lowered)
        .expect("callee arithmetic uses its ranged parameter requirement");
    terminal_verifier::verify_module(
        &closure.lowered.semantic_module,
        &closure.lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent replay checks both caller requirements and callee arithmetic");
}

#[test]
fn attached_unit_disjoint_sibling_claims_lower_as_one_aggregate_transfer() {
    let mut checked = hard_root_checked_fixture();
    let acknowledgement_identity = checked.facts.flow.terminal_unit_effects.machines[0]
        .structural_parameters[0]
        .type_identity
        .clone();
    let plans = &mut checked.facts.flow.terminal_unit_effects;
    plans
        .structural_types
        .push(checked_trees::CheckedUnitStructuralTypePlan {
            identity: "example::Token".to_owned(),
            shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
        });
    let acknowledgement = plans
        .structural_types
        .iter_mut()
        .find(|shape| shape.identity == acknowledgement_identity)
        .expect("acknowledgement shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &mut acknowledgement.shape else {
        panic!("acknowledgement is a record")
    };
    fields[0].identity = "#7".to_owned();
    fields[0].field_type = CheckedUnitStructuralFieldType::Structural {
        type_identity: "example::Token".to_owned(),
    };
    fields.insert(
        1,
        checked_trees::CheckedUnitStructuralFieldPlan {
            identity: "#9".to_owned(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: CheckedUnitStructuralFieldType::Structural {
                type_identity: "example::Token".to_owned(),
            },
        },
    );
    for boundary in &mut plans.boundary_machines {
        boundary.structural_parameters[0].multiplicity = Multiplicity::Affine;
    }
    for machine in &mut plans.machines {
        machine.structural_parameters[0].multiplicity = Multiplicity::Affine;
        machine.entry_claims[0].path =
            vec![CheckedUnitStructuralPathSegment::Field("#7".to_owned())];
        let mut sibling = machine.entry_claims[0].clone();
        let PermissionClaimIdentity::Established { ordinal, .. } = sibling.claim_identity else {
            panic!("source fixture establishes its entry claim")
        };
        sibling.claim_identity = unit_claim_at(machine.machine, machine.state, ordinal + 1);
        sibling.path = vec![CheckedUnitStructuralPathSegment::Field("#9".to_owned())];
        machine.entry_claims.push(sibling);
    }
    let root_sibling_claim = plans.machines[0].entry_claims[1].claim_identity;
    let CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = &mut plans.machines[0].operations[0]
    else {
        unreachable!()
    };
    claim_transfers.push(checked_trees::CheckedUnitClaimTransferPlan {
        claim_identity: root_sibling_claim,
        argument_index: 0,
    });
    let helper_sibling_claim = plans.machines[1].entry_claims[1].claim_identity;
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &mut plans.machines[1].operations[1]
    else {
        unreachable!()
    };
    completion_receipts.push(checked_trees::CheckedUnitClaimTransferPlan {
        claim_identity: helper_sibling_claim,
        argument_index: 0,
    });

    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("both sibling resources should cross the complete Unit closure");
    for machine in &lowered.semantic_module.machines {
        assert_eq!(
            machine.structural_parameters[0].multiplicity,
            StructuralMultiplicity::Affine
        );
        assert_eq!(machine.entry_claims.len(), 2);
        assert_eq!(machine.entry_claims[0].claim, claim_id(1));
        assert_eq!(
            machine.entry_claims[0].path,
            [StructuralPathSegment::Field("#7".to_owned())]
        );
        assert_eq!(machine.entry_claims[1].claim, claim_id(2));
        assert_eq!(
            machine.entry_claims[1].path,
            [StructuralPathSegment::Field("#9".to_owned())]
        );
    }
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("multi-field custody must have a canonical terminal encoding");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("canonical aggregate custody bytes"),
        lowered.semantic_module
    );
}

#[test]
fn attached_unit_boundary_rejects_missing_canonical_contract_custody() {
    let mut checked = hard_root_checked_fixture();
    let boundary = checked.facts.flow.terminal_unit_effects.boundary_machines[0].machine;
    checked
        .facts
        .contract_plans
        .machines
        .retain(|contract| contract.machine != boundary);
    checked
        .facts
        .contract_plans
        .crash_capsules
        .retain(|capsule| capsule.target_machine() != boundary);

    assert_eq!(
        lower_machine(&checked, TerminalMachineSelection::Name("Root::enter")),
        Err(LoweringError::Unsupported(
            "Unit boundary target is missing its canonical checked contract identity",
        )),
    );
}

#[test]
fn attached_unit_boundary_rejects_compact_equal_commitment_substitution() {
    let mut checked = hard_root_checked_fixture();
    let boundary = &mut checked.facts.flow.terminal_unit_effects.boundary_machines[0];
    let retained_report = boundary.contract_report_fingerprint;
    boundary.contract_commitment =
        checked_trees::MachineContractCommitment::from_digest([0x5a; 32]);
    assert_eq!(boundary.contract_report_fingerprint, retained_report);

    assert_eq!(
        lower_machine(&checked, TerminalMachineSelection::Name("Root::enter")),
        Err(LoweringError::Unsupported(
            "Unit boundary target contract compatibility coordinate or strong commitment drifted",
        )),
    );
}

#[test]
fn attached_unit_port_write_requires_exact_direct_checked_port_service() {
    let mut checked = hard_root_checked_fixture();
    let helper_symbol = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Helper::run")
        .expect("authored helper")
        .symbol;
    let empty = language_semantics::ServiceReachRowTable::EMPTY_ROW;
    let helper = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|machine| machine.machine == helper_symbol)
        .expect("helper plan");
    let CheckedUnitEffectOperationPlan::PortWrite { service_reach, .. } = &mut helper.operations[0]
    else {
        panic!("fixture begins helper with port output")
    };
    service_reach.direct = empty;

    assert!(matches!(
        lower_machine(&checked, TerminalMachineSelection::Name("Root::enter")),
        Err(LoweringError::Unsupported(
            "port output does not carry the unique exact checked PortIo service"
        ))
    ));
}
