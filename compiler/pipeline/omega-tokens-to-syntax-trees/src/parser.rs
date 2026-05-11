mod input;
mod capability;
mod data;
mod expression;
mod file;
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
    let (items, rest) = parse_file(input)?;

    if let Some(token) = rest.tokens.first() {
        return Err(ParseError::at_source_span(
            "unexpected token after file parse",
            rest.source_span(token),
        ));
    }

    Ok(SyntaxTrees::from_items(source_id, items))
}

#[cfg(test)]
mod tests {
    use super::parse_syntax_trees;
    use omega_source_files_to_tokens::Lexer;

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

        let tokens = Lexer::new(source).tokenize().expect("tokenize should succeed");
        let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
        assert_eq!(parsed.items.len(), 1);
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

        let tokens = Lexer::new(source).tokenize().expect("tokenize should succeed");
        let parsed = parse_syntax_trees(&tokens).expect("parse should succeed");
        assert_eq!(parsed.items.len(), 1);
    }
}
