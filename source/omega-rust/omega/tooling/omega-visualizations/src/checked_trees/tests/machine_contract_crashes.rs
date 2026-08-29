use super::*;

fn crash_source_coordinate_call(
    state: SymbolHandle,
    statement_ordinal: u32,
    call_ordinal: u32,
    target: SymbolHandle,
) -> psi_checked_trees::CheckedCrashCallSite {
    psi_checked_trees::CheckedCrashCallSite::new(
        psi_checked_trees::CrashCallSiteLocation::new(state, statement_ordinal, call_ordinal),
        target,
        target,
        1,
        Vec::new(),
    )
}

fn crash_target_coordinate_fixture() -> (
    CheckedTrees,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
) {
    let local_machine = SymbolHandle::from_arena_index(100);
    let local_state = SymbolHandle::from_arena_index(101);
    let generic_parameter = SymbolHandle::from_arena_index(103);
    let generic_contract = SymbolHandle::from_arena_index(104);
    let trait_owner = SymbolHandle::from_arena_index(105);
    let trait_signature = SymbolHandle::from_arena_index(106);
    let mut program = CheckedTrees::default();

    let mut local = Machine {
        symbol: local_machine,
        name: Identifier::generated("Local::run"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut local,
        State {
            symbol: local_state,
            name: Identifier::generated("entry"),
            ..Default::default()
        },
    );
    program.typed.push_machine(local);

    let mut generic_owner = Machine {
        symbol: SymbolHandle::from_arena_index(102),
        name: Identifier::generated("Generic::run"),
        ..Default::default()
    };
    program.typed.push_machine_type_parameter(
        &mut generic_owner,
        TypeParameter {
            symbol: generic_parameter,
            name: Identifier::generated("Worker"),
            kind: TypeParameterKind::Machine {
                contract: MachineParameterContract::Structural(StateSignature {
                    symbol: generic_contract,
                    name: Identifier::generated("invoke"),
                    ..Default::default()
                }),
            },
            ..Default::default()
        },
    );
    program.typed.push_machine(generic_owner);

    let mut definition = TraitDefinition {
        symbol: trait_owner,
        name: Identifier::generated("Boundary"),
        ..Default::default()
    };
    program.typed.push_trait_machine_signature(
        &mut definition,
        StateSignature {
            symbol: trait_signature,
            name: Identifier::generated("invoke"),
            ..Default::default()
        },
    );
    program.typed.push_trait_definition(definition);

    (
        program,
        local_machine,
        local_state,
        generic_parameter,
        trait_owner,
        trait_signature,
    )
}

#[test]
fn machine_contract_manifest_crash_target_coordinates_accept_exact_categories() {
    let (program, local_machine, local_state, generic_parameter, trait_owner, trait_signature) =
        crash_target_coordinate_fixture();

    let local =
        exact_manifest_crash_target(&program, local_machine, local_state, "checked crash call");
    let generic = exact_manifest_crash_target(
        &program,
        generic_parameter,
        generic_parameter,
        "checked crash call",
    );
    let trait_target =
        exact_manifest_crash_target(&program, trait_owner, trait_signature, "checked crash call");

    assert_eq!(local.owner_label, "Local::run");
    assert_eq!(local.state_label, "entry");
    assert!(!local.is_requirement);
    assert_eq!(generic.owner_label, "Worker");
    assert_eq!(generic.state_label, "Worker");
    assert!(generic.is_requirement);
    assert_eq!(trait_target.owner_label, "Boundary");
    assert_eq!(trait_target.state_label, "invoke");
    assert!(trait_target.is_requirement);
    assert!(local.overload_identity.starts_with("named-callable("));
    assert!(generic.overload_identity.starts_with("named-callable("));
    assert!(
        trait_target
            .overload_identity
            .starts_with("named-callable(")
    );
}

#[test]
#[should_panic(expected = "must name one exact retained callable target coordinate")]
fn machine_contract_manifest_crash_target_coordinates_reject_missing_target() {
    let (program, local_machine, _, _, _, _) = crash_target_coordinate_fixture();
    exact_manifest_crash_target(
        &program,
        local_machine,
        SymbolHandle::invalid(),
        "checked crash call",
    );
}

#[test]
#[should_panic(expected = "must name one exact retained callable target coordinate")]
fn machine_contract_manifest_crash_target_coordinates_reject_cross_machine_state() {
    let (mut program, local_machine, _, _, _, _) = crash_target_coordinate_fixture();
    let other_state = SymbolHandle::from_arena_index(108);
    let mut other = Machine {
        symbol: SymbolHandle::from_arena_index(107),
        name: Identifier::generated("Other::run"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut other,
        State {
            symbol: other_state,
            name: Identifier::generated("entry"),
            ..Default::default()
        },
    );
    program.typed.push_machine(other);
    exact_manifest_crash_target(&program, local_machine, other_state, "checked crash call");
}

#[test]
#[should_panic(expected = "duplicate exact local target-machine owners")]
fn machine_contract_manifest_crash_target_coordinates_reject_duplicate_local_owner() {
    let (mut program, local_machine, local_state, _, _, _) = crash_target_coordinate_fixture();
    let duplicate = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == local_machine)
        .expect("local machine")
        .clone();
    program.typed.push_machine(duplicate);
    exact_manifest_crash_target(&program, local_machine, local_state, "checked crash call");
}

#[test]
#[should_panic(expected = "duplicate exact local target states")]
fn machine_contract_manifest_crash_target_coordinates_reject_duplicate_local_state() {
    let machine_symbol = SymbolHandle::from_arena_index(110);
    let state_symbol = SymbolHandle::from_arena_index(111);
    let mut program = CheckedTrees::default();
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Duplicate::run"),
        ..Default::default()
    };
    for name in ["first", "second"] {
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: state_symbol,
                name: Identifier::generated(name),
                ..Default::default()
            },
        );
    }
    program.typed.push_machine(machine);
    exact_manifest_crash_target(&program, machine_symbol, state_symbol, "checked crash call");
}

