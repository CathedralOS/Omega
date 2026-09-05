//! Attached Unit closure and transfer regression families.

use super::*;

#[test]
fn attached_unit_hard_root_lowers_exact_checked_closure_with_dense_identities() {
    let checked = hard_root_checked_fixture();
    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("complete attached Unit closure should lower");
    let module = &lowered.semantic_module;

    assert_eq!(module.entry, machine_id(1));
    assert_eq!(module.structural_types.len(), 3);
    assert_eq!(
        module
            .structural_types
            .iter()
            .map(|declaration| declaration.id.get())
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(module.structural_domains[0].id, structural_domain_id(1));
    let acknowledgement = module
        .structural_types
        .iter()
        .find(|declaration| declaration.identity == "example::Acknowledgement")
        .expect("acknowledgement structural type");
    let StructuralTypeShape::Record { fields } = &acknowledgement.shape else {
        panic!("acknowledgement is a record")
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[1].relevance, terminal_psi::BindingRelevance::Erased);
    assert!(matches!(
        &fields[1].field_type,
        StructuralFieldType::Erased { type_identity }
            if type_identity == "named(name(example::Evidence))"
    ));
    assert_eq!(module.services[0].id, service_id(1));
    assert_eq!(module.services[0].identity, "PortIo");
    assert_eq!(module.boundary_machines[0].id, boundary_machine_id(1));
    assert_eq!(module.boundary_machines[0].requires.len(), 1);
    assert_eq!(module.machines.len(), 2);
    assert_eq!(module.machines[0].id, machine_id(1));
    assert_eq!(module.machines[1].id, machine_id(2));
    assert_eq!(module.machines[0].structural_parameters[0].position, 0);
    assert_eq!(module.machines[1].structural_parameters[0].position, 0);
    assert_eq!(module.machines[0].entry_claims[0].claim, claim_id(1));
    assert_eq!(module.machines[1].entry_claims[0].claim, claim_id(1));

    let [root_call] = module.machines[0].blocks[0].operations.as_slice() else {
        panic!("root emits one call before its Unit return")
    };
    let OperationKind::CallUnit {
        callee,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        ..
    } = &root_call.kind
    else {
        panic!("root operation should be CallUnit")
    };
    assert_eq!(*callee, machine_id(2));
    assert_eq!(structural_arguments[0].place, place_id(2));
    assert_eq!(claim_transfers[0].claim, claim_id(1));
    assert!(requirement_obligations.is_empty());

    let [port, settlement] = module.machines[1].blocks[0].operations.as_slice() else {
        panic!("helper emits port output and boundary settlement")
    };
    assert!(matches!(
        port.kind,
        OperationKind::PortWrite {
            service,
            port: 0x3f8,
            value: 0x5a,
        } if service == service_id(1)
    ));
    let OperationKind::BoundaryCall {
        boundary,
        structural_arguments,
        completion_receipts,
        ..
    } = &settlement.kind
    else {
        panic!("helper settlement should be BoundaryCall")
    };
    assert_eq!(*boundary, boundary_machine_id(1));
    assert_eq!(structural_arguments[0].place, place_id(3));
    assert_eq!(completion_receipts[0].claim, claim_id(1));
    assert!(matches!(
        module.machines[0].blocks[0].terminator,
        Terminator::ReturnUnit { edge, .. } if edge == edge_id(1)
    ));
    assert!(matches!(
        module.machines[1].blocks[0].terminator,
        Terminator::ReturnUnit { edge, .. } if edge == edge_id(2)
    ));
    assert!(lowered.proof_bundle.evidence.is_empty());
    assert_eq!(
        lower_machine(&checked, "example::Root::enter")
            .expect("repeat lowering")
            .semantic_module,
        *module,
        "canonical identities must be deterministic"
    );
}

#[test]
fn attached_unit_record_field_custody_crosses_call_and_boundary_settlement() {
    let mut checked = hard_root_checked_fixture();
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
        .find(|shape| shape.identity == "example::Acknowledgement")
        .expect("acknowledgement shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &mut acknowledgement.shape else {
        panic!("acknowledgement is a record")
    };
    fields[0].identity = "#7".to_owned();
    fields[0].field_type = CheckedUnitStructuralFieldType::Structural {
        type_identity: "example::Token".to_owned(),
    };
    for machine in &mut plans.machines {
        machine.entry_claims[0].path =
            vec![CheckedUnitStructuralPathSegment::Field("#7".to_owned())];
    }

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("record-field custody should cross the complete Unit closure");
    assert_eq!(
        lowered.semantic_module.machines[0].entry_claims[0].path,
        [StructuralPathSegment::Field("#7".to_owned())]
    );
    assert_eq!(
        lowered.semantic_module.machines[1].entry_claims[0].path,
        [StructuralPathSegment::Field("#7".to_owned())]
    );
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("aggregate custody must have a canonical terminal encoding");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("canonical aggregate custody bytes"),
        lowered.semantic_module
    );
}

