use crate::parser::input::{Input, ParseResult};
use crate::parser::state::parse_state_signature;
use omega_core::arena::{Handle, HandleSpan};
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
    let mut function_start = Handle::invalid();
    let mut function_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        input = input.take_keyword(KeywordKind::Entry, "entry")?;
        let (function, rest) = parse_library_function(syntax_trees, input)?;
        let handle = syntax_trees.items.append_library_function(function);
        if function_count == 0 {
            function_start = handle;
        }
        function_count = function_count
            .checked_add(1)
            .expect("library function span count overflow");
        input = rest;
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let functions = if function_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(function_start, function_count)
    };
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
    let mut trust_start = Handle::invalid();
    let mut trust_count = 0u32;

    loop {
        if input.at_keyword(KeywordKind::Entry) || input.at_punctuation(PunctuationKind::RightBrace)
        {
            break;
        }

        if input.at_keyword(KeywordKind::Trust) {
            input = input.take_keyword(KeywordKind::Trust, "trust")?;
            let (trust, rest) = parse_trust_level(input)?;
            let handle = syntax_trees.items.append_trust_level(trust);
            if trust_count == 0 {
                trust_start = handle;
            }
            trust_count = trust_count
                .checked_add(1)
                .expect("library function trust span count overflow");
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

    let trusts = if trust_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(trust_start, trust_count)
    };

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
