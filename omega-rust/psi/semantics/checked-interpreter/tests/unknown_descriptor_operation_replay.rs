use checked_interpreter::{
    FilesystemAccess, FilesystemInputUnknownDescriptorOperationReplayKind as Kind,
    FilesystemInputUnknownDescriptorOperationReplayRecord as Record,
    FilesystemInputUnknownDescriptorOperationWithErrnoReplayRecord as PairRecord, FilesystemReplay,
    InterpretOptions, interpret_entry_with_options,
};
use source::{SourceMap, SourceOrigin};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources;
use tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};
use typed_trees_to_checked_trees::lower_typed_trees;

const FILESYSTEM_HOST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../source/library/std/filesystem_host.omg"
));

fn checked_unknown_descriptor_operations() -> checked_trees::CheckedTrees {
    const SOURCE: &str = r#"
data Main { filesystem: FilesystemHost; result: i32; }

machine Main::close_unknown(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.close(-1);
}

machine Main::close_unknown_errno(&mut self) -> i32
reaches FilesystemHost
{
    self.result = self.filesystem.close(-1);
    let error: i32 = self.filesystem.errno();
    transition { _ -> (error) }
}

machine Main::sync_unknown(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.sync(-1);
}

machine Main::sync_unknown_errno(&mut self) -> i32
reaches FilesystemHost
{
    self.result = self.filesystem.sync(-1);
    let error: i32 = self.filesystem.errno();
    transition { _ -> (error) }
}

machine Main::sync_data_unknown(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.sync_data(-1);
}

machine Main::sync_data_unknown_errno(&mut self) -> i32
reaches FilesystemHost
{
    self.result = self.filesystem.sync_data(-1);
    let error: i32 = self.filesystem.errno();
    transition { _ -> (error) }
}

machine Main::duplicate_unknown(&mut self)
reaches FilesystemHost
{
    self.result = self.filesystem.duplicate(-1);
}

machine Main::duplicate_unknown_errno(&mut self) -> i32
reaches FilesystemHost
{
    self.result = self.filesystem.duplicate(-1);
    let error: i32 = self.filesystem.errno();
    transition { _ -> (error) }
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
            PathBuf::from("tests/unknown_descriptor_operations.omg"),
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
fn operand_free_unknown_descriptor_errno_pairs_execute_without_a_provider() {
    let checked = checked_unknown_descriptor_operations();
    let cases = [
        (Kind::Close, "Main::close_unknown_errno", 8),
        (Kind::Sync, "Main::sync_unknown_errno", 43),
        (Kind::SyncData, "Main::sync_data_unknown_errno", 44),
        (Kind::Duplicate, "Main::duplicate_unknown_errno", 45),
    ];

    for (kind, entry, operation_tag) in cases {
        let replay = FilesystemReplay::from_input_unknown_descriptor_operation_with_errno_record(
            PairRecord::new(Record::new(None, kind)),
        )
        .unwrap();
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(checked_interpreter::FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![operation_tag, 50]
        );

        let outcome = interpret_entry_with_options(
            &checked,
            entry,
            &[],
            InterpretOptions {
                filesystem: FilesystemAccess::ReplayFilesystem(replay),
                ..InterpretOptions::default()
            },
        );

        assert_eq!(outcome.error, None, "{entry}");
        assert_eq!(outcome.exit_code, 9, "{entry}");
        assert!(outcome.stdout.is_empty(), "{entry}");
        assert!(outcome.stderr.is_empty(), "{entry}");
    }
}

#[test]
fn unknown_descriptor_operation_family_executes_virtually_and_tears_down_empty() {
    let checked = checked_unknown_descriptor_operations();
    let cases = [
        (Kind::Close, "Main::close_unknown", 8),
        (Kind::Sync, "Main::sync_unknown", 43),
        (Kind::SyncData, "Main::sync_data_unknown", 44),
        (Kind::Duplicate, "Main::duplicate_unknown", 45),
    ];

    for (kind, entry, operation_tag) in cases {
        let replay = FilesystemReplay::from_input_unknown_descriptor_operation_record(Record::new(
            None, kind,
        ))
        .unwrap();
        assert_eq!(replay.attempts()[0].operation_tag(), operation_tag);

        let outcome = interpret_entry_with_options(
            &checked,
            entry,
            &[],
            InterpretOptions {
                filesystem: FilesystemAccess::ReplayFilesystem(replay),
                ..InterpretOptions::default()
            },
        );

        assert_eq!(outcome.error, None, "{entry}");
        assert_eq!(outcome.exit_code, 0, "{entry}");
        assert!(outcome.stdout.is_empty(), "{entry}");
        assert!(outcome.stderr.is_empty(), "{entry}");
    }
}
