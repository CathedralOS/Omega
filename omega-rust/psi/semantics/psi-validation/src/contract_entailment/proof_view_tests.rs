//! Ordinary proof-library names do not introduce compiler-owned term forms.

use super::*;
use crate::proven_machine_contract_expressions;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

fn parse(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn undeclared_proof_view_names_cannot_publish_reflexive_or_numeric_proofs() {
    for name in ["Bag", "Seq", "Range"] {
        for operator in ["==", ">=", "<="] {
            let program = parse(&format!(
                "machine theorem(items: &[u64]) ensures {name}(items) {operator} {name}(items) {{}}"
            ));
            assert_eq!(program.machines().len(), 1, "no authored view declaration");
            let expression = program
                .proof_facts
                .iter()
                .find_map(|(_, fact)| {
                    let ProofFact::Expression(expression) = fact else {
                        return None;
                    };
                    Some(*expression)
                })
                .expect("comparison claim");
            let ExpressionNode::Binary(comparison) =
                program.expression_table.expression(expression)
            else {
                panic!("binary claim");
            };
            let mut arithmetic = Engine::new(&program, &program.machines()[0]);
            assert!(
                arithmetic.normalize(comparison.left).is_none(),
                "{name} is not an integer atom"
            );
            assert!(matches!(
                arithmetic.judge(expression),
                Judgment::Unknown { .. }
            ));
            assert!(
                proven_machine_contract_expressions(&program, program.machines()[0].symbol)
                    .is_empty(),
                "undeclared {name} must not acquire a numeric or equality interpretation from {operator}"
            );
        }
    }
}

#[test]
fn undeclared_proof_view_names_cannot_publish_transported_hypotheses() {
    for name in ["Bag", "Seq", "Range"] {
        let program = parse(&format!(
            r#"
            machine theorem(items: &[u64], before: &[u64])
            requires {name}(items) == {name}(before)
            ensures {name}(items) == {name}(before)
            {{}}
            "#
        ));
        assert!(
            proven_machine_contract_expressions(&program, program.machines()[0].symbol).is_empty(),
            "{name} has no selected declaration to supply a proof term"
        );
    }
}

#[test]
fn ordinary_declared_machines_with_proof_view_names_use_their_actual_bodies() {
    for name in ["Bag", "Seq", "Range"] {
        for (right, expected) in [("value", true), ("other", false)] {
            let program = parse(&format!(
                r#"
                data ProofNat {{ case Zero; case Successor(previous: ProofNat); }}
                machine {name}(value: ProofNat) -> ProofNat {{ transition {{ _ -> value }} }}
                machine theorem(value: ProofNat, other: ProofNat)
                ensures {name}(value) == {right}
                {{}}
                "#
            ));
            let theorem = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "theorem")
                .expect("theorem");
            let outcomes = proven_machine_contract_expressions(&program, theorem.symbol);
            assert_eq!(
                !outcomes.is_empty(),
                expected,
                "ordinary {name} body must prove precisely its actual identity result, not {right} by spelling"
            );
        }
    }
}
