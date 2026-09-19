use super::{
    EXECUTABLE_SOURCE, THREE_CALL_SOURCE, THREE_ROOT_DISTINCT_SOURCE,
    THREE_ROOT_SHARED_EXECUTABLE_SOURCE, TWO_CALL_SOURCE,
};
use crate::nominal_affine_source::{
    AdmissionProfile, Lexer, OperationKind, OperationResult, ResolutionRequest, Terminator,
    decode_module, encode_module, lower_symbol_resolved_trees, lower_typed_trees,
    parse_syntax_trees, resolve,
};

#[test]
fn three_distinct_nominal_roots_cross_source_codec_and_verifier_in_reverse_order() {
    let tokens = Lexer::new(THREE_ROOT_DISTINCT_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("three distinct cleanup targets lower");

    assert_eq!(lowered.semantic_module.machines.len(), 4);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [first, second, third] = entry.structural_parameters.as_slice() else {
        panic!("three source roots remain structural parameters")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("ordered nominal cleanup return")
    };
    let [third_cleanup, second_cleanup, first_cleanup] = cleanups.as_slice() else {
        panic!("all three roots require nominal cleanup")
    };
    assert_eq!(
        [
            third_cleanup.place,
            second_cleanup.place,
            first_cleanup.place
        ],
        [third.place, second.place, first.place]
    );
    assert_ne!(
        third_cleanup.cleanup_machine,
        second_cleanup.cleanup_machine
    );
    assert_ne!(third_cleanup.cleanup_machine, first_cleanup.cleanup_machine);
    assert_ne!(
        second_cleanup.cleanup_machine,
        first_cleanup.cleanup_machine
    );

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("three distinct ordered cleanup actions verify");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn three_nominal_roots_may_share_one_executable_target_and_helper() {
    let tokens = Lexer::new(THREE_ROOT_SHARED_EXECUTABLE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("three shared executable cleanup actions lower");

    assert_eq!(lowered.semantic_module.machines.len(), 3);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [first, second, third] = entry.structural_parameters.as_slice() else {
        panic!("three source roots remain structural parameters")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("ordered nominal cleanup return")
    };
    assert_eq!(
        cleanups
            .iter()
            .map(|cleanup| cleanup.place)
            .collect::<Vec<_>>(),
        vec![third.place, second.place, first.place]
    );
    assert!(
        cleanups
            .iter()
            .all(|cleanup| cleanup.cleanup_machine == cleanups[0].cleanup_machine)
    );
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("shared cleanup target");
    assert_eq!(target.blocks[0].operations.len(), 1);

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("three shared executable cleanup actions verify");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}

#[test]
fn one_call_nominal_cleanup_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(EXECUTABLE_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("one-call nominal cleanup lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 3);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected executable nominal cleanup return")
    };
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("cleanup target");
    let [call] = target.blocks[0].operations.as_slice() else {
        panic!("cleanup target must contain exactly one call")
    };
    assert_eq!(call.result, OperationResult::Unit);
    let OperationKind::CallUnit {
        arguments,
        erased_arguments: _,
        callee,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = &call.kind
    else {
        panic!("cleanup operation must be an ordinary Unit call")
    };
    assert!(arguments.is_empty());
    assert!(structural_arguments.is_empty());
    assert!(claim_transfers.is_empty());
    assert!(requirement_obligations.is_empty());
    assert!(crash_continuations.is_empty());
    let helper = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == *callee)
        .expect("cleanup helper");
    assert_ne!(helper.id, target.id);
    assert_ne!(helper.id, entry.id);
    assert!(helper.structural_parameters.is_empty());
    assert!(helper.blocks[0].operations.is_empty());
    assert!(matches!(
        &helper.blocks[0].terminator,
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } if trivial_affine_discards.is_empty()
    ));

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts exact executable nominal cleanup closure");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module,
        "the three-machine cleanup closure is canonical artifact data"
    );
}

#[test]
fn two_call_nominal_cleanup_preserves_source_order_through_codec_and_verifier() {
    let tokens = Lexer::new(TWO_CALL_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("two-call nominal cleanup lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 4);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected executable nominal cleanup return")
    };
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("cleanup target");
    let [first_call, second_call] = target.blocks[0].operations.as_slice() else {
        panic!("cleanup target must contain exactly two calls")
    };
    let callees = [first_call, second_call].map(|operation| {
        assert_eq!(operation.result, OperationResult::Unit);
        let OperationKind::CallUnit {
            arguments,
            erased_arguments: _,
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } = &operation.kind
        else {
            panic!("cleanup operation must be an ordinary Unit call")
        };
        assert!(arguments.is_empty());
        assert!(structural_arguments.is_empty());
        assert!(claim_transfers.is_empty());
        assert!(requirement_obligations.is_empty());
        assert!(crash_continuations.is_empty());
        *callee
    });
    assert_ne!(callees[0], callees[1]);
    assert_eq!(
        lowered
            .semantic_module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<Vec<_>>(),
        vec![entry.id, target.id, callees[0], callees[1]],
        "the exact closure retains source call order"
    );
    for callee in callees {
        let helper = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == callee)
            .expect("cleanup helper");
        assert!(helper.structural_parameters.is_empty());
        assert!(helper.blocks[0].operations.is_empty());
    }

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts exact ordered two-call nominal cleanup closure");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module,
        "the four-machine ordered cleanup closure is canonical artifact data"
    );
}

#[test]
fn three_call_nominal_cleanup_preserves_exact_source_order_through_codec_and_verifier() {
    let tokens = Lexer::new(THREE_CALL_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("three-call nominal cleanup lowers");

    assert_eq!(lowered.semantic_module.machines.len(), 5);
    let entry = &lowered.semantic_module.machines[0];
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected executable nominal cleanup return")
    };
    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("cleanup target");
    let [first, second, third] = target.blocks[0].operations.as_slice() else {
        panic!("cleanup target retains exactly three calls")
    };
    let callees = [first, second, third].map(|operation| {
        let OperationKind::CallUnit { callee, .. } = operation.kind else {
            panic!("cleanup helper is an ordinary Unit call")
        };
        callee
    });
    assert_eq!(
        lowered
            .semantic_module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<Vec<_>>(),
        vec![entry.id, target.id, callees[0], callees[1], callees[2]]
    );

    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("three-call nominal cleanup verifies");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(decode_module(&bytes).unwrap(), lowered.semantic_module);
}
