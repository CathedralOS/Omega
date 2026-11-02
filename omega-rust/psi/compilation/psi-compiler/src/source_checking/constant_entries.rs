//! Retain complete constant initializer entries for ordinary source checking.
//!
//! Scalar completion later consumes their canonical Terminal applications; this
//! source operation neither evaluates selected values nor rebuilds checked facts.

/// Source checking owns complete initializer entries alongside authored bodies.
/// Rooting one requests ordinary preparation/checking, not execution or successful
/// closure. The declaration retains its original entry identity while the rooted
/// copy participates in the same specialization as every other callable.
pub(super) fn retain_evaluation_entries(
    program: &mut typed_trees::TypedTrees,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut entries = Vec::new();
    for declaration in program.const_declarations() {
        if declaration.evaluated_value.is_some() || !declaration.evaluation_entry.is_valid() {
            continue;
        }
        if !program
            .tables
            .machines
            .is_valid(declaration.evaluation_entry)
        {
            return Err(vec![diagnostics::Diagnostic::error(
                "retained constant lost its complete evaluation entry",
            )]);
        }
        let entry = program.tables.machines.get(declaration.evaluation_entry);
        if program.machine_by_symbol(entry.symbol).is_none() {
            entries.push(entry.clone());
        }
    }
    for entry in entries {
        program.push_machine(entry);
    }
    Ok(())
}
