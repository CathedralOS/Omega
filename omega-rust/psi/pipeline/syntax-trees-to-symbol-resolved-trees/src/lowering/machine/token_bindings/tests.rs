//! Token bindings survive resolution and duplicate owner-local shapes reject.

use crate::{ResolutionRequest, resolve};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::operator_spelling::OperatorSpelling;
use tokens_to_syntax_trees::parse_syntax_trees;

fn resolve_source(source: &str) -> Result<SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    resolve(ResolutionRequest::new(&syntax))
}

fn machine_spellings(program: &SymbolResolvedTrees) -> Vec<(String, Option<OperatorSpelling>)> {
    program
        .machines
        .iter()
        .map(|machine| (machine.name.to_string(), machine.spelling))
        .collect()
}

#[test]
fn token_bearing_machines_retain_their_spelling_and_named_machines_have_none() {
    let program = resolve_source(
        "data Vec2 { x: u64; y: u64; }
         pub machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 { left }
         machine [] Vec2::at(items: Vec2, index: u64) -> u64 { index }
         boundary machine == Vec2::equal(left: Vec2, right: Vec2) -> bool;
         machine - subtract(left: u64, right: u64) -> u64 { left }
         machine Vec2::length(items: Vec2) -> u64 { 0u64 }
         machine ordinary(value: u64) -> u64 { value }",
    )
    .expect("token-bearing machines resolve like named machines");
    assert_eq!(
        machine_spellings(&program),
        [
            ("Vec2::add".to_owned(), Some(OperatorSpelling::Add)),
            ("Vec2::at".to_owned(), Some(OperatorSpelling::Index)),
            ("Vec2::equal".to_owned(), Some(OperatorSpelling::Equal)),
            ("subtract".to_owned(), Some(OperatorSpelling::Subtract)),
            ("Vec2::length".to_owned(), None),
            ("ordinary".to_owned(), None),
        ]
    );
    // The owner check compares settled identities: the attached owner and the
    // operand types carry symbols, never only their spelling.
    let add = program.machines.iter().next().expect("attached machine");
    assert!(add.attached_data_symbol.is_valid());
    let entry = program.machine_state(program.machine_state_handles(add.states)[0]);
    for parameter in program.state_parameters(entry.parameters) {
        let symbol_resolved_trees::types::TypeReference::Named { symbol, .. } =
            &parameter.type_reference
        else {
            panic!("named operand type");
        };
        assert!(symbol.is_valid(), "{}", parameter.name);
    }
    // A boundary requirement keeps its supply mode; the token adds no body.
    let equal = program.machines.iter().nth(2).expect("boundary machine");
    assert_eq!(
        equal.supply_mode,
        language_semantics::MachineSupplyMode::Boundary
    );
    assert!(!equal.body_is_present);
}

#[test]
fn same_owner_token_and_operand_shape_rejects_at_the_second_declaration() {
    let source = "data Vec2 { x: u64; y: u64; }
         machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 { left }
         machine + Vec2::plus(left: Vec2, right: Vec2) -> Vec2 { right }";
    let diagnostics = resolve_source(source).expect_err("duplicate binding rejects");
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("one diagnostic per duplicate: {diagnostics:?}");
    };
    assert!(
        diagnostic.message.contains(
            "`Vec2::plus` binds the fixed operator token `+` already bound by `Vec2::add`"
        ),
        "{}",
        diagnostic.message
    );
    let span = diagnostic
        .source_span
        .expect("reported at the second declaration");
    assert_eq!(
        &source[span.span.start..span.span.end],
        "Vec2::plus",
        "the diagnostic points at the second declaration's name"
    );
}

#[test]
fn free_machines_in_one_scope_share_the_owner_check() {
    let diagnostics = resolve_source(
        "machine + add(left: u64, right: u64) -> u64 { left }
         machine + plus(left: u64, right: u64) -> u64 { right }",
    )
    .expect_err("root-scope free machines share one owner");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("`plus` binds the fixed operator token `+`")
    );
}

#[test]
fn distinct_operand_shapes_may_share_a_token() {
    let program = resolve_source(
        "data Vec2 { x: u64; y: u64; }
         machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 { left }
         machine + Vec2::scale(left: Vec2, factor: u64) -> Vec2 { left }
         machine + Vec2::add_borrowed(left: &Vec2, right: &Vec2) -> u64 { 0u64 }
         machine + Vec2::add_mutable(left: &mut Vec2, right: &Vec2) -> u64 { 0u64 }",
    )
    .expect("overloads by operand shape are not duplicates");
    assert_eq!(program.machines.len(), 4);
}

#[test]
fn distinct_tokens_and_distinct_owners_are_not_duplicates() {
    let program = resolve_source(
        "data Vec2 { x: u64; y: u64; }
         data Vec3 { x: u64; y: u64; z: u64; }
         machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 { left }
         machine - Vec2::subtract(left: Vec2, right: Vec2) -> Vec2 { left }
         machine + Vec3::add(left: Vec3, right: Vec3) -> Vec3 { left }
         machine + add(left: Vec2, right: Vec2) -> Vec2 { left }",
    )
    .expect("a different token, owner, or free scope is a separate binding");
    assert_eq!(program.machines.len(), 4);
}

#[test]
fn a_named_sibling_never_collides_with_a_token_binding() {
    let program = resolve_source(
        "data Vec2 { x: u64; y: u64; }
         machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 { left }
         machine Vec2::plus(left: Vec2, right: Vec2) -> Vec2 { right }",
    )
    .expect("a tokenless machine binds no token");
    assert_eq!(
        machine_spellings(&program),
        [
            ("Vec2::add".to_owned(), Some(OperatorSpelling::Add)),
            ("Vec2::plus".to_owned(), None),
        ]
    );
}
