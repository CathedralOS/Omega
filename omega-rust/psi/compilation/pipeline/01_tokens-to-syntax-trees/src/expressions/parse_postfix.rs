use crate::diagnostics::parse_error::ParseError;
use crate::expressions::context::ExpressionContext;
use crate::input::token_cursor::{Input, ParseResult};
use crate::type_syntax::parse_type::parse_cast_target_type_reference_handle;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression,
    TableCastExpression, TableIndexedExpression, TableMemberExpression, TableRangeExpression,
};
use tokens::{KeywordKind, PunctuationKind};

use super::parse_expression::{parse_expression_handle, parse_expression_handle_in};
use super::primary::parse_primary_expression_handle;

pub(crate) fn parse_argument_list_after_open_paren_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<
    'tokens,
    'source,
    (
        HandleSpan<ExpressionHandle>,
        Box<[syntax_trees::identifier::Identifier]>,
    ),
> {
    let mut arguments = Vec::new();

    if !input.at_punctuation(PunctuationKind::RightParen)
        && !input.at_punctuation(PunctuationKind::Semicolon)
    {
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

    let mut evidence_arguments = Vec::new();
    if input.at_punctuation(PunctuationKind::Semicolon) {
        input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        if input.at_punctuation(PunctuationKind::RightParen) {
            return Err(input
                .error_here("the `;` call-lane separator must be followed by an evidence term"));
        }
        loop {
            let (argument, rest) = input.take_identifier()?;
            evidence_arguments.push(argument);
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
    Ok(((arguments, evidence_arguments.into_boxed_slice()), input))
}

pub(crate) fn parse_postfix_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let (expression, input) = parse_primary_expression_handle(syntax_trees, input, context)?;
    parse_postfix_suffixes_handle(syntax_trees, expression, input)
}

// Keep suffix construction scratch off the stack while parsing a nested primary.
fn parse_postfix_suffixes_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut expression: ExpressionHandle,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    loop {
        if input.at_punctuation(PunctuationKind::Less)
            && let Some((machine_arguments, rest)) =
                try_parse_static_machine_arguments(syntax_trees, input)?
        {
            let after_open = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let ((arguments, evidence_arguments), rest) =
                parse_argument_list_after_open_paren_handle(syntax_trees, after_open)?;
            input = rest;
            expression = build_call_expression_handle(
                syntax_trees,
                expression,
                machine_arguments,
                arguments,
                evidence_arguments,
            )?;
            continue;
        }

        if input.at_punctuation(PunctuationKind::LeftParen) {
            input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let ((arguments, evidence_arguments), rest) =
                parse_argument_list_after_open_paren_handle(syntax_trees, input)?;
            input = rest;
            expression = build_call_expression_handle(
                syntax_trees,
                expression,
                Box::default(),
                arguments,
                evidence_arguments,
            )?;
            continue;
        }

        if input.at_punctuation(PunctuationKind::LeftBracket) {
            let operator_span = input
                .tokens
                .first()
                .map(|token| input.source_span(token))
                .expect("recognized index punctuation has a source token");
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
            syntax_trees
                .expressions
                .set_source_span(expression, operator_span);
            continue;
        }

        if input.at_punctuation(PunctuationKind::Dot) {
            let after_dot = input.take_punctuation(PunctuationKind::Dot, ".")?;
            let (member, rest) = after_dot.take_identifier()?;

            if rest.at_punctuation(PunctuationKind::Less) {
                let retired = match member.as_str() {
                    "accept_boundary" => Some(
                        "receiver-style boundary acceptance is retired; use Build::accept_boundary<Boundary>(builder)",
                    ),
                    "select_provider" => Some(
                        "receiver-style provider selection is retired; use Build::select_provider<Slot, Provider>(builder)",
                    ),
                    "exclude_service" => Some(
                        "receiver-style service exclusion is retired; use Build::exclude_service<BoundaryTrait>(builder)",
                    ),
                    "select_representation" => Some(
                        "receiver-style representation selection is retired; use Build::select_representation<Opaque, Conformance>(builder)",
                    ),
                    "require_wire_compatibility" => Some(
                        "receiver-style wire compatibility is retired; use Build::require_wire_compatibility<Edge, Lineage, Local, Peer, Fact>(builder)",
                    ),
                    "assume_binding_contract" => Some(
                        "receiver-style assumed binding is retired; use Build::assume_binding_contract<Required::operation, Supplied::operation>(builder)",
                    ),
                    _ => None,
                };
                if let Some(message) = retired {
                    return Err(rest.error_here(message));
                }
            }

            // ATOMICS STAGE 1 (ch17, M2): `atomic_place.load(ordering)` is the
            // IDENTITY on the place (reads the value). The closed ordering
            // vocabulary is validated before this stage-one desugar erases its
            // syntax; loads admit NoOrdering/Receive/GlobalOrder and reject the
            // publish-bearing orderings. Target-specific instruction strength
            // remains a downstream lowering obligation. `load` is not reserved at data/machine
            // definition sites so this rewrite only fires for the exact
            // one-argument call form; `x.load` stays an ordinary member read.
            if member.as_str() == "load" && rest.at_punctuation(PunctuationKind::LeftParen) {
                let after_open = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
                if !after_open.at_punctuation(PunctuationKind::RightParen) {
                    let (ordering_expression, after_ord) =
                        parse_expression_handle(syntax_trees, after_open)?;
                    if after_ord.at_punctuation(PunctuationKind::RightParen) {
                        let ordering =
                            memory_ordering_from_expression(syntax_trees, ordering_expression)
                                .map_err(|reason| after_open.error_here(reason))?;
                        if !ordering.valid_for_load() {
                            return Err(after_open.error_here(format!(
                                "atomic load cannot use `{}` ordering; use `NoOrdering`, `Receive`, or `GlobalOrder`",
                                ordering.name()
                            )));
                        }
                        input = after_ord.take_punctuation(PunctuationKind::RightParen, ")")?;
                        expression = syntax_trees.expressions.insert(ExpressionNode::Atomic(
                            syntax_trees::expression::TableAtomicExpression {
                                value: expression,
                                result: ExpressionHandle::invalid(),
                                ordering: language_core::atomic::AtomicOrderingPlan::Load(ordering),
                                result_custody:
                                    language_core::atomic::AtomicExpressionResultCustody::Scalar,
                            },
                        ));
                        continue;
                    }
                }
                return Err(after_open.error_here(
                    "`load` takes exactly one ordering argument: e.g. `self.counter.load(NoOrdering)`",
                ));
            }

            // ATOMICS STAGE 1 (ch17, M2): `atomic_place.store(value, ordering)`
            // is a write to the place. The call builder validates the ordering
            // before the statement desugar drops it. The Call node with target "store" is preserved so
            // the statement parser can recognise it and convert it into an
            // Assignment statement (`place = value`).
            // `x.store` without a following `(` stays an ordinary member read.

            input = rest;
            // Like an operator token, the authored member name is the stable
            // occurrence of this read; the receiver keeps its own span.
            let member_span = member.source_span();
            expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Member(TableMemberExpression {
                        receiver: expression,
                        member,
                        case_variant: None,
                    }));
            syntax_trees
                .expressions
                .set_source_span(expression, member_span);
            continue;
        }

        if input.at_keyword(KeywordKind::As) {
            let cast_source = input.current_source_span();
            input = input.take_keyword(KeywordKind::As, "as")?;
            // §5b RECAST: `&x as &T` / `&mut x as &mut T` -- the borrow
            // re-viewed under a second stated shape. The SOURCE borrow's `&`
            // is the ordinary borrow spelling, so the unary parser wraps the
            // recast in a Borrow carrying the authored access. Recast-ness
            // rides the cast node itself: an `&` after `as` selects the form
            // here and resolved->typed lowering judges it (size/align/facts).
            let mut form = language_core::cast_form::CastForm::Value;
            if input.at_punctuation(PunctuationKind::Ampersand) {
                input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
                form = if input.at_contextual("mut") {
                    input = input.take_contextual("mut")?;
                    language_core::cast_form::CastForm::RecastMutable
                } else {
                    language_core::cast_form::CastForm::RecastShared
                };
            }
            let target_start = input;
            let (target_type, rest) = parse_cast_target_type_reference_handle(syntax_trees, input)?;
            let consumed = target_start.tokens.len().saturating_sub(rest.tokens.len());
            let target_label_text = target_start.tokens[..consumed]
                .iter()
                .map(|token| token.lexeme.as_str())
                .collect::<String>();
            let mut target_label = HandleSpan::empty();
            syntax_trees
                .expressions
                .append_identifier_path_member_to_span(
                    &mut target_label,
                    syntax_trees::identifier::Identifier::generated(target_label_text),
                );
            input = rest;
            // Optional arithmetic DOMAIN cast suffix (`x as u8 in Saturating`),
            // decision 17 S2: re-tags the value's arithmetic domain so it can
            // legally take part in domained arithmetic (the escape hatch for the
            // mixed-domain rejection). `in` is the contextual membership keyword.
            let mut domain = numerics::arithmetic::ArithmeticDomain::Exact;
            let mut semantic_domain = arena::HandleSpan::empty();
            let mut semantic_domain_arguments = arena::HandleSpan::empty();
            if input.at_contextual("in") {
                if form.is_recast() {
                    return Err(input.error_here(
                        "a recast re-views a place's bytes and takes no arithmetic domain; domains retag VALUE casts (`x as u8 in Wrapping`)",
                    ));
                }
                let after_in = input.take_contextual("in")?;
                // The same static path a type's domain constraint takes:
                // `in Km`, `in u64::Km`, or a module-qualified
                // `in units::u64::Km`.
                let (domain_name, rest) =
                    crate::type_syntax::parse_type::parse_static_name(after_in)?;
                match numerics::arithmetic::ArithmeticDomain::from_name(domain_name.as_str()) {
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
                        let (arguments, rest) =
                            crate::type_syntax::parse_type::parse_domain_argument_handles(
                                syntax_trees,
                                rest,
                            )?;
                        semantic_domain_arguments = arguments;
                        input = rest;
                    }
                }
                if semantic_domain.is_empty() {
                    if rest.at_punctuation(PunctuationKind::Less) {
                        return Err(rest.error_here(
                            "compiler arithmetic domains do not take index arguments",
                        ));
                    }
                    input = rest;
                }
            }
            expression =
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Cast(TableCastExpression {
                        value: expression,
                        target_type,
                        target_label,
                        domain,
                        semantic_domain,
                        semantic_domain_arguments,
                        form,
                    }));
            syntax_trees
                .expressions
                .set_source_span(expression, cast_source);
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
    // A leading separator has no start; both forms share the same range tail.
    let (start, input, end_inclusive) = if let Some(end_inclusive) = range_separator(&input) {
        (ExpressionHandle::invalid(), input, end_inclusive)
    } else {
        let (start, input) =
            parse_expression_handle_in(syntax_trees, input, ExpressionContext::Default)?;
        let Some(end_inclusive) = range_separator(&input) else {
            return Ok((start, input));
        };
        (start, input, end_inclusive)
    };

    let input = take_range_separator(input, end_inclusive)?;
    let (end, input) = if input.at_punctuation(PunctuationKind::RightBracket) {
        // An inclusive separator with no end has no normalizable end bound.
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
    evidence_arguments: Box<[syntax_trees::identifier::Identifier]>,
) -> Result<ExpressionHandle, ParseError> {
    let expression = syntax_trees.expressions.expression(expression).clone();
    let call = match expression {
        ExpressionNode::Name(path) => {
            let (members, receiver) = call_path_members_and_receiver(syntax_trees, path);
            let target = members
                .last()
                .cloned()
                .ok_or_else(|| ParseError::new("missing call target"))?;

            // A namespace-qualified declaration is an ordinary machine call,
            // even when its leaf name and arity match a dot atomic operation.
            // Ordering validation belongs to that distinct operation syntax.
            Ok(TableCallExpression {
                target_is_static: true,
                receiver,
                target,
                machine_arguments,
                arguments,
                evidence_arguments,
                operational_acknowledgement: Default::default(),
            })
        }
        ExpressionNode::Member(member) => {
            validate_atomic_call_orderings(
                syntax_trees,
                member.member.as_str(),
                member.receiver,
                arguments,
            )?;
            Ok(TableCallExpression {
                target_is_static: false,
                receiver: member.receiver,
                target: member.member,
                machine_arguments,
                arguments,
                evidence_arguments,
                operational_acknowledgement: Default::default(),
            })
        }
        _ => Err(ParseError::new(
            "call target must be a path or member access",
        )),
    }?;
    // Like an operator token, the authored call target is the stable occurrence
    // of this expression. Argument and receiver occurrences remain distinct.
    let source = call.target.source_span();
    let expression = syntax_trees.expressions.insert(ExpressionNode::Call(call));
    syntax_trees.expressions.set_source_span(expression, source);
    Ok(expression)
}

pub(crate) fn memory_ordering_from_expression(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Result<language_core::atomic::MemoryOrdering, String> {
    let ExpressionNode::Name(path) = syntax_trees.expressions.expression(expression) else {
        return Err(
            "atomic ordering must be one of `NoOrdering`, `Receive`, `Publish`, `ReceivePublish`, or `GlobalOrder`"
                .to_owned(),
        );
    };
    let members = syntax_trees.expressions.identifier_path_members(*path);
    if members.len() != 1 {
        return Err(
            "atomic ordering must be an unqualified built-in name: `NoOrdering`, `Receive`, `Publish`, `ReceivePublish`, or `GlobalOrder`"
                .to_owned(),
        );
    }
    language_core::atomic::MemoryOrdering::from_name(members[0].as_str()).ok_or_else(|| {
        format!(
            "unknown atomic ordering `{}`; expected `NoOrdering`, `Receive`, `Publish`, `ReceivePublish`, or `GlobalOrder`",
            members[0].as_str()
        )
    })
}

/// Validate the ordering arguments on the exact postfix shapes the atomic
/// statement desugars recognize. The receiver gate avoids stealing free
/// functions with the same names; wrong arities remain ordinary call errors.
fn validate_atomic_call_orderings(
    syntax_trees: &SyntaxTrees,
    target: &str,
    receiver: ExpressionHandle,
    arguments: HandleSpan<ExpressionHandle>,
) -> Result<(), ParseError> {
    if !receiver.is_valid() {
        return Ok(());
    }
    let arguments = syntax_trees.expressions.expression_handles(arguments);
    match (target, arguments) {
        ("store", [_, ordering]) => {
            let ordering = memory_ordering_from_expression(syntax_trees, *ordering)
                .map_err(ParseError::new)?;
            if !ordering.valid_for_store() {
                return Err(ParseError::new(format!(
                    "atomic store cannot use `{}` ordering; use `NoOrdering`, `Publish`, or `GlobalOrder`",
                    ordering.name()
                )));
            }
        }
        (
            "fetch_add" | "fetch_sub" | "fetch_xor" | "fetch_or" | "fetch_and" | "swap",
            [_, ordering],
        ) => {
            let _ = memory_ordering_from_expression(syntax_trees, *ordering)
                .map_err(ParseError::new)?;
        }
        ("compare_exchange", [_, _, success, failure]) => {
            let success =
                memory_ordering_from_expression(syntax_trees, *success).map_err(ParseError::new)?;
            let failure =
                memory_ordering_from_expression(syntax_trees, *failure).map_err(ParseError::new)?;
            if !failure.valid_compare_exchange_failure(success) {
                return Err(ParseError::new(format!(
                    "atomic compare_exchange failure ordering `{}` is invalid for success ordering `{}`; failure cannot publish or be stronger than success",
                    failure.name(),
                    success.name()
                )));
            }
        }
        _ => {}
    }
    Ok(())
}

/// Recognize the unambiguous postfix shape `<Machine::symbol, ...>(` without
/// stealing ordinary comparison expressions (`a < b`). The paths remain
/// static declaration arguments; value arguments are parsed separately after
/// the opening parenthesis.
pub(super) fn try_parse_static_machine_arguments<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<Option<(Box<[StaticMachineArgument]>, Input<'tokens, 'source>)>, ParseError> {
    if !input.at_punctuation(PunctuationKind::Less) {
        return Ok(None);
    }

    // Decide the complete `<...>(` production, not an expression followed by
    // an arbitrary `>`. Otherwise a const expression can consume the actual
    // call operands as a shift and stop at a later comparison. Each candidate
    // is bounded before `(` and parsed transactionally; only a complete pack
    // publishes nodes. Groups may contain their own calls and comparisons.
    let mut group_depth = 0usize;
    let mut previous = None;
    for (offset, token) in input.tokens.iter().enumerate() {
        if token.is_non_semantic() {
            continue;
        }
        let punctuation = token.punctuation();
        if group_depth == 0
            && punctuation == Some(PunctuationKind::LeftParen)
            && matches!(
                previous,
                Some(PunctuationKind::Greater | PunctuationKind::GreaterGreater)
            )
        {
            let mut prefix = input;
            prefix.tokens = &input.tokens[..offset];
            let mut scratch = SyntaxTrees::new(input.source_id);
            if let Ok(Some((arguments, end))) = parse_static_call_arguments(&mut scratch, prefix)
                && end.tokens.is_empty()
                && !end.at_punctuation(PunctuationKind::Greater)
            {
                let arguments = arguments
                    .iter()
                    .map(|argument| syntax_trees.copy_static_machine_argument(&scratch, argument))
                    .collect::<Vec<_>>();
                let mut rest = input;
                rest.tokens = &input.tokens[offset..];
                return Ok(Some((arguments.into_boxed_slice(), rest)));
            }
        }
        match punctuation {
            Some(
                PunctuationKind::LeftParen
                | PunctuationKind::LeftBracket
                | PunctuationKind::LeftBrace,
            ) => group_depth += 1,
            Some(
                PunctuationKind::RightParen
                | PunctuationKind::RightBracket
                | PunctuationKind::RightBrace,
            ) => {
                let Some(depth) = group_depth.checked_sub(1) else {
                    break;
                };
                group_depth = depth;
            }
            Some(PunctuationKind::Semicolon) if group_depth == 0 => break,
            _ => {}
        }
        previous = punctuation;
    }
    Ok(None)
}

fn parse_static_call_arguments<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<Option<(Box<[StaticMachineArgument]>, Input<'tokens, 'source>)>, ParseError> {
    let mut cursor = input.take_punctuation(PunctuationKind::Less, "<")?;
    let mut arguments = Vec::new();
    loop {
        let Some((argument, rest)) = try_parse_static_argument(syntax_trees, cursor)? else {
            return Ok(None);
        };
        arguments.push(argument);
        cursor = rest;

        if cursor.at_punctuation(PunctuationKind::Comma) {
            cursor = cursor.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }
        if !cursor.at_punctuation(PunctuationKind::Greater) {
            return Ok(None);
        }
        cursor = cursor.take_punctuation(PunctuationKind::Greater, ">")?;
        return Ok(Some((arguments.into_boxed_slice(), cursor)));
    }
}

fn try_parse_static_argument<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> Result<Option<(StaticMachineArgument, Input<'tokens, 'source>)>, ParseError> {
    let authored = input;
    if input.at_punctuation(PunctuationKind::LeftBracket)
        || input.at_punctuation(PunctuationKind::Ampersand)
    {
        return parse_static_type_argument(syntax_trees, input).map(Some);
    }
    let Ok((first, rest)) = input.take_identifier() else {
        return try_parse_static_value_argument(syntax_trees, input);
    };
    input = rest;
    // `name.member` is spelled before the binder kind is known. Evidence
    // projections and runtime field reads share this shape; checking admits a
    // field of a local or parameter only in a runtime `Value` slot.
    let argument = if input.at_punctuation(PunctuationKind::Dot) {
        input = input.take_punctuation(PunctuationKind::Dot, ".")?;
        let (member, rest) = input.take_identifier()?;
        input = rest;
        StaticMachineArgument {
            type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
            path: Box::default(),
            application: None,
            const_literal: None,
            evidence_projection: Some(syntax_trees::expression::EvidenceProjection {
                term: first,
                member,
            }),
        }
    } else {
        let mut path = vec![first];
        while input.at_punctuation(PunctuationKind::ColonColon) {
            let after_separator = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
            let Ok((member, rest)) = after_separator.take_identifier() else {
                return Ok(None);
            };
            path.push(member);
            input = rest;
        }

        if input.at_punctuation(PunctuationKind::LeftBracket) || input.at_contextual("in") {
            return parse_static_type_argument(syntax_trees, authored).map(Some);
        }
        let application = if let Some((application, rest)) =
            try_parse_static_symbol_application(syntax_trees, input)?
        {
            input = rest;
            Some(application)
        } else {
            None
        };
        StaticMachineArgument {
            type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
            path: path.into_boxed_slice(),
            application,
            const_literal: None,
            evidence_projection: None,
        }
    };

    // Names and projections share this decision: a lexed `>>` can continue
    // either operand as a shift, or close its surrounding applications.
    if input.at_punctuation(PunctuationKind::GreaterGreater)
        && let Some(value) = try_parse_static_value_argument(syntax_trees, authored)?
    {
        return Ok(Some(value));
    }
    if !(input.at_punctuation(PunctuationKind::Comma)
        || input.at_punctuation(PunctuationKind::Greater))
    {
        return try_parse_static_value_argument(syntax_trees, authored);
    }

    Ok(Some((argument, input)))
}

/// Values share the const-expression grammar with data applications. Keep the
/// authored tree until the selected declaration supplies its expected carrier;
/// parsing must not choose integer division or erase named operand custody.
fn try_parse_static_value_argument<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<Option<(StaticMachineArgument, Input<'tokens, 'source>)>, ParseError> {
    let mut scratch = SyntaxTrees::new(input.source_id);
    let parsed =
        super::parse_expression::parse_const_integer_expression_handle(&mut scratch, input);
    let (expression, rest) = match parsed {
        Ok(parsed)
            if parsed.1.at_punctuation(PunctuationKind::Comma)
                || parsed.1.at_punctuation(PunctuationKind::Greater) =>
        {
            parsed
        }
        _ => {
            // A lexed `>>` may close two applications instead of shifting.
            // Prefer a complete expression; otherwise retry the delimited
            // prefix transactionally, leaving both closers for their owners.
            let mut boundaries = Vec::new();
            let mut remaining = input;
            while let Ok((_, boundary)) = remaining.split_at_top_level_punctuation(
                PunctuationKind::GreaterGreater,
                "static argument delimiter",
            ) {
                boundaries.push(boundary);
                remaining = boundary.take_punctuation(PunctuationKind::GreaterGreater, ">>")?;
            }
            let mut selected = None;
            for rest in boundaries.into_iter().rev() {
                let mut prefix = input;
                prefix.tokens = &input.tokens[..input.tokens.len() - rest.tokens.len()];
                scratch = SyntaxTrees::new(input.source_id);
                if let Ok((expression, end)) =
                    super::parse_expression::parse_const_integer_expression_handle(
                        &mut scratch,
                        prefix,
                    )
                    && end.tokens.is_empty()
                    && !end.at_punctuation(PunctuationKind::Greater)
                {
                    selected = Some((expression, rest));
                    break;
                }
            }
            let Some(selected) = selected else {
                return Ok(None);
            };
            selected
        }
    };
    if !(rest.at_punctuation(PunctuationKind::Comma)
        || rest.at_punctuation(PunctuationKind::Greater))
        || matches!(
            scratch.expressions.expression(expression),
            ExpressionNode::Name(_) | ExpressionNode::Member(_)
        )
    {
        return Ok(None);
    }
    let (type_reference, const_literal) = match scratch.expressions.expression(expression) {
        ExpressionNode::Integer(literal) => (Default::default(), Some(literal.clone())),
        _ => {
            let expression = syntax_trees.copy_expression_from(&scratch, expression);
            (
                syntax_trees.type_references.insert(
                    syntax_trees::types::TypeReferenceNode::ConstExpression(expression),
                ),
                None,
            )
        }
    };
    Ok(Some((
        StaticMachineArgument {
            type_reference,
            path: Box::default(),
            application: None,
            const_literal,
            evidence_projection: None,
        },
        rest,
    )))
}

pub(crate) fn try_parse_static_symbol_application<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<
    Option<(
        Box<syntax_trees::expression::StaticSymbolApplication>,
        Input<'tokens, 'source>,
    )>,
    ParseError,
> {
    if input.at_punctuation(PunctuationKind::Less) {
        let mut cursor = input.take_punctuation(PunctuationKind::Less, "<")?;
        let mut lifetime_arguments = Vec::new();
        let mut arguments = Vec::new();
        let mut saw_non_lifetime = false;
        loop {
            if cursor.at_punctuation(PunctuationKind::Apostrophe) {
                if saw_non_lifetime {
                    return Err(cursor.error_here(
                        "lifetime arguments precede type, const, and static-machine arguments",
                    ));
                }
                cursor = cursor.take_punctuation(PunctuationKind::Apostrophe, "'")?;
                let (lifetime, rest) = cursor.take_identifier()?;
                lifetime_arguments.push(lifetime);
                cursor = rest;
            } else {
                saw_non_lifetime = true;
                let Some((argument, rest)) = try_parse_static_argument(syntax_trees, cursor)?
                else {
                    return Ok(None);
                };
                arguments.push(argument);
                cursor = rest;
            }

            if cursor.at_punctuation(PunctuationKind::Comma) {
                cursor = cursor.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }
            if !cursor.at_punctuation(PunctuationKind::Greater) {
                return Ok(None);
            }
            cursor = cursor.take_punctuation(PunctuationKind::Greater, ">")?;
            break Ok(Some((
                Box::new(syntax_trees::expression::StaticSymbolApplication {
                    lifetime_arguments: lifetime_arguments.into_boxed_slice(),
                    arguments: arguments.into_boxed_slice(),
                }),
                cursor,
            )));
        }
    } else {
        Ok(None)
    }
}

fn parse_static_type_argument<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<(StaticMachineArgument, Input<'tokens, 'source>), ParseError> {
    let (type_reference, rest) =
        crate::type_syntax::parse_type::parse_type_reference_handle(syntax_trees, input)?;
    Ok((
        StaticMachineArgument {
            type_reference,
            path: Box::default(),
            application: None,
            const_literal: None,
            evidence_projection: None,
        },
        rest,
    ))
}

/// Read a call target name path's members and build its receiver: the
/// path's prefix minus the final member, or an invalid handle when the
/// target is a single member.
pub(crate) fn call_path_members_and_receiver(
    syntax_trees: &mut SyntaxTrees,
    path: HandleSpan<syntax_trees::identifier::Identifier>,
) -> (Vec<syntax_trees::identifier::Identifier>, ExpressionHandle) {
    let members = syntax_trees
        .tables
        .expressions
        .identifier_path_members(path)
        .to_vec();
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
    (members, receiver)
}
