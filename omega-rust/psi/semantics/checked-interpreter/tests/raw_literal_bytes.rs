use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::interpret_entry;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn interpreter_writes_non_utf8_literal_bytes_exactly() {
    let tokens = Lexer::new(
        r#"
        boundary trait Console {
            machine write_line(text: &[u8]);
        }

        data Main<'s> { console: &'s mut Console; }

        machine Main::main(&mut self) reaches Console {
            self.console.write_line("\x80A");
        }
        "#,
    )
    .tokenize()
    .expect("tokenize raw bytes");
    let syntax = parse_syntax_trees(&tokens).expect("parse raw bytes");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve raw bytes");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type raw bytes");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check raw bytes");

    let outcome = interpret_entry(
        &checked,
        BuildMachineEntry::Name("Main::main"),
        &[],
        InterpretOptions::default(),
    );

    assert_eq!(outcome.error, None);
    assert_eq!(outcome.stdout, [0x80, b'A', b'\n']);
}
