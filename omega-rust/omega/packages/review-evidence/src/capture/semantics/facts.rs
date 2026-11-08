use crate::capture::EvidenceOption;
use diagnostics::Diagnostic;

pub(crate) fn exactly_one<'item, Item>(
    mut matches: impl Iterator<Item = &'item Item>,
    subject: &str,
    fact_kind: &str,
) -> Result<&'item Item, Vec<Diagnostic>> {
    let first = matches.next().or_evidence(format!(
        "reviewed callable `{subject}` has no exact checked {fact_kind} row"
    ))?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has duplicate checked {fact_kind} rows"
        ))]);
    }
    Ok(first)
}

/// One exact retained synchronous-invocation row for `machine` out of the rows
/// the caller chose to read (the review program's ledger or its custody copy).
pub(crate) fn machine_invocation<'item>(
    rows: &'item [checked_trees::MachineSynchronousInvocationFact],
    machine: symbols::SymbolHandle,
    subject: &str,
    fact_kind: &str,
) -> Result<&'item checked_trees::MachineSynchronousInvocationFact, Vec<Diagnostic>> {
    exactly_one(
        rows.iter().filter(|fact| fact.machine == machine),
        subject,
        fact_kind,
    )
}
