//! Composed Unit boundary attachments, receiver custody and corruption replay.

use super::{CheckedTrees, lower_machine};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{
    OperationKind, StructuralAccess, StructuralMultiplicity, StructuralPlaceDeclaration, Terminator,
};
use typed_trees_to_checked_trees::checked_trees::CheckedUnitEffectOperationPlan;
fn checked_composed_claim() -> typed_trees_to_checked_trees::checked_trees::CheckedTrees {
    crate::front_end::checked_program(
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
fn static_boundary_attachment_composes_without_receiver_custody() {
    for after_handoff in [false, true] {
        let (prefix, suffix) = if after_handoff {
            (
                "transition { _ -> choose(input) } state choose(input: u32) {",
                "}",
            )
        } else {
            ("", "")
        };
        let source = format!(
            "pub data Math {{}} data Root {{}}
             pub boundary requirement Math::choose(value: u32) -> u32;
             machine identity(value: u32) -> u32 {{ value }}
             machine Root::enter(input: u32) {{
                 {prefix}
                 let ordinary: u32 = identity(input);
                 let selected: u32 = Math::choose(ordinary);
                 transition selected == input {{ true -> done() _ -> done() }}
                 {suffix}
                 state done() {{}}
             }}"
        );
        let checked = crate::front_end::checked_program(&source);
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
            .unwrap_or_else(|error| panic!("after_handoff={after_handoff}: {error:?}"));
        let [boundary] = lowered.semantic_module.boundary_machines.as_slice() else {
            panic!("one exact static boundary requirement");
        };
        assert!(
            boundary.attachment.is_some(),
            "nominal owner remains retained"
        );
        assert!(
            boundary.structural_parameters.is_empty(),
            "no invented self"
        );
        assert!(matches!(
            boundary.result,
            terminal_psi::BoundaryMachineResult::Scalar(_)
        ));
        let calls = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::BoundaryCall {
                    arguments,
                    structural_arguments,
                    ..
                } => Some((arguments, structural_arguments)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(arguments, structural_arguments)] = calls.as_slice() else {
            panic!("one retained boundary call");
        };
        assert_eq!(arguments.len(), 1);
        assert!(structural_arguments.is_empty());
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("static attachment and scalar result independently verify");
        let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
        assert_eq!(
            terminal_codec::decode_module(&bytes).expect("decode"),
            lowered.semantic_module
        );

        let mut invalid = lowered.semantic_module.clone();
        let arguments = invalid
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                OperationKind::BoundaryCall { arguments, .. } => Some(arguments),
                _ => None,
            })
            .expect("boundary scalar arguments");
        arguments.clear();
        assert!(
            terminal_verifier::verify_module(
                &invalid,
                &lowered.proof_bundle,
                &proof_admission::AdmissionProfile::default(),
            )
            .is_err(),
            "static attachment does not waive exact operand replay"
        );

        let mut missing_owner = checked.clone();
        missing_owner
            .facts
            .flow
            .terminal_unit_effects
            .boundary_machines[0]
            .attachment_type_identity = None;
        assert!(
            matches!(
                lower_machine(
                    &missing_owner,
                    TerminalMachineSelection::Name("Root::enter")
                ),
                Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
                    "Unit graph boundary attachment lost its authored owner"
                ))
            ),
            "a static signature still retains its authored nominal attachment"
        );
        let mut substituted_owner = checked.clone();
        let other_identity = substituted_owner
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .find_map(|plan| plan.attachment_type_identity.clone())
            .expect("Root has its own retained attachment");
        assert!(
            substituted_owner
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .iter()
                .any(|shape| shape.identity == other_identity)
        );
        substituted_owner
            .facts
            .flow
            .terminal_unit_effects
            .boundary_machines[0]
            .attachment_type_identity = Some(other_identity);
        assert!(
            matches!(
                lower_machine(
                    &substituted_owner,
                    TerminalMachineSelection::Name("Root::enter")
                ),
                Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
                    "Unit graph boundary attachment disagrees with its authored nominal identity"
                ))
            ),
            "another existing structural type cannot replace the boundary owner"
        );
    }
}

