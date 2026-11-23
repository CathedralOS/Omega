use crate::errors::parse_error::ParseError;
use crate::token_input::cursor::{Input, ParseResult, parse_path_handle_span};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::UseItem;
use tokens::{PunctuationKind as P, TokenKind};

pub(crate) fn parse_use_item<'tokens, 'source>(
    trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, UseItem> {
    // Existing library sources still use namespace imports during migration.
    if let Ok((_, next)) = input.take_identifier()
        && (next.at_path_member() || next.at_punctuation(P::Semicolon))
    {
        let (path, rest) = parse_path_handle_span(input, |name| {
            trees.items.append_identifier_path_member(name)
        })?;
        return Ok((
            UseItem::Namespace { path },
            rest.take_punctuation(P::Semicolon, ";")?,
        ));
    }
    let (root, mut rest) = if input.at_punctuation(P::LeftBrace) {
        let (root, rest) = input
            .take_punctuation(P::LeftBrace, "{")?
            .take_identifier()?;
        let rest = rest
            .take_punctuation(P::RightBrace, "}")?
            .take_punctuation(P::Slash, "/")?;
        (Some(root), rest)
    } else {
        (None, input)
    };
    let mut components = HandleSpan::empty();
    let stem = loop {
        let beginning = rest;
        let (component, next) = file_component(rest)?;
        let implicit_alias = beginning.tokens.first().is_some_and(|token| {
            token.kind == TokenKind::Identifier && token.span == component.source_span().span
        });
        let is_last = next.at_punctuation(P::Dot);
        let stem = (is_last && implicit_alias).then(|| component.clone());
        components.push_contiguous(trees.items.append_identifier_path_member(component));
        if is_last {
            rest = next.take_punctuation(P::Dot, ".")?.take_contextual("omg")?;
            break stem;
        }
        rest = next.take_punctuation(P::Slash, "/ or the .omg filename extension")?;
    };
    let alias = if rest.at_contextual("as") {
        let at_alias = rest.take_contextual("as")?;
        if !at_alias
            .tokens
            .first()
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            return Err(at_alias.error_here("expected a file import alias"));
        }
        let (alias, next) = at_alias.take_identifier()?;
        rest = next;
        alias
    } else {
        stem.ok_or_else(|| rest.error_here("this filename needs an explicit file alias (as name)"))?
    };
    Ok((
        UseItem::File {
            root,
            components,
            alias,
            span: input.source_span_until(rest),
        },
        rest.take_punctuation(P::Semicolon, ";")?,
    ))
}

fn file_component<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Identifier> {
    let mut rest = input;
    let mut end = None;
    while let Some(token) = rest.tokens.first() {
        let spelling = rest.spelling(token);
        if end.is_some_and(|end| end != token.span.start)
            || !spelling
                .chars()
                .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
        {
            break;
        }
        end = Some(token.span.end);
        rest = rest.expect_token()?.1;
    }
    if end.is_none() {
        return Err(input.error_here("expected a logical filename component"));
    }
    Ok((input.name_until(rest), rest))
}

pub(crate) fn check_file_aliases(
    trees: &SyntaxTrees,
    roots: &syntax_trees::ParsedRootItems,
    source: &str,
) -> Result<(), ParseError> {
    let mut aliases = roots.items[roots.import_range.clone()]
        .iter()
        .filter_map(|root| {
            if let syntax_trees::item::Item::Use(UseItem::File { alias, .. }) =
                trees.root_item(*root)
            {
                Some(alias)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    aliases.sort_by_key(|alias| (alias.as_str(trees), alias.source_span().span.start));
    for pair in aliases.windows(2) {
        let name = pair[0].as_str(trees);
        if name == pair[1].as_str(trees) {
            let first = source_text::SourcePosition::of(source, pair[0].source_span().span.start);
            return Err(ParseError::at_source_span(
                format!(
                    "duplicate file import alias `{name}`, first imported at line {}, column {}",
                    first.line, first.column
                ),
                pair[1].source_span(),
            ));
        }
    }
    Ok(())
}
