use crate::parse_error::ParseError;
use crate::parser::context::ExpressionContext;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use omega_core::arena::HandleSpan;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression,
    TableCastExpression, TableIndexedExpression, TableMemberExpression, TableRangeExpression,
};
use omega_tokens::{KeywordKind, PunctuationKind};

use super::primary::parse_primary_expression_handle;
use super::{parse_expression_handle, parse_expression_handle_in};

pub(in crate::parser) fn parse_argument_list_after_open_paren_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<ExpressionHandle>> {
    let mut arguments = Vec::new();

    if !input.at_punctuation(PunctuationKind::RightParen) {
        loop {
            let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
            arguments.push(expression);
            input = rest;

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                if input.at_punctuation(PunctuationKind::RightParen) {
                    break;
                }
                continue;
            }

            break;
        }
    }

    input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    let arguments = syntax_trees
        .expressions
        .insert_expression_handles(arguments);
    Ok((arguments, input))
}

pub(super) fn parse_postfix_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let (mut expression, mut input) =
        parse_primary_expression_handle(syntax_trees, input, context)?;

    loop {
        if input.at_punctuation(PunctuationKind::Less)
            && let Some((machine_arguments, rest)) = try_parse_static_machine_arguments(input)?
        {
            let after_open = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (arguments, rest) =
                parse_argument_list_after_open_paren_handle(syntax_trees, after_open)?;
            input = rest;
            expression = build_call_expression_handle(
                syntax_trees,
                expression,
                machine_arguments,
                arguments,
            )?;
            continue;
        }

        if input.at_punctuation(PunctuationKind::LeftParen) {
            input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (arguments, rest) =
                parse_argument_list_after_open_paren_handle(syntax_trees, input)?;
            input = rest;
            expression =
                build_call_expression_handle(syntax_trees, expression, Box::default(), arguments)?;
            continue;
        }

        if input.at_punctuation(PunctuationKind::LeftBracket) {
            input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
            let (index, rest) = parse_index_or_range_expression_handle(syntax_trees, input)?;
            input = rest.take_punctuation(PunctuationKind::RightBracket, "]")?;
            expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Indexed(TableIndexedExpression {
                        collection: expression,
                        index,
                    }));
            continue;
        }

        if input.at_punctuation(PunctuationKind::Dot) {
            let after_dot = input.take_punctuation(PunctuationKind::Dot, ".")?;
            let (member, rest) = after_dot.take_identifier()?;

            // CH10 ROOT GRANT (GR3): `b.accept_boundary<pkg::symbol>();` --
            // the final build's grant spelling. General angle-bracket call
            // arguments name static MACHINES; this declaration names a
            // trust symbol of any kind, so the carve recognizes the exact form and
            // desugars to a MARKER-NAMED zero-argument member call
            // (`accept_boundary#<path>` -- the asm#hlt / __destructure#
            // marker convention; `#` cannot appear in identifiers, so the
            // encoding is unambiguous). The build-config evaluation serves
            // the marker and records the grant; outside a build machine the
            // marker name fails resolution loudly rather than silently.
            if member.as_str() == "accept_boundary" && rest.at_punctuation(PunctuationKind::Less) {
                let mut path_input = rest.take_punctuation(PunctuationKind::Less, "<")?;
                let mut rendered = String::new();
                loop {
                    let (segment, next) = path_input.take_identifier()?;
                    if !rendered.is_empty() {
                        rendered.push_str("::");
                    }
                    rendered.push_str(segment.as_str());
                    if next.at_punctuation(PunctuationKind::ColonColon) {
                        path_input = next.take_punctuation(PunctuationKind::ColonColon, "::")?;
                        continue;
                    }
                    path_input = next.take_punctuation(PunctuationKind::Greater, ">")?;
                    break;
                }
                let after_open = path_input.take_punctuation(PunctuationKind::LeftParen, "(")?;
                if !after_open.at_punctuation(PunctuationKind::RightParen) {
                    return Err(after_open.error_here(
                        "`accept_boundary` takes its symbol in angle brackets and no \
                         value arguments: `b.accept_boundary<pkg::symbol>();`",
                    ));
                }
                input = after_open.take_punctuation(PunctuationKind::RightParen, ")")?;
                expression =
                    syntax_trees
                        .expressions
                        .insert(ExpressionNode::Call(TableCallExpression {
                            receiver: expression,
                            target: omega_syntax_trees::identifier::Identifier::generated(format!(
                                "accept_boundary#{rendered}"
                            )),
                            machine_arguments: Box::default(),
                            arguments: HandleSpan::empty(),
                        }));
                continue;
            }

            // PRV4c PROVIDER SLOT SELECTION:
            // `b.select_provider<BoundaryTrait, ProviderType>();` is a
            // build-declaration marker, not a generic machine call. Static
            // machine arguments deliberately name MACHINES; these two paths
            // name TYPES, so routing the spelling through that surface would
            // lie about their kind. The build-config pass harvests the marker
            // only from the authoritative build machine and validates both
            // paths against derived provider candidates.
            if member.as_str() == "select_provider" && rest.at_punctuation(PunctuationKind::Less) {
                let mut path_input = rest.take_punctuation(PunctuationKind::Less, "<")?;
                let mut rendered = Vec::new();
                for argument_index in 0..2 {
                    let mut path = String::new();
                    loop {
                        let (segment, next) = path_input.take_identifier()?;
                        if !path.is_empty() {
                            path.push_str("::");
                        }
                        path.push_str(segment.as_str());
                        if next.at_punctuation(PunctuationKind::ColonColon) {
                            path_input =
                                next.take_punctuation(PunctuationKind::ColonColon, "::")?;
                            continue;
                        }
                        path_input = next;
                        break;
                    }
                    rendered.push(path);
                    path_input = if argument_index == 0 {
                        path_input.take_punctuation(PunctuationKind::Comma, ",")?
                    } else {
                        path_input.take_punctuation(PunctuationKind::Greater, ">")?
                    };
                }
                let after_open = path_input.take_punctuation(PunctuationKind::LeftParen, "(")?;
                if !after_open.at_punctuation(PunctuationKind::RightParen) {
                    return Err(after_open.error_here(
                        "`select_provider` takes a boundary-trait type and provider type in angle brackets and no value arguments: `b.select_provider<Console, TestConsole>();`",
                    ));
                }
                input = after_open.take_punctuation(PunctuationKind::RightParen, ")")?;
                expression =
                    syntax_trees
                        .expressions
                        .insert(ExpressionNode::Call(TableCallExpression {
                            receiver: expression,
                            target: omega_syntax_trees::identifier::Identifier::generated(format!(
                                "select_provider#{}#{}",
                                rendered[0], rendered[1]
                            )),
                            machine_arguments: Box::default(),
                            arguments: HandleSpan::empty(),
                        }));
                continue;
            }

            // ATOMICS STAGE 1 (ch17, M2): `atomic_place.load(ordering)` is the
            // IDENTITY on the place (reads the value). On x86_64, Relaxed/
            // Acquire/Release/AcqRel loads are plain aligned `mov` -- the
            // ordering argument is accepted syntactically but collapsed here.
            // SeqCst load is also a plain mov on x86_64 (only SeqCst STORE
            // requires mfence/xchg). `load` is not reserved at data/machine
            // definition sites so this rewrite only fires for the exact
            // one-argument call form; `x.load` stays an ordinary member read.
            if member.as_str() == "load" && rest.at_punctuation(PunctuationKind::LeftParen) {
                let after_open = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
                if !after_open.at_punctuation(PunctuationKind::RightParen) {
                    // Consume the single ordering argument (an identifier like
                    // `Relaxed`, `Acquire`, etc.) -- we only validate the
                    // argument count here; the identifier itself is not
                    // type-checked (it is dropped).
                    let (_, after_ord) = parse_expression_handle(syntax_trees, after_open)?;
                    if after_ord.at_punctuation(PunctuationKind::RightParen) {
                        input = after_ord.take_punctuation(PunctuationKind::RightParen, ")")?;
                        // `expression` stays unchanged -- load() is the identity.
                        continue;
                    }
                }
                return Err(after_open.error_here(
                    "`load` takes exactly one ordering argument: e.g. `self.counter.load(Relaxed)`",
                ));
            }

            // ATOMICS STAGE 1 (ch17, M2): `atomic_place.store(value, ordering)`
            // is a write to the place.  The ordering argument is consumed and
            // dropped.  On x86_64 Relaxed/Acquire/Release/AcqRel stores are
            // plain aligned `mov`; SeqCst store uses `xchg` or `mfence+mov`
            // (not yet differentiated here -- all orderings lower to plain mov
            // in stage 1). The Call node with target "store" is preserved so
            // the statement parser can recognise it and convert it into an
            // Assignment statement (`place = value`).
            // `x.store` without a following `(` stays an ordinary member read.

            input = rest;
            expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Member(TableMemberExpression {
                        receiver: expression,
                        member,
                        case_variant: None,
                    }));
            continue;
        }

        if input.at_keyword(KeywordKind::As) {
            input = input.take_keyword(KeywordKind::As, "as")?;
            // §5b RECAST: `&x as &T` / `&mut x as &mut T` -- the borrow
            // re-viewed under a second stated shape. The SOURCE borrow's `&`
            // is the ordinary borrow spelling (a shared `&` vanishes in the
            // unary parser; `&mut` wraps Mutable), so recast-ness rides the
            // cast node itself: an `&` after `as` selects the form here and
            // the resolved->typed lowering judges it (size/align/facts).
            let mut form = omega_core::cast_form::CastForm::Value;
            if input.at_punctuation(PunctuationKind::Ampersand) {
                input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
                form = if input.at_contextual("mut") {
                    input = input.take_contextual("mut")?;
                    omega_core::cast_form::CastForm::RecastMutable
                } else {
                    omega_core::cast_form::CastForm::RecastShared
                };
            }
            let (target_type, rest) = parse_path_handle_span(input, |member| {
                syntax_trees
                    .expressions
                    .append_identifier_path_member(member)
            })?;
            input = rest;
            // Optional arithmetic DOMAIN cast suffix (`x as u8 in Saturating`),
            // decision 17 S2: re-tags the value's arithmetic domain so it can
            // legally take part in domained arithmetic (the escape hatch for the
            // mixed-domain rejection). `in` is the contextual membership keyword.
            let mut domain = omega_core::arithmetic::ArithmeticDomain::Exact;
            let mut semantic_domain = omega_core::arena::HandleSpan::empty();
            if input.at_contextual("in") {
                if form.is_recast() {
                    return Err(input.error_here(
                        "a recast re-views a place's bytes and takes no arithmetic domain; domains retag VALUE casts (`x as u8 in Wrapping`)",
                    ));
                }
                let after_in = input.take_contextual("in")?;
                let (domain_name, rest) = after_in.take_identifier()?;
                match omega_core::arithmetic::ArithmeticDomain::from_name(domain_name.as_str()) {
                    Some(parsed) => domain = parsed,
                    // A non-policy name is the semantic-domain qualification
                    // spelling (decision 19) -- carried whole; validation
                    // judges it against the program's DECLARED domains (the
                    // parser cannot see other items).
                    None => {
                        syntax_trees
                            .expressions
                            .append_identifier_path_member_to_span(
                                &mut semantic_domain,
                                domain_name.clone(),
                            );
                    }
                }
                input = rest;
            }
            expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Cast(TableCastExpression {
                        value: expression,
                        target_type,
                        domain,
                        semantic_domain,
                        form,
                    }));
            continue;
        }

        break;
    }

    Ok((expression, input))
}

