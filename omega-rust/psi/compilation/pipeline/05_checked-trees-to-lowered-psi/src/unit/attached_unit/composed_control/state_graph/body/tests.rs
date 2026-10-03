use checked_trees::{
    CheckedScalarBindingDestination, CheckedStoreRoot, CheckedUnitEffectOperationPlan,
};

const SOURCE: &str = "
    data Output { value: u32; }
    machine checkpoint() {}
    machine write(output: &mut Output) {
        let mut first: u32 = 1;
        let mut second: u32 = 2;
        let captured: u32 = first;
        checkpoint();
        first = 70;
        second = captured;
        output.value = first;
        transition second == 1 {
            true -> done(output)
            false -> done(output)
        }
        state done(output: &mut Output) {}
    }
";

#[test]
fn reference_completions_follow_value_custody_not_borrow_spelling() {
    for access in ["&", "&mut"] {
        for completion in [
            format!("({access} pair.first)"),
            format!("transition {{ _ -> ({access} pair.first) }}"),
        ] {
            let source = format!(
                "data Item {{ value: u32; }}
                 data Pair {{ first: Item; second: Item; }}
                 machine pick(pair: {access} Pair) -> {access} Item {{ {completion} }}"
            );
            let checked = crate::front_end::checked_program(&source);
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "pick")
                .expect("pick machine");
            assert!(
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .composed_for_machine(machine.symbol)
                    .is_some(),
                "reference access and completion spelling must not exclude the ordinary graph"
            );
            let lowered =
                crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("pick"))
                    .unwrap_or_else(|error| panic!("{source}: {error:?}"));
            terminal_verifier::verify_module(
                &lowered.semantic_module,
                &lowered.proof_bundle,
                &proof_admission::AdmissionProfile::default(),
            )
            .expect("reference completions retain independently checked custody");
        }
    }
}

#[test]
fn reference_completion_rejects_changed_path_access_and_position() {
    for (returned, expression) in [("&mut Item", "(&mut pair.first)"), ("&Pair", "pair")] {
        let checked = crate::front_end::checked_program(&format!(
            "data Item {{ value: u32; }}
         data Pair {{ first: Item; second: Item; }}
         machine pick(pair: &mut Pair) -> {returned} {{ {expression} }}",
        ));
        let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("pick"))
            .expect("projected and weakened reference completions lower");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("independent verification retains the returned access");
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "pick")
            .expect("pick machine");
        let source = &checked.machine_states(machine)[0];
        let original = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine.symbol)
            .expect("reference completion graph");
        let replay = |plan: &checked_trees::CheckedComposedUnitControlMachinePlan| {
            let ordinal = super::validate(&checked, machine.symbol, source, &plan.states[0])?;
            super::super::returns::validate_structural(
                &checked,
                plan,
                source,
                &plan.states[0],
                ordinal,
            )
        };
        replay(original).expect("authored completion replays");
        for mutation in 0..3 {
            let mut changed = original.clone();
            let operation = changed.states[0]
                .operations
                .iter_mut()
                .find(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::EstablishReference { .. }
                    )
                })
                .expect("reference establishment");
            let CheckedUnitEffectOperationPlan::EstablishReference { result, source } = operation
            else {
                unreachable!()
            };
            match mutation {
                0 => source
                    .path
                    .push(checked_trees::CheckedUnitStructuralPathSegment::Referent),
                1 => {
                    source.access = match source.access {
                        checked_trees::CheckedStructuralAccess::SharedBorrow => {
                            checked_trees::CheckedStructuralAccess::MutableBorrow
                        }
                        _ => checked_trees::CheckedStructuralAccess::SharedBorrow,
                    }
                }
                2 => result.statement_index += 1,
                _ => unreachable!(),
            }
            assert!(
                replay(&changed).is_err(),
                "forged reference completion {mutation}"
            );
        }
    }
}

#[test]
fn mutable_prefix_locals_keep_establishment_across_body_effects() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("write"))
        .expect("a prior scalar binding must establish the storage its later assignment uses");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent verification retains local storage and captured values");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.states.len() == 2)
        .expect("composed write");
    let entry = &plan.states[0];
    assert!(
        entry
            .bindings
            .iter()
            .all(|binding| binding.destination == CheckedScalarBindingDestination::Immutable)
    );
    assert_eq!(
        entry
            .operations
            .iter()
            .filter(|operation| matches!(
                operation,
                CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
            ))
            .count(),
        2
    );
}

#[test]
fn scalar_return_computation_follows_local_effects() {
    let checked = crate::front_end::checked_program(
        "pub boundary machine + Float::add(left: f64, right: f64) -> f64;
         pub boundary machine * Float::multiply(left: f64, right: f64) -> f64;
         machine calculate(value: f64) -> f64 {
             transition { _ -> finish(value) }
             state finish(value: f64) {
                 let mut result: f64 = 1.0;
                 result = result * value;
                 transition { _ -> (value + result) }
             }
         }",
    );
    let lowered =
        crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("calculate"))
            .expect("return computation must follow the local's completed effects");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("return computation retains source and local custody");
}

