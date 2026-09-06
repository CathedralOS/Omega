use super::*;
use checked_trees::CheckedUnitStructuralArgumentSourcePlan as ArgumentSource;
use checked_trees_to_lowered_psi::LoweringError;

const CHAIN_WITH_SPARE: &str = "data Value { number: u64; }
    machine forward(value: Value) -> Value { value }
    machine Main::caller(value: Value, spare: Value) {
        let first: Value = forward(value);
        let second: Value = forward(first);
    }";

#[test]
fn an_authored_result_cannot_be_replaced_with_a_live_same_type_parameter() {
    assert_input_rejoin(true);
}

#[test]
fn an_authored_parameter_cannot_be_replaced_with_a_live_same_type_result() {
    assert_input_rejoin(false);
}

fn assert_input_rejoin(authored_uses_result: bool) {
    let source = if authored_uses_result {
        CHAIN_WITH_SPARE.to_owned()
    } else {
        CHAIN_WITH_SPARE.replace("forward(first)", "forward(spare)")
    };
    let mut checked = checked(&source);
    let mut lowered = lower_machine(&checked, "Main::caller").expect("authored inputs lower");
    let caller_state = lowered.source_call_occurrences[0].source_state;
    let caller = &mut lowered.semantic_module.machines[0];
    let spare = caller.structural_parameters[1].place;
    let block = &mut caller.blocks[0];
    let [first, second] = block.operations.as_mut_slice() else {
        panic!("two ordinary structural producers");
    };
    let OperationResult::Structural(first_result) = &first.result else {
        panic!("first producer result");
    };
    let OperationResult::Structural(second_result) = &second.result else {
        panic!("second producer result");
    };
    let OperationKind::CallStructuralWithScalarArguments {
        structural_arguments,
        ..
    } = &mut second.kind
    else {
        panic!("second ordinary producer");
    };
    let (authored_input, forged_input) = if authored_uses_result {
        (first_result.place, spare)
    } else {
        (spare, first_result.place)
    };
    assert_eq!(structural_arguments[0].place, authored_input);
    structural_arguments[0].place = forged_input;
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut block.terminator
    else {
        panic!("caller cleanup");
    };
    assert_eq!(
        trivial_affine_discards.as_slice(),
        &[second_result.place, forged_input]
    );
    *trivial_affine_discards = vec![second_result.place, authored_input];
    // Both alternatives conserve ownership. Only the checked-source rejoin
    // can establish which live, same-typed input the author actually supplied.
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("alternate input and adjusted cleanup are independently valid");

    let caller = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|machine| machine.state == caller_state)
        .expect("checked caller");
    let [first, second, cleanup] = caller.operations.as_mut_slice() else {
        panic!("two checked producers and their return");
    };
    let CheckedUnitEffectOperationPlan::StructuralCall {
        discard_result_on_return,
        ..
    } = first
    else {
        panic!("first checked producer");
    };
    assert_eq!(*discard_result_on_return, !authored_uses_result);
    *discard_result_on_return = authored_uses_result;
    let CheckedUnitEffectOperationPlan::StructuralCall {
        structural_arguments,
        ..
    } = second
    else {
        panic!("second checked producer");
    };
    structural_arguments[0].source = if authored_uses_result {
        ArgumentSource::Parameter { parameter_index: 1 }
    } else {
        ArgumentSource::StructuralResult { binding_ordinal: 0 }
    };
    let CheckedUnitEffectOperationPlan::ReturnUnit {
        trivial_affine_discards,
        ..
    } = cleanup
    else {
        panic!("checked return cleanup");
    };
    assert_eq!(
        trivial_affine_discards.as_slice(),
        if authored_uses_result { &[1][..] } else { &[] }
    );
    *trivial_affine_discards = if authored_uses_result {
        Vec::new()
    } else {
        vec![1]
    };
    let expected = if authored_uses_result {
        "ordinary structural call source is not an exact authored claim-free affine parameter"
    } else {
        "Unit structural result argument does not rejoin its exact authored local"
    };
    assert_eq!(
        lower_machine(&checked, "Main::caller").expect_err("forged authored input must reject"),
        LoweringError::Unsupported(expected)
    );
}
