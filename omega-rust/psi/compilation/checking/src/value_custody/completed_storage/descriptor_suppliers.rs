//! Exact producer associations retained by completed descriptor values.
/// None names an unchanged profile. Some(empty) withholds supplier coverage
/// when an alternative loses the association; it never selects another arm.
pub fn descriptor_suppliers(
    control: &checked_trees::FlowControlFacts,
    profile: arena::Handle<checked_trees::FlowOperandRetention>,
    active: &mut Vec<arena::Handle<checked_trees::FlowOperandRetention>>,
) -> Option<Vec<checked_trees::FlowOperandRetainedSupplier>> {
    if !control.operand_retentions.is_valid(profile) {
        return None;
    }
    if active.contains(&profile) {
        return Some(Vec::new());
    }
    let row = control.operand_retentions.get(profile);
    if !row.suppliers.is_empty() {
        return Some(
            control
                .operand_retained_suppliers
                .span_or_empty(row.suppliers)
                .to_vec(),
        );
    }
    if row.alternatives.is_empty() {
        return None;
    }
    active.push(profile);
    let mut values = Vec::new();
    let mut missing = false;
    let mut unavailable = false;
    for alternative in row.alternatives.handles() {
        match descriptor_suppliers(control, alternative, active) {
            None => missing = true,
            Some(sources) if sources.is_empty() => unavailable = true,
            Some(sources) => values.extend(sources),
        }
    }
    active.pop();
    if unavailable || (missing && !values.is_empty()) {
        Some(Vec::new())
    } else if missing {
        None
    } else {
        Some(values)
    }
}