fn parse_index_or_range_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    // Leading `..` / `..=` branch: open-start range (`..b`, `..`, `..=b`).
    if let Some(end_inclusive) = range_separator(&input) {
        let input = take_range_separator(input, end_inclusive)?;
        let (end, input) = if input.at_punctuation(PunctuationKind::RightBracket) {
            // A trailing inclusive separator with no end expression is invalid:
            // `..=` does not name a normalizable end.
            if end_inclusive {
                return Err(ParseError::new(
                    "inclusive range `..=` requires an end expression",
                ));
            }
            (ExpressionHandle::invalid(), input)
        } else {
            let (end, input) =
                parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)?;
            (end, input)
        };
        return Ok((
            syntax_trees
                .expressions
                .insert(ExpressionNode::Range(TableRangeExpression {
                    start: ExpressionHandle::invalid(),
                    end,
                    end_inclusive,
                })),
            input,
        ));
    }

    let (start, input) =
        parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)?;
    let Some(end_inclusive) = range_separator(&input) else {
        return Ok((start, input));
    };

    let input = take_range_separator(input, end_inclusive)?;
    let (end, input) = if input.at_punctuation(PunctuationKind::RightBracket) {
        // `start..=` with no end has no normalizable end bound: reject it.
        if end_inclusive {
            return Err(ParseError::new(
                "inclusive range `..=` requires an end expression",
            ));
        }
        (ExpressionHandle::invalid(), input)
    } else {
        let (end, input) =
            parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)?;
        (end, input)
    };
    Ok((
        syntax_trees
            .expressions
            .insert(ExpressionNode::Range(TableRangeExpression {
                start,
                end,
                end_inclusive,
            })),
        input,
    ))
}

