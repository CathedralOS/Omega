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
use omega_syntax_trees::{SyntaxTrees, item::ItemHandle};
use omega_tokens::Token;

pub fn parse_syntax_trees(tokens: &[Token<'_>]) -> Result<SyntaxTrees, ParseError> {
    parse_syntax_trees_with_id(SourceId::default(), tokens)
}

pub fn parse_syntax_trees_with_id(
    source_id: SourceId,
    tokens: &[Token<'_>],
) -> Result<SyntaxTrees, ParseError> {
    let mut syntax_trees = SyntaxTrees::new(source_id);
    parse_syntax_trees_into_with_id(&mut syntax_trees, source_id, tokens)?;

    Ok(syntax_trees)
}

pub fn parse_syntax_trees_into_with_id(
    syntax_trees: &mut SyntaxTrees,
    source_id: SourceId,
    tokens: &[Token<'_>],
) -> Result<Vec<ItemHandle>, ParseError> {
    let input = Input::new(source_id, tokens);
    let mut root_items = Vec::new();
    let ((), rest) = parse_file(syntax_trees, input, &mut root_items)?;

    if let Some(token) = rest.tokens.first() {
        return Err(ParseError::at_source_span(
            "unexpected token after file parse",
            rest.source_span(token),
        ));
    }

    Ok(root_items)
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
        machine Game::new() -> Game {
            let game: Game;
            -> game;
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
        data Main {
        }

        machine Main::main(&mut self) -> i32 {
            transition {
                _ -> running()
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
        assert_eq!(parsed.root_item_count(), 2);
    }

    #[test]
    fn parses_attached_main_state_name_as_main() {
        let source = r#"
        data Main {
        }

        machine Main::main(&mut self) {}
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
        let machine = parsed
            .root_items()
            .find_map(|item| match item {
                omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .expect("machine root item");
        let state_handle = parsed
            .items
            .state_handles(machine.states)
            .first()
            .copied()
            .expect("entry state");
        let state = parsed.items.state(state_handle);
        assert_eq!(state.name.as_str(), "main");
    }

    #[test]
    fn parses_self_parameter_with_dedicated_self_type() {
        let source = r#"
        data Main {
        }

        machine Main::main(&mut self) {}
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
        let machine = parsed
            .root_items()
            .find_map(|item| match item {
                omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .expect("machine root item");
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
        data Main {
        }

        machine Main::main(&mut self) {
            self;
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
        let machine = parsed
            .root_items()
            .find_map(|item| match item {
                omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .expect("machine root item");
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
    fn parses_nested_call_arguments_as_contiguous_expression_spans() {
        let source = r#"
        data Player {
            xp: i32;
            level: i32;
        }

        data Main {
            xp_table: Player;
        }

        machine Main::main(&mut self, player: &mut Player) {
            player.xp = max(0, player.xp - self.xp_required(player.level));

            state xp_required(&mut self, level: i32) -> i32 {
                10
            }
        }
        "#;

        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
        let machine = parsed
            .root_items()
            .find_map(|item| match item {
                omega_syntax_trees::item::Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .expect("machine root item");
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
                .expect("assignment statement"),
        );
        let StatementNode::Assignment(assignment) = statement else {
            panic!("expected assignment statement");
        };

        assert_eq!(
            parsed.expressions.display_name(assignment.value),
            "max(0, player.xp - self.xp_required(player.level))"
        );
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
