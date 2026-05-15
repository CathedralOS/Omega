mod capability;
mod context;
mod data;
mod diagnostics;
mod expression;
mod file;
mod input;
mod invariant;
mod item;
mod library;
mod machine;
mod platform;
mod state;
mod statement;
mod target;
mod transition;
mod trust;
mod type_reference;
mod use_item;

use crate::parse_error::ParseError;
use file::parse_file;
use input::Input;
use omega_core::source::SourceId;
use omega_syntax_trees::SyntaxTrees;
use omega_tokens::Token;

pub fn parse_syntax_trees(tokens: &[Token<'_>]) -> Result<SyntaxTrees, ParseError> {
    parse_syntax_trees_with_id(SourceId::default(), tokens)
}

pub fn parse_syntax_trees_with_id(
    source_id: SourceId,
    tokens: &[Token<'_>],
) -> Result<SyntaxTrees, ParseError> {
    let input = Input::new(source_id, tokens);
    let mut syntax_trees = SyntaxTrees::new(source_id);
    let ((), rest) = parse_file(&mut syntax_trees, input)?;

    if let Some(token) = rest.tokens.first() {
        return Err(ParseError::at_source_span(
            "unexpected token after file parse",
            rest.source_span(token),
        ));
    }

    Ok(syntax_trees)
}

#[cfg(test)]
mod tests {
    use super::parse_syntax_trees;
    use omega_source_files_to_tokens::Lexer;
    use omega_syntax_trees::expression::ExpressionNode;
    use omega_syntax_trees::statement::StatementNode;
    use omega_syntax_trees::types::TypeReferenceNode;

    #[test]
    fn parses_dungeon_machine_surface() {
        let source = r#"
        machine Game::new -> Game {
            pub entry() {
                let game: Game;
                -> game;
            }
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
        assert_eq!(parsed.root_item_count(), 1);
    }

    #[test]
    fn parses_dungeon_state_flow() {
        let source = r#"
        machine main -> i32 {
            pub entry(&mut self) {
                transition {
                    _ -> running()
                }
            }

            state running(&mut self) {
                -> 0;
            }
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
        assert_eq!(parsed.root_item_count(), 1);
    }

    #[test]
    fn parses_main_entry_state_name_as_entry() {
        let source = r#"
        machine main {
            pub entry(&mut self) {}
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
        let machine = match parsed.root_items().next().expect("root item") {
            omega_syntax_trees::item::Item::Machine(machine) => machine,
            _ => panic!("expected machine root item"),
        };
        let state_handle = parsed
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state");
        let state = parsed.items.state(state_handle);
        assert_eq!(state.name.as_str(), "entry");
    }

    #[test]
    fn parses_self_parameter_with_dedicated_self_type() {
        let source = r#"
        machine main {
            pub entry(&mut self) {}
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
        let machine = match parsed.root_items().next().expect("root item") {
            omega_syntax_trees::item::Item::Machine(machine) => machine,
            _ => panic!("expected machine root item"),
        };
        let state = parsed.items.state(
            parsed
                .items
                .state_handles(machine.states)
                .first()
                .copied()
                .expect("entry state"),
        );
        let parameter = parsed.items.state_parameter(
            parsed
                .items
                .state_parameters(state.parameters)
                .first()
                .copied()
                .expect("self parameter"),
        );

        assert!(parameter.is_self);
        assert!(matches!(
            parsed
                .type_references
                .type_reference(parameter.type_reference),
            TypeReferenceNode::SelfType
        ));
    }

    #[test]
    fn parses_self_expression_as_dedicated_node() {
        let source = r#"
        machine main {
            pub entry(&mut self) {
                self;
            }
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
        let machine = match parsed.root_items().next().expect("root item") {
            omega_syntax_trees::item::Item::Machine(machine) => machine,
            _ => panic!("expected machine root item"),
        };
        let state = parsed.items.state(
            parsed
                .items
                .state_handles(machine.states)
                .first()
                .copied()
                .expect("entry state"),
        );
        let statement = parsed.statements.statement(
            parsed
                .items
                .statements(state.statements)
                .first()
                .copied()
                .expect("expression statement"),
        );
        let StatementNode::Expression(expression) = statement else {
            panic!("expected expression statement");
        };

        assert!(matches!(
            parsed.expressions.expression(*expression),
            ExpressionNode::SelfValue
        ));
    }

    #[test]
    fn rejects_self_as_ordinary_declaration_name() {
        let source = r#"
        data self {}
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        assert!(parse_syntax_trees(&tokens).is_err());
    }
}
