use crate::expressions::static_arguments::parse_static_argument_expression;
use crate::syntax_storage::tables::type_refs;
use crate::token_input::cursor::{Input, ParseResult};
use arena::HandleSpan;
use numerics::arithmetic::ArithmeticDomain;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::identifier::Identifier;
use syntax_trees::types::{
    DomainConstraint, FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use tokens::PunctuationKind;

pub(crate) fn parse_static_name<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Identifier> {
    let (first, mut input) = input.take_identifier()?;
    if !input.at_punctuation(PunctuationKind::ColonColon) {
        return Ok((first, input));
    }
    let mut name = first.as_str(&input).to_owned();
    let mut source_span = first.source_span();
    while input.at_punctuation(PunctuationKind::ColonColon) {
        input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (member, rest) = input.take_identifier()?;
        name.push_str("::");
        name.push_str(member.as_str(&input));
        source_span.span.end = member.source_span().span.end;
        input = rest;
    }
    Ok((Identifier::new(name, source_span), input))
}

pub(crate) fn parse_type_reference_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    parse_type_reference_handle_with_trailing_domain(syntax_trees, input, true)
}

pub(crate) fn parse_cast_target_type_reference_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    parse_type_reference_handle_with_trailing_domain(syntax_trees, input, false)
}

fn parse_type_reference_handle_with_trailing_domain<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    allow_trailing_domain: bool,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    let outer_depth = input.depth();
    // Every expression a type contains (an array extent, a constant
    // argument, a range bound) is a constant position.
    let outer_constant = input.constant_position();
    let input = input.with_constant_position(true).deepen()?;
    let (type_reference, rest) =
        parse_type_reference_handle_inner(syntax_trees, input, allow_trailing_domain)?;
    Ok((
        type_reference,
        rest.with_depth(outer_depth)
            .with_constant_position(outer_constant),
    ))
}

fn parse_type_reference_handle_inner<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    allow_trailing_domain: bool,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    if input.at_punctuation(PunctuationKind::Ampersand) {
        return parse_borrow_type_reference(syntax_trees, input, allow_trailing_domain);
    }
    if input.at_punctuation(PunctuationKind::LeftParen) {
        let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        return Ok((type_refs(syntax_trees).insert_unit(), input));
    }

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        let input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
        let (element_type, input) = parse_type_reference_handle(syntax_trees, input)?;
        if input.at_punctuation(PunctuationKind::Semicolon) {
            let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            let (expression, input) =
                crate::expressions::parse::parse_expression_handle(syntax_trees, input)?;
            let length = match syntax_trees.expressions.expression(expression) {
                ExpressionNode::Integer(value) => {
                    let text = value.to_string();
                    match text.parse::<usize>() {
                        Ok(value) => FixedArrayLength::Literal(value),
                        Err(_) => {
                            let use_type = syntax_trees
                                .type_references
                                .insert(TypeReferenceNode::ConstExpression(expression));
                            FixedArrayLength::Expression(use_type)
                        }
                    }
                }
                ExpressionNode::Name {
                    members: path,
                    application: path_application,
                } if path_application.is_none() => {
                    let members = syntax_trees
                        .expressions
                        .identifier_path_members(path.to_owned());
                    let name = Identifier::new(
                        members
                            .iter()
                            .map(|name| name.as_str(syntax_trees))
                            .collect::<Vec<_>>()
                            .join("::"),
                        syntax_trees.expressions.source_span(expression),
                    );
                    FixedArrayLength::ConstParameter(name)
                }
                _ => {
                    let use_type = syntax_trees
                        .type_references
                        .insert(TypeReferenceNode::ConstExpression(expression));
                    FixedArrayLength::Expression(use_type)
                }
            };
            let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
            let array = syntax_trees
                .type_references
                .insert(TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                });
            return if allow_trailing_domain {
                apply_in_domain_suffix(syntax_trees, array, input)
            } else {
                Ok((array, input))
            };
        }

        let type_reference = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Slice { element_type });

        let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
        return if allow_trailing_domain {
            apply_in_domain_suffix(syntax_trees, type_reference, input)
        } else {
            Ok((type_reference, input))
        };
    }

    if input.at_keyword(tokens::KeywordKind::Machine) {
        let source_span = input.current_source_span();
        let input = input.take_keyword(tokens::KeywordKind::Machine, "machine")?;
        let (signature, input) = crate::parameters::requirements::parse_runtime_machine_contract(
            syntax_trees,
            Identifier::new("$callable", source_span),
            input,
        )?;
        let signature = syntax_trees.items.insert_state_signature(&signature);
        return Ok((
            type_refs(syntax_trees).insert(TypeReferenceNode::Callable { signature }),
            input,
        ));
    }

    // Runtime trait selectors have no source meaning. Reject before
    // producing a tree instead of carrying an obsolete selector type
    // into resolution and eventually failing during elaboration.
    if input.at_contextual("dyn") {
        return Err(
            input.error_here("`dyn` types are retired; use machine values and `some` packages")
        );
    }

    let authored = input;
    let checkpoint = syntax_trees.append_checkpoint();
    let (base_name, input) = parse_static_name(input)?;
    let (reference, rest) =
        parse_named_type_reference(syntax_trees, base_name, input, allow_trailing_domain)?;
    if !rest.at_punctuation(PunctuationKind::ColonColon) {
        return Ok((reference, rest));
    }
    // An application followed by a member cannot be flattened into the
    // ordinary Generic node: Trait<T>::member and Trait::member<T>
    // apply different declarations. Reuse the scoped selection syntax.
    syntax_trees.rewind_appended(checkpoint);
    let Some((selection, rest)) =
        crate::expressions::postfix::parse_static_name_selection(syntax_trees, authored)?
    else {
        return Err(authored.error_here("expected a scoped type selection"));
    };
    let reference = type_refs(syntax_trees).insert(TypeReferenceNode::StaticSelection(selection));
    if allow_trailing_domain {
        apply_in_domain_suffix(syntax_trees, reference, rest)
    } else {
        Ok((reference, rest))
    }
}

