//! Contract clauses on machines and signatures: `parse_contract_clauses`
//! reads a machine's clauses, `signature` a signature's,
//! `facts` the proof-fact lists they share, and
//! `conformance` the `satisfies` clauses and external provider bindings.

pub(crate) mod conformance;
pub(crate) mod facts;
pub(crate) mod parse_contract_clauses;
pub(crate) mod signature;

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
        "Suspend" => "suspends",
        "Block" => "blocks",
        "thread_block" => "blocks",
        "sync_wait" => "the appropriate independent `suspends` and/or `blocks` clause",
        _ => return Ok(()),
    };
    Err(input.error_here(format!(
        "`reaches {}` is invalid: `reaches` contains boundary-service identities only; write `{replacement}` as an independent operational clause",
        service.as_str(&input)
    )))
}
/// One `reaches` member plus its `,`/`+` separator, shared by the machine
/// clause and trait signature clause loops. Each owner stages members until
/// recursive callable parsing finishes, then publishes one contiguous span.
pub(crate) fn take_service_reach_row_member<'tokens, 'source>(
    input: crate::input::token_cursor::Input<'tokens, 'source>,
    service_reaches: &mut Vec<syntax_trees::identifier::Identifier>,
) -> Result<
    crate::input::token_cursor::Input<'tokens, 'source>,
    crate::diagnostics::parse_error::ParseError,
> {
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

/// The whole `reaches <Service> + ...` clause both grammars share: the `<=`
/// installation-bound rules plus the member loop bounded by the caller's own
/// clause-boundary set. The caller publishes its staged members after parsing.
pub(crate) fn take_operational_reaches_clause<'tokens, 'source>(
    input: crate::input::token_cursor::Input<'tokens, 'source>,
    service_reach_keyword_source_spans: &mut Vec<source::SourceSpan>,
    service_reaches: &mut Vec<syntax_trees::identifier::Identifier>,
    service_reach_is_installation_bound: &mut bool,
    at_clause_boundary: impl Fn(crate::input::token_cursor::Input<'_, '_>) -> bool,
) -> Result<
    crate::input::token_cursor::Input<'tokens, 'source>,
    crate::diagnostics::parse_error::ParseError,
> {
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

/// Publish an owner's complete clause list only after its nested callable
/// contracts have been parsed. Recursive owners may use the same arena without
/// interleaving their rows into this owner's contiguous span.
pub(crate) fn publish_capability_contracts(
    syntax_trees: &mut syntax_trees::SyntaxTrees,
    contracts: Vec<syntax_trees::item::CapabilityContract>,
) -> arena::HandleSpan<syntax_trees::item::CapabilityContract> {
    let mut span = crate::handle_spans::HandleSpanBuilder::new();
    for contract in contracts {
        span.push(syntax_trees.items.append_capability_contract(contract));
    }
    span.finish()
}

/// A semicolon belongs to the complete bodyless declaration, never a header
/// clause. Concrete machine and trait-member owners call this before taking
/// their terminator. Nested `requires machine` owners deliberately do not:
/// after their own terminator, a following clause belongs to the outer scope.
pub(crate) fn reject_machine_clause_separator(
    syntax_trees: &mut syntax_trees::SyntaxTrees,
    input: crate::input::token_cursor::Input<'_, '_>,
) -> Result<(), crate::diagnostics::parse_error::ParseError> {
    use tokens::PunctuationKind;
    if !input.at_punctuation(PunctuationKind::Semicolon) {
        return Ok(());
    }
    let after = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    if !after.tokens.is_empty()
        && !after.at_punctuation(PunctuationKind::RightBrace)
        && !crate::declarations::parse_declaration::starts_following_item(syntax_trees, after)
    {
        return Err(input.error_here(
            "machine header clauses have no trailing semicolons; one semicolon closes a complete bodyless declaration",
        ));
    }
    Ok(())
}
