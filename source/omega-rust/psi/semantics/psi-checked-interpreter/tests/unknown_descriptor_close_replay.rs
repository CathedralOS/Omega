use psi_checked_interpreter::{
    FilesystemAccess, FilesystemInputUnknownDescriptorCloseReplayRecord, FilesystemReplay,
    InterpretOptions, interpret_entry_with_options,
};
use psi_source::{SourceMap, SourceOrigin};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources;
use psi_tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};
use psi_typed_trees_to_checked_trees::lower_typed_trees;
use std::path::PathBuf;
use std::sync::Arc;

const FILESYSTEM_HOST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/std/filesystem_host.omg"
));

fn checked_unknown_descriptor_close() -> psi_checked_trees::CheckedTrees {
    const SOURCE: &str = r#"
data Main { filesystem: FilesystemHost; result: i32; }

machine Main::main(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.close(-1);
}
"#;
    let mut sources = SourceMap::default();
    let filesystem_host_source_id = sources
        .add_with_metadata(
            PathBuf::from("source/library/std/filesystem_host.omg"),
            FILESYSTEM_HOST.to_owned(),
            PathBuf::from("source/library/std"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("tests/unknown_descriptor_close.omg"),
            SOURCE.to_owned(),
            PathBuf::from("tests"),
            None,
            SourceOrigin::User,
        )
        .source_id;
    let filesystem_host_tokens = Lexer::new(FILESYSTEM_HOST)
        .tokenize()
        .expect("tokenize canonical filesystem host");
    let mut syntax = parse_syntax_trees_with_id(filesystem_host_source_id, &filesystem_host_tokens)
        .expect("parse canonical filesystem host");
    let tokens = Lexer::new(SOURCE)
        .tokenize()
        .expect("tokenize replay fixture");
    parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens).expect("parse replay fixture");
    let resolved = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("resolve replay fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type replay fixture");
    lower_typed_trees(typed).expect("check replay fixture")
}

#[test]
fn failed_unknown_descriptor_close_executes_virtually_and_tears_down_empty() {
    let replay = FilesystemReplay::from_input_unknown_descriptor_close_record(
        FilesystemInputUnknownDescriptorCloseReplayRecord::new(None),
    )
    .unwrap();

    let outcome = interpret_entry_with_options(
        &checked_unknown_descriptor_close(),
        "Main::main",
        &[],
        InterpretOptions {
            filesystem: FilesystemAccess::ReplayFilesystem(replay),
            ..InterpretOptions::default()
        },
    );

    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 0);
    assert!(outcome.stdout.is_empty());
    assert!(outcome.stderr.is_empty());
}
