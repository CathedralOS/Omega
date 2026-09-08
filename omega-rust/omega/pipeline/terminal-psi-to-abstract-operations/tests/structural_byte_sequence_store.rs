//! Verified byte-field writes must not disappear at the native boundary.

use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_psi::OperationKind;
use terminal_psi_to_abstract_operations::{
    ArtifactLoweringError, LoweringError, lower_artifact_sections,
    lower_artifact_sections_for_native_realization, lower_artifact_sections_for_optimization,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn verified_mutable_byte_view_write_rejects_unrealized_state_bindings() {
    let source = r#"
        machine put(out: &mut [u8], byte: u8) {
            transition out.len > 0 {
                true -> store(out, byte)
                false -> done()
            }
            state store(out: &mut [u8], byte: u8) { out[0] = byte; }
            state done() {}
        }
    "#;
    for (entry, source) in [
        ("put", source.to_owned()),
        (
            "run",
            format!("{source}\n machine run(out: &mut [u8;3]) {{ put(out,65); put(out,0); }}"),
        ),
    ] {
        let tokens = Lexer::new(&source).tokenize().expect("tokenize source");
        let syntax = parse_syntax_trees(&tokens).expect("parse source");
        let resolved = lower_syntax_trees(&syntax).expect("resolve source");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
        let checked = lower_typed_trees(typed).expect("check source");
        let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, entry)
            .expect("guarded write lowers to Terminal");
        let writer = terminal
            .semantic_module
            .machines
            .iter()
            .find(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| {
                        matches!(operation.kind, OperationKind::ByteSequenceWrite { .. })
                    })
            })
            .expect("retained writer")
            .id;
        let semantic_bytes = encode_module(&terminal.semantic_module).expect("encode semantics");
        let proof_bytes = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
        let profile = AdmissionProfile::default();
        terminal_verifier::verify_module(
            &terminal.semantic_module,
            &terminal.proof_bundle,
            &profile,
        )
        .expect("write has independently verified bounds");
        for result in [
            lower_artifact_sections(&semantic_bytes, &proof_bytes, &profile).map(|_| ()),
            lower_artifact_sections_for_optimization(&semantic_bytes, &proof_bytes, &profile)
                .map(|_| ()),
            lower_artifact_sections_for_native_realization(&semantic_bytes, &proof_bytes, &profile)
                .map(|_| ()),
        ] {
            // This source reaches the earlier state-binding fence, before the
            // separate per-operation ByteSequenceWrite realization fence.
            assert!(
                matches!(
                    result,
                    Err(ArtifactLoweringError::Lowering(
                        LoweringError::UnsupportedStructuralSuccessorArguments { machine, .. }
                    )) if machine == writer
                ),
                "a verified mutable view cannot lose its write during projection: {result:?}"
            );
        }
    }
}

#[test]
fn verified_bounded_byte_field_replacement_rejects_before_native_projection() {
    for literal in ["XXX", "X", ""] {
        let source = format!(
            r#"
            domain [u8; 3]::Utf8 requires valid_utf8(self);
            data Record {{ out: [u8; 3] in Utf8; }}
            machine Record::replace(&mut self) {{ self.out = "{literal}"; }}
            "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize source");
        let syntax = parse_syntax_trees(&tokens).expect("parse source");
        let resolved = lower_syntax_trees(&syntax).expect("resolve source");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
        let checked = lower_typed_trees(typed).expect("check source");
        let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, "Record::replace")
            .expect("bounded byte-field replacement lowers to Terminal");
        let semantic_bytes = encode_module(&terminal.semantic_module).expect("encode semantics");
        let proof_bytes = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
        let module = decode_module(&semantic_bytes).expect("decode canonical semantics");
        let proof = decode_proof_bundle(&proof_bytes).expect("decode canonical proof");
        let profile = AdmissionProfile::default();
        terminal_verifier::verify_module(&module, &proof, &profile)
            .expect("canonical replacement is valid before Omega rejection");
        let stores = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::StructuralByteSequenceFieldStore { .. }
                )
            })
            .map(|operation| operation.id)
            .collect::<Vec<_>>();
        let [store] = stores.as_slice() else {
            panic!("source must retain exactly one byte-field replacement")
        };
        for result in [
            lower_artifact_sections(&semantic_bytes, &proof_bytes, &profile).map(|_| ()),
            lower_artifact_sections_for_optimization(&semantic_bytes, &proof_bytes, &profile)
                .map(|_| ()),
            lower_artifact_sections_for_native_realization(&semantic_bytes, &proof_bytes, &profile)
                .map(|_| ()),
        ] {
            assert!(matches!(
                result,
                Err(ArtifactLoweringError::Lowering(
                    LoweringError::UnsupportedStructuralByteSequenceFieldStore(operation)
                )) if operation == *store
            ));
        }
    }
}
