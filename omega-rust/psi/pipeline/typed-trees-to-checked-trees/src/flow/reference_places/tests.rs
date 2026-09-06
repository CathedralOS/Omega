use super::*;
use facts::{NormalizedWriteFrame, PlaceSegment};

#[test]
fn operand_frames_must_preserve_both_binding_and_referent() {
    let source = "data Context { scheduler: u64; counter: u64; }
        machine observe(value: u64) -> u64 { value }
        machine probe(context: &mut Context) -> u64 {
            let mut borrowed: &Context = &context;
            transition { _ -> observe(borrowed.scheduler) }
        }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "probe")
        .unwrap();
    let typed_state = &program.machine_states(machine)[0];
    let statements = program
        .statement_table
        .statements(typed_state.statement_nodes);
    let StatementNode::LocalData(local) = &statements[0] else {
        panic!("reference declaration")
    };
    let context = program.state_parameters(typed_state)[0].symbol;
    let scheduler = program
        .data_definitions()
        .iter()
        .flat_map(|definition| program.data_members(definition))
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == "scheduler" => {
                Some(field.symbol)
            }
            _ => None,
        })
        .unwrap();
    let state = FlowStateFact {
        machine_symbol: machine.symbol,
        state_symbol: typed_state.symbol,
        ..Default::default()
    };
    let original = CanonicalPlace {
        root: PlaceRoot::Symbol(local.symbol),
        segments: vec![PlaceSegment::Field { symbol: scheduler }],
    };
    let referent = CanonicalPlace {
        root: PlaceRoot::Symbol(context),
        segments: original.segments.clone(),
    };
    let index = statements.len() - 1;
    let binding_write = NormalizedWriteFrame::complete(vec!["borrowed".to_owned()]);
    // The old referent alone is disjoint from replacing the local binding.
    assert_eq!(
        preserve_frame(
            &program,
            machine,
            &state,
            index,
            std::slice::from_ref(&referent),
            &binding_write
        ),
        Some(())
    );
    let places = [original, referent];
    for frame in [
        binding_write,
        NormalizedWriteFrame::complete(vec!["context.scheduler".to_owned()]),
        NormalizedWriteFrame::opaque(),
    ] {
        assert_eq!(
            preserve_frame(&program, machine, &state, index, &places, &frame),
            None
        );
    }
    assert_eq!(
        preserve_frame(
            &program,
            machine,
            &state,
            index,
            &places,
            &NormalizedWriteFrame::complete(vec!["context.counter".to_owned()])
        ),
        Some(())
    );
}
