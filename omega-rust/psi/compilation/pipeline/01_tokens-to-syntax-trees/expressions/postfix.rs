use crate::errors::parse_error::ParseError;
use crate::expressions::context::ExpressionContext;
use crate::syntax_storage::tables::{exprs, exprs_of};
use crate::token_input::cursor::{Input, ParseResult};
use crate::types::references::parse_cast_target_type_reference_handle;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, StaticMachineArgument, StaticSymbolApplication,
    TableCallExpression, TableCastExpression, TableMemberExpression,
};
use tokens::{KeywordKind, PunctuationKind};

use crate::types::references::parse_domain_argument_handles;
use numerics::arithmetic::ArithmeticDomain;
use syntax_trees::identifier::Identifier;

/// A primary operand and its postfix suffixes, for statement grammar
/// that reads a call or place target without operators around it.
pub(crate) fn parse_postfix_expression_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    context: ExpressionContext,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    crate::expressions::parse::run_expression(
        syntax_trees,
        input,
        crate::expressions::parse::Goal::Postfix(context),
    )
}

/// The end of a call's argument list once its value arguments are
/// parsed: the optional `;` evidence lane, then the closing `)`.
pub(crate) fn finish_argument_lanes<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    arguments: Vec<ExpressionHandle>,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<
    'tokens,
    'source,
    (
        HandleSpan<ExpressionHandle>,
        Box<[Identifier]>,
        source::SourceSpan,
    ),
> {
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

    let closing = input;
    input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    let closing = closing.current_source_span();
    let arguments = syntax_trees
        .expressions
        .insert_expression_handles(arguments);
    Ok((
        (arguments, evidence_arguments.into_boxed_slice(), closing),
        input,
    ))
}

/// A `.member` projection after the operand parsed so far, or the
/// start of an atomic `.load(ordering)` whose ordering the caller parses.
pub(crate) enum MemberSuffix<'tokens, 'source> {
    Member(ExpressionHandle, Input<'tokens, 'source>),
    Load { after_open: Input<'tokens, 'source> },
}