#[test]
fn composed_boundary_receiver_keeps_linear_owned_custody() {
    let baseline = checked_composed_claim();
    for corruption in ["attachment", "multiplicity", "access", "receiver"] {
        let mut invalid = baseline.clone();
        let boundary = &mut invalid.facts.flow.terminal_unit_effects.boundary_machines[0];
        match corruption {
            "attachment" => boundary.attachment_type_identity = None,
            "multiplicity" => {
                boundary.structural_parameters[0].multiplicity =
                    language_semantics::Multiplicity::Affine
            }
            "access" => boundary.structural_parameters[0].access =
                typed_trees_to_checked_trees::checked_trees::CheckedStructuralAccess::SharedBorrow,
            _ => boundary.structural_parameters[0].is_self = false,
        }
        let result = lower_machine(&invalid, TerminalMachineSelection::Name("Root::enter"));
        if corruption == "receiver" {
            assert!(
                matches!(
                    result,
                    Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
                        "Unit graph boundary receiver disagrees with its authored signature"
                    ))
                ),
                "receiver erasure must fail the independent signature rejoin: {result:?}"
            );
        } else {
            assert!(
                result.is_err(),
                "receiver custody corruption must reject: {corruption}"
            );
        }
    }
}

#[test]
fn linear_settlement_composes_after_a_state_handoff() {
    let checked = crate::front_end::checked_program(
        r#"
            pub data Receipt [linear] { value: u64; }
            boundary machine Receipt::settle(self) ensures true;

            machine enter(flag: bool, first: Receipt, second: Receipt) {
                transition { _ -> choose(flag, first, second) }
                state choose(flag: bool, first: Receipt, second: Receipt) {
                    transition flag {
                        true -> yes(first, second)
                        _ -> no(first, second)
                    }
                }
                state yes(first: Receipt, second: Receipt) {
                    first.settle();
                    second.settle();
                }
                state no(first: Receipt, second: Receipt) {
                    second.settle();
                    first.settle();
                }
            }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("enter"))
        .expect("linear settlement must not depend on the number of preceding states");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("shared graph preserves linear settlement");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn lowers_one_whole_root_linear_claim_through_both_boundary_leaves() {
    let checked = checked_composed_claim();
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
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
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("claim-bearing composed Unit module verifies");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("decode"),
        lowered.semantic_module
    );
}

#[test]
fn claim_bearing_composed_unit_rejects_plan_and_fact_corruption() {
    let baseline = checked_composed_claim();
    let rejects = |checked: &CheckedTrees, corruption: &str| {
        assert!(
            matches!(
                lower_machine(checked, TerminalMachineSelection::Name("Root::enter")),
                Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(_))
            ),
            "{corruption}"
        );
    };

    let mut edge = baseline.clone();
    let plan = &mut edge.facts.flow.terminal_unit_effects.composed_machines[0];
    let typed_trees_to_checked_trees::checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
        &mut plan.states[0].terminator
    else {
        unreachable!()
    };
    when_true.transfers[0].source =
        typed_trees_to_checked_trees::checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter { index: 1 };
    rejects(&edge, "edge parameter");

    let mut claim = baseline.clone();
    claim.facts.flow.terminal_unit_effects.composed_machines[0].states[1].entry_claims[0]
        .claim_identity = language_semantics::PermissionClaimIdentity::Unknown;
    rejects(&claim, "entry claim");

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
    rejects(&receipt, "receipt identity");

    let mut duplicate = baseline.clone();
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &mut duplicate.facts.flow.terminal_unit_effects.composed_machines[0].states[1].operations
        [0]
    else {
        unreachable!()
    };
    completion_receipts.push(completion_receipts[0].clone());
    rejects(&duplicate, "duplicate receipt");

    let mut omitted = baseline.clone();
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &mut omitted.facts.flow.terminal_unit_effects.composed_machines[0].states[1].operations[0]
    else {
        unreachable!()
    };
    completion_receipts.clear();
    rejects(&omitted, "missing receipt");

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
                && event.kind == language_semantics::PermissionEventKind::Consume)
                .then_some(handle)
        })
        .expect("leaf consumption fact");
    facts
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(consumption)
        .kind = language_semantics::PermissionEventKind::Transfer;
    rejects(&facts, "consumption fact");

    let mut boundary = baseline;
    boundary.facts.flow.terminal_unit_effects.boundary_machines[0].attachment_type_identity = None;
    rejects(&boundary, "boundary attachment");
}
