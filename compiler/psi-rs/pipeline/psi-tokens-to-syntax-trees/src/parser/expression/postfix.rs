use crate::parse_error::ParseError;
use crate::parser::context::ExpressionContext;
use crate::parser::input::{Input, ParseResult};
use crate::parser::type_reference::parse_cast_target_type_reference_handle;
use psi_arena::HandleSpan;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression,
    TableCastExpression, TableIndexedExpression, TableMemberExpression, TableRangeExpression,
};
use psi_tokens::{KeywordKind, PunctuationKind};

use super::primary::parse_primary_expression_handle;
use super::{parse_expression_handle, parse_expression_handle_in};

pub(in crate::parser) fn parse_argument_list_after_open_paren_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<
    'tokens,
    'source,
    (
        HandleSpan<ExpressionHandle>,
        Box<[psi_syntax_trees::identifier::Identifier]>,
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

            // TARGET ROOT SLOT BINDING:
            // `builder.roots.bind(target::Slot, Application::start);` is a
            // build declaration over two symbol identities, not a runtime
            // field access followed by a call with first-class machine
            // values. Collapse the whole surface to one marker call on the
            // Build receiver. This keeps `Build` free of a fake runtime
            // `roots` field while preserving the ordinary, map-shaped source
            // spelling chosen by the target-slot design.
            if member.as_str() == "roots" && rest.at_punctuation(PunctuationKind::Dot) {
                let after_roots_dot = rest.take_punctuation(PunctuationKind::Dot, ".")?;
                let (operation, after_operation) = after_roots_dot.take_identifier()?;
                if operation.as_str() == "bind"
                    && after_operation.at_punctuation(PunctuationKind::LeftParen)
                {
                    let mut path_input =
                        after_operation.take_punctuation(PunctuationKind::LeftParen, "(")?;
                    let (slot, rest) = parse_symbol_path(path_input)?;
                    path_input = rest.take_punctuation(PunctuationKind::Comma, ",")?;
                    let (implementation, rest) = parse_symbol_path(path_input)?;
                    input = rest.take_punctuation(PunctuationKind::RightParen, ")")?;
                    expression = syntax_trees.expressions.insert(ExpressionNode::Call(
                        TableCallExpression {
                            receiver: expression,
                            target: psi_syntax_trees::identifier::Identifier::generated(format!(
                                "bind_root#{slot}#{implementation}"
                            )),
                            machine_arguments: Box::default(),
                            arguments: HandleSpan::empty(),
                            evidence_arguments: Box::default(),
                            operational_acknowledgement: Default::default(),
                        },
                    ));
                    continue;
                }
            }

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
                            target: psi_syntax_trees::identifier::Identifier::generated(format!(
                                "accept_boundary#{rendered}"
                            )),
                            machine_arguments: Box::default(),
                            arguments: HandleSpan::empty(),
                            evidence_arguments: Box::default(),
                            operational_acknowledgement: Default::default(),
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
                            target: psi_syntax_trees::identifier::Identifier::generated(format!(
                                "select_provider#{}#{}",
                                rendered[0], rendered[1]
                            )),
                            machine_arguments: Box::default(),
                            arguments: HandleSpan::empty(),
                            evidence_arguments: Box::default(),
                            operational_acknowledgement: Default::default(),
                        }));
                continue;
            }

            // CH21 EDGE-SPECIFIC WIRE COMPATIBILITY DEMAND:
            //
            // `b.require_wire_compatibility<
            //     Edge, Lineage, Local, Peer,
            //     Readable, Writable, PreserveUnknown, Canonical, CompleteMigration
            // >();`
            //
            // The first four arguments name ordinary types. Every following
            // argument is one requested fact from the closed vocabulary; a
            // demand spells only the facts its channel/store actually needs.
            // Like provider selection, this is an authoritative build marker,
            // not a runtime generic call. The build-config pass harvests the
            // encoded paths and the interpreter serves it as a no-op.
            if member.as_str() == "require_wire_compatibility"
                && rest.at_punctuation(PunctuationKind::Less)
            {
                let mut path_input = rest.take_punctuation(PunctuationKind::Less, "<")?;
                let mut rendered = Vec::new();
                loop {
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
                    if path_input.at_punctuation(PunctuationKind::Comma) {
                        path_input = path_input.take_punctuation(PunctuationKind::Comma, ",")?;
                        continue;
                    }
                    path_input = path_input.take_punctuation(PunctuationKind::Greater, ">")?;
                    break;
                }
                if rendered.len() < 5 {
                    return Err(path_input.error_here(
                        "`require_wire_compatibility` takes Edge, Lineage, Local, Peer, \
                         then at least one requested fact (`Readable`, `Writable`, \
                         `PreserveUnknown`, `Canonical`, or `CompleteMigration`)",
                    ));
                }
                let mut facts: Vec<&str> = Vec::new();
                for fact in &rendered[4..] {
                    if !matches!(
                        fact.as_str(),
                        "Readable"
                            | "Writable"
                            | "PreserveUnknown"
                            | "Canonical"
                            | "CompleteMigration"
                    ) {
                        return Err(path_input.error_here(format!(
                            "unknown wire compatibility fact `{fact}`; expected `Readable`, \
                             `Writable`, `PreserveUnknown`, `Canonical`, or \
                             `CompleteMigration`"
                        )));
                    }
                    if facts.contains(&fact.as_str()) {
                        return Err(path_input.error_here(format!(
                            "wire compatibility fact `{fact}` is requested twice"
                        )));
                    }
                    facts.push(fact);
                }
                let after_open = path_input.take_punctuation(PunctuationKind::LeftParen, "(")?;
                if !after_open.at_punctuation(PunctuationKind::RightParen) {
                    return Err(after_open.error_here(
                        "`require_wire_compatibility` takes types/facts in angle brackets and \
                         no value arguments",
                    ));
                }
                input = after_open.take_punctuation(PunctuationKind::RightParen, ")")?;
                expression =
                    syntax_trees
                        .expressions
                        .insert(ExpressionNode::Call(TableCallExpression {
                            receiver: expression,
                            target: psi_syntax_trees::identifier::Identifier::generated(format!(
                                "wire_compatibility#{}",
                                rendered.join("#")
                            )),
                            machine_arguments: Box::default(),
                            arguments: HandleSpan::empty(),
                            evidence_arguments: Box::default(),
                            operational_acknowledgement: Default::default(),
                        }));
                continue;
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
                            psi_syntax_trees::expression::TableAtomicExpression {
                                value: expression,
                                result: ExpressionHandle::invalid(),
                                ordering: psi_language_core::atomic::AtomicOrderingPlan::Load(
                                    ordering,
                                ),
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
            let mut form = psi_language_core::cast_form::CastForm::Value;
            if input.at_punctuation(PunctuationKind::Ampersand) {
                input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
                form = if input.at_contextual("mut") {
                    input = input.take_contextual("mut")?;
                    psi_language_core::cast_form::CastForm::RecastMutable
                } else {
                    psi_language_core::cast_form::CastForm::RecastShared
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
                    psi_syntax_trees::identifier::Identifier::generated(target_label_text),
                );
            input = rest;
            // Optional arithmetic DOMAIN cast suffix (`x as u8 in Saturating`),
            // decision 17 S2: re-tags the value's arithmetic domain so it can
            // legally take part in domained arithmetic (the escape hatch for the
            // mixed-domain rejection). `in` is the contextual membership keyword.
            let mut domain = psi_numerics::arithmetic::ArithmeticDomain::Exact;
            let mut semantic_domain = psi_arena::HandleSpan::empty();
            let mut semantic_domain_arguments = psi_arena::HandleSpan::empty();
            if input.at_contextual("in") {
                if form.is_recast() {
                    return Err(input.error_here(
                        "a recast re-views a place's bytes and takes no arithmetic domain; domains retag VALUE casts (`x as u8 in Wrapping`)",
                    ));
                }
                let after_in = input.take_contextual("in")?;
                let (domain_name, rest) = after_in.take_identifier()?;
                match psi_numerics::arithmetic::ArithmeticDomain::from_name(domain_name.as_str()) {
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
                            crate::parser::type_reference::parse_domain_argument_handles(
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
            continue;
        }

        break;
    }

    Ok((expression, input))
}

fn parse_symbol_path<'tokens, 'source>(
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, String> {
    let mut rendered = String::new();
    loop {
        let (segment, rest) = input.take_identifier()?;
        if !rendered.is_empty() {
            rendered.push_str("::");
        }
        rendered.push_str(segment.as_str());
        if rest.at_punctuation(PunctuationKind::ColonColon) {
            input = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
            continue;
        }
        return Ok((rendered, rest));
    }
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
    evidence_arguments: Box<[psi_syntax_trees::identifier::Identifier]>,
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

            validate_atomic_call_orderings(syntax_trees, target.as_str(), receiver, arguments)?;

            Ok(syntax_trees
                .tables
                .expressions
                .insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target,
                    machine_arguments,
                    arguments,
                    evidence_arguments,
                    operational_acknowledgement: Default::default(),
                })))
        }
        ExpressionNode::Member(member) => {
            validate_atomic_call_orderings(
                syntax_trees,
                member.member.as_str(),
                member.receiver,
                arguments,
            )?;
            Ok(syntax_trees
                .expressions
                .insert(ExpressionNode::Call(TableCallExpression {
                    receiver: member.receiver,
                    target: member.member,
                    machine_arguments,
                    arguments,
                    evidence_arguments,
                    operational_acknowledgement: Default::default(),
                })))
        }
        _ => Err(ParseError::new(
            "call target must be a path or member access",
        )),
    }
}

