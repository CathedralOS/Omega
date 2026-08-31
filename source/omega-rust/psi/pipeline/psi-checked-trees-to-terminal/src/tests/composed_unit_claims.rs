//! Claim-bearing composed Unit control and independent corruption replay.

use super::*;

fn checked_composed_claim() -> psi_checked_trees::CheckedTrees {
    checked_source(
        r#"
            pub data Receipt [linear] { value: u64; }
            boundary machine Receipt::settle(self) ensures true;

            data Root {}
            machine Root::enter(flag: bool, receipt: Receipt) {
                transition flag {
                    true -> yes(receipt)
                    _ -> no(receipt)
                }
                state yes(receipt: Receipt) { receipt.settle(); }
                state no(receipt: Receipt) { receipt.settle(); }
            }
        "#,
    )
}

#[test]
fn lowers_one_whole_root_linear_claim_through_both_boundary_leaves() {
    let checked = checked_composed_claim();
    let lowered = lower_machine(&checked, "Root::enter")
        .expect("the exclusive branches should lower one shared linear claim");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("linear composed route emits one machine")
    };
    let [parameter] = machine.structural_parameters.as_slice() else {
        panic!("composed machine retains one structural parameter")
    };
    assert_eq!(parameter.multiplicity, StructuralMultiplicity::Linear);
    assert_eq!(parameter.access, StructuralAccess::Owned);
    let [entry_claim] = machine.entry_claims.as_slice() else {
        panic!("composed machine retains one entry claim")
    };
    assert_eq!(entry_claim.input, parameter.place);
    assert!(entry_claim.path.is_empty());
    assert!(matches!(
        machine.structural_places.as_slice(),
        [StructuralPlaceDeclaration {
            id,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }] if *id == parameter.place
    ));
    assert_eq!(machine.blocks.len(), 3);
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("entry block should retain the conditional")
    };
    for successor in [when_true, when_false] {
        assert!(successor.arguments.is_empty());
        assert!(successor.trivial_affine_discards.is_empty());
    }
    for leaf in &machine.blocks[1..] {
        let [operation] = leaf.operations.as_slice() else {
            panic!("each leaf emits one boundary settlement")
        };
        assert!(matches!(
            &operation.kind,
            OperationKind::BoundaryCall {
                arguments,
                structural_arguments,
                completion_receipts,
                ..
            } if arguments.is_empty()
                && matches!(structural_arguments.as_slice(), [argument]
                    if argument.place == parameter.place
                        && argument.path.is_empty()
                        && argument.access == StructuralAccess::Owned)
                && matches!(completion_receipts.as_slice(), [receipt]
                    if receipt.claim == entry_claim.claim
                        && receipt.argument_index == 0)
        ));
        assert!(matches!(
            leaf.terminator,
            Terminator::ReturnUnit {
                ref trivial_affine_discards,
                ..
            } if trivial_affine_discards.is_empty()
        ));
    }
    let [boundary] = lowered.semantic_module.boundary_machines.as_slice() else {
        panic!("both leaves share one canonical attached boundary")
    };
    assert!(boundary.attachment.is_some());
    assert_eq!(boundary.structural_parameters.len(), 1);
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("claim-bearing composed Unit module verifies");
    let bytes = psi_terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        psi_terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn claim_bearing_composed_unit_rejects_plan_and_fact_corruption() {
    let baseline = checked_composed_claim();
    let rejects = |checked: &CheckedTrees| {
        assert!(matches!(
            lower_machine(checked, "Root::enter"),
            Err(LoweringError::Unsupported(_))
        ));
    };

    let mut edge = baseline.clone();
    let plan = &mut edge.facts.flow.terminal_unit_effects.composed_machines[0];
    let psi_checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true, ..
    } = &mut plan.states[0].terminator
    else {
        unreachable!()
    };
    when_true.transfers[0].source_parameter_index = 1;
    rejects(&edge);

    let mut claim = baseline.clone();
    claim.facts.flow.terminal_unit_effects.composed_machines[0].states[1].entry_claims[0]
        .claim_identity = psi_language_semantics::PermissionClaimIdentity::Unknown;
    rejects(&claim);

    let mut receipt = baseline.clone();
    let plan = &mut receipt.facts.flow.terminal_unit_effects.composed_machines[0];
    let entry_claim = plan.states[0].entry_claims[0].claim_identity;
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &mut plan.states[1].operations[0]
    else {
        unreachable!()
    };
    completion_receipts[0].claim_identity = entry_claim;
    rejects(&receipt);

    let mut facts = baseline.clone();
    let leaf = facts.facts.flow.terminal_unit_effects.composed_machines[0].states[1].state;
    let consumption = facts
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.state_symbol == leaf
                && event.kind == psi_language_semantics::PermissionEventKind::Consume)
                .then_some(handle)
        })
        .expect("leaf consumption fact");
    facts
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(consumption)
        .kind = psi_language_semantics::PermissionEventKind::Transfer;
    rejects(&facts);

    let mut boundary = baseline;
    boundary.facts.flow.terminal_unit_effects.boundary_machines[0].attachment_type_identity = None;
    rejects(&boundary);
}