#[test]
fn attached_unit_nested_record_claim_lowers_through_complete_closure() {
    let mut checked = hard_root_checked_fixture();
    let plans = &mut checked.facts.flow.terminal_unit_effects;
    plans.structural_types.extend([
        checked_trees::CheckedUnitStructuralTypePlan {
            identity: "example::Pocket".to_owned(),
            shape: CheckedUnitStructuralTypeShape::Record {
                fields: vec![checked_trees::CheckedUnitStructuralFieldPlan {
                    identity: "#9".to_owned(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: CheckedUnitStructuralFieldType::Structural {
                        type_identity: "example::Token".to_owned(),
                    },
                }],
            },
        },
        checked_trees::CheckedUnitStructuralTypePlan {
            identity: "example::Token".to_owned(),
            shape: CheckedUnitStructuralTypeShape::Record { fields: Vec::new() },
        },
    ]);
    let acknowledgement = plans
        .structural_types
        .iter_mut()
        .find(|shape| shape.identity == "example::Acknowledgement")
        .expect("acknowledgement shape");
    let CheckedUnitStructuralTypeShape::Record { fields } = &mut acknowledgement.shape else {
        panic!("acknowledgement is a record")
    };
    fields[0].identity = "#7".to_owned();
    fields[0].field_type = CheckedUnitStructuralFieldType::Structural {
        type_identity: "example::Pocket".to_owned(),
    };
    for boundary in &mut plans.boundary_machines {
        boundary.structural_parameters[0].multiplicity = Multiplicity::Affine;
    }
    for machine in &mut plans.machines {
        machine.structural_parameters[0].multiplicity = Multiplicity::Affine;
        machine.entry_claims[0].path = vec![
            CheckedUnitStructuralPathSegment::Field("#7".to_owned()),
            CheckedUnitStructuralPathSegment::Field("#9".to_owned()),
        ];
    }

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("nested record custody should cross the complete Unit closure");
    for machine in &lowered.semantic_module.machines {
        assert_eq!(
            machine.structural_parameters[0].multiplicity,
            StructuralMultiplicity::Affine
        );
        assert_eq!(machine.entry_claims.len(), 1);
        assert_eq!(
            machine.entry_claims[0].path,
            [
                StructuralPathSegment::Field("#7".to_owned()),
                StructuralPathSegment::Field("#9".to_owned()),
            ]
        );
    }
    let acknowledgement = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|shape| shape.identity == "example::Acknowledgement")
        .expect("lowered acknowledgement shape");
    let StructuralTypeShape::Record { fields } = &acknowledgement.shape else {
        panic!("acknowledgement is a record")
    };
    assert!(matches!(
        &fields[0].field_type,
        StructuralFieldType::Structural(structural_type)
            if lowered.semantic_module.structural_types.iter().any(|shape| {
                shape.id == *structural_type && shape.identity == "example::Pocket"
            })
    ));
}

