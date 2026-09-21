//! A direct call to a public nongeneric receiver-free `F32::`/`F64::`
//! top-level boundary requirement retains its arguments' arithmetic policy
//! exactly as a named float operator use does; an ordinary machine still
//! rejects the implicit weakening.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;

fn typed(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("symbols");
    lower_symbol_resolved_trees(&resolved).expect("types")
}

const REQUIREMENT: &str = r#"
pub data F32 {}
pub boundary requirement F32::multiply_then_add(left: f32, right: f32, addend: f32) -> f32;
data Main { scaled: f32 in Saturating; }
machine Main::run(&mut self) -> f32 {
    let product: f32 = F32::multiply_then_add(self.scaled, 2.0f32, 1.0f32);
    transition { _ -> (product) }
}
"#;

const OPERATOR: &str = r#"
pub data F32 {}
pub boundary operator F32::multiply_then_add(left: f32, right: f32, addend: f32) -> f32;
data Main { scaled: f32 in Saturating; }
machine Main::run(&mut self) -> f32 {
    let product: f32 = F32::multiply_then_add(self.scaled, 2.0f32, 1.0f32);
    transition { _ -> (product) }
}
"#;

const ORDINARY_MACHINE: &str = r#"
pub data Scale {}
pub machine Scale::multiply_then_add(left: f32, right: f32, addend: f32) -> f32 {
    transition { _ -> (left * right + addend) }
}
data Main { scaled: f32 in Saturating; }
machine Main::run(&mut self) -> f32 {
    let product: f32 = Scale::multiply_then_add(self.scaled, 2.0f32, 1.0f32);
    transition { _ -> (product) }
}
"#;

#[test]
fn a_saturating_argument_keeps_its_policy_at_a_float_requirement_call() {
    for (label, source) in [("requirement", REQUIREMENT), ("operator", OPERATOR)] {
        let program = typed(source);
        let diagnostics = validation::validate_program(&program)
            .err()
            .unwrap_or_default();
        assert!(
            !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("implicit domain weakening")),
            "{label}: a policy-qualified operand of a named float requirement is adapted, not weakened: {diagnostics:#?}"
        );
    }
}

#[test]
fn an_ordinary_machine_call_still_weakens_a_saturating_argument() {
    let program = typed(ORDINARY_MACHINE);
    let diagnostics = validation::validate_program(&program)
        .err()
        .unwrap_or_default();
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("implicit domain weakening in argument `left`")
                && diagnostic.message.contains("Saturating")
        }),
        "an ordinary machine parameter still rejects the implicit policy drop: {diagnostics:#?}"
    );
}
