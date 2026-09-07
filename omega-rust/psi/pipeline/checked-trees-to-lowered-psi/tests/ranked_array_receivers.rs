//! Checked ranked receivers retain primitive-array types through canonical Psi.

use checked_trees::CheckedStructuralAccess;
use semantic_vocabulary::{
    IeeeFloatFormat, IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralPlaceKind,
    StructuralTypeId,
};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_psi::{
    ProofBundle, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralPlaceDeclaration, StructuralTypeShape, TerminalModule,
    TerminalRankedSuccessorArgument, Terminator,
};
use terminal_verifier::{ModuleError, VerificationError};
use tokens_to_syntax_trees::parse_syntax_trees;

const SOURCE: &str = r#"
    data Root { values: [u64; 3]; }

    machine Root::countdown(&mut self, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(remaining - 1)
            _ -> done()
        }
        state done(&mut self) {}
    }
"#;

fn canonical_countdown(field_type: &str) -> (TerminalModule, ProofBundle) {
    let source = SOURCE.replace("[u64; 3]", field_type);
    let tokens = Lexer::new(&source).tokenize().expect("tokenize countdown");
    let syntax = parse_syntax_trees(&tokens).expect("parse countdown");
    let resolved = lower_syntax_trees(&syntax).expect("resolve countdown");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type countdown");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("ranked primitive-array receiver checks");
    let plan = checked
        .facts
        .flow
        .terminal_structural_unit_controls
        .machines
        .iter()
        .find(|plan| plan.ranked_scc.is_some())
        .expect("source retains a checked ranked component");
    let header = plan
        .states
        .iter()
        .find(|state| state.state == plan.ranked_scc.as_ref().unwrap().header_state)
        .expect("checked ranked header");
    let [receiver] = header.structural_parameters.as_slice() else {
        panic!("one checked receiver")
    };
    assert!(receiver.is_self);
    assert_eq!(receiver.position, 0);
    assert_eq!(receiver.access, CheckedStructuralAccess::MutableBorrow);

    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::countdown")
        .expect("ranked lowering exports primitive array element declarations");
    drop(checked);
    let semantic_bytes =
        terminal_codec::encode_module(&lowered.semantic_module).expect("encode ranked semantics");
    let proof_bytes =
        terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("encode ranked proof");
    let module = terminal_codec::decode_module(&semantic_bytes).expect("decode ranked semantics");
    let proof = terminal_codec::decode_proof_bundle(&proof_bytes).expect("decode ranked proof");
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        semantic_bytes
    );
    assert_eq!(
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
        proof_bytes
    );
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);

    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module_for_interpretation(&module, &proof, &profile)
        .expect("canonical ranked receiver independently verifies");
    assert!(matches!(
        terminal_verifier::verify_module(&module, &proof, &profile),
        Err(VerificationError::Module(ModuleError::NonExecutableRankedScc(machine)))
            if machine == module.entry
    ));

    let [machine] = module.machines.as_slice() else {
        panic!("one ranked entry machine")
    };
    assert_eq!(machine.id, module.entry);
    let rank = machine
        .ranked_scc
        .as_ref()
        .expect("retained countdown rank");
    assert_eq!(
        rank.rank_type,
        IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
    );
    assert_eq!(rank.lower_bound, IntegerValue::Unsigned(0));
    assert_eq!(
        rank.upper_bound,
        IntegerValue::Unsigned(u128::from(u32::MAX))
    );
    assert!(!rank.covered_cyclic_edges.is_empty());
    let [receiver] = machine.structural_parameters.as_slice() else {
        panic!("one canonical structural receiver")
    };
    assert!(receiver.is_self);
    assert_eq!(receiver.position, 0);
    assert_eq!(receiver.access, StructuralAccess::MutableBorrow);
    assert_eq!(receiver.multiplicity, StructuralMultiplicity::Unrestricted);
    assert!(receiver.qualifications.is_empty());
    assert!(receiver.projected_qualifications.is_empty());
    assert!(matches!(
        machine.structural_places.as_slice(),
        [StructuralPlaceDeclaration {
            id,
            kind: StructuralPlaceKind::Parameter { position: 0, is_self: true },
        }] if *id == receiver.place
    ));
    let exits = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => Some(trivial_affine_discards),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [discards] = exits.as_slice() else {
        panic!("one Unit exit")
    };
    assert!(
        discards.is_empty(),
        "a borrowed receiver creates no owned cleanup"
    );
    (module, proof)
}

