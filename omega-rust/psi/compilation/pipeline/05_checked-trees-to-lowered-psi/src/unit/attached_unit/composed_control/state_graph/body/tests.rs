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
        let holders: &mut [Holder] = bag.holders.as_mut_slice();
        transition second {
            true -> &mut holders[1]
            false -> &mut holders[0]
        }
    }
"#;

#[test]
fn guarded_reference_arms_replay_exact_authored_sources() {
    use checked_trees::{
        CheckedComposedUnitControlTerminatorPlan, CheckedStructuralAccess,
        CheckedUnitStructuralPathSegment,
    };

    let checked = crate::front_end::checked_program(GUARDED_REFERENCE_SOURCE);
    let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("pick"))
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
        .find(|machine| machine.name.as_str() == "pick")
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
