/// A bare operational clause permits the effect. An explicit Boolean
/// states its bound without conflating `false` with omission.
pub(crate) fn operational_bound<'tokens, 'source>(
    input: crate::token_input::cursor::Input<'tokens, 'source>,
) -> crate::token_input::cursor::ParseResult<'tokens, 'source, bool> {
    use tokens::KeywordKind;
    if input.at_keyword(KeywordKind::False) {
        Ok((false, input.take_keyword(KeywordKind::False, "false")?))
    } else if input.at_keyword(KeywordKind::True) {
        Ok((true, input.take_keyword(KeywordKind::True, "true")?))
    } else {
        Ok((true, input))
    }
}

use crate::errors::parse_error::ParseError;

pub(crate) fn parse_crash_cause<'tokens, 'source>(
    cause: &syntax_trees::identifier::Identifier,
    after_cause: crate::token_input::cursor::Input<'tokens, 'source>,
) -> Result<syntax_trees::item::CrashCause, ParseError> {
    use syntax_trees::item::CrashCause;
    match cause.as_str(&after_cause) {
        "Trap" => Ok(CrashCause::Trap),
        "Abort" => Ok(CrashCause::Abort),
        _ => Err(after_cause.error_here(format!(
            "unknown crash cause `{}`; expected `Trap` or `Abort`",
            cause.as_str(&after_cause)
        ))),
    }
}

pub(crate) fn reject_retired_operational_reach(
    service: &syntax_trees::identifier::Identifier,
    input: crate::token_input::cursor::Input<'_, '_>,
) -> Result<(), ParseError> {
    let replacement = match service.as_str(&input) {
        "Suspend" => "suspends",
        "Block" | "thread_block" => "blocks",
        "sync_wait" => "the appropriate independent `suspends` and/or `blocks` clause",
        _ => return Ok(()),
    };
    Err(input.error_here(format!(
        "`reaches {}` is invalid: `reaches` contains boundary-service identities only; write `{replacement}` as an independent operational clause",
        service.as_str(&input)
    )))
}
pub(crate) fn take_service_reach_row_member<'tokens, 'source>(
    input: crate::token_input::cursor::Input<'tokens, 'source>,
    service_reaches: &mut Vec<syntax_trees::identifier::Identifier>,
) -> Result<crate::token_input::cursor::Input<'tokens, 'source>, ParseError> {
    use tokens::PunctuationKind;
    let (service, rest) = input.take_identifier()?;
    reject_retired_operational_reach(&service, rest)?;
    service_reaches.push(service);
    let mut input = rest;
    if input.at_punctuation(PunctuationKind::Comma) {
        input = input.take_punctuation(PunctuationKind::Comma, ",")?;
    } else if input.at_punctuation(PunctuationKind::Plus) {
        input = input.take_punctuation(PunctuationKind::Plus, "+")?;
    }
    Ok(input)
}

pub(crate) fn take_operational_reaches_clause<'tokens, 'source>(
    input: crate::token_input::cursor::Input<'tokens, 'source>,
    service_reach_keyword_source_spans: &mut Vec<source::SourceSpan>,
    service_reaches: &mut Vec<syntax_trees::identifier::Identifier>,
    service_reach_is_installation_bound: &mut bool,
    at_clause_boundary: impl Fn(crate::token_input::cursor::Input<'_, '_>) -> bool,
) -> Result<crate::token_input::cursor::Input<'tokens, 'source>, ParseError> {
    use tokens::PunctuationKind;
    service_reach_keyword_source_spans.push(input.current_source_span());
    let mut input = input.take_contextual("reaches")?;
    let service_count_before_clause = service_reaches.len();
    if input.at_punctuation(PunctuationKind::LessEqual) {
        if *service_reach_is_installation_bound || !service_reaches.is_empty() {
            return Err(input.error_here(
                "an installation-bound reach row must be declared once as `reaches <= Bound`",
            ));
        }
        *service_reach_is_installation_bound = true;
        input = input.take_punctuation(PunctuationKind::LessEqual, "<=")?;
    } else if *service_reach_is_installation_bound {
        return Err(input.error_here(
            "an installation-bound reach bound cannot be combined with another `reaches` clause",
        ));
    }
    // `reaches none` spells the explicit empty service row. It keeps the
    // keyword span so later stages still distinguish it from omission and
    // contributes no member, exactly like the keyword followed directly by
    // a clause boundary, which stays accepted until the retirement sweep.
    if !*service_reach_is_installation_bound && input.at_contextual("none") {
        let after_none = input.take_contextual("none")?;
        if !at_clause_boundary(after_none) {
            return Err(after_none.error_here(
                    "`reaches none` admits no service members; write `reaches <Service> + ...` to name services",
                ));
        }
        return Ok(after_none);
    }
    while !at_clause_boundary(input) {
        input = take_service_reach_row_member(input, service_reaches)?;
    }
    if *service_reach_is_installation_bound && service_reaches.len() == service_count_before_clause
    {
        return Err(input.error_here(
            "an installation-bound reach row requires a nonempty upper bound after `reaches <=`",
        ));
    }
    Ok(input)
}

pub(crate) fn publish_capability_contracts(
    syntax_trees: &mut syntax_trees::SyntaxTrees,
    contracts: Vec<syntax_trees::item::CapabilityContract>,
) -> arena::HandleSpan<syntax_trees::item::CapabilityContract> {
    let mut span = crate::syntax_storage::handle_spans::HandleSpanBuilder::new();
    for contract in contracts {
        span.push(syntax_trees.items.append_capability_contract(contract));
    }
    span.finish()
}

/// `crashes none` promises that no crash route exists. It names no cause,
/// admits no route guard and cannot share a header with a `crashes Cause`
/// route. The header then publishes no crash contract, which is the empty
/// cause set an omitted clause denotes on a published surface today; the
/// explicit spelling exists so that promise is readable without the rule.
pub(crate) fn finish_crashes_none<'tokens, 'source>(
    after_none: crate::token_input::cursor::Input<'tokens, 'source>,
    other_route_present: bool,
    at_clause_boundary: impl Fn(crate::token_input::cursor::Input<'_, '_>) -> bool,
) -> Result<crate::token_input::cursor::Input<'tokens, 'source>, ParseError> {
    if other_route_present {
        return Err(after_none
            .error_here("`crashes none` cannot share a header with a `crashes Cause` route"));
    }
    if !at_clause_boundary(after_none) {
        return Err(after_none
            .error_here("`crashes none` admits no route guards; name a cause to guard a route"));
    }
    Ok(after_none)
}
