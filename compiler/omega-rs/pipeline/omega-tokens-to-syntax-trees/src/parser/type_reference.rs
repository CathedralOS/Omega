use crate::parser::expression::{
    parse_const_integer_expression_handle, parse_expression_handle_without_struct_literals,
};
use crate::parser::input::{Input, ParseResult};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_type_reference_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    // Bound nesting depth: nested array types (`[[...;1];1]`) recurse through
    // this choke point. Deepen on entry to reject pathological nesting before it
    // overflows the stack, restoring the caller's depth on exit (see the mirror
    // in `parse_expression_handle_in`).
    let outer_depth = input.depth();
    let input = input.deepen()?;
    let (type_reference, rest) = parse_type_reference_handle_inner(syntax_trees, input)?;
    Ok((type_reference, rest.with_depth(outer_depth)))
}

fn parse_type_reference_handle_inner<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
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
            return apply_in_domain_suffix(syntax_trees, array, input);
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
        return apply_in_domain_suffix(syntax_trees, type_reference, input);
    }

    if input.at_keyword(KeywordKind::SelfType) {
        let input = input.take_keyword(KeywordKind::SelfType, "Self")?;
        return Ok((syntax_trees.type_references.insert_self_type(), input));
    }

    if input.at_contextual("dyn") {
        let input = input.take_contextual("dyn")?;
        let (trait_name, input) = input.take_identifier()?;
        return Ok((
            syntax_trees
                .type_references
                .insert(TypeReferenceNode::DynamicTrait(trait_name)),
            input,
        ));
    }

    let (base_name, mut input) = input.take_identifier()?;

    let mut type_reference = if input.at_punctuation(PunctuationKind::Less) {
        input = input.take_punctuation(PunctuationKind::Less, "<")?;
        let mut argument_start = Handle::invalid();
        let mut argument_count = 0u32;
        let mut first_argument = TypeReferenceHandle::invalid();

        loop {
            // Const data arguments share the generic argument list with type
            // arguments. Keep literal values and scoped const paths as
            // symbol-free named leaves until the declaration's parameter kinds
            // are available during validation/layout; literal decimal spelling
            // is canonical.
            let (argument, rest) = if input.at_integer() {
                let expression_start = input;
                let (expression, rest) =
                    parse_const_integer_expression_handle(syntax_trees, input)?;
                let value = evaluate_closed_const_integer_expression(syntax_trees, expression)
                    .map_err(|reason| expression_start.error_here(reason))?;
                (
                    syntax_trees
                        .type_references
                        .insert_named(Identifier::generated(value.to_string())),
                    rest,
                )
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
                let (expression, expression_rest) =
                    parse_const_integer_expression_handle(syntax_trees, input)?;
                if matches!(
                    syntax_trees.expressions.expression(expression),
                    ExpressionNode::Binary(_) | ExpressionNode::Call(_)
                ) && (expression_rest.at_punctuation(PunctuationKind::Comma)
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
        if base_name.as_str() == "Join" && argument_count == 1 {
            return Err(input.error_here(
                "`Join<T>` is retired: task activation returns a linear `Task<T>`; \
                 settle it with `finish()` or transfer it to another owner",
            ));
        } else if base_name.as_str() == "Slice" && argument_count == 1 {
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

    let (type_reference, input) = apply_in_domain_suffix(syntax_trees, type_reference, input)?;
    Ok((type_reference, input))
}

/// Fold the first richer const-argument slice before generic-instance
/// synthesis: closed, non-negative integer expressions. The folded decimal
/// leaf is the same representation a literal argument already used, so symbol
/// resolution, type identity, layout, and runtime specialization remain
/// unchanged. Symbolic expressions are intentionally left for the subsequent
/// const-fact slice, where declaration types and parameter bindings are known.
fn evaluate_closed_const_integer_expression(
    syntax_trees: &SyntaxTrees,
    expression: omega_syntax_trees::expression::ExpressionHandle,
) -> Result<u64, String> {
    match syntax_trees.expressions.expression(expression) {
        ExpressionNode::Integer(value) => value.value_u64().ok_or_else(|| {
            "const data arguments must be non-negative integers that fit `u64`".to_string()
        }),
        ExpressionNode::Binary(binary) => {
            let left = evaluate_closed_const_integer_expression(syntax_trees, binary.left)?;
            let right = evaluate_closed_const_integer_expression(syntax_trees, binary.right)?;
            match binary.operator {
                BinaryOperator::Add => left.checked_add(right).ok_or_else(|| {
                    "const data argument addition overflows `u64`".to_string()
                }),
                BinaryOperator::Subtract => left.checked_sub(right).ok_or_else(|| {
                    "const data argument subtraction produces a negative value".to_string()
                }),
                BinaryOperator::Multiply => left.checked_mul(right).ok_or_else(|| {
                    "const data argument multiplication overflows `u64`".to_string()
                }),
                BinaryOperator::Divide => left.checked_div(right).ok_or_else(|| {
                    "const data argument division by zero is invalid".to_string()
                }),
                BinaryOperator::Modulo => left.checked_rem(right).ok_or_else(|| {
                    "const data argument remainder by zero is invalid".to_string()
                }),
                BinaryOperator::ShiftLeft => u32::try_from(right)
                    .ok()
                    .and_then(|amount| left.checked_shl(amount))
                    .ok_or_else(|| {
                        "const data argument left shift exceeds the `u64` width".to_string()
                    }),
                BinaryOperator::ShiftRight => u32::try_from(right)
                    .ok()
                    .and_then(|amount| left.checked_shr(amount))
                    .ok_or_else(|| {
                        "const data argument right shift exceeds the `u64` width".to_string()
                    }),
                BinaryOperator::BitwiseAnd => Ok(left & right),
                BinaryOperator::BitwiseOr => Ok(left | right),
                BinaryOperator::BitwiseXor => Ok(left ^ right),
                _ => Err(
                    "const data arguments currently support only non-negative integer arithmetic, shifts, and bitwise operators"
                        .to_string(),
                ),
            }
        }
        _ => Err(
            "const data arguments currently support only closed non-negative integer expressions"
                .to_string(),
        ),
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
        let (domain_name, rest) = cursor.take_identifier()?;
        // A PARAMETERIZED domain name (`in OmegaLayout<Save>`, ch20 "grammars
        // are layout policies") flattens to one instance name -- the same
        // monomorphization-by-instantiation shape as generic data.
        let (domain_name, rest) = if rest.at_punctuation(PunctuationKind::Less) {
            let mut flat = String::from(domain_name.as_str());
            flat.push('<');
            let mut argument_cursor = rest.take_punctuation(PunctuationKind::Less, "<")?;
            loop {
                let (argument, next) = argument_cursor.take_identifier()?;
                flat.push_str(argument.as_str());
                if next.at_punctuation(PunctuationKind::Comma) {
                    flat.push_str(", ");
                    argument_cursor = next.take_punctuation(PunctuationKind::Comma, ",")?;
                    continue;
                }
                argument_cursor = next.take_punctuation(PunctuationKind::Greater, ">")?;
                break;
            }
            flat.push('>');
            (Identifier::generated(flat), argument_cursor)
        } else {
            (domain_name, rest)
        };
        constraints.push(
            match omega_core::arithmetic::ArithmeticDomain::from_name(domain_name.as_str()) {
                Some(domain) => TypeConstraintNode::ArithmeticDomain(domain),
                None => TypeConstraintNode::Domain(domain_name),
            },
        );

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

pub(super) fn parse_type_reference_handle_allowing_borrow<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    let (is_reference, lifetime, is_mutable, input) =
        if input.at_punctuation(PunctuationKind::Ampersand) {
            let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
            // Explicit lifetime (`&'buf T`, `&'buf mut T`); frozen decision 15
            // stage 2. The tick precedes `mut`/`relaxed`, mirroring Rust's
            // `&'a mut T`. A bare `&T` keeps the elided (`None`) form.
            let (lifetime, input) = parse_optional_lifetime(input)?;
            let (is_mutable, input) = if input.at_contextual("mut") {
                (true, input.take_contextual("mut")?)
            } else {
                (false, input)
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
            (true, lifetime, is_mutable, input)
        } else {
            (false, None, false, input)
        };

    let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
    let type_reference = if is_reference {
        syntax_trees
            .type_references
            .insert(TypeReferenceNode::Reference {
                referee: type_reference,
                is_mutable,
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
) -> ParseResult<'tokens, 'source, Option<omega_syntax_trees::identifier::Identifier>> {
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
            let constraint = if input.at_contextual("range") {
                return Err(input.error_here(
                    "the `range<a, b>` constraint syntax is removed; use `a..=b` (inclusive) or `a..b` (exclusive)",
                ));
            } else if input.at_name_like() {
                let (name, rest) = input.take_identifier()?;
                input = rest;
                TypeConstraintNode::Named(name)
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
                                omega_core::literals::IntegerLiteral::from_value(value - 1),
                            ))
                        }
                        _ => {
                            let one = syntax_trees.expressions.insert(ExpressionNode::Integer(
                                omega_core::literals::IntegerLiteral::from_value(1),
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
