//! Contract clauses on machines, signatures and states: `parse_contract_clauses`
//! reads a machine's clauses, `signature` a signature's, `state_arrival` a
//! state's arrival contracts, `facts` the proof-fact lists they share, and
//! `conformance` the `satisfies` clauses and external provider bindings.

pub(crate) mod conformance;
pub(crate) mod facts;
pub(crate) mod parse_contract_clauses;
pub(crate) mod signature;
pub(crate) mod state_arrival;

/// The `crashes Trap|Abort` spelling check the signature and operator
/// contract clauses share.
pub(crate) fn parse_crash_cause<'tokens, 'source>(
    cause: &syntax_trees::identifier::Identifier,
    after_cause: crate::input::token_cursor::Input<'tokens, 'source>,
) -> Result<syntax_trees::item::CrashCause, crate::diagnostics::parse_error::ParseError> {
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
    input: crate::input::token_cursor::Input<'_, '_>,
) -> Result<(), crate::diagnostics::parse_error::ParseError> {
    let replacement = match service.as_str(&input) {
        "Suspend" => "suspends;",
        "Block" => "blocks;",
        "thread_block" => "blocks;",
        "sync_wait" => "the appropriate independent `suspends;` and/or `blocks;` clause",
        _ => return Ok(()),
    };
    Err(input.error_here(format!(
        "`reaches {}` is invalid: `reaches` contains boundary-service identities only; write `{replacement}` as an independent operational clause",
        service.as_str(&input)
    )))
}
/// One `reaches` member plus its `,`/`+` separator, shared by the machine
/// clause and trait signature clause loops; the caller's builder accumulates
/// the contiguous span.
pub(crate) fn take_service_reach_row_member<'tokens, 'source>(
    input: crate::input::token_cursor::Input<'tokens, 'source>,
    syntax_trees: &mut syntax_trees::SyntaxTrees,
    service_reaches: &mut crate::handle_spans::HandleSpanBuilder<
        syntax_trees::identifier::Identifier,
    >,
) -> Result<
    crate::input::token_cursor::Input<'tokens, 'source>,
    crate::diagnostics::parse_error::ParseError,
> {
    use tokens::PunctuationKind;
    let (service, rest) = input.take_identifier()?;
    reject_retired_operational_reach(&service, rest)?;
    let handle = syntax_trees.items.append_identifier_path_member(service);
    service_reaches.push(handle);
    let mut input = rest;
    if input.at_punctuation(PunctuationKind::Comma) {
        input = input.take_punctuation(PunctuationKind::Comma, ",")?;
    } else if input.at_punctuation(PunctuationKind::Plus) {
        input = input.take_punctuation(PunctuationKind::Plus, "+")?;
    }
    Ok(input)
}

/// The whole `reaches <Service> + ...` clause both grammars share: the `<=`
/// installation-bound rules plus the member loop bounded by the caller's own
/// clause-boundary set. The caller's builder accumulates the contiguous span.
pub(crate) fn take_operational_reaches_clause<'tokens, 'source>(
    input: crate::input::token_cursor::Input<'tokens, 'source>,
    syntax_trees: &mut syntax_trees::SyntaxTrees,
    service_reach_keyword_source_spans: &mut Vec<source::SourceSpan>,
    service_reaches: &mut crate::handle_spans::HandleSpanBuilder<
        syntax_trees::identifier::Identifier,
    >,
    service_reach_is_installation_bound: &mut bool,
    at_clause_boundary: impl Fn(crate::input::token_cursor::Input<'_, '_>) -> bool,
) -> Result<
    crate::input::token_cursor::Input<'tokens, 'source>,
    crate::diagnostics::parse_error::ParseError,
> {
    use tokens::PunctuationKind;
    service_reach_keyword_source_spans.push(input.current_source_span());
    let mut input = input.take_contextual("reaches")?;
    let service_count_before_clause = service_reaches.count();
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
    while !at_clause_boundary(input) {
        input = take_service_reach_row_member(input, syntax_trees, service_reaches)?;
    }
    if *service_reach_is_installation_bound
        && service_reaches.count() == service_count_before_clause
    {
        return Err(input.error_here(
            "an installation-bound reach row requires a nonempty upper bound after `reaches <=`",
        ));
    }
    Ok(input)
}

/// Append one capability contract to the owning table and record it in the
/// caller's clause-span builder.
pub(crate) fn record_capability_contract(
    syntax_trees: &mut syntax_trees::SyntaxTrees,
    contract: syntax_trees::item::CapabilityContract,
    contracts: &mut crate::handle_spans::HandleSpanBuilder<syntax_trees::item::CapabilityContract>,
) {
    let handle = syntax_trees.items.append_capability_contract(contract);
    contracts.push(handle);
}
