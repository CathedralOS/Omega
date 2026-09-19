//! Top-level mathematical `let` and `boundary let` parsing.
//!
//! `let name<binders>(parameters): Type = term;` declares a transparent
//! mathematical term definition; `boundary let name<binders>(parameters):
//! Type;` declares the same signature without a body — the named-assumption
//! form (wiki/spec/proofs/mathematical_bindings.md). Both are one declaration
//! surface for mathematical functions, predicates and type-valued results:
//! the generic binders carry the universe/level and type parameters, the
//! parameter list is the ordered ordinary telescope, and the result type may
//! itself be an arrow (`A -> B`, `(value: A) -> F(value)`).
//!
//! An empty parameter list names a nullary definition. Parameters reuse the
//! shared `name [properties]: Type` binding grammar, so `[erased]` marks a
//! proof-side occurrence exactly as on machine signatures. Nothing here
//! resolves names or implies executability; the parsed shape lands on
//! `SyntaxTreeRoots::mathematical_definitions` for the resolution leg.

use crate::expressions::parse_expression::parse_expression_handle;
use crate::input::token_cursor::{Input, ParseResult};
use crate::parameters::binding_properties::parse_binding_relevance_brackets;
use crate::parameters::parse_generic_parameters::{
    GenericParameterSyntax, parse_generic_parameters,
};
use crate::type_syntax::parse_type::parse_type_reference_handle;
use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{
    MathematicalDefinition, MathematicalDefinitionBody, MathematicalParameterNode,
    MathematicalTypeHandle, MathematicalTypeNode,
};
use tokens::PunctuationKind;

/// Parse the declaration tail after a consumed `let` keyword. `boundary`
/// selects the bodyless named-assumption form: `boundary let name(...): T;`.
pub(super) fn parse_let_definition<'tokens, 'source>(
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
             over carriers with `where` facts on the citing machine instead",
        ));
    }

    let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    let mut parameter_start = Handle::invalid();
    let mut parameter_count = 0u32;
    let mut input = input;
    if !input.at_punctuation(PunctuationKind::RightParen) {
        loop {
            let (parameter, rest) = parse_mathematical_parameter(syntax_trees, input)?;
            let handle = syntax_trees.items.insert_mathematical_parameter(parameter);
            let handle = syntax_trees
                .items
                .append_mathematical_parameter_handle(handle);
            if parameter_count == 0 {
                parameter_start = handle;
            }
            parameter_count = parameter_count
                .checked_add(1)
                .expect("mathematical parameter span count overflow");
            input = rest;

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }
            break;
        }
    }
    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    let parameters = if parameter_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(parameter_start, parameter_count)
    };

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

/// One telescope parameter: `name [erased]?: Type`. The type is a
/// mathematical type, so a parameter may carry an arrow (`f: A -> B`).
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

/// A mathematical type: an ordinary type reference or a dependent function
/// `domain -> codomain`. `->` associates to the right, so `A -> B -> C` is
/// `A -> (B -> C)`. Depth is bounded through the same choke point the shared
/// type parser uses.
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

/// Parse one arrow domain: an ordinary type reference, a `()` unit (through
/// the shared type parser), a grouped mathematical type `(A -> B)`, or a
/// named dependent binder `(value: A)` that scopes over the codomain.
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

    // `()` stays on the ordinary parser — it owns the unit type.
    let after_paren = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    if after_paren.at_punctuation(PunctuationKind::RightParen) {
        let (type_reference, rest) = parse_type_reference_handle(syntax_trees, input)?;
        let handle = syntax_trees
            .items
            .insert_mathematical_type(MathematicalTypeNode::Ordinary(type_reference));
        let (domain, rest) = fold_type_applications(syntax_trees, handle, rest)?;
        return Ok(((None, domain), rest));
    }

    // `(name: T)` is a named dependent binder; `(T)` groups any mathematical
    // type. The two differ only in the identifier+colon prefix, so probe it
    // on a copy before committing.
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

/// Fold trailing `(args)` applications into `callee(args)` nodes — `F(value)`,
/// `C(x, y)`, `F(x)(y)`. Only non-binder domains take arguments: a `(x: T)`
/// binder is a Π domain, not a callable family.
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
