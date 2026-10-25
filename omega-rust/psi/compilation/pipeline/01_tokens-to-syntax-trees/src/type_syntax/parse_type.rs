//! Generic argument syntax retains the authored value expression.
//!
//! Binder kind, carrier and selected operations belong to semantic admission.
//! A direct name remains ambiguous between a type and a value until resolution;
//! every other value tree is retained without parser-side evaluation.

use crate::expressions::parse_expression::{
    parse_const_integer_expression_handle, parse_expression_handle_without_struct_literals,
};
use crate::input::token_cursor::{Input, ParseResult};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::identifier::Identifier;
use syntax_trees::types::{
    DomainConstraint, FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use tokens::PunctuationKind;

/// Type, extent and domain names share the ordinary static-path grammar,
/// including a cast's `in <Domain>` qualification. Keep one source span so
/// later selection retains the authored occurrence.
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

/// Cast/recast targets leave their trailing `in <Domain>` for the cast parser:
/// it is an arithmetic/semantic qualification on the conversion, and recasts
/// reject it. Nested type references retain their ordinary domain grammar.
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
    // Bound nesting depth: nested array types (`[[...;1];1]`) recurse through
    // this choke point. Deepen on entry to reject pathological nesting before it
    // overflows the stack, restoring the caller's depth on exit (see the mirror
    // in `parse_expression_handle_in`).
    let outer_depth = input.depth();
    let input = input.deepen()?;
    let (type_reference, rest) =
        parse_type_reference_handle_inner(syntax_trees, input, allow_trailing_domain)?;
    Ok((type_reference, rest.with_depth(outer_depth)))
}

fn parse_type_reference_handle_inner<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    allow_trailing_domain: bool,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    if input.at_punctuation(PunctuationKind::Ampersand) {
        // References compose inside static arguments and other type constructors.
        // Keep cast suffix ownership through the recursive referee as well.
        return parse_borrow_type_reference(syntax_trees, input, allow_trailing_domain);
    }
    if input.at_punctuation(PunctuationKind::LeftParen) {
        let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        return Ok((syntax_trees.type_references.insert_unit(), input));
    }

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        let input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
        let (element_type, input) = parse_type_reference_handle(syntax_trees, input)?;
        if input.at_punctuation(PunctuationKind::Semicolon) {
            let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            // Extents are ordinary constant positions, not a second expression
            // grammar. Retain non-leaf expressions for checked semantic evaluation.
            let (expression, input) =
                crate::expressions::parse_expression::parse_expression_handle(syntax_trees, input)?;
            let length = match syntax_trees.expressions.expression(expression) {
                ExpressionNode::Integer(value) => {
                    // Preserve the existing literal extent representation when
                    // its exact nonnegative magnitude is already known.
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
                ExpressionNode::Name(path) => {
                    let members = syntax_trees.expressions.identifier_path_members(*path);
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
            // A fixed array can carry an encoding domain: `[u8; 32] in Utf8`.
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
        // A slice can carry an encoding domain: `[u8] in Utf8` (ch8).
        return if allow_trailing_domain {
            apply_in_domain_suffix(syntax_trees, type_reference, input)
        } else {
            Ok((type_reference, input))
        };
    }

    // `dyn Trait<Args>` names a trait application with the same argument
    // grammar as `satisfies Trait<Args>`. A selector names no data carrier
    // and no implementation: `Impl as dyn Trait<Args>` chooses one.
    if input.at_contextual("dyn") {
        let input = input.take_contextual("dyn")?;
        let ((trait_name, lifetime_arguments, trait_arguments), input) =
            crate::contracts::conformance::parse_conformance::parse_conformance_trait_application(
                syntax_trees,
                input,
            )?;
        if !lifetime_arguments.is_empty() {
            return Err(input
                .error_here("a dynamic selector takes no lifetime arguments; it borrows nothing"));
        }
        if input.at_punctuation(PunctuationKind::ColonColon) {
            return Err(input.error_here(
                "`dyn Data::Implementation` is retired; name the trait application (`dyn Trait<Data>`) and select an implementation with `Implementation as dyn Trait<Data>`",
            ));
        }
        return Ok((
            syntax_trees
                .type_references
                .insert(TypeReferenceNode::DynamicTrait {
                    trait_name,
                    trait_arguments,
                }),
            input,
        ));
    }

    let (base_name, mut input) = parse_static_name(input)?;

    let mut type_reference = if input.at_punctuation(PunctuationKind::Less) {
        input = input.take_punctuation(PunctuationKind::Less, "<")?;
        let mut lifetime_arguments = Vec::new();
        // Every argument's own children -- its base type and, for a nested
        // application, that application's arguments -- land in the same
        // type-reference handle table as this list. Collect the argument
        // handles and insert the span once after the list closes, the way
        // `parse_domain_argument_handles` already does: appending each handle
        // as it is parsed interleaves a nested application's arguments with
        // this list and leaves the two authored spans overlapping.
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

            continue;
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
        // TASK RUNTIME TR1: reject the erased stage-1 handle explicitly.
        // `Task<T>` becomes a real linear core type in TR2; silently folding a
        // lifecycle claim into its result would recreate the bug this pass
        // removes.
        if base_name.as_str(syntax_trees) == "Join" {
            return Err(input.error_here(
                "`Join<T>` is retired: task activation returns a linear `Task<T>`; \
                 settle it with `finish()` or transfer it to another owner",
            ));
        } else if base_name.as_str(syntax_trees) == "Slice"
            && lifetime_arguments.is_empty()
            && argument_count == 1
        {
            // `Slice<T>` is the canonical slice type; `[T]` is its alias. Both fold
            // to the same `Slice` node, so the spellings are interchangeable
            // (`Slice<u8> in Utf8` == `[u8] in Utf8`). Like `Join`, `Slice` is a
            // reserved type name so the fold is unambiguous with user generics.
            syntax_trees
                .type_references
                .insert(TypeReferenceNode::Slice {
                    element_type: first_argument,
                })
        } else {
            syntax_trees
                .type_references
                .insert(TypeReferenceNode::Generic {
                    base_name,
                    lifetime_arguments,
                    arguments,
                })
        }
    } else {
        syntax_trees.type_references.insert_named(base_name)
    };

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        let (constraints, rest) = parse_type_constraint_handles(syntax_trees, input)?;
        input = rest;
        type_reference = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Constrained {
                base_type: type_reference,
                constraints,
            });
    }

    let (type_reference, input) = if allow_trailing_domain {
        apply_in_domain_suffix(syntax_trees, type_reference, input)?
    } else {
        (type_reference, input)
    };
    Ok((type_reference, input))
}

