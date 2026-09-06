use crate::parser::expression::{
    parse_const_integer_expression_handle, parse_expression_handle_without_struct_literals,
};
use crate::parser::input::{Input, ParseResult};
use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};
use syntax_trees::identifier::Identifier;
use syntax_trees::types::{
    DomainConstraint, FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use tokens::{KeywordKind, PunctuationKind};

#[cfg(test)]
mod remainder_tests;

pub(super) fn parse_type_reference_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    parse_type_reference_handle_with_trailing_domain(syntax_trees, input, true)
}

/// Cast/recast targets leave their trailing `in <Domain>` for the cast parser:
/// it is an arithmetic/semantic qualification on the conversion, and recasts
/// reject it. Nested type references retain their ordinary domain grammar.
pub(super) fn parse_cast_target_type_reference_handle<'tokens, 'source>(
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
    if input.at_punctuation(PunctuationKind::LeftParen) {
        let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        return Ok((syntax_trees.type_references.insert_unit(), input));
    }

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        let input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
        let (element_type, mut input) = parse_type_reference_handle(syntax_trees, input)?;
        if input.at_punctuation(PunctuationKind::Semicolon) {
            let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            let (length, input) = if input
                .tokens
                .first()
                .is_some_and(crate::parser::input::is_identifier_token_for_parser)
            {
                let (name, input) = input.take_identifier()?;
                // `[T; table_size()]`: a zero-argument machine call in length
                // position is CONST-EVALUATED at compile time (comptime stage 1).
                if input.at_punctuation(PunctuationKind::LeftParen) {
                    let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
                    if !input.at_punctuation(PunctuationKind::RightParen) {
                        return Err(input.error_here(
                            "const arguments in array-length calls are not supported yet; \
                             the const-evaluated machine must take zero parameters",
                        ));
                    }
                    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
                    (FixedArrayLength::ConstCall(name), input)
                } else {
                    (FixedArrayLength::ConstParameter(name), input)
                }
            } else {
                let (length, input) = input.take_integer()?;
                let length = usize::try_from(length)
                    .map_err(|_| input.error_here("expected non-negative array length"))?;
                (FixedArrayLength::Literal(length), input)
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

        let mut type_reference = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Slice { element_type });

        if input.at_punctuation(PunctuationKind::Comma) {
            let input_after_comma = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (constraints, rest) =
                parse_type_constraint_handles(syntax_trees, input_after_comma)?;
            input = rest;
            type_reference = syntax_trees
                .type_references
                .insert(TypeReferenceNode::Constrained {
                    base_type: type_reference,
                    constraints,
                });
        }

        let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
        // A slice can carry an encoding domain: `[u8] in Utf8` (ch8).
        return if allow_trailing_domain {
            apply_in_domain_suffix(syntax_trees, type_reference, input)
        } else {
            Ok((type_reference, input))
        };
    }

    if input.at_keyword(KeywordKind::SelfType) {
        let input = input.take_keyword(KeywordKind::SelfType, "Self")?;
        return Ok((syntax_trees.type_references.insert_self_type(), input));
    }

    if input.at_contextual("dyn") {
        let input = input.take_contextual("dyn")?;
        let (trait_name, mut input) = input.take_identifier()?;
        let conformance = if input.at_punctuation(PunctuationKind::ColonColon) {
            input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
            let (conformance, rest) = input.take_identifier()?;
            input = rest;
            Some(conformance)
        } else {
            None
        };
        return Ok((
            syntax_trees
                .type_references
                .insert(TypeReferenceNode::DynamicTrait {
                    name: trait_name,
                    conformance,
                }),
            input,
        ));
    }

    let (base_name, mut input) = input.take_identifier()?;

    let mut type_reference = if input.at_punctuation(PunctuationKind::Less) {
        input = input.take_punctuation(PunctuationKind::Less, "<")?;
        let mut lifetime_arguments = Vec::new();
        let mut argument_start = Handle::invalid();
        let mut argument_count = 0u32;
        let mut first_argument = TypeReferenceHandle::invalid();

        loop {
            if input.at_punctuation(PunctuationKind::Apostrophe) {
                if argument_count != 0 {
                    return Err(input.error_here(
                        "lifetime arguments precede type, const, and machine arguments",
                    ));
                }
                let input_after_tick = input.take_punctuation(PunctuationKind::Apostrophe, "'")?;
                let (lifetime, rest) = input_after_tick.take_identifier()?;
                lifetime_arguments.push(lifetime);
                input = rest;

                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    continue;
                }
                break;
            }

            // Const data arguments share the generic argument list with type
            // arguments. Keep literal values and scoped const paths as
            // symbol-free named leaves until the declaration's parameter kinds
            // are available during validation/layout; literal decimal spelling
            // is canonical.
            let (argument, rest) = if input.at_integer()
                || input.at_punctuation(PunctuationKind::Minus)
            {
                let expression_start = input;
                let (expression, rest) =
                    parse_const_integer_expression_handle(syntax_trees, input)?;
                if const_expression_requires_semantic_admission(syntax_trees, expression) {
                    (
                        syntax_trees
                            .type_references
                            .insert(TypeReferenceNode::ConstExpression(expression)),
                        rest,
                    )
                } else {
                    let value = evaluate_closed_const_integer_expression(syntax_trees, expression)
                        .map_err(|reason| expression_start.error_here(reason))?;
                    (
                        syntax_trees
                            .type_references
                            .insert_named(Identifier::generated(value.to_string())),
                        rest,
                    )
                }
            } else if input
                .tokens
                .first()
                .is_some_and(crate::parser::input::is_identifier_token_for_parser)
            {
                // An identifier-starting argument is ambiguous until the base
                // declaration supplies its parameter kinds: `Box<T>` is a type
                // argument, `Buffer<N>` is a const argument, and
                // `Buffer<N + 1>` is an unmistakable symbolic const expression.
                // Parse the delimiter-safe integer operator subset first. A
                // binary result is retained through the pre-resolution generic
                // instance pass; a lone name keeps the established type/scoped-
                // const leaf representation.
                let const_expression =
                    parse_const_integer_expression_handle(syntax_trees, input).ok();
                if let Some((expression, expression_rest)) = const_expression
                    && matches!(
                        syntax_trees.expressions.expression(expression),
                        ExpressionNode::Binary(_) | ExpressionNode::Call(_)
                    )
                    && (expression_rest.at_punctuation(PunctuationKind::Comma)
                        || expression_rest.at_punctuation(PunctuationKind::Greater))
                {
                    (
                        syntax_trees
                            .type_references
                            .insert(TypeReferenceNode::ConstExpression(expression)),
                        expression_rest,
                    )
                } else {
                    let (scope, after_scope) = input.take_identifier()?;
                    if after_scope.at_punctuation(PunctuationKind::ColonColon) {
                        let after_separator =
                            after_scope.take_punctuation(PunctuationKind::ColonColon, "::")?;
                        let (name, rest) = after_separator.take_identifier()?;
                        (
                            syntax_trees
                                .type_references
                                .insert_named(Identifier::generated(format!("{scope}::{name}"))),
                            rest,
                        )
                    } else {
                        parse_type_reference_handle(syntax_trees, input)?
                    }
                }
            } else {
                parse_type_reference_handle(syntax_trees, input)?
            };
            if argument_count == 0 {
                first_argument = argument;
            }
            let handle = syntax_trees
                .type_references
                .append_type_reference_handle(argument);
            if argument_count == 0 {
                argument_start = handle;
            }
            argument_count = argument_count
                .checked_add(1)
                .expect("type reference argument span count overflow");
            input = rest;

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }

            break;
        }

        input = input.take_punctuation(PunctuationKind::Greater, ">")?;
        let arguments = if argument_count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(argument_start, argument_count)
        };
        // TASK RUNTIME TR1: reject the erased stage-1 handle explicitly.
        // `Task<T>` becomes a real linear core type in TR2; silently folding a
        // lifecycle claim into its result would recreate the bug this pass
        // removes.
        if base_name.as_str() == "Join" {
            return Err(input.error_here(
                "`Join<T>` is retired: task activation returns a linear `Task<T>`; \
                 settle it with `finish()` or transfer it to another owner",
            ));
        } else if base_name.as_str() == "Slice"
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