fn assert_array_shape(module: &TerminalModule, lengths: &[u64], leaf: ScalarType) {
    let receiver = &module.machines[0].structural_parameters[0];
    let root = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == receiver.structural_type)
        .expect("declared receiver root");
    let StructuralTypeShape::Record { fields } = &root.shape else {
        panic!("authored Root is a record")
    };
    let [field] = fields.as_slice() else {
        panic!("Root retains exactly its values field")
    };
    assert_eq!(field.identity, "values");
    let StructuralFieldType::Structural(mut element) = field.field_type else {
        panic!("values references a fixed array")
    };
    for expected_length in lengths {
        let declaration = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == element)
            .expect("declared array at each nesting level");
        let StructuralTypeShape::FixedArray {
            element: nested,
            length,
        } = declaration.shape
        else {
            panic!("array nesting retains its exact shape")
        };
        assert_eq!(length, *expected_length);
        element = nested;
    }
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == element)
        .expect("declared primitive leaf");
    assert_eq!(
        declaration.shape,
        StructuralTypeShape::PrimitiveScalar(leaf),
        "the primitive leaf must not become a synthetic record"
    );
    assert_eq!(
        module.structural_types.len(),
        lengths.len() + 2,
        "only Root, each fixed array, and its primitive leaf are retained"
    );
}

#[test]
fn ranked_integer_array_receiver_retains_exact_length_and_leaf() {
    let (module, _) = canonical_countdown("[u64; 3]");
    assert_array_shape(
        &module,
        &[3],
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
    );
}

#[test]
fn ranked_boolean_array_receiver_retains_boolean_leaf() {
    let (module, _) = canonical_countdown("[bool; 5]");
    assert_array_shape(&module, &[5], ScalarType::Boolean);
}

#[test]
fn ranked_float_array_receivers_retain_ieee_formats() {
    for (field_type, format) in [
        ("[f32; 3]", IeeeFloatFormat::Binary32),
        ("[f64; 3]", IeeeFloatFormat::Binary64),
    ] {
        let (module, _) = canonical_countdown(field_type);
        assert_array_shape(&module, &[3], ScalarType::IeeeFloat(format));
    }
}

#[test]
fn ranked_nested_array_receiver_retains_both_lengths_and_primitive_leaf() {
    let (module, _) = canonical_countdown("[[u16; 3]; 2]");
    assert_array_shape(
        &module,
        &[2, 3],
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap()),
    );
}

#[test]
fn ranked_array_receiver_rejects_missing_primitive_type_declaration() {
    let (mut module, proof) = canonical_countdown("[u64; 3]");
    let leaf = module
        .structural_types
        .iter()
        .find(|declaration| matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_)))
        .expect("primitive leaf")
        .id;
    module
        .structural_types
        .retain(|declaration| declaration.id != leaf);
    assert!(matches!(
        terminal_verifier::verify_module_for_interpretation(
            &module, &proof, &proof_admission::AdmissionProfile::default(),
        ),
        Err(VerificationError::Module(ModuleError::UnknownStructuralType(missing))) if missing == leaf
    ));
}

#[test]
fn ranked_array_receiver_rejects_dangling_array_element_reference() {
    let (mut module, proof) = canonical_countdown("[u64; 3]");
    let missing = StructuralTypeId::new(u64::MAX).unwrap();
    assert!(
        module
            .structural_types
            .iter()
            .all(|declaration| declaration.id != missing)
    );
    let array = module
        .structural_types
        .iter_mut()
        .find(|declaration| matches!(declaration.shape, StructuralTypeShape::FixedArray { .. }))
        .expect("fixed array");
    let StructuralTypeShape::FixedArray { element, .. } = &mut array.shape else {
        unreachable!()
    };
    *element = missing;
    assert!(matches!(
        terminal_verifier::verify_module_for_interpretation(
            &module, &proof, &proof_admission::AdmissionProfile::default(),
        ),
        Err(VerificationError::Module(ModuleError::UnknownStructuralType(actual))) if actual == missing
    ));
}

#[test]
fn ranked_array_receiver_rejects_altered_rank_successor_argument() {
    let (mut module, proof) = canonical_countdown("[u64; 3]");
    let rank = module.machines[0]
        .ranked_scc
        .as_mut()
        .expect("ranked component");
    let TerminalRankedSuccessorArgument::UnsignedParameterMinusOne { argument_index, .. } =
        &mut rank.covered_cyclic_edges[0].successor_argument;
    assert_eq!(*argument_index, 0);
    *argument_index = 1;
    assert!(
        terminal_verifier::verify_module_for_interpretation(
            &module,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err()
    );
}

#[test]
fn ranked_array_receiver_rejects_missing_decrease_evidence() {
    let (module, mut proof) = canonical_countdown("[u64; 3]");
    assert!(
        !proof.evidence.is_empty(),
        "checked countdown supplies proof evidence"
    );
    proof.evidence.clear();
    assert!(matches!(
        terminal_verifier::verify_module_for_interpretation(
            &module,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        ),
        Err(VerificationError::MissingEvidence(_))
    ));
}