/// Data and domain applications share type/value argument syntax. Binder kinds
/// are decided after resolution, so neither caller may restrict a named index
/// to a const leaf or discard the structure of a nested type application.
fn parse_static_argument_reference<'tokens, 'source>(
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
        let (expression, rest) = parse_const_integer_expression_handle(syntax_trees, input)?;
        if let ExpressionNode::Name(_) = syntax_trees.expressions.expression(expression) {
            // Parentheses do not turn a direct binder or named constant into
            // a computed index. Retain the same leaf as its unparenthesized
            // spelling, independently of which application owns the pack.
            let name = Identifier::new(
                syntax_trees.display_expression_name(expression),
                syntax_trees.expressions.source_span(expression),
            );
            return Ok((syntax_trees.type_references.insert_named(name), rest));
        }
        Ok((
            syntax_trees
                .type_references
                .insert(TypeReferenceNode::ConstExpression(expression)),
            rest,
        ))
    } else if input
        .tokens
        .first()
        .is_some_and(crate::input::token_cursor::is_identifier_token_for_parser)
    {
        // An identifier-starting argument is ambiguous until the base
        // declaration supplies its parameter kinds: `Box<T>` is a type
        // argument, `Buffer<N>` is a const argument, and
        // `Buffer<N + 1>` is an unmistakable symbolic const expression.
        // Retain a delimiter-safe expression recipe until the receiving
        // binder determines its Value/Const role. A lone name remains
        // ambiguous with a type and keeps the ordinary resolved leaf.
        let const_expression = parse_const_integer_expression_handle(syntax_trees, input).ok();
        if let Some((expression, expression_rest)) = const_expression
            && !matches!(
                syntax_trees.expressions.expression(expression),
                ExpressionNode::Name(_)
            )
            && (expression_rest.at_punctuation(PunctuationKind::Comma)
                || expression_rest.at_punctuation(PunctuationKind::Greater))
        {
            Ok((
                syntax_trees
                    .type_references
                    .insert(TypeReferenceNode::ConstExpression(expression)),
                expression_rest,
            ))
        } else {
            parse_type_reference_handle(syntax_trees, input)
        }
    } else {
        parse_type_reference_handle(syntax_trees, input)
    }
}

