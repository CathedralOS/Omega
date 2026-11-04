//! An authored token binding whose operands name no declared type or domain
//! joins a primitive operator family, which only the core semantic-home owner
//! publishes. The spelled path is not a home: use sites select by operand
//! types alone. Every introducer answers the same rule.

use super::{ResolutionRequest, resolve};
use source_files_to_tokens::Lexer;

const PRIMITIVE_FAMILY: &str = "joins a primitive operator family";

fn diagnostics(source_text: &str) -> Vec<String> {
    let tokens = Lexer::new(source_text).tokenize().unwrap();
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add("primitive-family/main.omg".into(), source_text.to_owned())
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    match resolve(ResolutionRequest {
        syntax,
        sources: Some(std::sync::Arc::new(sources)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    }) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect(),
    }
}

fn rejects_as_primitive_family(source_text: &str) -> bool {
    diagnostics(source_text)
        .iter()
        .any(|message| message.contains(PRIMITIVE_FAMILY))
}

#[test]
fn every_introducer_over_primitive_operands_joins_the_primitive_family() {
    for declaration in [
        "boundary operator - Mine::replacement(left: u32, right: u32) -> u32;",
        "boundary operator - u32::replacement(left: u32, right: u32) -> u32;",
        "boundary machine == Comparison::equal(left: i32, right: i32) -> bool;",
        "boundary machine == Float::equal(left: f64, right: f64) -> bool;",
        "operator + Mine::add(left: i32, right: i32) -> i32;",
        "machine + add(left: u8, right: u8) -> u8 { left }",
    ] {
        assert!(rejects_as_primitive_family(declaration), "{declaration}");
    }
}

#[test]
fn a_compiler_policy_qualifier_is_not_a_declared_home() {
    assert!(rejects_as_primitive_family(
        "boundary operator - Mine::replacement(left: u32 in Wrapping, right: u32 in Wrapping) -> u32 in Wrapping;"
    ));
}

#[test]
fn a_declared_data_or_domain_operand_is_a_home_whatever_the_path() {
    for source_text in [
        "data Meter { value: u32; }\nboundary machine + Float::add(left: Meter, right: Meter) -> Meter;",
        "domain u32::Mine;\nboundary machine - Comparison::subtract(left: u32 in Mine, right: u32 in Mine) -> u32;",
    ] {
        assert_eq!(
            diagnostics(source_text),
            Vec::<String>::new(),
            "{source_text}"
        );
    }
}