pub(crate) fn parse_named_type_reference<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    base_name: Identifier,
    mut input: Input<'tokens, 'source>,
    allow_trailing_domain: bool,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    let type_reference = if input.at_punctuation(PunctuationKind::Less) {
        input = input.take_punctuation(PunctuationKind::Less, "<")?;
        let mut lifetime_arguments = Vec::new();
        let mut arguments: Vec<TypeReferenceHandle> = Vec::new();

        loop {
            if input.at_punctuation(PunctuationKind::Apostrophe) {
                if !arguments.is_empty() {
                    return Err(input.error_here(
                        "lifetime arguments precede type, const, and machine arguments",
                    ));
                }
                let input_after_tick = input.take_punctuation(PunctuationKind::Apostrophe, "'")?;
                let (lifetime, rest) = input_after_tick.take_identifier()?;
                lifetime_arguments.push(lifetime);
                input = rest;
                let Some(next) = input.take_comma()? else {
                    break;
                };
                input = next;
                continue;
            }

            let (argument, rest) = parse_static_argument_reference(syntax_trees, input)?;
            arguments.push(argument);
            input = rest;
            let Some(next) = input.take_comma()? else {
                break;
            };
            input = next;
        }

        input = input.take_punctuation(PunctuationKind::Greater, ">")?;
        let argument_count = arguments.len();
        let first_argument = arguments
            .first()
            .copied()
            .unwrap_or_else(TypeReferenceHandle::invalid);
        let arguments = syntax_trees
            .type_references
            .insert_type_reference_handles(arguments);
        if base_name.as_str(syntax_trees) == "Join" {
            return Err(input.error_here(
                "`Join<T>` is retired: task activation returns a linear `Task<T>`; \
                 settle it with `finish()` or transfer it to another owner",
            ));
        } else if base_name.as_str(syntax_trees) == "Slice"
            && lifetime_arguments.is_empty()
            && argument_count == 1
        {
            type_refs(syntax_trees).insert(TypeReferenceNode::Slice {
                element_type: first_argument,
            })
        } else {
            type_refs(syntax_trees).insert(TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            })
        }
    } else {
        type_refs(syntax_trees).insert_named(base_name)
    };

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        return Err(input.error_here(
            "type brackets are retired; express scalar bounds in the owning data or machine `requires` clause, result `ensures` clause, or the value's established facts",
        ));
    }

    let (type_reference, input) = if allow_trailing_domain {
        apply_in_domain_suffix(syntax_trees, type_reference, input)?
    } else {
        (type_reference, input)
    };
    Ok((type_reference, input))
}

pub(crate) fn parse_static_argument_reference<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    if input.at_integer()
        || input
            .tokens
            .first()
            .is_some_and(|token| token.float_literal_kind().is_some())
        || input.at_punctuation(PunctuationKind::Minus)
        || input.at_punctuation(PunctuationKind::Exclamation)
        || input.at_punctuation(PunctuationKind::Tilde)
        || (input.at_punctuation(PunctuationKind::LeftParen)
            && input
                .take_punctuation(PunctuationKind::LeftParen, "(")
                .is_ok_and(|rest| !rest.at_punctuation(PunctuationKind::RightParen)))
        || input.at_contextual("true")
        || input.at_contextual("false")
    {
        let (expression, rest) = parse_static_argument_expression(syntax_trees, input)?
            .ok_or_else(|| input.error_here("expected static argument expression"))?;
        if let ExpressionNode::Name { .. } = syntax_trees.expressions.expression(expression) {
            let name = Identifier::new(
                syntax_trees.display_expression_name(expression),
                syntax_trees.expressions.source_span(expression),
            );
            return Ok((type_refs(syntax_trees).insert_named(name), rest));
        }
        Ok((
            type_refs(syntax_trees).insert(TypeReferenceNode::ConstExpression(expression)),
            rest,
        ))
    } else if input
        .tokens
        .first()
        .is_some_and(crate::token_input::cursor::is_identifier_token_for_parser)
    {
        let checkpoint = syntax_trees.append_checkpoint();
        let const_expression = parse_static_argument_expression(syntax_trees, input)?;
        if let Some((expression, expression_rest)) = const_expression
            && !matches!(
                syntax_trees.expressions.expression(expression),
                ExpressionNode::Name { .. }
            )
            && (expression_rest.at_punctuation(PunctuationKind::Comma)
                || expression_rest.at_punctuation(PunctuationKind::Greater))
        {
            Ok((
                type_refs(syntax_trees).insert(TypeReferenceNode::ConstExpression(expression)),
                expression_rest,
            ))
        } else {
            syntax_trees.rewind_appended(checkpoint);
            parse_type_reference_handle(syntax_trees, input)
        }
    } else {
        parse_type_reference_handle(syntax_trees, input)
    }
}

