use super::checked;
use checked_trees::{CheckedComposedUnitControlMachinePlan, CheckedUnitEffectOperationPlan};
use semantic_vocabulary::{IntegerValue, StructuralPlaceKind};
use terminal_interpreter::{
    TerminalEffect, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalScalarValue, TerminalStructuralValue,
};
use terminal_psi::{StructuralAccess, StructuralFieldType, StructuralTypeShape};

const SOURCE: &str = r#"
    boundary trait Output {
        machine write(value: u64) reaches Output;
        machine finish(value: u64) reaches Output;
        machine helper(value: u64) reaches Output;
    }
    data Counter { counter: u64; output: Output; }
    machine Counter::run(&mut self) reaches Output {
        self.counter = 0;
        self.output.write(self.counter);
        self.counter = 1;
        self.output.write(self.counter);
        transition { _ -> work() }
        state work(&mut self) {
            transition self.counter < 2 {
                true -> step()
                _ -> done()
            }
        }
        state step(&mut self) {
            self.record();
            self.counter = 2;
            self.output.write(self.counter);
            transition { _ -> work() }
        }
        state done(&mut self) {
            self.counter = 3;
            self.output.finish(self.counter);
        }
    }
    machine Counter::record(&mut self) reaches Output {
        self.output.helper(9);
    }
"#;

#[test]
fn cyclic_provider_fields_reload_with_exact_roots_and_ordered_stores() {
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked(SOURCE), "Counter::run")
        .expect("general cyclic receiver graph retains provider-field calls");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let module = terminal_codec::decode_module(&semantic).unwrap();
    assert_eq!(module, lowered.semantic_module);
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert!(entry.blocks.len() >= 4);
    assert!(
        entry.ranked_scc.is_none(),
        "no invented termination witness"
    );
    let [receiver] = entry.structural_parameters.as_slice() else {
        panic!("one persistent mutable receiver");
    };
    assert!(receiver.is_self);
    assert_eq!(receiver.access, StructuralAccess::MutableBorrow);
    assert_eq!(entry.attachment, Some(receiver.structural_type));
    let attachment = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == receiver.structural_type)
        .unwrap();
    let StructuralTypeShape::Record { fields } = &attachment.shape else {
        panic!("counter attachment is a record");
    };
    assert_eq!(fields.len(), 2);
    let output = fields
        .iter()
        .find(|field| field.identity == "output")
        .unwrap();
    assert!(
        matches!(&output.field_type, StructuralFieldType::Erased { type_identity } if type_identity == "named(name(Output))")
    );
    let boundary = |name: &str| {
        module
            .boundary_machines
            .iter()
            .find(|boundary| boundary.identity.contains(name))
            .unwrap()
            .id
    };
    let mut expected = vec![boundary("Output::write"), boundary("Output::finish")];
    expected.sort();
    let roots = entry
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary,
            } => {
                assert_eq!(attachment, receiver.structural_type);
                assert_eq!(
                    field, output.id,
                    "scalar stores never require a provider root"
                );
                Some(boundary)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        roots, expected,
        "only the graph's direct requirements belong here"
    );
    let helper_roots = module
        .machines
        .iter()
        .filter(|machine| machine.id != module.entry)
        .flat_map(|machine| &machine.structural_places)
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ProviderAttachment { boundary, .. } => Some(boundary),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(helper_roots, [boundary("Output::helper")]);
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        &[],
        &[TerminalStructuralValue {
            opaque_identity: 17,
            structural_type: receiver.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
    )
    .expect("canonical graph independently verifies and reloads");
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(1000);
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    let effects = execution
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall {
                boundary,
                arguments,
                structural_arguments,
                ..
            } = effect
            else {
                panic!("only Output calls");
            };
            assert!(
                structural_arguments.is_empty(),
                "provider roots are never runtime arguments"
            );
            let [
                TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(value),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one unsigned counter");
            };
            (*boundary, *value)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        effects,
        [
            (boundary("Output::write"), 0),
            (boundary("Output::write"), 1),
            (boundary("Output::helper"), 9),
            (boundary("Output::write"), 2),
            (boundary("Output::finish"), 3)
        ],
        "stores and ordinary calls retain their source order across the backedge"
    );
}

fn graph_plan_mut(
    checked: &mut checked_trees::CheckedTrees,
) -> &mut CheckedComposedUnitControlMachinePlan {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Counter::run")
        .unwrap()
        .symbol;
    checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .find(|plan| plan.machine == machine)
        .expect("general graph has a checked composed plan")
}

fn baseline() -> checked_trees::CheckedTrees {
    let checked = checked(SOURCE);
    checked_trees_to_lowered_psi::lower_machine(&checked, "Counter::run")
        .expect("untampered provider graph must lower before negative controls");
    checked
}