/// Returns `Some(end_inclusive)` if the input is positioned at a range separator
/// (`..` -> `Some(false)`, `..=` -> `Some(true)`), otherwise `None`.
fn range_separator(input: &Input<'_, '_>) -> Option<bool> {
    if input.at_punctuation(PunctuationKind::DotDotEqual) {
        Some(true)
    } else if input.at_punctuation(PunctuationKind::DotDot) {
        Some(false)
    } else {
        None
    }
}

fn take_range_separator<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    end_inclusive: bool,
) -> Result<Input<'tokens, 'source>, ParseError> {
    if end_inclusive {
        input.take_punctuation(PunctuationKind::DotDotEqual, "..=")
    } else {
        input.take_punctuation(PunctuationKind::DotDot, "..")
    }
}

fn build_call_expression_handle(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    machine_arguments: Box<[StaticMachineArgument]>,
    arguments: HandleSpan<ExpressionHandle>,
) -> Result<ExpressionHandle, ParseError> {
    let expression = syntax_trees.expressions.expression(expression).clone();
    match expression {
        ExpressionNode::Name(path) => {
            let members = syntax_trees
                .tables
                .expressions
                .identifier_path_members(path)
                .to_vec();
            let target = members
                .last()
                .cloned()
                .ok_or_else(|| ParseError::new("missing call target"))?;
            let receiver = if members.len() <= 1 {
                ExpressionHandle::invalid()
            } else {
                let receiver_path = syntax_trees
                    .tables
                    .expressions
                    .copy_identifier_path_prefix(path, members.len() - 1);
                syntax_trees
                    .tables
                    .expressions
                    .insert(ExpressionNode::Name(receiver_path))
            };

            Ok(syntax_trees
                .tables
                .expressions
                .insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target,
                    machine_arguments,
                    arguments,
                })))
        }
        ExpressionNode::Member(member) => {
            Ok(syntax_trees
                .expressions
                .insert(ExpressionNode::Call(TableCallExpression {
                    receiver: member.receiver,
                    target: member.member,
                    machine_arguments,
                    arguments,
                })))
        }
        _ => Err(ParseError::new(
            "call target must be a path or member access",
        )),
    }
}

