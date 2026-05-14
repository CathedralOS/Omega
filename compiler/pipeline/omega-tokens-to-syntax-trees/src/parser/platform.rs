use crate::parser::input::{Input, ParseResult};
use crate::parser::state::parse_state_signature;
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::Platform;
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_platform<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Platform> {
    let (name, mut input) = input.take_identifier()?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut state_start = Handle::invalid();
    let mut state_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_keyword(KeywordKind::Pub) {
            input = input.take_keyword(KeywordKind::Pub, "pub")?;
            if input.at_keyword(KeywordKind::Entry) {
                input = input.take_keyword(KeywordKind::Entry, "entry")?;
            }
        } else if input.at_keyword(KeywordKind::Entry) {
            input = input.take_keyword(KeywordKind::Entry, "entry")?;
        } else {
            input = input.take_keyword(KeywordKind::Fn, "fn")?;
        }

        let (signature, rest) = parse_state_signature(syntax_trees, input)?;
        let handle = syntax_trees.items.insert_state_signature_tree(
            &signature,
            &mut syntax_trees.type_references,
            &mut syntax_trees.expressions,
        );
        let handle = syntax_trees.items.append_state_signature_handle(handle);
        if state_count == 0 {
            state_start = handle;
        }
        state_count = state_count
            .checked_add(1)
            .expect("platform state signature span count overflow");
        input = if rest.at_punctuation(PunctuationKind::Semicolon) {
            rest.take_punctuation(PunctuationKind::Semicolon, ";")?
        } else {
            rest
        };
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let states = if state_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(state_start, state_count)
    };
    Ok((Platform { name, states }, input))
}