#[test]
#[should_panic(expected = "duplicate exact generic requirement targets")]
fn machine_contract_manifest_crash_target_coordinates_reject_ambiguous_generic() {
    let target = SymbolHandle::from_arena_index(113);
    let mut program = CheckedTrees::default();
    for (owner, name) in [(114, "First"), (115, "Second")] {
        let mut machine = Machine {
            symbol: SymbolHandle::from_arena_index(owner),
            name: Identifier::generated(name),
            ..Default::default()
        };
        program.typed.push_machine_type_parameter(
            &mut machine,
            TypeParameter {
                symbol: target,
                name: Identifier::generated("Worker"),
                kind: TypeParameterKind::Machine {
                    contract: MachineParameterContract::Structural(StateSignature {
                        symbol: target,
                        name: Identifier::generated("invoke"),
                        ..Default::default()
                    }),
                },
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);
    }
    exact_manifest_crash_target(&program, target, target, "checked crash call");
}

#[test]
#[should_panic(expected = "duplicate exact trait target owners")]
fn machine_contract_manifest_crash_target_coordinates_reject_ambiguous_trait() {
    let (mut program, _, _, _, trait_owner, trait_signature) = crash_target_coordinate_fixture();
    let duplicate = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == trait_owner)
        .expect("trait owner")
        .clone();
    program.typed.push_trait_definition(duplicate);
    exact_manifest_crash_target(&program, trait_owner, trait_signature, "checked crash call");
}

#[test]
#[should_panic(expected = "target owner must resolve to one retained callable category")]
fn machine_contract_manifest_crash_target_coordinates_reject_category_collision() {
    let (mut program, local_machine, local_state, _, _, _) = crash_target_coordinate_fixture();
    let mut definition = TraitDefinition {
        symbol: local_machine,
        name: Identifier::generated("CollidingBoundary"),
        ..Default::default()
    };
    program.typed.push_trait_machine_signature(
        &mut definition,
        StateSignature {
            symbol: local_state,
            name: Identifier::generated("entry"),
            ..Default::default()
        },
    );
    program.typed.push_trait_definition(definition);
    exact_manifest_crash_target(&program, local_machine, local_state, "checked crash call");
}