/// Shifts and bitwise operations need declared width; remainder needs operand
/// type admission. Preserve these expressions for the semantic owner rather
/// than erasing the operation while parsing a constant argument.
fn const_expression_requires_semantic_admission(
    syntax_trees: &SyntaxTrees,
    expression: syntax_trees::expression::ExpressionHandle,
) -> bool {
    let ExpressionNode::Binary(binary) = syntax_trees.expressions.expression(expression) else {
        return false;
    };
    matches!(
        binary.operator,
        BinaryOperator::Modulo
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
    ) || const_expression_requires_semantic_admission(syntax_trees, binary.left)
        || const_expression_requires_semantic_admission(syntax_trees, binary.right)
}

fn const_expression_contains_name(
    syntax_trees: &SyntaxTrees,
    expression: syntax_trees::expression::ExpressionHandle,
) -> bool {
    match syntax_trees.expressions.expression(expression) {
        ExpressionNode::Name(_) => true,
        ExpressionNode::Binary(binary) => {
            const_expression_contains_name(syntax_trees, binary.left)
                || const_expression_contains_name(syntax_trees, binary.right)
        }
        ExpressionNode::Unary(unary) => const_expression_contains_name(syntax_trees, unary.operand),
        _ => false,
    }
}

/// Fold the first richer const-argument slice before generic-instance
/// synthesis: closed integer expressions in the language's current 64-bit
/// signed/unsigned envelope. The folded decimal
/// leaf is the same representation a literal argument already used, so symbol
/// resolution, type identity, layout, and runtime specialization remain
/// unchanged. Symbolic expressions are intentionally left for the subsequent
/// const-fact slice, where declaration types and parameter bindings are known.
fn evaluate_closed_const_integer_expression(
    syntax_trees: &SyntaxTrees,
    expression: syntax_trees::expression::ExpressionHandle,
) -> Result<i128, String> {
    match syntax_trees.expressions.expression(expression) {
        ExpressionNode::Integer(value) => const_literal_value(value),
        ExpressionNode::Binary(binary) => {
            let left = evaluate_closed_const_integer_expression(syntax_trees, binary.left)?;
            let right = evaluate_closed_const_integer_expression(syntax_trees, binary.right)?;
            match binary.operator {
                BinaryOperator::Add => checked_const_integer(left.checked_add(right), "addition"),
                BinaryOperator::Subtract => {
                    checked_const_integer(left.checked_sub(right), "subtraction")
                }
                BinaryOperator::Multiply => {
                    checked_const_integer(left.checked_mul(right), "multiplication")
                }
                BinaryOperator::Divide => left.checked_div(right).ok_or_else(|| {
                    "const data argument division by zero is invalid".to_string()
                }),
                BinaryOperator::Modulo => left.checked_rem(right).ok_or_else(|| {
                    "const data argument remainder by zero is invalid".to_string()
                }),
                BinaryOperator::ShiftLeft if left >= 0 && right >= 0 => {
                    let shifted = u32::try_from(right)
                        .ok()
                        .filter(|amount| *amount < u64::BITS)
                        .and_then(|amount| left.checked_shl(amount));
                    checked_const_integer(shifted, "left shift")
                }
                BinaryOperator::ShiftRight if left >= 0 && right >= 0 => u32::try_from(right)
                    .ok()
                    .filter(|amount| *amount < u64::BITS)
                    .and_then(|amount| left.checked_shr(amount))
                    .ok_or_else(|| {
                        "const data argument right shift exceeds the `u64` width".to_string()
                    }),
                BinaryOperator::BitwiseAnd if left >= 0 && right >= 0 => Ok(left & right),
                BinaryOperator::BitwiseOr if left >= 0 && right >= 0 => Ok(left | right),
                BinaryOperator::BitwiseXor if left >= 0 && right >= 0 => Ok(left ^ right),
                _ => Err(
                    "signed const data arguments support arithmetic; signed shifts and bitwise operators require declared-width semantics"
                        .to_string(),
                ),
            }
        }
        _ => Err(
            "const data arguments currently support only closed integer expressions".to_string(),
        ),
    }
}

