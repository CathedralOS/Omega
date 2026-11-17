use crate::errors::parse_error::ParseError;
use crate::expressions::parse::parse_expression_handle;
use crate::syntax_storage::tables::{items, type_refs};
use crate::token_input::cursor::{Input, ParseResult};
use crate::types::references::parse_static_argument_reference;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::SatisfiesClause;
use syntax_trees::types::TypeReferenceHandle;
use tokens::{KeywordKind, PunctuationKind};

pub(crate) fn parse_satisfies_traits<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<SatisfiesClause>> {
    if !input.at_contextual("satisfies") {
        return Ok((HandleSpan::empty(), input));
    }

    input = input.take_contextual("satisfies")?;
    let mut clauses = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();

    loop {
        let ((trait_name, lifetime_arguments, arguments), mut rest) =
            crate::contracts::conformance::parse_conformance_trait_application(
                syntax_trees,
                input,
            )?;

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
        let mut via = None;
        let mut via_expression = syntax_trees::expression::ExpressionHandle::invalid();
        let mut via_keyword_source_span = None;
        if rest.at_contextual("via") {
            via_keyword_source_span = Some(rest.current_source_span());
            let next = rest.take_contextual("via")?;
            let is_bootstrap_binding = next
                .take_identifier()
                .is_ok_and(|(root, _)| root.as_str(syntax_trees) == "ForeignBinding");
            if is_bootstrap_binding {
                let (binding, next) =
                    crate::contracts::bindings::parse_external_provider_binding(next)?;
                via = Some(binding);
                rest = next;
            } else {
                let (expression, next) = parse_expression_handle(syntax_trees, next)?;
                via_expression = expression;
                rest = next;
            }
        }

        let handle = items(syntax_trees).append_satisfies_clause(SatisfiesClause {
            trait_name,
            lifetime_arguments,
            arguments,
            requirement,
            alias,
            via,
            via_expression,
            via_keyword_source_span,
        });
        clauses.push(handle);
        input = rest;
        let Some(next) = input.take_comma()? else {
            break;
        };
        input = next;
    }

    Ok((clauses.finish(), input))
}

pub(crate) fn parse_optional_satisfies_type_arguments<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> Result<(HandleSpan<TypeReferenceHandle>, Input<'tokens, 'source>), ParseError> {
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
            type_refs(syntax_trees).insert_type_reference_handles(arguments),
            input,
        ));
    }
}

pub(crate) fn parse_satisfies_type_argument<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    parse_static_argument_reference(syntax_trees, input)
}
pub(crate) fn parse_conformance_trait_application<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, (Identifier, Vec<Identifier>, HandleSpan<TypeReferenceHandle>)> {
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
                    crate::contracts::conformance::parse_satisfies_type_argument(
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
        type_refs(syntax_trees).insert_type_reference_handles(arguments)
    } else {
        HandleSpan::empty()
    };
    Ok(((trait_name, lifetime_arguments, trait_arguments), rest))
}