fn reject_authored_provider_receiver_mutation(mutation: &str) {
    let original = baseline();
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Counter::run")
        .unwrap();
    let state = &original.machine_states(machine)[0];
    let owner = original
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == machine.attached_data_symbol)
        .unwrap();
    let counter = original
        .data_members(owner)
        .iter()
        .find_map(|member| {
            let checked_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.name.as_str() == "counter").then_some(field.symbol)
        })
        .unwrap();
    let mut changed = original.clone();
    let mut scalar_path = arena::HandleSpan::empty();
    if mutation == "scalar path" || mutation == "scalar path and field" {
        for member in ["self", "counter"] {
            changed.typed.statement_table.push_name_path_member(
                &mut scalar_path,
                checked_trees::name::Identifier::generated(member),
            );
        }
    }
    let checked_trees::statement::StatementNode::Call(call) = &mut changed
        .typed
        .statement_table
        .statements_mut(state.statement_nodes)[1]
    else {
        panic!("provider invocation is an authored statement call");
    };
    let target = call.target_symbol;
    let arguments = call.arguments;
    assert!(call.receiver_root_symbol.is_valid());
    assert!(call.receiver_symbol.is_valid());
    assert_ne!(call.receiver_symbol, counter);
    match mutation {
        "scalar field" => call.receiver_symbol = counter,
        "stale field" => call.receiver_symbol = symbols::SymbolHandle::invalid(),
        "stale root" => call.receiver_root_symbol = symbols::SymbolHandle::invalid(),
        "scalar path" => call.receiver = scalar_path,
        "scalar path and field" => {
            call.receiver = scalar_path;
            call.receiver_symbol = counter;
        }
        _ => unreachable!(),
    }
    assert_eq!(call.target_symbol, target);
    assert_eq!(call.arguments, arguments);
    assert_eq!(
        changed.facts, original.facts,
        "only authored receiver custody changes; all captured flow and plans remain untouched"
    );
    checked_trees_to_lowered_psi::lower_machine(&changed, "Counter::run").expect_err(mutation);
}

#[test]
fn authored_provider_receiver_rejects_scalar_field_with_unchanged_plan() {
    reject_authored_provider_receiver_mutation("scalar field");
}

#[test]
fn authored_provider_receiver_rejects_stale_field_with_unchanged_plan() {
    reject_authored_provider_receiver_mutation("stale field");
}

#[test]
fn authored_provider_receiver_rejects_stale_root_with_unchanged_plan() {
    reject_authored_provider_receiver_mutation("stale root");
}

#[test]
fn authored_provider_receiver_rejects_scalar_path_with_unchanged_symbols() {
    reject_authored_provider_receiver_mutation("scalar path");
}

#[test]
fn authored_provider_receiver_rejects_scalar_path_and_field_with_unchanged_plan() {
    reject_authored_provider_receiver_mutation("scalar path and field");
}

#[test]
fn checked_attachment_requirements_reject_omitted_extra_stale_and_wrong_fields() {
    let baseline = baseline();
    let helper = baseline
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Counter::record")
        .unwrap()
        .symbol;
    let helper_requirements = &baseline
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(helper)
        .unwrap()
        .provider_attachment_requirements;
    assert_eq!(helper_requirements.len(), 1);
    let helper_requirement = helper_requirements[0].clone();
    for mutation in [
        "omitted",
        "duplicate",
        "reordered",
        "stale boundary",
        "scalar field",
        "unknown field",
        "wrong carrier",
        "callee requirement",
        "missing attachment",
    ] {
        let mut changed = baseline.clone();
        let plan = graph_plan_mut(&mut changed);
        assert_eq!(plan.provider_attachment_requirements.len(), 2);
        let requirements = &mut plan.provider_attachment_requirements;
        match mutation {
            "omitted" => {
                requirements.pop();
            }
            "duplicate" => requirements.push(requirements[0].clone()),
            "reordered" => requirements.swap(0, 1),
            "stale boundary" => requirements[0].boundary = symbols::SymbolHandle::invalid(),
            "scalar field" => requirements[0].field_identity = "counter".into(),
            "unknown field" => requirements[0].field_identity = "absent".into(),
            "wrong carrier" => {
                requirements[0].provider_type_identity = "named(name(Counter))".into()
            }
            "callee requirement" => {
                requirements.push(helper_requirement.clone());
                requirements.sort_by_key(|requirement| {
                    (
                        requirement.boundary.arena_index(),
                        requirement.boundary.generation(),
                    )
                });
            }
            "missing attachment" => plan.attachment_type_identity = None,
            _ => unreachable!(),
        }
        let error = checked_trees_to_lowered_psi::lower_machine(&changed, "Counter::run")
            .expect_err(mutation);
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::Unsupported(_)
            ),
            "{mutation}: {error:?}"
        );
    }
}