pub(in crate::parser) fn memory_ordering_from_expression(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Result<psi_language_core::atomic::MemoryOrdering, String> {
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
    psi_language_core::atomic::MemoryOrdering::from_name(members[0].as_str()).ok_or_else(|| {
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
fn try_parse_static_machine_arguments<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> Result<Option<(Box<[StaticMachineArgument]>, Input<'tokens, 'source>)>, ParseError> {
    if !input.at_punctuation(PunctuationKind::Less) {
        return Ok(None);
    }

    let mut cursor = input.take_punctuation(PunctuationKind::Less, "<")?;
    let mut arguments = Vec::new();
    loop {
        let Some((argument, rest)) = try_parse_static_argument(cursor)? else {
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
        if !cursor.at_punctuation(PunctuationKind::LeftParen) {
            return Ok(None);
        }
        return Ok(Some((arguments.into_boxed_slice(), cursor)));
    }
}

fn try_parse_static_argument<'tokens, 'source>(
    mut input: Input<'tokens, 'source>,
) -> Result<Option<(StaticMachineArgument, Input<'tokens, 'source>)>, ParseError> {
    if input.at_integer() {
        let (literal, rest) = input.take_integer_literal()?;
        return Ok(Some((
            StaticMachineArgument {
                path: Box::default(),
                application: None,
                const_literal: Some(literal),
                evidence_projection: None,
            },
            rest,
        )));
    }

    let Ok((first, rest)) = input.take_identifier() else {
        return Ok(None);
    };
    input = rest;
    if input.at_punctuation(PunctuationKind::Dot) {
        input = input.take_punctuation(PunctuationKind::Dot, ".")?;
        let (member, rest) = input.take_identifier()?;
        return Ok(Some((
            StaticMachineArgument {
                path: Box::default(),
                application: None,
                const_literal: None,
                evidence_projection: Some(psi_syntax_trees::expression::EvidenceProjection {
                    term: first,
                    member,
                }),
            },
            rest,
        )));
    }

    let mut path = vec![first];
    while input.at_punctuation(PunctuationKind::ColonColon) {
        let after_separator = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let Ok((member, rest)) = after_separator.take_identifier() else {
            return Ok(None);
        };
        path.push(member);
        input = rest;
    }

    let application = if input.at_punctuation(PunctuationKind::Less) {
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
                let Some((argument, rest)) = try_parse_static_argument(cursor)? else {
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
            input = cursor;
            break;
        }
        Some(Box::new(
            psi_syntax_trees::expression::StaticSymbolApplication {
                lifetime_arguments: lifetime_arguments.into_boxed_slice(),
                arguments: arguments.into_boxed_slice(),
            },
        ))
    } else {
        None
    };

    Ok(Some((
        StaticMachineArgument {
            path: path.into_boxed_slice(),
            application,
            const_literal: None,
            evidence_projection: None,
        },
        input,
    )))
}
