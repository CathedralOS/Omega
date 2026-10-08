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
    match cause.as_str() {
        "Trap" => Ok(CrashCause::Trap),
        "Abort" => Ok(CrashCause::Abort),
        _ => Err(after_cause.error_here(format!(
            "unknown crash cause `{}`; expected `Trap` or `Abort`",
            cause.as_str()
        ))),
    }
}

pub(crate) fn reject_retired_operational_reach(
    service: &syntax_trees::identifier::Identifier,
    input: crate::input::token_cursor::Input<'_, '_>,
) -> Result<(), crate::diagnostics::parse_error::ParseError> {
    let replacement = match service.as_str() {
        "Suspend" => "suspends;",
        "Block" => "blocks;",
        "thread_block" => "blocks;",
        "sync_wait" => "the appropriate independent `suspends;` and/or `blocks;` clause",
        _ => return Ok(()),
    };
    Err(input.error_here(format!(
        "`reaches {}` is invalid: `reaches` contains boundary-service identities only; write `{replacement}` as an independent operational clause",
        service.as_str()
    )))
}
/// One `reaches` member plus its `,`/`+` separator, shared by the machine
/// clause and trait signature clause loops; `service_start`/`service_count`
/// accumulate the contiguous span the caller drains afterward.
pub(crate) fn take_service_reach_row_member<'tokens, 'source>(
    input: crate::input::token_cursor::Input<'tokens, 'source>,
    syntax_trees: &mut syntax_trees::SyntaxTrees,
    service_start: &mut arena::Handle<syntax_trees::identifier::Identifier>,
    service_count: &mut u32,
    overflow_message: &'static str,
) -> Result<
    crate::input::token_cursor::Input<'tokens, 'source>,
    crate::diagnostics::parse_error::ParseError,
> {
    use tokens::PunctuationKind;
    let (service, rest) = input.take_identifier()?;
    reject_retired_operational_reach(&service, rest)?;
    let handle = syntax_trees.items.append_identifier_path_member(service);
    if *service_count == 0 {
        *service_start = handle;
    }
    *service_count = service_count.checked_add(1).expect(overflow_message);
    let mut input = rest;
    if input.at_punctuation(PunctuationKind::Comma) {
        input = input.take_punctuation(PunctuationKind::Comma, ",")?;
    } else if input.at_punctuation(PunctuationKind::Plus) {
        input = input.take_punctuation(PunctuationKind::Plus, "+")?;
    }
    Ok(input)
}