fn apply_in_domain_suffix<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    type_reference: TypeReferenceHandle,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    if !input.at_contextual("in") {
        return Ok((type_reference, input));
    }
    let mut cursor = input.take_contextual("in")?;
    let mut constraints = Vec::new();
    loop {
        let (domain_name, rest) = parse_static_name(cursor)?;
        let (arguments, rest) = parse_domain_argument_handles(syntax_trees, rest)?;
        if domain_name.as_str(syntax_trees) == "Carry::Portable" {
            if !arguments.is_empty() {
                return Err(rest.error_here("compiler carry domains do not take index arguments"));
            }
            constraints.extend(language_core::CarryPermission::ALL.map(|permission| {
                TypeConstraintNode::Domain(DomainConstraint {
                    name: Identifier::generated(permission.name()),
                    arguments: HandleSpan::empty(),
                })
            }));
        } else {
            constraints.push(
                match ArithmeticDomain::from_name(domain_name.as_str(syntax_trees)) {
                    Some(domain) if arguments.is_empty() => {
                        TypeConstraintNode::ArithmeticDomain(domain)
                    }
                    Some(_) => {
                        return Err(rest.error_here(
                            "compiler arithmetic domains do not take index arguments",
                        ));
                    }
                    None => TypeConstraintNode::Domain(DomainConstraint {
                        name: domain_name,
                        arguments,
                    }),
                },
            );
        }

        if !rest.at_punctuation(PunctuationKind::Ampersand) {
            cursor = rest;
            break;
        }
        cursor = rest.take_punctuation(PunctuationKind::Ampersand, "&")?;
    }
    let constraints = type_refs(syntax_trees).insert_constraints(constraints);
    let type_reference = syntax_trees
        .type_references
        .insert(TypeReferenceNode::Constrained {
            base_type: type_reference,
            constraints,
        });
    Ok((type_reference, cursor))
}

pub(crate) fn parse_domain_argument_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<TypeReferenceHandle>> {
    if !input.at_punctuation(PunctuationKind::Less) {
        return Ok((HandleSpan::empty(), input));
    }
    let mut input = input.take_punctuation(PunctuationKind::Less, "<")?;
    let mut arguments = Vec::new();
    loop {
        let (argument, rest) = parse_static_argument_reference(syntax_trees, input)?;
        input = rest;
        arguments.push(argument);
        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }
        input = input.take_punctuation(PunctuationKind::Greater, ">")?;
        break;
    }
    Ok((
        type_refs(syntax_trees).insert_type_reference_handles(arguments),
        input,
    ))
}

pub(crate) fn parse_type_reference_handle_allowing_borrow<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    parse_borrow_type_reference(syntax_trees, input, true)
}

fn parse_borrow_type_reference<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    allow_trailing_domain: bool,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    let (is_reference, lifetime, access, input) =
        if input.at_punctuation(PunctuationKind::Ampersand) {
            let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
            let (lifetime, input) = parse_optional_lifetime(input)?;
            let (access, input) = input.take_reference_access()?;
            if input.at_contextual("relaxed") {
                return Err(input.error_here(
                    "`&relaxed` is retired: invariant windows (ch11) supersede the \
                     relax surface -- take the ordinary reference",
                ));
            }
            (true, lifetime, access, input)
        } else {
            (false, None, language_core::ReferenceAccess::Shared, input)
        };

    let (type_reference, input) = parse_type_reference_handle_with_trailing_domain(
        syntax_trees,
        input,
        allow_trailing_domain,
    )?;
    let type_reference = if is_reference {
        type_refs(syntax_trees).insert(TypeReferenceNode::Reference {
            referee: type_reference,
            access,
            lifetime,
        })
    } else {
        type_reference
    };

    Ok((type_reference, input))
}

pub(crate) fn parse_optional_lifetime<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Option<syntax_trees::identifier::Identifier>> {
    if !input.at_punctuation(PunctuationKind::Apostrophe) {
        return Ok((None, input));
    }
    let input = input.take_punctuation(PunctuationKind::Apostrophe, "'")?;
    let (name, input) = input.take_identifier()?;
    Ok((Some(name), input))
}
