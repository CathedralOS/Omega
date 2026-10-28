use crate::input::token_cursor::{Input, ParseResult};
use crate::type_syntax::parse_type::parse_type_reference_handle;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::SatisfiesClause;
use tokens::{KeywordKind, PunctuationKind};

pub(crate) fn parse_satisfies_traits<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<SatisfiesClause>> {
    if !input.at_contextual("satisfies") {
        return Ok((HandleSpan::empty(), input));
    }

    input = input.take_contextual("satisfies")?;
    let mut clauses = crate::handle_spans::HandleSpanBuilder::new();

    loop {
        let ((trait_name, lifetime_arguments, arguments), mut rest) =
            crate::contracts::conformance::parse_conformance::parse_conformance_trait_application(
                syntax_trees,
                input,
            )?;

        // The single-requirement binding (rearrange settle 2026-07-18):
        // `satisfies Trait::requirement [as Alias]` conforms THIS machine to
        // that one requirement; the alias names the satisfier when signatures
        // collide or the same machine fills different slots (plural algebras).
        if !rest.at_punctuation(PunctuationKind::ColonColon) {
            return Err(rest.error_here(format!(
                "bare `satisfies {}` on a machine is retired; bind one exact requirement as `satisfies {}::requirement`",
                trait_name.as_str(syntax_trees),
                trait_name.as_str(syntax_trees),
            )));
        }
        let next = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (member, next) = next.take_identifier()?;
        let requirement = Some(member);
        rest = next;
        let mut alias = None;
        if rest.at_keyword(KeywordKind::As) {
            let next = rest.take_keyword(KeywordKind::As, "as")?;
            let (name, next) = next.take_identifier()?;
            alias = Some(name);
            rest = next;
        }
        if rest.at_contextual("via") {
            return Err(rest.error_here(
                "`via` binding clauses are retired; bind the exact foreign requirement in the owning package Build with `Build::bind<Requirement>(builder, target, locator)` or a typed table binding operation",
            ));
        }

        let handle = syntax_trees.items.append_satisfies_clause(SatisfiesClause {
            trait_name,
            lifetime_arguments,
            arguments,
            requirement,
            alias,
        });
        clauses.push(handle);
        input = rest;

        let Some(next) = input.take_comma()? else {
            break;
        };

        input = next;

        continue;
    }

    Ok((clauses.finish(), input))
}

pub(crate) fn parse_optional_satisfies_type_arguments<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> Result<
    (
        HandleSpan<syntax_trees::types::TypeReferenceHandle>,
        Input<'tokens, 'source>,
    ),
    crate::diagnostics::parse_error::ParseError,
> {
    if !input.at_punctuation(PunctuationKind::Less) {
        return Ok((HandleSpan::empty(), input));
    }

    input = input.take_punctuation(PunctuationKind::Less, "<")?;
    let mut arguments = Vec::new();
    loop {
        let (argument, rest) = parse_satisfies_type_argument(syntax_trees, input)?;
        arguments.push(argument);
        input = rest;

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }

        let input = input.take_punctuation(PunctuationKind::Greater, ">")?;
        return Ok((
            syntax_trees
                .type_references
                .insert_type_reference_handles(arguments),
            input,
        ));
    }
}

pub(crate) fn parse_satisfies_type_argument<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, syntax_trees::types::TypeReferenceHandle> {
    if input
        .tokens
        .first()
        .is_some_and(crate::input::token_cursor::is_identifier_token_for_parser)
    {
        let (first, mut rest) = input.take_identifier()?;
        if rest.at_punctuation(PunctuationKind::ColonColon) {
            let mut members = vec![first];
            while rest.at_punctuation(PunctuationKind::ColonColon) {
                rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
                let (member, next) = rest.take_identifier()?;
                members.push(member);
                rest = next;
            }
            let name = crate::input::paths::join_path_identifier(syntax_trees, &members);
            return Ok((syntax_trees.type_references.insert_named(name), rest));
        }
    }
    parse_type_reference_handle(syntax_trees, input)
}
pub(crate) fn parse_conformance_trait_application<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<
    'tokens,
    'source,
    (
        syntax_trees::identifier::Identifier,
        Vec<syntax_trees::identifier::Identifier>,
        HandleSpan<syntax_trees::types::TypeReferenceHandle>,
    ),
> {
    let (trait_name, mut rest) = input.take_identifier()?;
    let mut lifetime_arguments = Vec::new();
    let trait_arguments = if rest.at_punctuation(PunctuationKind::Less) {
        rest = rest.take_punctuation(PunctuationKind::Less, "<")?;
        let mut arguments = Vec::new();
        let mut saw_non_lifetime = false;
        loop {
            if rest.at_punctuation(PunctuationKind::Apostrophe) {
                if saw_non_lifetime {
                    return Err(rest.error_here(
                        "lifetime arguments precede type, const, and machine arguments",
                    ));
                }
                rest = rest.take_punctuation(PunctuationKind::Apostrophe, "'")?;
                let (lifetime, next) = rest.take_identifier()?;
                lifetime_arguments.push(lifetime);
                rest = next;
            } else {
                saw_non_lifetime = true;
                let (argument, next) =
                    crate::contracts::conformance::parse_conformance::parse_satisfies_type_argument(
                        syntax_trees,
                        rest,
                    )?;
                arguments.push(argument);
                rest = next;
            }
            if rest.at_punctuation(PunctuationKind::Comma) {
                rest = rest.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }
            rest = rest.take_punctuation(PunctuationKind::Greater, ">")?;
            break;
        }
        syntax_trees
            .type_references
            .insert_type_reference_handles(arguments)
    } else {
        HandleSpan::empty()
    };
    Ok(((trait_name, lifetime_arguments, trait_arguments), rest))
}
