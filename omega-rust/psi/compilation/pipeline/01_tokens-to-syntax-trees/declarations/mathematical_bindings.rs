use crate::expressions::parse::parse_expression_handle;
use crate::parameters::generics::{GenericParameterSyntax, parse_generic_parameters};
use crate::parameters::relevance::parse_binding_relevance_brackets;
use crate::token_input::cursor::{Input, ParseResult};
use crate::types::references::parse_type_reference_handle;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{
    MathematicalDefinition, MathematicalDefinitionBody, MathematicalParameterNode,
    MathematicalTypeHandle, MathematicalTypeNode,
};
use tokens::PunctuationKind;

pub(crate) fn parse_let_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    boundary: bool,
) -> ParseResult<'tokens, 'source, MathematicalDefinition> {
    let (name, input) = input.take_identifier()?;
    let (generics, input) = parse_generic_parameters(
        syntax_trees,
        input,
        GenericParameterSyntax::MathematicalDefinition,
    )?;
    if !generics.lifetime_parameters.is_empty() {
        return Err(input.error_here(
            "a mathematical `let` takes no lifetime parameters; universe binders are \
             value-shaped `u: core::Level` entries in the same telescope",
        ));
    }
    if !generics.conformance_bounds.is_empty() {
        return Err(input.error_here(
            "a mathematical `let` takes no `satisfies` conformance binders; parameterize \
             over carriers with `requires` facts on the citing machine instead",
        ));
    }

    let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    let mut parameters = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();
    let mut input = input;
    if !input.at_punctuation(PunctuationKind::RightParen) {
        loop {
            let (parameter, rest) = parse_mathematical_parameter(syntax_trees, input)?;
            let handle = syntax_trees.items.insert_mathematical_parameter(parameter);
            let handle = syntax_trees
                .items
                .append_mathematical_parameter_handle(handle);
            parameters.push(handle);
            input = rest;
            let Some(next) = input.take_comma()? else {
                break;
            };
            input = next;
        }
    }
    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    let parameters = parameters.finish();

    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (result, input) = parse_mathematical_type(syntax_trees, input)?;

    if boundary {
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((
            MathematicalDefinition {
                name,
                is_public: false,
                type_parameters: generics.type_parameters,
                parameters,
                result,
                body: MathematicalDefinitionBody::Assumption,
            },
            input,
        ));
    }

    if input.at_punctuation(PunctuationKind::Semicolon) {
        return Err(input.error_here(
            "a top-level `let` needs a body: `= term;`. The bodyless assumption form \
             is spelled `boundary let`",
        ));
    }
    let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
    let (term, input) = parse_expression_handle(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((
        MathematicalDefinition {
            name,
            is_public: false,
            type_parameters: generics.type_parameters,
            parameters,
            result,
            body: MathematicalDefinitionBody::Definition(term),
        },
        input,
    ))
}

fn parse_mathematical_parameter<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, MathematicalParameterNode> {
    let (name, input) = input.take_identifier()?;
    let (relevance, input) = parse_binding_relevance_brackets(input, "parameter")?;
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (ty, input) = parse_mathematical_type(syntax_trees, input)?;
    Ok((
        MathematicalParameterNode {
            name,
            relevance,
            ty,
        },
        input,
    ))
}

fn parse_mathematical_type<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, MathematicalTypeHandle> {
    let outer_depth = input.depth();
    let input = input.deepen()?;
    let (handle, rest) = parse_mathematical_type_inner(syntax_trees, input)?;
    Ok((handle, rest.with_depth(outer_depth)))
}

fn parse_mathematical_type_inner<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, MathematicalTypeHandle> {
    let ((binder, domain), input) = parse_arrow_domain(syntax_trees, input)?;
    if input.at_punctuation(PunctuationKind::Arrow) {
        let input = input.take_punctuation(PunctuationKind::Arrow, "->")?;
        let (codomain, rest) = parse_mathematical_type(syntax_trees, input)?;
        let handle = syntax_trees
            .items
            .insert_mathematical_type(MathematicalTypeNode::Arrow {
                binder,
                domain,
                codomain,
            });
        return Ok((handle, rest));
    }
    if binder.is_some() {
        return Err(input.error_here("a named domain binder `(x: T)` must be followed by `->`"));
    }
    Ok((domain, input))
}

fn parse_arrow_domain<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, (Option<Identifier>, MathematicalTypeHandle)> {
    if !input.at_punctuation(PunctuationKind::LeftParen) {
        let (type_reference, rest) = parse_type_reference_handle(syntax_trees, input)?;
        let handle = syntax_trees
            .items
            .insert_mathematical_type(MathematicalTypeNode::Ordinary(type_reference));
        let (domain, rest) = fold_type_applications(syntax_trees, handle, rest)?;
        return Ok(((None, domain), rest));
    }

    let after_paren = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    if after_paren.at_punctuation(PunctuationKind::RightParen) {
        let (type_reference, rest) = parse_type_reference_handle(syntax_trees, input)?;
        let handle = syntax_trees
            .items
            .insert_mathematical_type(MathematicalTypeNode::Ordinary(type_reference));
        let (domain, rest) = fold_type_applications(syntax_trees, handle, rest)?;
        return Ok(((None, domain), rest));
    }

    if let Ok((binder, after_name)) = after_paren.take_identifier()
        && after_name.at_punctuation(PunctuationKind::Colon)
    {
        let after_colon = after_name.take_punctuation(PunctuationKind::Colon, ":")?;
        let (domain, after_domain) = parse_mathematical_type(syntax_trees, after_colon)?;
        if after_domain.at_punctuation(PunctuationKind::Comma) {
            return Err(after_domain.error_here(
                "a dependent binder names one domain type; curry further parameters \
                 through nested `->` domains, not a comma list",
            ));
        }
        let rest = after_domain.take_punctuation(PunctuationKind::RightParen, ")")?;
        return Ok(((Some(binder), domain), rest));
    }

    let (inner, after_inner) = parse_mathematical_type(syntax_trees, after_paren)?;
    let rest = after_inner.take_punctuation(PunctuationKind::RightParen, ")")?;
    let (domain, rest) = fold_type_applications(syntax_trees, inner, rest)?;
    Ok(((None, domain), rest))
}

fn fold_type_applications<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut domain: MathematicalTypeHandle,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, MathematicalTypeHandle> {
    while input.at_punctuation(PunctuationKind::LeftParen) {
        let mut rest = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let mut arguments = Vec::new();
        if !rest.at_punctuation(PunctuationKind::RightParen) {
            loop {
                let (argument, after) = parse_expression_handle(syntax_trees, rest)?;
                arguments.push(argument);
                rest = after;
                if rest.at_punctuation(PunctuationKind::Comma) {
                    rest = rest.take_punctuation(PunctuationKind::Comma, ",")?;
                    continue;
                }
                break;
            }
        }
        let rest = rest.take_punctuation(PunctuationKind::RightParen, ")")?;
        let arguments = syntax_trees
            .expressions
            .insert_expression_handles(arguments);
        domain = syntax_trees
            .items
            .insert_mathematical_type(MathematicalTypeNode::Application {
                callee: domain,
                arguments,
            });
        input = rest;
    }
    Ok((domain, input))
}
