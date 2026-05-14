use crate::parser::input::{Input, ParseResult};
use crate::parser::state::parse_state_signature;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{LibraryDefinition, LibraryFunction, TrustLevel};
use omega_tokens::{KeywordKind, PunctuationKind, Token};

pub(super) fn parse_library_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, LibraryDefinition> {
    let ((name, path), mut input) = if input.tokens.first().is_some_and(Token::is_string_literal) {
        let (path, input) = input.take_string()?;
        ((None, path), input)
    } else {
        let (name, input) = input.take_identifier()?;
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (path, input) = input.take_string()?;
        ((Some(name), path), input)
    };

    input = input.take_keyword(KeywordKind::CallingConvention, "calling_convention")?;
    let (calling_convention, input2) = input.take_identifier()?;
    input = input2.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut functions = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        input = input.take_keyword(KeywordKind::Fn, "fn")?;
        let (function, rest) = parse_library_function(syntax_trees, input)?;
        functions.push(function);
        input = rest;
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((
        LibraryDefinition {
            name,
            path,
            calling_convention,
            functions,
        },
        input,
    ))
}

fn parse_library_function<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, LibraryFunction> {
    let (signature, mut input) = parse_state_signature(syntax_trees, input)?;
    let mut symbol = None;
    let mut calling_convention = None;
    let mut trusts = Vec::new();

    loop {
        if input.at_keyword(KeywordKind::Fn) || input.at_punctuation(PunctuationKind::RightBrace) {
            break;
        }

        if input.at_keyword(KeywordKind::Trust) {
            input = input.take_keyword(KeywordKind::Trust, "trust")?;
            let (trust, rest) = parse_trust_level(input)?;
            trusts.push(trust);
            input = if rest.at_punctuation(PunctuationKind::Semicolon) {
                rest.take_punctuation(PunctuationKind::Semicolon, ";")?
            } else {
                rest
            };
        } else if input.at_contextual("symbol") {
            input = input.take_contextual("symbol")?;
            let (value, rest) = input.take_string()?;
            symbol = Some(value);
            input = if rest.at_punctuation(PunctuationKind::Semicolon) {
                rest.take_punctuation(PunctuationKind::Semicolon, ";")?
            } else {
                rest
            };
        } else if input.at_keyword(KeywordKind::CallingConvention) {
            input = input.take_keyword(KeywordKind::CallingConvention, "calling_convention")?;
            let (value, rest) = input.take_identifier()?;
            calling_convention = Some(value);
            input = if rest.at_punctuation(PunctuationKind::Semicolon) {
                rest.take_punctuation(PunctuationKind::Semicolon, ";")?
            } else {
                rest
            };
        } else if input.at_punctuation(PunctuationKind::Semicolon) {
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        } else {
            return Err(input.error_here("expected library function item"));
        }
    }

    Ok((
        LibraryFunction {
            signature,
            symbol,
            calling_convention,
            trusts,
        },
        input,
    ))
}

fn parse_trust_level<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TrustLevel> {
    if input.at_keyword(KeywordKind::Host) {
        let input = input.take_keyword(KeywordKind::Host, "host")?;
        Ok((TrustLevel::Host, input))
    } else {
        let (name, input) = input.take_identifier()?;
        Ok((TrustLevel::Named(name), input))
    }
}