pub(crate) fn parse_member_suffix<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut expression: ExpressionHandle,
    input: Input<'tokens, 'source>,
) -> Result<MemberSuffix<'tokens, 'source>, ParseError> {
    let mut access_source_span = input.source_span(
        input
            .tokens
            .first()
            .expect("recognized member punctuation has a source token"),
    );
    let after_dot = input.take_punctuation(PunctuationKind::Dot, ".")?;
    let (member, rest) = after_dot.take_identifier()?;
    access_source_span.span.end = member.source_span().span.end;

    if rest.at_punctuation(PunctuationKind::Less) {
        let retired = match member.as_str(syntax_trees) {
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

    if member.as_str(syntax_trees) == "load" && rest.at_punctuation(PunctuationKind::LeftParen) {
        let after_open = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
        return Ok(MemberSuffix::Load { after_open });
    }

    let member_span = member.source_span();
    expression = exprs(syntax_trees).insert(ExpressionNode::Member(TableMemberExpression {
        receiver: expression,
        access_source_span,
        member,
        case_variant: None,
    }));
    exprs(syntax_trees).set_source_span(expression, member_span);
    Ok(MemberSuffix::Member(expression, rest))
}

/// A cast written after the operand parsed so far, starting at `as`.
pub(crate) fn parse_cast_suffix<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut expression: ExpressionHandle,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    let cast_source = input.current_source_span();
    input = input.take_keyword(KeywordKind::As, "as")?;
    if at_cast_entry(input) {
        return parse_cast_entries(syntax_trees, expression, input, false);
    }
    let mut form = language_core::cast_form::CastForm::Value;
    if input.at_punctuation(PunctuationKind::Ampersand) {
        input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
        let access = if input.at_contextual("mut") {
            input = input.take_contextual("mut")?;
            language_core::ReferenceAccess::Mutable
        } else if input.at_contextual("write") {
            input = input.take_contextual("write")?;
            language_core::ReferenceAccess::WriteOnly
        } else {
            language_core::ReferenceAccess::Shared
        };
        form = language_core::cast_form::CastForm::Recast(access);
    }
    let target_start = input;
    let (target_type, rest) = parse_cast_target_type_reference_handle(syntax_trees, input)?;
    let consumed = target_start.tokens.len().saturating_sub(rest.tokens.len());
    let target_label_text = target_start.tokens[..consumed]
        .iter()
        .map(|token| target_start.text(token).as_str())
        .collect::<String>();
    let mut target_label = HandleSpan::empty();
    exprs(syntax_trees).append_identifier_path_member_to_span(
        &mut target_label,
        Identifier::generated(target_label_text),
    );
    input = rest;
    let mut domain = ArithmeticDomain::Exact;
    let mut semantic_domain = arena::HandleSpan::empty();
    let mut semantic_domain_arguments = arena::HandleSpan::empty();
    if input.at_contextual("in") {
        let after_in = input.take_contextual("in")?;
        let (domain_name, rest) = crate::types::references::parse_static_name(after_in)?;
        match ArithmeticDomain::from_name(domain_name.as_str(syntax_trees)) {
            Some(_) if form.is_recast() => {
                return Err(input.error_here(
                        "a recast re-views a place's bytes and takes no arithmetic domain; domains retag VALUE casts (`x as u8 in Wrapping`)",
                    ));
            }
            Some(parsed) => domain = parsed,
            None => {
                exprs(syntax_trees).append_identifier_path_member_to_span(
                    &mut semantic_domain,
                    domain_name.clone(),
                );
                let (arguments, rest) = parse_domain_argument_handles(syntax_trees, rest)?;
                semantic_domain_arguments = arguments;
                input = rest;
            }
        }
        if semantic_domain.is_empty() {
            if rest.at_punctuation(PunctuationKind::Less) {
                return Err(
                    rest.error_here("compiler arithmetic domains do not take index arguments")
                );
            }
            input = rest;
        }
    }
    expression = exprs(syntax_trees).insert(ExpressionNode::Cast(TableCastExpression {
        value: expression,
        target_type,
        target_label,
        domain,
        semantic_domain,
        semantic_domain_arguments,
        form,
    }));
    exprs(syntax_trees).set_source_span(expression, cast_source);
    if at_cast_entry(input) {
        if form.is_recast() {
            return Err(
                input.error_here("a recast re-views a place's bytes and takes no domain entries")
            );
        }
        if domain != ArithmeticDomain::Exact || !semantic_domain.is_empty() {
            return Err(input.error_here(
                        "a cast names its domains either with entries (`as T +D`) or with the older `as T in D`, not both",
                    ));
        }
        (expression, input) = parse_cast_entries(syntax_trees, expression, input, true)?;
    }
    Ok((expression, input))
}

fn at_cast_entry(input: Input<'_, '_>) -> bool {
    input.at_punctuation(PunctuationKind::Plus)
        || input.at_punctuation(PunctuationKind::Minus)
        || input.at_punctuation(PunctuationKind::Equal)
}

/*
    The run of `=D`, `-D` and `+D` entries after `as` (domains.md,
    "Grouping"). A `+` or `-` here always begins an entry, so no name lookup
    is needed and arithmetic on a cast result takes parentheses. Each entry
    becomes its own cast node over the previous one, because a cast node
    names one domain; typing accounts for the run as one simultaneous cast.
*/
fn parse_cast_entries<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut expression: ExpressionHandle,
    mut input: Input<'tokens, 'source>,
    after_carrier: bool,
) -> ParseResult<'tokens, 'source, ExpressionHandle> {
    use language_core::cast_form::{CastEntry, CastEntryKind, CastForm};
    let mut first = true;
    while at_cast_entry(input) {
        let kind = if input.at_punctuation(PunctuationKind::Plus) {
            CastEntryKind::Add
        } else if input.at_punctuation(PunctuationKind::Minus) {
            CastEntryKind::Remove
        } else {
            CastEntryKind::Keep
        };
        let sign = kind.sign();
        if after_carrier && kind != CastEntryKind::Add {
            return Err(input.error_here(format!(
                    "a carrier cast `as T` may only add domains; `{sign}` entries belong to a domain-only cast written first, as in `value as -D as T +D`. For arithmetic on the cast result, parenthesise the cast: `(value as T) {sign} ...`",
                )));
        }
        let entry_start = input;
        let entry_source = input.current_source_span();
        let after_sign = match kind {
            CastEntryKind::Add => input.take_punctuation(PunctuationKind::Plus, "+")?,
            CastEntryKind::Remove => input.take_punctuation(PunctuationKind::Minus, "-")?,
            CastEntryKind::Keep => input.take_punctuation(PunctuationKind::Equal, "=")?,
        };
        if !after_sign.at_name_like() {
            return Err(after_sign.error_here(format!(
                    "`{sign}` after a cast begins a domain entry such as `{sign}Km` and must be followed by a domain path; for arithmetic on the cast result, parenthesise the cast: `(value as T) {sign} ...`",
                )));
        }
        let (domain_name, rest) = crate::types::references::parse_static_name(after_sign)?;
        let mut domain = ArithmeticDomain::Exact;
        let mut semantic_domain = arena::HandleSpan::empty();
        let mut semantic_domain_arguments = arena::HandleSpan::empty();
        match ArithmeticDomain::from_name(domain_name.as_str(syntax_trees)) {
            Some(parsed) => {
                if rest.at_punctuation(PunctuationKind::Less) {
                    return Err(
                        rest.error_here("compiler arithmetic domains do not take index arguments")
                    );
                }
                domain = parsed;
                input = rest;
            }
            None => {
                exprs(syntax_trees)
                    .append_identifier_path_member_to_span(&mut semantic_domain, domain_name);
                let (arguments, rest) = parse_domain_argument_handles(syntax_trees, rest)?;
                semantic_domain_arguments = arguments;
                input = rest;
            }
        }
        let consumed = entry_start.tokens.len().saturating_sub(input.tokens.len());
        let label_text = entry_start.tokens[..consumed]
            .iter()
            .map(|token| entry_start.text(token).as_str())
            .collect::<String>();
        let mut target_label = HandleSpan::empty();
        exprs(syntax_trees).append_identifier_path_member_to_span(
            &mut target_label,
            Identifier::generated(label_text),
        );
        // Typing replaces this placeholder with the operand's carrier.
        let target_type = crate::syntax_storage::tables::type_refs(syntax_trees).insert_unit();
        expression = exprs(syntax_trees).insert(ExpressionNode::Cast(TableCastExpression {
            value: expression,
            target_type,
            target_label,
            domain,
            semantic_domain,
            semantic_domain_arguments,
            form: CastForm::Entry(CastEntry {
                kind,
                first,
                after_carrier,
            }),
        }));
        // Each entry reports at its own sign, not at the shared `as`.
        exprs(syntax_trees).set_source_span(expression, entry_source);
        first = false;
    }
    Ok((expression, input))
}