#[test]
fn attached_unit_disjoint_sibling_claims_lower_as_one_aggregate_transfer() {
    let mut checked = hard_root_checked_fixture();
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
        .find(|shape| shape.identity == "example::Acknowledgement")
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
        sibling.claim_identity = unit_claim_at(machine.machine, machine.state, 1);
        sibling.path = vec![CheckedUnitStructuralPathSegment::Field("#9".to_owned())];
        machine.entry_claims.push(sibling);
    }
    let root = plans.machines[0].machine;
    let root_state = plans.machines[0].state;
    let CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = &mut plans.machines[0].operations[0]
    else {
        unreachable!()
    };
    claim_transfers.push(checked_trees::CheckedUnitClaimTransferPlan {
        claim_identity: unit_claim_at(root, root_state, 1),
        argument_index: 0,
    });
    let helper = plans.machines[1].machine;
    let helper_state = plans.machines[1].state;
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        completion_receipts,
        ..
    } = &mut plans.machines[1].operations[1]
    else {
        unreachable!()
    };
    completion_receipts.push(checked_trees::CheckedUnitClaimTransferPlan {
        claim_identity: unit_claim_at(helper, helper_state, 1),
        argument_index: 0,
    });

    let lowered = lower_machine(&checked, "example::Root::enter")
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
fn attached_unit_affine_argument_lowers_as_an_owned_transfer_without_a_claim_row() {
    let mut checked = hard_root_checked_fixture();
    let plans = &mut checked.facts.flow.terminal_unit_effects.machines;
    for plan in plans.iter_mut() {
        plan.structural_parameters[0].multiplicity = Multiplicity::Affine;
        plan.entry_claims.clear();
    }
    let CheckedUnitEffectOperationPlan::CallUnit {
        claim_transfers, ..
    } = &mut plans[0].operations[0]
    else {
        unreachable!()
    };
    claim_transfers.clear();
    plans[1].operations.retain(|operation| {
        !matches!(
            operation,
            CheckedUnitEffectOperationPlan::BoundaryCall { .. }
        )
    });
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        trivial_affine_discards,
        ..
    } = plans[1].operations.last_mut().unwrap()
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![0];

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("the checked affine Unit transfer should lower and verify");
    assert_eq!(
        lowered.semantic_module.machines[0].structural_parameters[0].multiplicity,
        StructuralMultiplicity::Affine
    );
    let OperationKind::CallUnit {
        claim_transfers, ..
    } = &lowered.semantic_module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    assert!(claim_transfers.is_empty());
}

#[test]
fn attached_unit_affine_return_lowers_exact_no_code_discard() {
    let mut checked = hard_root_checked_fixture();
    let root = &mut checked.facts.flow.terminal_unit_effects.machines[0];
    root.structural_parameters[0].multiplicity = Multiplicity::Affine;
    root.entry_claims.clear();
    root.operations = vec![CheckedUnitEffectOperationPlan::ReturnUnit {
        statement_index: 0,
        trivial_affine_local_discard_ordinals: Vec::new(),
        trivial_affine_discards: vec![0],
    }];
    let root_reach_span = checked.facts.service_reaches.root_machines;
    checked
        .facts
        .service_reaches
        .machines
        .span_mut_or_empty(root_reach_span)[0]
        .concrete_effective = language_semantics::ServiceReachRowTable::EMPTY_ROW;

    let lowered = lower_machine(&checked, "example::Root::enter")
        .expect("checked affine discard should lower as explicit return-edge cleanup");
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("the no-call closure should contain only its root")
    };
    let [block] = machine.blocks.as_slice() else {
        panic!("the no-call root should contain one block")
    };
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &block.terminator
    else {
        panic!("affine cleanup should remain attached to the Unit return")
    };
    assert_eq!(
        trivial_affine_discards,
        &[machine.structural_parameters[0].place]
    );
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("affine discard must have a canonical terminal encoding");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("canonical affine discard bytes"),
        lowered.semantic_module
    );
}

#[test]
fn attached_unit_hard_root_fails_closed_on_missing_transitive_member() {
    let mut checked = hard_root_checked_fixture();
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .retain(|machine| machine.contract_report_fingerprint != 0x202);

    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "attached Unit closure is missing a checked transitive machine plan"
        ))
    ));
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
        lower_machine(&checked, "example::Root::enter"),
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
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "Unit boundary target contract compatibility coordinate or strong commitment drifted",
        )),
    );
}