#[test]
#[should_panic(expected = "must be an exact requirement owner/signature pair")]
fn machine_contract_manifest_crash_target_coordinates_reject_local_capsule() {
    let (mut program, local_machine, local_state, _, _, _) = crash_target_coordinate_fixture();
    program
        .facts
        .contract_plans
        .crash_capsules
        .push(psi_checked_trees::CrashContractCapsule::new(
            local_machine,
            local_state,
            0x1111,
            Vec::new(),
        ));
    validated_manifest_crash_capsules(&program);
}

#[test]
#[should_panic(expected = "duplicate exact target coordinates")]
fn machine_contract_manifest_crash_target_coordinates_reject_duplicate_capsule() {
    let (mut program, _, _, _, trait_owner, trait_signature) = crash_target_coordinate_fixture();
    for fingerprint in [0x1111, 0x2222] {
        program.facts.contract_plans.crash_capsules.push(
            psi_checked_trees::CrashContractCapsule::new(
                trait_owner,
                trait_signature,
                fingerprint,
                Vec::new(),
            ),
        );
    }
    validated_manifest_crash_capsules(&program);
}

#[test]
fn machine_contract_manifest_crash_target_coordinates_preserve_capsule_payload() {
    let (mut program, _, _, _, trait_owner, trait_signature) = crash_target_coordinate_fixture();
    let bucket =
        psi_checked_trees::CrashRouteBucket::unconditional(psi_checked_trees::CrashCause::Abort);
    program
        .facts
        .contract_plans
        .crash_capsules
        .push(psi_checked_trees::CrashContractCapsule::new(
            trait_owner,
            trait_signature,
            0x1234,
            vec![bucket.clone()],
        ));

    let validated = validated_manifest_crash_capsules(&program);

    assert_eq!(validated.len(), 1);
    assert_eq!(
        validated[0].capsule.target_contract_report_fingerprint(),
        0x1234
    );
    assert_eq!(validated[0].capsule.published_buckets(), [bucket]);
    assert_eq!(validated[0].target.owner_label, "Boundary");
    assert_eq!(validated[0].target.state_label, "invoke");
}

#[test]
fn machine_contract_manifest_crash_source_coordinates_accept_exact_site_and_call() {
    let (program, machine_symbol, state_symbol, _) = crash_source_coordinate_fixture();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("crash source machine");
    let site = exact_manifest_crash_source_state(&program, machine, state_symbol, 1, "site");
    let call = crash_source_coordinate_call(state_symbol, 2, 1, state_symbol);
    let call_state = exact_manifest_crash_call_source(&program, machine, &call);

    assert_eq!(site.name.as_str(), "entry");
    assert_eq!(call_state.symbol, site.symbol);
    assert_eq!(call.location().statement_ordinal(), 2);
    assert_eq!(call.location().call_ordinal(), 1);
}

#[test]
#[should_panic(expected = "source state must belong to its exact contract machine")]
fn machine_contract_manifest_crash_source_coordinates_reject_cross_machine_site_state() {
    let (program, machine_symbol, _, other_state) = crash_source_coordinate_fixture();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("crash source machine");
    exact_manifest_crash_source_state(&program, machine, other_state, 1, "site");
}

#[test]
#[should_panic(expected = "source state must belong to its exact contract machine")]
fn machine_contract_manifest_crash_source_coordinates_reject_missing_site_state() {
    let (program, machine_symbol, _, _) = crash_source_coordinate_fixture();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("crash source machine");
    exact_manifest_crash_source_state(&program, machine, SymbolHandle::invalid(), 1, "site");
}

#[test]
#[should_panic(expected = "site statement must belong to its exact typed state")]
fn machine_contract_manifest_crash_source_coordinates_reject_out_of_range_site() {
    let (program, machine_symbol, state_symbol, _) = crash_source_coordinate_fixture();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("crash source machine");
    exact_manifest_crash_source_state(&program, machine, state_symbol, 3, "site");
}

