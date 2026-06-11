use crate::parser::input::{Input, ParseResult};
use crate::parser::state::parse_state_signature;
use crate::parser::trait_definition::parse_signature_clauses;
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
            } else {
                return Err(input.expected_one_of_here(&["`entry`"]));
            }
        } else if input.at_keyword(KeywordKind::Entry) {
            input = input.take_keyword(KeywordKind::Entry, "entry")?;
        } else {
            return Err(input.expected_one_of_here(&["`pub entry`", "`entry`"]));
        }

        let (mut signature, mut rest) = parse_state_signature(syntax_trees, input)?;
        // Platform entries share the bodyless-signature clause grammar with
        // trait machine signatures: `effects`/`requires`/`ensures` may follow
        // the parameter list. Only enter the clause parser when a clause is
        // actually present so entries without a trailing semicolon keep
        // parsing as before.
        if rest.at_contextual("effects")
            || rest.at_contextual("requires")
            || rest.at_contextual("ensures")
        {
            let ((effects, contracts), after_clauses) =
                parse_signature_clauses(syntax_trees, rest)?;
            signature.effects = effects;
            signature.contracts = contracts;
            rest = after_clauses;
        }
        let handle = syntax_trees.items.insert_state_signature(&signature);
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
