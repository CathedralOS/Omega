use crate::parser::input::{Input, ParseResult};
use crate::parser::state::parse_state_signature;
use psi_arena::{Handle, HandleSpan};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::item::{BoundaryLevel, LibraryDefinition, LibraryFunction};
use psi_tokens::{KeywordKind, PunctuationKind, Token};

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
    let mut boundary_start = Handle::invalid();
    let mut boundary_count = 0u32;

    loop {
        if input.at_keyword(KeywordKind::Entry) || input.at_punctuation(PunctuationKind::RightBrace)
        {
            break;
        }

        if input.at_contextual("boundary") {
            input = input.take_contextual("boundary")?;
            let (boundary, rest) = parse_boundary_level(input)?;
            let handle = syntax_trees.items.append_boundary_level(boundary);
            if boundary_count == 0 {
                boundary_start = handle;
            }
            boundary_count = boundary_count
                .checked_add(1)
                .expect("library function boundary span count overflow");
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

    let boundaries = if boundary_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(boundary_start, boundary_count)
    };

    Ok((
        LibraryFunction {
            signature,
            symbol,
            calling_convention,
            boundaries,
        },
        input,
    ))
}

fn parse_boundary_level<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, BoundaryLevel> {
    if input.at_keyword(KeywordKind::Host) {
        let input = input.take_keyword(KeywordKind::Host, "host")?;
        Ok((BoundaryLevel::Host, input))
    } else {
        let (name, input) = input.take_identifier()?;
        Ok((BoundaryLevel::Named(name), input))
    }
}