#[test]
#[should_panic(expected = "call statement must belong to its exact typed state")]
fn machine_contract_manifest_crash_source_coordinates_reject_out_of_range_call() {
    let (program, machine_symbol, state_symbol, _) = crash_source_coordinate_fixture();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("crash source machine");
    let call = crash_source_coordinate_call(state_symbol, 3, 1, state_symbol);
    exact_manifest_crash_call_source(&program, machine, &call);
}

#[test]
#[should_panic(expected = "must name one exact checked flow state")]
fn machine_contract_manifest_crash_source_coordinates_reject_missing_flow_state() {
    let (mut program, machine_symbol, state_symbol, _) = crash_source_coordinate_fixture();
    program
        .facts
        .flow
        .control
        .states
        .for_each_mut(|_, flow| flow.machine_symbol = SymbolHandle::invalid());
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("crash source machine");
    let call = crash_source_coordinate_call(state_symbol, 2, 1, state_symbol);
    exact_manifest_crash_call_source(&program, machine, &call);
}

#[test]
#[should_panic(expected = "must name exactly one checked flow state")]
fn machine_contract_manifest_crash_source_coordinates_reject_duplicate_flow_state() {
    let (mut program, machine_symbol, state_symbol, _) = crash_source_coordinate_fixture();
    let duplicate = program
        .facts
        .flow
        .control
        .states
        .iter()
        .next()
        .map(|(_, flow)| flow.clone())
        .expect("flow state");
    program.facts.flow.control.states.insert(duplicate);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("crash source machine");
    let call = crash_source_coordinate_call(state_symbol, 2, 1, state_symbol);
    exact_manifest_crash_call_source(&program, machine, &call);
}

#[test]
#[should_panic(expected = "must retain an exact valid call span")]
fn machine_contract_manifest_crash_source_coordinates_reject_invalid_call_span() {
    let (mut program, machine_symbol, state_symbol, _) = crash_source_coordinate_fixture();
    program.facts.flow.control.states.for_each_mut(|_, flow| {
        flow.calls = psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_arena_index(999), 1);
    });
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("crash source machine");
    let call = crash_source_coordinate_call(state_symbol, 2, 1, state_symbol);
    exact_manifest_crash_call_source(&program, machine, &call);
}

#[test]
#[should_panic(expected = "must name one exact checked flow call")]
fn machine_contract_manifest_crash_source_coordinates_reject_missing_flow_call() {
    let (program, machine_symbol, state_symbol, _) = crash_source_coordinate_fixture();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("crash source machine");
    let call = crash_source_coordinate_call(state_symbol, 2, 0, state_symbol);
    exact_manifest_crash_call_source(&program, machine, &call);
}

#[test]
#[should_panic(expected = "must name exactly one checked flow call")]
fn machine_contract_manifest_crash_source_coordinates_reject_duplicate_flow_call() {
    let (mut program, machine_symbol, state_symbol, _) = crash_source_coordinate_fixture();
    let duplicate = program
        .facts
        .flow
        .control
        .calls
        .iter()
        .next()
        .map(|(_, call)| call.clone())
        .expect("flow call");
    let calls = program
        .facts
        .flow
        .control
        .calls
        .insert_many([duplicate.clone(), duplicate]);
    program
        .facts
        .flow
        .control
        .states
        .for_each_mut(|_, flow| flow.calls = calls);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("crash source machine");
    let call = crash_source_coordinate_call(state_symbol, 2, 1, state_symbol);
    exact_manifest_crash_call_source(&program, machine, &call);
}

#[test]
#[should_panic(expected = "must retain its exact checked flow target")]
fn machine_contract_manifest_crash_source_coordinates_reject_target_drift() {
    let (mut program, machine_symbol, state_symbol, _) = crash_source_coordinate_fixture();
    program
        .facts
        .flow
        .control
        .calls
        .for_each_mut(|_, flow| flow.target_symbol = SymbolHandle::invalid());
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("crash source machine");
    let call = crash_source_coordinate_call(state_symbol, 2, 1, state_symbol);
    exact_manifest_crash_call_source(&program, machine, &call);
}