pub(crate) fn range_separator(input: &Input<'_, '_>) -> Option<bool> {
    if input.at_punctuation(PunctuationKind::DotDotEqual) {
        Some(true)
    } else if input.at_punctuation(PunctuationKind::DotDot) {
        Some(false)
    } else {
        None
    }
}

pub(crate) fn take_range_separator<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    end_inclusive: bool,
) -> Result<Input<'tokens, 'source>, ParseError> {
    if end_inclusive {
        input.take_punctuation(PunctuationKind::DotDotEqual, "..=")
    } else {
        input.take_punctuation(PunctuationKind::DotDot, "..")
    }
}

pub(crate) fn build_call_expression_handle(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
    machine_arguments: Box<[StaticMachineArgument]>,
    arguments: HandleSpan<ExpressionHandle>,
    evidence_arguments: Box<[Identifier]>,
    source: source::SourceSpan,
) -> Result<ExpressionHandle, ParseError> {
    let callee = expression;
    let expression = exprs_of(syntax_trees).expression(expression);
    let call = match expression {
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } => {
            if path_application.is_some() {
                return Err(ParseError::new(
                    "a declaration description is not an executable callable value",
                ));
            }
            let path = path.to_owned();
            let (target, receiver) = call_path_target_and_receiver(syntax_trees, path);
            let target = target.ok_or_else(|| ParseError::new("missing call target"))?;
            Ok(TableCallExpression {
                callee,
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
                member.member.as_str(syntax_trees),
                member.receiver,
                arguments,
            )?;
            Ok(TableCallExpression {
                callee,
                target_is_static: false,
                receiver: member.receiver,
                target: member.member.clone(),
                machine_arguments,
                arguments,
                evidence_arguments,
                operational_acknowledgement: Default::default(),
            })
        }
        _ => Ok(TableCallExpression {
            callee,
            receiver: ExpressionHandle::invalid(),
            target: Identifier::new("$callable", exprs_of(syntax_trees).source_span(callee)),
            target_is_static: false,
            machine_arguments,
            arguments,
            evidence_arguments,
            operational_acknowledgement: Default::default(),
        }),
    }?;
    let call = spell_test_group_facet(syntax_trees, call);
    let expression = exprs(syntax_trees).insert(ExpressionNode::Call(call));
    exprs(syntax_trees).set_source_span(expression, source);
    Ok(expression)
}

