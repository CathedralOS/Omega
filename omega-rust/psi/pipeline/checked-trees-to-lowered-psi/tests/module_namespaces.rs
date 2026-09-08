use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source::{SourceId, SourceMap};
use source_files_to_tokens::Lexer;
use std::{path::PathBuf, sync::Arc};
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees::SyntaxTrees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn qualified_same_leaf_machines_publish_independently_executable_artifacts() {
    let mut sources = SourceMap::default();
    let mut syntax = SyntaxTrees::new(SourceId::default());
    for (path, source) in [
        (
            "combat.omg",
            "module dungeon::combat; machine value() -> u32\nrequires 7u32 == 7u32\nensures 7u32 == 7u32\n{ 7 }",
        ),
        (
            "rooms.omg",
            "module dungeon::rooms; machine value() -> u32\nrequires 9u32 == 9u32\nensures 9u32 == 9u32\n{ 9 }",
        ),
    ] {
        let source_id = sources
            .add(PathBuf::from(path), source.to_owned())
            .source_id;
        let tokens = Lexer::new(source).tokenize().expect("tokenize module");
        parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens).expect("parse module");
    }
    let resolved =
        lower_syntax_trees_with_sources(&syntax, Arc::new(sources)).expect("resolve modules");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type modules");
    let checked = lower_typed_trees(typed).expect("check modules");
    let mut artifacts = Vec::new();
    for (qualified, expected) in [
        ("dungeon::combat::value", 7u128),
        ("dungeon::rooms::value", 9u128),
    ] {
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, qualified)
            .expect("select qualified module machine");
        artifacts.push((
            encode_module(&lowered.semantic_module).expect("encode semantics"),
            encode_proof_bundle(&lowered.proof_bundle).expect("encode proof"),
            expected,
        ));
    }
    drop(checked);
    drop(resolved);
    drop(syntax);
    for (semantics, proof, expected) in artifacts {
        // Independent decoding and verification precede execution; no source
        // or checked producer state crosses the artifact boundary.
        let result =
            interpret_terminal_artifact(&semantics, &proof, &AdmissionProfile::default(), &[])
                .expect("verify and execute published artifact");
        assert_eq!(
            result,
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"),
                value: IntegerValue::Unsigned(expected),
            })
        );
    }
}