fn const_literal_value(value: &numerics::literals::IntegerLiteral) -> Result<i128, String> {
    value
        .value_i64()
        .map(i128::from)
        .or_else(|| value.value_u64().map(i128::from))
        .ok_or_else(|| {
            "const data arguments must fit either the signed or unsigned 64-bit range".to_string()
        })
}

fn checked_const_integer(value: Option<i128>, operation: &str) -> Result<i128, String> {
    let value = value.ok_or_else(|| format!("const data argument {operation} overflows"))?;
    if value < i128::from(i64::MIN) || value > i128::from(u64::MAX) {
        return Err(format!(
            "const data argument {operation} exceeds the signed/unsigned 64-bit envelope"
        ));
    }
    Ok(value)
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
        let (first_domain_name, mut rest) = cursor.take_identifier()?;
        let mut qualified_domain_name = first_domain_name.as_str().to_owned();
        let mut qualified_domain_span = first_domain_name.source_span();
        while rest.at_punctuation(PunctuationKind::ColonColon) {
            rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
            let (member, next) = rest.take_identifier()?;
            qualified_domain_name.push_str("::");
            qualified_domain_name.push_str(member.as_str());
            qualified_domain_span.span.end = member.source_span().span.end;
            rest = next;
        }
        let domain_name = if qualified_domain_name == first_domain_name.as_str() {
            first_domain_name
        } else {
            Identifier::new(qualified_domain_name, qualified_domain_span)
        };
        let (arguments, rest) = parse_domain_argument_handles(syntax_trees, rest)?;
        if domain_name.as_str() == "Carry::Portable" {
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
                match numerics::arithmetic::ArithmeticDomain::from_name(domain_name.as_str()) {
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
/// application. PDI2 accepts closed literals/named constants and direct const
/// binders; PDI3 owns operator expressions over those binders.
pub(super) fn parse_domain_argument_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<TypeReferenceHandle>> {
    if !input.at_punctuation(PunctuationKind::Less) {
        return Ok((HandleSpan::empty(), input));
    }
    let mut input = input.take_punctuation(PunctuationKind::Less, "<")?;
    let mut arguments = Vec::new();
    loop {
        // Parse the whole proof-static expression first. A lone name remains
        // the PDI2 closed-constant/direct-binder leaf; a compound expression
        // is retained for PDI3 normalization after names and operators have
        // exact symbols. This also keeps `>` as the argument-pack delimiter:
        // the dedicated const-expression parser deliberately stops before the
        // comparison layer.
        let (expression, rest) = parse_const_integer_expression_handle(syntax_trees, input)?;
        input = rest;
        let argument = match syntax_trees.expressions.expression(expression) {
            ExpressionNode::Name(_) => {
                let name = syntax_trees.expressions.display_name(expression);
                syntax_trees
                    .type_references
                    .insert_named(Identifier::generated(name))
            }
            _ if const_expression_contains_name(syntax_trees, expression)
                || const_expression_requires_semantic_admission(syntax_trees, expression) =>
            {
                syntax_trees
                    .type_references
                    .insert(TypeReferenceNode::ConstExpression(expression))
            }
            _ => {
                let value = evaluate_closed_const_integer_expression(syntax_trees, expression)
                    .map_err(|reason| input.error_here(reason))?;
                syntax_trees
                    .type_references
                    .insert_named(Identifier::generated(value.to_string()))
            }
        };
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

pub(super) fn parse_type_reference_handle_allowing_borrow<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    let (is_reference, lifetime, access, input) =
        if input.at_punctuation(PunctuationKind::Ampersand) {
            let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
            // Explicit lifetime (`&'buf T`, `&'buf mut T`); frozen decision 15
            // stage 2. The tick precedes `mut`/`relaxed`, mirroring Rust's
            // `&'a mut T`. A bare `&T` keeps the elided (`None`) form.
            let (lifetime, input) = parse_optional_lifetime(input)?;
            let (access, input) = if input.at_contextual("mut") {
                (
                    language_core::ReferenceAccess::Mutable,
                    input.take_contextual("mut")?,
                )
            } else if input.at_contextual("write") {
                (
                    language_core::ReferenceAccess::WriteOnly,
                    input.take_contextual("write")?,
                )
            } else {
                (language_core::ReferenceAccess::Shared, input)
            };
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

    let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
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
pub(super) fn parse_optional_lifetime<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Option<syntax_trees::identifier::Identifier>> {
    if !input.at_punctuation(PunctuationKind::Apostrophe) {
        return Ok((None, input));
    }
    let input = input.take_punctuation(PunctuationKind::Apostrophe, "'")?;
    let (name, input) = input.take_identifier()?;
    Ok((Some(name), input))
}

pub(super) fn parse_type_constraint_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<TypeConstraintNode>> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
    let mut constraint_start = Handle::invalid();
    let mut constraint_count = 0u32;

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
                // The node stores an INCLUSIVE maximum, so an exclusive bound is
                // normalised to `max - 1` at parse: a LITERAL bound folds here
                // (`[0..8]` stores 7); a SYMBOLIC bound (`[0..self.count]`, the
                // R1 dependent-range surface) synthesizes `max - 1` as a Binary
                // so every downstream consumer keeps reading an inclusive
                // maximum unchanged.
                let (minimum, rest) =
                    parse_expression_handle_without_struct_literals(syntax_trees, input)?;
                if rest.at_punctuation(PunctuationKind::DotDotEqual) {
                    let rest = rest.take_punctuation(PunctuationKind::DotDotEqual, "..=")?;
                    let (maximum, rest) =
                        parse_expression_handle_without_struct_literals(syntax_trees, rest)?;
                    input = rest;
                    TypeConstraintNode::Range { minimum, maximum }
                } else if rest.at_punctuation(PunctuationKind::DotDot) {
                    let rest = rest.take_punctuation(PunctuationKind::DotDot, "..")?;
                    let (end_exclusive, rest) =
                        parse_expression_handle_without_struct_literals(syntax_trees, rest)?;
                    input = rest;
                    let maximum = match syntax_trees.expressions.expression(end_exclusive) {
                        ExpressionNode::Integer(literal) => {
                            let Some(value) = literal.value_i64() else {
                                return Err(input.error_here(
                                    "exclusive range bound exceeds i64; this position needs a parse-time number",
                                ));
                            };
                            syntax_trees.expressions.insert(ExpressionNode::Integer(
                                numerics::literals::IntegerLiteral::from_value(value - 1),
                            ))
                        }
                        _ => {
                            let one = syntax_trees.expressions.insert(ExpressionNode::Integer(
                                numerics::literals::IntegerLiteral::from_value(1),
                            ));
                            syntax_trees.expressions.insert(ExpressionNode::Binary(
                                TableBinaryExpression {
                                    left: end_exclusive,
                                    operator: BinaryOperator::Subtract,
                                    right: one,
                                },
                            ))
                        }
                    };
                    TypeConstraintNode::Range { minimum, maximum }
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

            let handle = syntax_trees.type_references.append_constraint(constraint);
            if constraint_count == 0 {
                constraint_start = handle;
            }
            constraint_count = constraint_count
                .checked_add(1)
                .expect("type constraint span count overflow");

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }

            break;
        }
    }

    input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
    let constraints = if constraint_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(constraint_start, constraint_count)
    };
    Ok((constraints, input))
}
