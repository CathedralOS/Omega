use psi_diagnostics::Diagnostic;

pub(crate) fn exactly_one<'item, Item>(
    mut matches: impl Iterator<Item = &'item Item>,
    subject: &str,
    fact_kind: &str,
) -> Result<&'item Item, Vec<Diagnostic>> {
    let first = matches.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has no exact checked {fact_kind} row"
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has duplicate checked {fact_kind} rows"
        ))]);
    }
    Ok(first)
}
