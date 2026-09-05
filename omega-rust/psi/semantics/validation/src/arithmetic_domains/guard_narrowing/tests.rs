use super::*;
use typed_trees::statement::StatementNode;

fn program_with_guard(condition: &str) -> TypedTrees {
    let source = format!(
        "machine value(remaining: u32) -> u32 {{
             transition {condition} {{ true -> 1u32 false -> 0u32 }}
         }}"
    );
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn nested_boolean_guard_wrappers_preserve_integer_bound_polarity() {
    for (condition, positive_rank) in [
        ("remaining > 0", true),
        ("!(remaining > 0)", false),
        ("!!(remaining > 0)", true),
        ("(!(remaining > 0)) == false", true),
        ("false == (!(remaining > 0))", true),
        ("(!(remaining > 0)) != true", true),
        ("true != (!(remaining > 0))", true),
        ("!((remaining > 0) != false)", false),
    ] {
        let program = program_with_guard(condition);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let StatementNode::Transition(transition) =
            &program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("authored transition");
        };
        let selected = guard_narrowed_env(
            &program,
            machine,
            Some(state),
            &transition.guard,
            &ValueEnv::new(),
        );
        let fallback = fall_through_narrowed_env(
            &program,
            machine,
            Some(state),
            &transition.guard,
            &ValueEnv::new(),
        );
        let positive = Interval {
            low: Some(1),
            high: Some(i64::from(u32::MAX)),
        };
        let zero = Interval::constant(0);
        assert_eq!(
            selected.get("remaining"),
            Some(if positive_rank { positive } else { zero }),
            "{condition}: selected"
        );
        assert_eq!(
            fallback.get("remaining"),
            Some(if positive_rank { zero } else { positive }),
            "{condition}: fallback"
        );
    }
}