/// Apply an optional `in <Domain>` suffix to a just-parsed type reference.
///
/// The name is an arithmetic overflow domain (`Wrapping`/`Saturating`/`Trapping`;
/// frozen decision 17) when it matches one -- opting a primitive into defined
/// overflow behaviour while bare arithmetic stays exact (a proof obligation).
/// Otherwise it is a DECLARED encoding domain on the carrier (`[u8] in Utf8`;
/// ch8 "domains over carriers"). Either way the domain rides as a constraint on
/// a `Constrained` type-reference -- the same carrier as `a..=b` -- so
/// layout/codegen see through to the base type.
///
/// An unknown encoding-domain name is NOT a parse error: validation resolves it
/// against `domain ...::Name` declarations (rejecting typos with a clear
/// message). `in` is the contextual membership keyword; in TYPE position nothing
/// else consumes a trailing `in`, so this suffix is additive. Factored out so it
/// applies after slice/array returns too (`[u8] in Utf8`), not just named types.
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
                match numerics::arithmetic::ArithmeticDomain::from_name(
                    domain_name.as_str(syntax_trees),
                ) {
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
    let constraints = syntax_trees.type_references.insert_constraints(constraints);
    let type_reference = syntax_trees
        .type_references
        .insert(TypeReferenceNode::Constrained {
            base_type: type_reference,
            constraints,
        });
    Ok((type_reference, cursor))
}

/// Parse the proof-static argument pack used by a domain-family declaration or
/// application. Types retain recursive structure; values retain expressions
/// requiring semantic admission. Resolution determines the receiving role.
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
        syntax_trees
            .type_references
            .insert_type_reference_handles(arguments),
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
            // Explicit lifetime (`&'buf T`, `&'buf mut T`); frozen decision 15
            // stage 2. The tick precedes `mut`/`relaxed`, mirroring Rust's
            // `&'a mut T`. A bare `&T` keeps the elided (`None`) form.
            let (lifetime, input) = parse_optional_lifetime(input)?;
            let (access, input) = input.take_reference_access()?;
            // `relaxed` references RETIRED with the relax surface (owner,
            // 2026-07-17): ch11 windows carry the momentary-violation
            // semantics; the reference marker has no meaning left.
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
        syntax_trees
            .type_references
            .insert(TypeReferenceNode::Reference {
                referee: type_reference,
                access,
                lifetime,
            })
    } else {
        type_reference
    };

    Ok((type_reference, input))
}

/// Parse a `'name` lifetime if present, returning its identifier. The lexer
/// emits `'` as `Apostrophe` punctuation immediately followed by the name
/// identifier (frozen decision 15 stage 2).
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

pub(crate) fn parse_type_constraint_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<TypeConstraintNode>> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
    // A bound expression can carry its own constrained type reference (a cast
    // target, for instance), whose constraints land in this same table. Collect
    // the run and insert it once, the way the `in <Domain>` suffix already does.
    let mut constraints: Vec<TypeConstraintNode> = Vec::new();

    if !input.at_punctuation(PunctuationKind::RightBracket) {
        loop {
            // An identifier may start an ordinary range bound. Parse that
            // expression before distinguishing a range from retired proof sugar.
            // Data and generic-parameter properties use parse_property_brackets.
            let starts_with_name = input.at_name_like();
            let constraint = if input.at_contextual("range")
                && input
                    .take_identifier()?
                    .1
                    .at_punctuation(PunctuationKind::Less)
            {
                return Err(input.error_here(
                    "the `range<a, b>` constraint syntax is removed; use `a..=b` (inclusive) or `a..b` (exclusive)",
                ));
            } else {
                // Range refinement: `min..=max` (inclusive) or `min..max` (exclusive).
                // Preserve the authored endpoint and boundary kind. Semantic
                // normalization checks its original arithmetic before taking a
                // predecessor in proof integers, without a runtime subtraction
                // or lost literal landing here.
                let (minimum, rest) =
                    parse_expression_handle_without_struct_literals(syntax_trees, input)?;
                if rest.at_punctuation(PunctuationKind::DotDotEqual) {
                    let rest = rest.take_punctuation(PunctuationKind::DotDotEqual, "..=")?;
                    let (maximum, rest) =
                        parse_expression_handle_without_struct_literals(syntax_trees, rest)?;
                    input = rest;
                    TypeConstraintNode::Range {
                        minimum,
                        maximum,
                        end_inclusive: true,
                    }
                } else if rest.at_punctuation(PunctuationKind::DotDot) {
                    let rest = rest.take_punctuation(PunctuationKind::DotDot, "..")?;
                    let (end_exclusive, rest) =
                        parse_expression_handle_without_struct_literals(syntax_trees, rest)?;
                    input = rest;
                    TypeConstraintNode::Range {
                        minimum,
                        maximum: end_exclusive,
                        end_inclusive: false,
                    }
                } else if starts_with_name {
                    return Err(rest.error_here(
                        "named proof constraints in type brackets are retired; use `in Domain` for a declared value domain or express the fact in contracts",
                    ));
                } else {
                    return Err(
                        rest.error_here("range constraint requires `..` or `..=` between bounds")
                    );
                }
            };

            constraints.push(constraint);

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }

            break;
        }
    }

    input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
    let constraints = syntax_trees.type_references.insert_constraints(constraints);
    Ok((constraints, input))
}