#[test]
fn mutable_local_assignment_rejects_a_different_established_home() {
    let checked = crate::front_end::checked_program(SOURCE);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "write")
        .expect("source machine");
    let source = &checked.machine_states(machine)[0];
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine.symbol)
        .expect("composed write");
    let mut state = plan.states[0].clone();
    let homes = state
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { symbol, .. } => Some(*symbol),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(homes.len(), 2);
    let destination = state
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { destination, .. } => {
                Some(destination)
            }
            _ => None,
        })
        .expect("authored local assignment");
    assert_eq!(*destination, CheckedStoreRoot::Local { symbol: homes[0] });
    *destination = CheckedStoreRoot::Local { symbol: homes[1] };
    assert!(
        super::validate(&checked, machine.symbol, source, &state).is_err(),
        "equal types and prior establishment cannot substitute another local's identity"
    );
}

const GUARDED_REFERENCE_SOURCE: &str = r#"
    data Holder { value: u64; }
    data Bag { holders: [Holder; 3]; }
    machine pick(bag: &mut Bag, second: bool) -> &mut Holder {
        let holder_view: &mut [Holder] = bag.holders.as_mut_slice();
        transition second {
            true -> &mut holder_view[1]
            false -> &mut holder_view[0]
        }
    }
"#;

#[test]
fn guarded_reference_arms_replay_exact_authored_sources() {
    assert_guarded_reference_sources(GUARDED_REFERENCE_SOURCE, "pick");
}

#[test]
fn guarded_attached_reference_arms_replay_exact_authored_sources() {
    let attached = GUARDED_REFERENCE_SOURCE
        .replace(
            "machine pick(bag: &mut Bag,",
            "machine Bag::pick(&mut self,",
        )
        .replace("bag.holders", "self.holders");
    assert_guarded_reference_sources(&attached, "Bag::pick");
}

fn assert_guarded_reference_sources(source_text: &str, machine_name: &str) {
    use checked_trees::{
        CheckedComposedUnitControlTerminatorPlan, CheckedStructuralAccess,
        CheckedUnitStructuralPathSegment,
    };

    let checked = crate::front_end::checked_program(source_text);
    let lowered = crate::lower_machine(
        &checked,
        crate::TerminalMachineSelection::Name(machine_name),
    )
    .expect("guarded references lower with their exact arm sources and view cleanup");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("guarded reference returns preserve exact affine cleanup");

    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("pick machine");
    let source = &checked.machine_states(machine)[0];
    let original = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine.symbol)
        .expect("guarded reference plan")
        .clone();
    let start = super::validate(&checked, machine.symbol, source, &original.states[0])
        .expect("guarded prefix replays");
    super::super::guarded::validate(&checked, &original, source, &original.states[0], start)
        .expect("guarded arms replay");

    let mut missing_cleanup = original.clone();
    let operation = missing_cleanup.states[0]
        .operations
        .iter_mut()
        .find(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
            )
        })
        .expect("mutable view producer");
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        discard_result_on_return,
        ..
    } = operation
    else {
        unreachable!()
    };
    *discard_result_on_return = false;
    assert!(
        super::validate(&checked, machine.symbol, source, &missing_cleanup.states[0]).is_err(),
        "the unique view release cannot omit its physical affine cleanup"
    );

    let mut forged_path = original.clone();
    let (prior, forged) = {
        let CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } =
            &mut forged_path.states[0].terminator
        else {
            panic!("guarded reference terminator")
        };
        let CheckedUnitEffectOperationPlan::EstablishReference { source, .. } =
            &mut return_values[0]
        else {
            panic!("guarded reference arm")
        };
        let prior = source.clone();
        let Some(CheckedUnitStructuralPathSegment::FixedIndex(index)) = source.path.last_mut()
        else {
            panic!("fixed arm index")
        };
        *index = 2;
        (prior, source.clone())
    };
    let candidate = forged_path
        .result_reference_sources
        .iter_mut()
        .find(|candidate| candidate.source == prior)
        .expect("matching arm candidate");
    candidate.source = forged;
    assert!(
        super::super::guarded::validate(
            &checked,
            &forged_path,
            source,
            &forged_path.states[0],
            start,
        )
        .is_err(),
        "forging an arm and its candidate receipt together bypassed authored replay"
    );

    let mut shared_arm = original.clone();
    let CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } =
        &mut shared_arm.states[0].terminator
    else {
        panic!("guarded reference terminator")
    };
    let CheckedUnitEffectOperationPlan::EstablishReference {
        source: arm_source, ..
    } = &mut return_values[0]
    else {
        panic!("guarded reference arm")
    };
    arm_source.access = CheckedStructuralAccess::SharedBorrow;
    assert!(
        super::super::guarded::validate(
            &checked,
            &shared_arm,
            source,
            &shared_arm.states[0],
            start,
        )
        .is_err(),
        "a mutable return arm was retagged as shared"
    );

    let mut missing_candidate = original.clone();
    missing_candidate.result_reference_sources.pop();
    assert!(
        super::super::guarded::validate(
            &checked,
            &missing_candidate,
            source,
            &missing_candidate.states[0],
            start,
        )
        .is_err(),
        "guarded arms no longer cover the published candidate roster"
    );
}
