use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{StructuralFieldType, StructuralMultiplicity, StructuralTypeShape, Terminator};
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
    data Token { value: u64; }
    machine Token::drop(&mut self) {}

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
    let Terminator::ReturnUnitNominalAffine { cleanup, .. } = &block.terminator else {
        panic!("expected executable nominal cleanup return")
    };
    assert_eq!(cleanup.place, root.place);
    assert_eq!(cleanup.structural_type, root.structural_type);

    let target = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == cleanup.cleanup_machine)
        .expect("cleanup target machine");
    assert_eq!(target.attachment, Some(cleanup.structural_type));
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
fn one_scalar_field_nominal_cleanup_crosses_source_lowering_codec_and_verifier() {
    let tokens = Lexer::new(SCALAR_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("one-scalar-field nominal cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let Terminator::ReturnUnitNominalAffine { cleanup, .. } = &entry.blocks[0].terminator else {
        panic!("expected executable nominal cleanup return")
    };
    let cleanup_type = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == cleanup.structural_type)
        .expect("cleanup structural type");
    let StructuralTypeShape::Record { fields } = &cleanup_type.shape else {
        panic!("cleanup type remains a record")
    };
    let [field] = fields.as_slice() else {
        panic!("bounded cleanup record retains one field")
    };
    assert_eq!(field.identity, "value");
    assert!(!field.relevance.is_erased());
    assert!(matches!(field.field_type, StructuralFieldType::Scalar(_)));

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier accepts one-scalar-field nominal cleanup closure");
    let bytes = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&bytes).expect("semantic module decodes"),
        lowered.semantic_module,
        "the primitive field and nominal cleanup identity are canonical artifact data"
    );
}