pub(crate) fn memory_ordering_from_expression(
    syntax_trees: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Result<language_core::atomic::MemoryOrdering, String> {
    let ExpressionNode::Name {
        members: path,
        application: path_application,
    } = exprs_of(syntax_trees).expression(expression)
    else {
        return Err(
            "atomic ordering must be one of `NoOrdering`, `Receive`, `Publish`, `ReceivePublish`, or `GlobalOrder`"
                .to_owned(),
        );
    };
    if path_application.is_some() {
        return Err("atomic ordering cannot have static arguments".to_owned());
    }
    let members = exprs_of(syntax_trees).identifier_path_members(path.to_owned());
    if members.len() != 1 {
        return Err(
            "atomic ordering must be an unqualified built-in name: `NoOrdering`, `Receive`, `Publish`, `ReceivePublish`, or `GlobalOrder`"
                .to_owned(),
        );
    }
    language_core::atomic::MemoryOrdering::from_name(members[0].as_str(syntax_trees)).ok_or_else(|| {
        format!(
            "unknown atomic ordering `{}`; expected `NoOrdering`, `Receive`, `Publish`, `ReceivePublish`, or `GlobalOrder`",
            members[0].as_str(syntax_trees)
        )
    })
}

fn validate_atomic_call_orderings(
    syntax_trees: &SyntaxTrees,
    target: &str,
    receiver: ExpressionHandle,
    arguments: HandleSpan<ExpressionHandle>,
) -> Result<(), ParseError> {
    if !receiver.is_valid() {
        return Ok(());
    }
    let arguments = exprs_of(syntax_trees).expression_handles(arguments);
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
        (member, [_, _, success, failure])
            if member == "compare_exchange"
                || language_core::atomic::AtomicCompareExchangeObservationAxis::single_attempt_member(
                    member,
                )
                .is_some() =>
        {
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

pub(crate) fn try_parse_static_machine_arguments<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<Option<(Box<[StaticMachineArgument]>, Input<'tokens, 'source>)>, ParseError> {
    if !input.at_punctuation(PunctuationKind::Less) {
        return Ok(None);
    }

    let mut group_depth = 0usize;
    let mut previous = None;
    for (offset, token) in input.tokens.iter().enumerate() {
        if token.is_non_semantic() {
            continue;
        }
        let Some(punctuation) = token.punctuation() else {
            previous = None;
            continue;
        };
        if group_depth == 0
            && punctuation == PunctuationKind::LeftParen
            && matches!(
                previous,
                Some(PunctuationKind::Greater | PunctuationKind::GreaterGreater)
            )
        {
            let mut prefix = input;
            prefix.tokens = &input.tokens[..offset];
            let checkpoint = syntax_trees.append_checkpoint();
            if let Ok(Some((arguments, end))) = parse_static_call_arguments(syntax_trees, prefix)
                && end.tokens.is_empty()
                && !end.at_punctuation(PunctuationKind::Greater)
            {
                let mut rest = input;
                rest.tokens = &input.tokens[offset..];
                return Ok(Some((arguments, rest)));
            }
            syntax_trees.rewind_appended(checkpoint);
        }
        match punctuation {
            PunctuationKind::LeftParen
            | PunctuationKind::LeftBracket
            | PunctuationKind::LeftBrace => group_depth += 1,
            PunctuationKind::RightParen
            | PunctuationKind::RightBracket
            | PunctuationKind::RightBrace => {
                let Some(depth) = group_depth.checked_sub(1) else {
                    break;
                };
                group_depth = depth;
            }
            PunctuationKind::Semicolon
            | PunctuationKind::Colon
            | PunctuationKind::Equal
            | PunctuationKind::Arrow
                if group_depth == 0 =>
            {
                break;
            }
            _ => {}
        }
        previous = Some(punctuation);
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

/*
    Names and applied members have the same scoped syntax in static operands
    and type positions. Keep each application's arguments on its own path
    component; namespace resolution later decides whether it names a type,
    requirement, or executable selection. Operand delimiters belong to the caller.
*/
pub(crate) fn parse_static_name_selection<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<Option<(StaticMachineArgument, Input<'tokens, 'source>)>, ParseError> {
    let Ok((first, mut input)) = input.take_identifier() else {
        return Ok(None);
    };
    if input.at_punctuation(PunctuationKind::Dot) {
        input = input.take_punctuation(PunctuationKind::Dot, ".")?;
        let (member, rest) = input.take_identifier()?;
        return Ok(Some((
            StaticMachineArgument {
                type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
                path: Box::default(),
                application: None,
                const_literal: None,
                evidence_projection: Some(syntax_trees::expression::EvidenceProjection {
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
    let application = if let Some((mut application, rest)) =
        try_parse_static_symbol_application(syntax_trees, input)?
    {
        input = rest;
        if input.at_punctuation(PunctuationKind::ColonColon) {
            let next = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
            let Some((member, rest)) = parse_static_name_selection(syntax_trees, next)? else {
                return Ok(None);
            };
            application.member = exprs(syntax_trees).insert_static_member(member);
            input = rest;
        }
        Some(application)
    } else {
        None
    };
    Ok(Some((
        StaticMachineArgument {
            type_reference: syntax_trees::types::TypeReferenceHandle::invalid(),
            path: path.into_boxed_slice(),
            application,
            const_literal: None,
            evidence_projection: None,
        },
        input,
    )))
}

fn try_parse_static_argument<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> Result<Option<(StaticMachineArgument, Input<'tokens, 'source>)>, ParseError> {
    let authored = input;
    let argument_checkpoint = syntax_trees.append_checkpoint();
    if input.at_punctuation(PunctuationKind::LeftBracket)
        || input.at_punctuation(PunctuationKind::Ampersand)
    {
        return parse_static_type_argument(syntax_trees, input).map(Some);
    }
    let Some((argument, rest)) = parse_static_name_selection(syntax_trees, input)? else {
        syntax_trees.rewind_appended(argument_checkpoint);
        return try_parse_static_value_argument(syntax_trees, input);
    };
    input = rest;
    if input.at_punctuation(PunctuationKind::LeftBracket) || input.at_contextual("in") {
        syntax_trees.rewind_appended(argument_checkpoint);
        return parse_static_type_argument(syntax_trees, authored).map(Some);
    }

    if input.at_punctuation(PunctuationKind::GreaterGreater)
        && let Some(value) = try_parse_static_value_argument(syntax_trees, authored)?
    {
        if argument.application.is_some() {
            syntax_trees.rewind_appended(argument_checkpoint);
            return try_parse_static_value_argument(syntax_trees, authored);
        }
        return Ok(Some(value));
    }
    if !(input.at_punctuation(PunctuationKind::Comma)
        || input.at_punctuation(PunctuationKind::Greater))
    {
        syntax_trees.rewind_appended(argument_checkpoint);
        return try_parse_static_value_argument(syntax_trees, authored);
    }

    Ok(Some((argument, input)))
}

fn try_parse_static_value_argument<'tokens, 'source>(
    syntax: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<Option<(StaticMachineArgument, Input<'tokens, 'source>)>, ParseError> {
    let checkpoint = syntax.append_checkpoint();
    let result = parse_static_value_alternatives(syntax, input);
    if !matches!(&result, Ok(Some((argument, _))) if argument.const_literal.is_none()) {
        syntax.rewind_appended(checkpoint);
    }
    result
}

fn parse_static_value_alternatives<'tokens, 'source>(
    scratch: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<Option<(StaticMachineArgument, Input<'tokens, 'source>)>, ParseError> {
    let Some((expression, rest)) =
        crate::expressions::static_arguments::parse_static_argument_expression(scratch, input)?
    else {
        return Ok(None);
    };
    if !(rest.at_punctuation(PunctuationKind::Comma)
        || rest.at_punctuation(PunctuationKind::Greater))
        || matches!(
            scratch.expressions.expression(expression),
            ExpressionNode::Name { .. }
        )
    {
        return Ok(None);
    }
    let (type_reference, const_literal) = match scratch.expressions.expression(expression) {
        ExpressionNode::Integer(literal) => (Default::default(), Some(literal.clone())),
        _ => (
            scratch.type_references.insert(
                syntax_trees::types::TypeReferenceNode::ConstExpression(expression),
            ),
            None,
        ),
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
) -> Result<Option<(Box<StaticSymbolApplication>, Input<'tokens, 'source>)>, ParseError> {
    let checkpoint = syntax_trees.append_checkpoint();
    let result = parse_static_symbol_application(syntax_trees, input);
    if !matches!(&result, Ok(Some(_))) {
        syntax_trees.rewind_appended(checkpoint);
    }
    result
}

fn parse_static_symbol_application<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> Result<Option<(Box<StaticSymbolApplication>, Input<'tokens, 'source>)>, ParseError> {
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
                Box::new(StaticSymbolApplication {
                    member: arena::Handle::invalid(),
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
        crate::types::references::parse_type_reference_handle(syntax_trees, input)?;
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

/*
    `owner.tests.group<Requirement>()` registers a test group, and
    `owner.tests.service<Group, Slot>()` selects its runner-supplied service,
    through the Build's `tests` facet (wiki/spec/build/testing.md). The facet is a value
    operand, so its receiver takes the same member projection an ordinary
    `owner.tests` value does rather than the static path a dotted call
    prefix otherwise becomes. Every other call keeps its parsed receiver.
*/
fn spell_test_group_facet(
    syntax_trees: &mut SyntaxTrees,
    mut call: TableCallExpression,
) -> TableCallExpression {
    if !matches!(call.target.as_str(syntax_trees), "group" | "service")
        || call.machine_arguments.is_empty()
        || !call.receiver.is_valid()
    {
        return call;
    }
    let ExpressionNode::Name {
        members: path,
        application: path_application,
    } = exprs_of(syntax_trees).expression(call.receiver)
    else {
        return call;
    };
    if path_application.is_some() {
        return call;
    }
    let path = path.to_owned();
    let members = exprs_of(syntax_trees)
        .identifier_path_members(path)
        .to_vec();
    let [.., facet] = members.as_slice() else {
        return call;
    };
    if members.len() < 2 || facet.as_str(syntax_trees) != "tests" {
        return call;
    }
    let facet = facet.clone();
    let owner_path = syntax_trees
        .expressions
        .copy_identifier_path_prefix(path, members.len() - 1);
    let owner = exprs(syntax_trees).insert(ExpressionNode::Name {
        members: owner_path,
        application: None,
    });
    let receiver = exprs(syntax_trees).insert(ExpressionNode::Member(TableMemberExpression {
        receiver: owner,
        access_source_span: facet.source_span(),
        member: facet.clone(),
        case_variant: None,
    }));
    exprs(syntax_trees).set_source_span(receiver, facet.source_span());
    call.receiver = receiver;
    call.target_is_static = false;
    call
}

pub(crate) fn call_path_target_and_receiver(
    syntax_trees: &mut SyntaxTrees,
    path: HandleSpan<Identifier>,
) -> (Option<Identifier>, ExpressionHandle) {
    let members = exprs_of(syntax_trees).identifier_path_members(path);
    let count = members.len();
    let target = members.last().cloned();
    let receiver = if count <= 1 {
        ExpressionHandle::invalid()
    } else {
        let receiver_path = syntax_trees
            .expressions
            .copy_identifier_path_prefix(path, count - 1);
        exprs(syntax_trees).insert(ExpressionNode::Name {
            members: receiver_path,
            application: None,
        })
    };
    (target, receiver)
}

#[cfg(test)]
#[path = "../tests/unit/postfix_transactions.rs"]
mod transaction_tests;