#[test]
fn attached_unit_port_write_requires_exact_direct_checked_port_service() {
    let mut checked = hard_root_checked_fixture();
    let empty = language_semantics::ServiceReachRowTable::EMPTY_ROW;
    let helper = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|machine| machine.contract_report_fingerprint == 0x202)
        .expect("helper plan");
    let CheckedUnitEffectOperationPlan::PortWrite { service_reach, .. } = &mut helper.operations[0]
    else {
        panic!("fixture begins helper with port output")
    };
    service_reach.direct = empty;

    assert!(matches!(
        lower_machine(&checked, "example::Root::enter"),
        Err(LoweringError::Unsupported(
            "port output does not carry the unique exact checked PortIo service"
        ))
    ));
}

#[test]
fn attached_unit_borrowed_self_roots_an_ordinary_field_argument_beside_provider_places() {
    // A borrowed attachment stays ambient while only provider-specialized
    // fields are addressed. Once an ordinary data field is a call argument,
    // the borrowed `self` is retained as structural parameter 0 so the
    // argument has a place to project from; the provider roots stay.
    let checked = checked_source(
        r#"
        domain [u8; 16]::Utf8
        requires
            valid_utf8(self);

        boundary trait Console {
            machine write_line(text: &[u8])
            reaches Console;
            machine read_line(out_line: &mut [u8])
            reaches Console;
        }

        data Main { console: Console; pause: [u8; 16] in Utf8; }
        machine Main::main(&mut self)
        reaches Console
        {
            self.console.write_line("ready");
            self.console.read_line(&mut self.pause);
        }
        "#,
    );
    let selection = machine_dispatch::select_terminal_machine(&checked, "Main::main")
        .expect("Main::main is the unique terminal selection");
    let lowered = machine_dispatch::lower_selected_machine(&checked, selection)
        .expect("borrowed self with an ordinary field argument should lower")
        .terminal;
    let [machine] = lowered.semantic_module.machines.as_slice() else {
        panic!("the closure contains only its root")
    };
    let attachment = machine.attachment.expect("Main stays the attachment");
    let [receiver] = machine.structural_parameters.as_slice() else {
        panic!("the borrowed self is the only structural parameter")
    };
    assert!(receiver.is_self);
    assert_eq!(receiver.position, 0);
    assert_eq!(receiver.structural_type, attachment);
    assert_eq!(
        receiver.access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    assert!(machine.structural_places.iter().any(|place| {
        place.id == receiver.place
            && place.kind
                == StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: true,
                }
    }));
    assert_eq!(
        machine
            .structural_places
            .iter()
            .filter(|place| matches!(
                place.kind,
                StructuralPlaceKind::ProviderAttachment { attachment: root, .. }
                    if root == attachment
            ))
            .count(),
        2,
        "both provider requirements keep their specialization roots"
    );
    let structural_arguments = machine.blocks[0]
        .operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::BoundaryCall {
                structural_arguments,
                ..
            } => Some(structural_arguments),
            _ => None,
        })
        .next_back()
        .expect("read_line stays the last boundary call");
    assert!(matches!(
        structural_arguments.as_slice(),
        [argument]
            if argument.place == receiver.place
                && argument.access == terminal_psi::StructuralAccess::MutableBorrow
                && matches!(
                    argument.path.as_slice(),
                    [StructuralPathSegment::Field(field)] if field == "pause"
                )
    ));
    // The projected byte carrier is not yet admissible to the Terminal
    // verifier: `resolve_structural_path` walks only `Structural` fields and
    // `pause` is a `ByteSequence` field. An owned `self` stops at the same
    // site. Pinned so the flip is noticed when the verifier admits it.
    assert!(matches!(
        lower_machine(&checked, "Main::main"),
        Err(LoweringError::InvalidTerminalModule(
            terminal_verifier::ModuleError::InvalidStructuralArgumentPath {
                argument_index: 0,
                ..
            }
        ))
    ));
}