#[test]
fn checked_graph_replays_boundary_call_identity_and_effect_coordinates() {
    let baseline = baseline();
    for mutation in [
        "omitted call",
        "duplicate call",
        "reordered store",
        "wrong coordinate",
        "same signature boundary",
    ] {
        let mut changed = baseline.clone();
        let plan = graph_plan_mut(&mut changed);
        let finish = plan
            .states
            .iter()
            .flat_map(|state| &state.operations)
            .rfind(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::BoundaryCall { .. }
                )
            })
            .unwrap()
            .clone();
        let entry = &mut plan.states[0];
        assert!(matches!(
            entry.operations[0],
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
        ));
        assert!(matches!(
            entry.operations[1],
            CheckedUnitEffectOperationPlan::BoundaryCall { .. }
        ));
        match mutation {
            "omitted call" => {
                entry.operations.remove(1);
            }
            "duplicate call" => entry.operations.insert(1, entry.operations[1].clone()),
            "reordered store" => entry.operations.swap(0, 2),
            "wrong coordinate" => {
                let CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. } =
                    &mut entry.operations[1]
                else {
                    unreachable!()
                };
                coordinate.statement_index += 1;
            }
            "same signature boundary" => {
                let CheckedUnitEffectOperationPlan::BoundaryCall {
                    target_machine: replacement_machine,
                    target_state: replacement_state,
                    target_contract_report_fingerprint: replacement_contract,
                    service_reach: replacement_reach,
                    ..
                } = finish
                else {
                    unreachable!()
                };
                let CheckedUnitEffectOperationPlan::BoundaryCall {
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    service_reach,
                    ..
                } = &mut entry.operations[1]
                else {
                    unreachable!()
                };
                assert_ne!(*target_machine, replacement_machine);
                *target_machine = replacement_machine;
                *target_state = replacement_state;
                *target_contract_report_fingerprint = replacement_contract;
                *service_reach = replacement_reach;
                // Both requirements remain directly called elsewhere: exact root coverage alone cannot detect this substitution.
            }
            _ => unreachable!(),
        }
        let error = checked_trees_to_lowered_psi::lower_machine(&changed, "Counter::run")
            .expect_err(mutation);
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::Unsupported(_)
            ),
            "{mutation}: {error:?}"
        );
    }
}

#[test]
fn canonical_cyclic_attachment_roots_reject_missing_extra_and_substituted_identity() {
    let lowered = checked_trees_to_lowered_psi::lower_machine(&baseline(), "Counter::run").unwrap();
    let module = terminal_codec::decode_module(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
    )
    .unwrap();
    terminal_verifier::validate_module(&module).expect("canonical tamper baseline validates");
    for mutation in [
        "omitted",
        "duplicate",
        "stale boundary",
        "scalar field",
        "unknown field",
        "wrong attachment",
        "callee requirement",
        "missing attachment",
    ] {
        let mut changed = module.clone();
        let helper_boundary = changed
            .boundary_machines
            .iter()
            .find(|boundary| boundary.identity.contains("Output::helper"))
            .unwrap()
            .id;
        let entry = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed.entry)
            .unwrap();
        let root_position = entry
            .structural_places
            .iter()
            .position(|place| matches!(place.kind, StructuralPlaceKind::ProviderAttachment { .. }))
            .unwrap();
        match mutation {
            "omitted" => {
                entry.structural_places.remove(root_position);
            }
            "duplicate" => {
                let mut duplicate = entry.structural_places[root_position];
                duplicate.id = semantic_vocabulary::PlaceId::new(
                    entry
                        .structural_places
                        .iter()
                        .map(|place| place.id.get())
                        .max()
                        .unwrap()
                        + 1,
                )
                .unwrap();
                entry.structural_places.push(duplicate);
            }
            "missing attachment" => entry.attachment = None,
            _ => {
                let StructuralPlaceKind::ProviderAttachment {
                    attachment,
                    field,
                    boundary,
                } = &mut entry.structural_places[root_position].kind
                else {
                    unreachable!()
                };
                match mutation {
                    "stale boundary" => {
                        *boundary = semantic_vocabulary::BoundaryMachineId::new(u64::MAX).unwrap()
                    }
                    "scalar field" => {
                        let declaration = changed
                            .structural_types
                            .iter()
                            .find(|declaration| declaration.id == *attachment)
                            .unwrap();
                        let StructuralTypeShape::Record { fields } = &declaration.shape else {
                            unreachable!()
                        };
                        *field = fields
                            .iter()
                            .find(|field| field.identity == "counter")
                            .unwrap()
                            .id;
                    }
                    "unknown field" => {
                        *field = semantic_vocabulary::StructuralFieldId::new(u64::MAX).unwrap()
                    }
                    "wrong attachment" => {
                        *attachment = semantic_vocabulary::StructuralTypeId::new(u64::MAX).unwrap()
                    }
                    "callee requirement" => *boundary = helper_boundary,
                    _ => unreachable!(),
                }
            }
        }
        assert!(
            terminal_verifier::validate_module(&changed).is_err(),
            "canonical {mutation} must reject"
        );
        assert!(
            terminal_codec::encode_module(&changed).is_err(),
            "canonical encoder must reject {mutation}"
        );
    }
}
