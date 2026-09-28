use crate::input::token_cursor::{Input, ParseResult};
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{ModuleDeclaration, PackageDeclaration};
use tokens::PunctuationKind;

pub(super) fn parse_module_declaration<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ModuleDeclaration> {
    let (path, input) = parse_dot_or_colon_path(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;
    let input = take_optional_semicolon(input)?;

    Ok((ModuleDeclaration { path }, input))
}

pub(super) fn parse_package_declaration<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, PackageDeclaration> {
    let (path, input) = parse_dot_or_colon_path(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;
    let input = take_optional_semicolon(input)?;

    Ok((PackageDeclaration { path }, input))
}

fn parse_dot_or_colon_path<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    mut append_member: impl FnMut(
        syntax_trees::identifier::Identifier,
    ) -> arena::Handle<syntax_trees::identifier::Identifier>,
) -> ParseResult<'tokens, 'source, arena::HandleSpan<syntax_trees::identifier::Identifier>> {
    let (first, mut rest) = input.take_identifier()?;
    let start = append_member(first);
    let mut count = 1u32;

    loop {
        if rest.at_punctuation(PunctuationKind::Dot) {
            rest = rest.take_punctuation(PunctuationKind::Dot, ".")?;
        } else if rest.at_punctuation(PunctuationKind::ColonColon) {
            rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
        } else {
            break;
        }

        let (member, next) = rest.take_identifier()?;
        append_member(member);
        count = count
            .checked_add(1)
            .expect("module/package path member span count overflow");
        rest = next;
    }

    Ok((arena::HandleSpan::from_parts(start, count), rest))
}

pub(super) fn take_optional_semicolon<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> Result<Input<'tokens, 'source>, crate::diagnostics::parse_error::ParseError> {
    if input.at_punctuation(PunctuationKind::Semicolon) {
        input.take_punctuation(PunctuationKind::Semicolon, ";")
    } else {
        Ok(input)
    }
}