/// Recognize the unambiguous postfix shape `<Machine::symbol, ...>(` without
/// stealing ordinary comparison expressions (`a < b`). The paths remain
/// static declaration arguments; value arguments are parsed separately after
/// the opening parenthesis.
fn try_parse_static_machine_arguments<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> Result<Option<(Box<[StaticMachineArgument]>, Input<'tokens, 'source>)>, ParseError> {
    if !input.at_punctuation(PunctuationKind::Less) {
        return Ok(None);
    }

    let mut cursor = input.take_punctuation(PunctuationKind::Less, "<")?;
    let mut arguments = Vec::new();
    loop {
        let Ok((first, rest)) = cursor.take_identifier() else {
            return Ok(None);
        };
        cursor = rest;
        let mut path = vec![first];
        while cursor.at_punctuation(PunctuationKind::ColonColon) {
            let after_separator = cursor.take_punctuation(PunctuationKind::ColonColon, "::")?;
            let Ok((member, rest)) = after_separator.take_identifier() else {
                return Ok(None);
            };
            path.push(member);
            cursor = rest;
        }
        arguments.push(StaticMachineArgument {
            path: path.into_boxed_slice(),
        });

        if cursor.at_punctuation(PunctuationKind::Comma) {
            cursor = cursor.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }
        if !cursor.at_punctuation(PunctuationKind::Greater) {
            return Ok(None);
        }
        cursor = cursor.take_punctuation(PunctuationKind::Greater, ">")?;
        if !cursor.at_punctuation(PunctuationKind::LeftParen) {
            return Ok(None);
        }
        return Ok(Some((arguments.into_boxed_slice(), cursor)));
    }
}
