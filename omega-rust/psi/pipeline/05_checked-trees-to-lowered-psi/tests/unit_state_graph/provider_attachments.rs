use checked_trees::CheckedComposedUnitControlMachinePlan;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::StructuralTypeShape;

const SOURCE: &str = r#"
    boundary trait Output {
        machine write(value: u64) reaches Output;
        machine finish(value: u64) reaches Output;
        machine helper(value: u64) reaches Output;
    }
    data Counter<'s> { counter: u64; output: &'s mut Output; }
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
    let checked = crate::front_end::checked_program(SOURCE);
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Counter::run"),
    )
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
    checked_trees_to_lowered_psi::lower_machine(
        &changed,
        TerminalMachineSelection::Name("Counter::run"),
    )
    .expect_err(mutation);
}

#[test]
fn authored_provider_receiver_rejects_scalar_field_with_unchanged_plan() {
    reject_authored_provider_receiver_mutation("scalar field");
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
        let error = checked_trees_to_lowered_psi::lower_machine(
            &changed,
            TerminalMachineSelection::Name("Counter::run"),
        )
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
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &baseline(),
        TerminalMachineSelection::Name("Counter::run"),
    )
    .unwrap();
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

/// A carrier path is direct storage access on one referent. A record field
/// that is itself a borrow names separate storage, so stepping through it
/// would publish a pointer hop as an inline field offset --
/// "Cases, reference crossings and nonprimitive leaves reject"
/// (wiki/spec/terminal-psi/structural_access.md, Store vocabulary). Admitting
/// borrow-carrying attachment records must not admit that path with them.
const REFERENCE_CARRIER_SOURCE: &str = r#"
    boundary trait Output {
        machine write(value: u64) reaches Output;
    }
    data Inner { value: u64; }
    data Outer<'s> { inner: &'s mut Inner; output: &'s mut Output; }
    machine Outer::run(&mut self) reaches Output {
        self.inner.value = 7;
        self.output.write(9);
    }
"#;

#[test]
fn a_store_path_never_crosses_a_borrowed_carrier_field() {
    let checked = crate::front_end::checked_program(REFERENCE_CARRIER_SOURCE);
    let error = format!(
        "{:?}",
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Outer::run")
        )
        .expect_err("a carrier hop through `&mut Inner` is not an inline field offset")
    );
    assert!(
        error.contains("missing a checked transitive machine plan"),
        "unexpected rejection: {error}"
    );
}
