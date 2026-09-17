use super::{TableCall, TypedTrees, is_wire_codec_call, known_wire_codec_call_written_paths};
use typed_trees::statement::StatementNode;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("symbols");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("types")
}

const SCHEMA_PRELUDE: &str = "data PointMsg { #0 x: u32; #1 y: u32; } \
    data PointSample { x: u32; y: u32; } \
    data WireVerdict { case Invalid; case Sound; } \
    data Main { buffer: [u8; 32]; written: u64; read: u64; verdict: WireVerdict; }";

fn main_state_frame(body: &str) -> Option<Vec<String>> {
    let program = typed(&format!(
        "{SCHEMA_PRELUDE} machine Main::main(&mut self) {{ {body} }}"
    ));
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main");
    let state = &program.machine_states(machine)[0];
    let resolver = crate::CallFrameResolver::new(&program).expect("frame resolver");
    resolver
        .inferred_state_write_frame(machine, state)
        .complete_paths()
        .map(<[String]>::to_vec)
}

fn first_call(program: &TypedTrees) -> &TableCall {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main");
    let state = &program.machine_states(machine)[0];
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::Call(call) => Some(call),
            _ => None,
        })
        .expect("call statement")
}

#[test]
fn encode_call_frame_names_exactly_its_exclusively_borrowed_places() {
    assert_eq!(
        main_state_frame(
            "let q: PointSample; q.x = 3; q.y = 4; \
             PointMsg::encode(&q, &mut self.buffer, &mut self.written);"
        ),
        Some(vec!["self.buffer".to_owned(), "self.written".to_owned()]),
    );
}

#[test]
fn decode_call_frame_keeps_the_local_destination_out_of_the_state_frame() {
    assert_eq!(
        main_state_frame(
            "let d: PointSample; d.x = 0; d.y = 0; \
             PointMsg::decode(&mut d, &self.buffer, &mut self.read, &mut self.verdict);"
        ),
        Some(vec!["self.read".to_owned(), "self.verdict".to_owned()]),
    );
}

#[test]
fn all_shared_argument_codec_call_contributes_nothing() {
    assert_eq!(
        main_state_frame(
            "let q: PointSample; q.x = 3; q.y = 4; \
             PointMsg::encode(&q, &self.buffer, &self.written);"
        ),
        Some(Vec::new()),
    );
}

#[test]
fn codec_call_with_an_unborrowed_argument_stays_opaque() {
    let program = typed(&format!(
        "{SCHEMA_PRELUDE} machine Main::main(&mut self) {{ \
         let q: PointSample; q.x = 3; q.y = 4; \
         PointMsg::encode(q, &mut self.buffer, &mut self.written); }}"
    ));
    let call = first_call(&program);
    assert!(is_wire_codec_call(&program, call));
    assert_eq!(known_wire_codec_call_written_paths(&program, call), None);
}

#[test]
fn a_user_machine_call_is_not_a_wire_codec_call() {
    let program = typed(
        "data Main { count: u64; } \
         machine Main::main(&mut self) { self.bump(); } \
         machine Main::bump(&mut self) { self.count = 1; }",
    );
    let call = first_call(&program);
    assert!(!is_wire_codec_call(&program, call));
}
