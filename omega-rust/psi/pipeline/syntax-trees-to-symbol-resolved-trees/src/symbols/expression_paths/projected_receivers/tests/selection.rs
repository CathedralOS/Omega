use super::*;

fn resolve(source: &str) -> symbol_resolved_trees::SymbolResolvedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize receiver");
    let syntax = parse_syntax_trees(&tokens).expect("parse receiver");
    lower_syntax_trees(&syntax).expect("resolve receiver")
}

fn read_target(program: &symbol_resolved_trees::SymbolResolvedTrees) -> SymbolHandle {
    let calls = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, node)| match node {
            ExpressionNode::Call(call) if call.target.as_str() == "read" => {
                Some(call.target_symbol)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!calls.is_empty(), "source must retain its method call");
    assert!(
        calls
            .iter()
            .all(|target| target.is_valid() && *target == calls[0]),
        "method targets: {calls:?}"
    );
    calls[0]
}

#[test]
fn self_payload_does_not_inherit_a_same_named_state_symbol() {
    let program = resolve(
        "data Cell {} data Tree { case Branch(right: Cell); }
        machine Cell::read(self) -> u64 { 1 }
        machine Tree::right(self) -> u64 {
            transition self { Tree::Branch { right } -> right.read() }
        }",
    );
    let target = read_target(&program);
    let declaration = program.symbols.get(target).parent;
    assert_eq!(program.symbols.name(declaration), "Cell::read");
}

#[test]
fn payload_and_index_projections_compose_in_both_orders() {
    for source in [
        "data Cell {} data Bucket { cell: Cell; }
         data Choice { case Item(values: [Bucket; 2]); }
         machine Cell::read(self) -> u64 { 1 }
         machine select(value: Choice, index: u64) -> u64 {
             transition value { Choice::Item { values } -> (values[index].cell.read()) }
         }",
        "data Cell {} data Choice { case Item(cell: Cell); }
         machine Cell::read(self) -> u64 { 1 }
         machine select(values: [Choice; 2], index: u64) -> u64 {
             transition values[index] { Choice::Item { cell } -> cell.read() }
         }",
    ] {
        let program = resolve(source);
        let declaration = program.symbols.get(read_target(&program)).parent;
        assert_eq!(program.symbols.name(declaration), "Cell::read");
    }
}

#[test]
fn inherited_payload_selects_by_nominal_owner_before_source_preference() {
    use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
    use std::{path::PathBuf, sync::Arc};
    use tokens_to_syntax_trees::parse_syntax_trees_with_id;

    for shadows_owner in [true, false] {
        let base = format!(
            "data Cell {{}} data Choice {{ case Item(cell: Cell); }} {}",
            if shadows_owner {
                "machine Cell::read(self) -> u64 { 1 }"
            } else {
                ""
            }
        );
        let extension = format!(
            "{}
            machine Cell::read(self) -> u64 {{ 2 }}
            machine select(value: Choice) -> u64 {{
                transition value {{ Choice::Item {{ cell }} -> cell.read() }}
            }}",
            if shadows_owner { "data Cell {}" } else { "" }
        );
        let mut sources = SourceMap::default();
        let base_id = sources
            .add(PathBuf::from("base.omg"), base.clone())
            .source_id;
        let extension_id = sources
            .add_with_metadata_and_resolution_stratum(
                PathBuf::from("extension.omg"),
                extension.clone(),
                PathBuf::from("."),
                None,
                SourceOrigin::User,
                SourceResolutionStratum::CurrentActivationExtension,
            )
            .source_id;
        let mut syntax = parse_syntax_trees_with_id(
            extension_id,
            &Lexer::new(&extension).tokenize().expect("extension tokens"),
        )
        .expect("extension syntax");
        syntax.extend_from(
            &parse_syntax_trees_with_id(
                base_id,
                &Lexer::new(&base).tokenize().expect("base tokens"),
            )
            .expect("base syntax"),
        );
        let program = crate::lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
            .expect("resolve sources");
        let selected = read_target(&program);
        assert_eq!(
            program
                .symbols
                .symbol_provenance_source_span(selected)
                .unwrap()
                .source_id,
            if shadows_owner { base_id } else { extension_id }
        );
    }
}
