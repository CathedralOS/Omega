use psi_core::{IntegerSign, IntegerType, ScalarType};
use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{
    OperationKind, OperationResult, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeShape, Terminator,
};
use psi_terminal_codec::{decode_module, encode_module};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Token {}
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const SCALAR_SOURCE: &str = r#"
    data Token { flag: bool; tag: u8; delta: i16; payload: u64; address: addr; }
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const TWO_ROOT_SOURCE: &str = r#"
    data Token {}
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(first: Token, second: Token) {}
"#;

const EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token { flag: bool; }
    machine Token::drop(&mut self) {
        Helper::touch();
    }

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const TWO_CALL_SOURCE: &str = r#"
    data First {}
    machine First::touch() {}
    data Second {}
    machine Second::touch() {}

    data Token { flag: bool; }
    machine Token::drop(&mut self) {
        First::touch();
        Second::touch();
    }

    data Root {}
    machine Root::enter(token: Token) {}
"#;

#[test]
fn empty_nominal_cleanup_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("empty nominal cleanup lowers");

    assert_eq!(
        lowered.semantic_module.machines.len(),
        2,
        "the cleanup target is part of the executable terminal closure"
    );
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [root] = entry.structural_parameters.as_slice() else {
        panic!("nominal cleanup source slice has one structural root")
    };
    assert_eq!(root.multiplicity, StructuralMultiplicity::Affine);
    assert!(root.qualifications.is_empty());
    let [block] = entry.blocks.as_slice() else {
        panic!("nominal cleanup source slice has one block")
    };
    assert!(block.operations.is_empty());
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &block.terminator else {
        panic!("expected executable nominal cleanup return")
    };
    assert_eq!(cleanups[0].place, root.place);
    assert_eq!(cleanups[0].structural_type, root.structural_type);

    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanups[0].cleanup_machine)
        .expect("cleanup target machine");
    assert_eq!(target.attachment, Some(cleanups[0].structural_type));
    assert!(target.structural_parameters.is_empty());
    assert!(target.blocks[0].operations.is_empty());
    assert!(matches!(
        &target.blocks[0].terminator,
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } if trivial_affine_discards.is_empty()
    ));

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts exact nominal cleanup closure");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module,
        "nominal cleanup target identity is canonical artifact data"
    );
}

#[test]
fn wide_mixed_primitive_record_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(SCALAR_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("wide flat scalar nominal cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected executable nominal cleanup return")
    };
    let cleanup_type = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == cleanups[0].structural_type)
        .expect("cleanup structural type");
    let StructuralTypeShape::Record { fields } = &cleanup_type.shape else {
        panic!("cleanup type remains a record")
    };
    let [flag, tag, delta, payload, address] = fields.as_slice() else {
        panic!("bounded cleanup record retains all five fields")
    };
    for (field, identity, scalar_type) in [
        (flag, "flag", ScalarType::Boolean),
        (
            tag,
            "tag",
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
        ),
        (
            delta,
            "delta",
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).expect("i16")),
        ),
        (
            payload,
            "payload",
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64")),
        ),
        (
            address,
            "address",
            ScalarType::Integer(IntegerType::address(64).expect("addr")),
        ),
    ] {
        assert_eq!(field.identity, identity);
        assert!(!field.relevance.is_erased());
        let StructuralFieldType::Scalar(actual) = &field.field_type else {
            panic!("wide cleanup record retains scalar carriers")
        };
        assert_eq!(*actual, scalar_type);
    }

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts wide flat scalar nominal cleanup closure");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module,
        "the primitive field and nominal cleanup identity are canonical artifact data"
    );
}

#[test]
fn two_nominal_roots_cleanup_in_reverse_parameter_order_and_may_share_a_target() {
    let tokens = Lexer::new(TWO_ROOT_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("two nominal roots lower");

    assert_eq!(
        lowered.semantic_module.machines.len(),
        2,
        "same-type roots share one exact cleanup target"
    );
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [first, second] = entry.structural_parameters.as_slice() else {
        panic!("two source roots remain structural parameters")
    };
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &entry.blocks[0].terminator else {
        panic!("expected ordered nominal cleanup return")
    };
    let [second_cleanup, first_cleanup] = cleanups.as_slice() else {
        panic!("both roots require nominal cleanup")
    };
    assert_eq!(second_cleanup.place, second.place);
    assert_eq!(first_cleanup.place, first.place);
    assert_eq!(second_cleanup.structural_type, second.structural_type);
    assert_eq!(first_cleanup.structural_type, first.structural_type);
    assert_eq!(
        second_cleanup.cleanup_machine, first_cleanup.cleanup_machine,
        "same-type roots reuse the same exact cleanup target"
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts ordered two-root nominal cleanup");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module
    );
}

#[test]
fn one_call_nominal_cleanup_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(EXECUTABLE_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
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
        callee,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = &call.kind
    else {
        panic!("cleanup operation must be an ordinary Unit call")
    };
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

    psi_terminal_verifier::verify_module(
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
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
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } = &operation.kind
        else {
            panic!("cleanup operation must be an ordinary Unit call")
        };
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

    psi_terminal_verifier::verify_module(
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
