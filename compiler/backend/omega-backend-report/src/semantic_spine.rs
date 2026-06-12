use crate::BackendReportInput;
use omega_abstract_operations::AbstractOwnershipEventSource;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_machine_bytes::EncodedMachineBoundarySummary;

pub(super) fn write_artifact_semantic_spine(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    output.push_str("## Artifact Semantic Spine\n");
    output.push_str(&format!(
        "values: {}\n",
        backend_plan.value_summary().values.len()
    ));
    write_ownership_events(output, backend_plan);
    write_boundary_policy_checks(output, backend_plan.boundary_summary());
    output.push('\n');
}

/// Render the lowered ownership summary event by event: which place moves or
/// must be cleaned up, in which machine/state, and at which source point.
///
/// These lines are the backend-visible form of the abstract ownership
/// summary preserved through the whole spine. Drops here are exit-edge
/// cleanup OBLIGATIONS (chapter 16's edge-cleanup rule before move
/// subtraction), not emitted cleanup code: no current type carries a cleanup
/// machine, so lowering real cleanup operations awaits drop-bearing types.
fn write_ownership_events(output: &mut String, backend_plan: &BackendReportInput<'_>) {
    let ownership = backend_plan.ownership_summary();

    output.push_str(&format!("moves: {}\n", ownership.moves.len()));
    for (_, event) in ownership.moves.iter() {
        output.push_str(&ownership_event_line(
            backend_plan,
            "move",
            event.source_key,
            event.source,
            event.root,
            event.segments,
        ));
    }

    output.push_str(&format!("drops: {}\n", ownership.drops.len()));
    for (_, event) in ownership.drops.iter() {
        output.push_str(&ownership_event_line(
            backend_plan,
            "drop",
            event.source_key,
            event.source,
            event.root,
            event.segments,
        ));
    }
}

fn ownership_event_line(
    backend_plan: &BackendReportInput<'_>,
    action: &str,
    source_key: omega_control_flow::StateKey,
    source: AbstractOwnershipEventSource,
    root: omega_facts::PlaceRoot,
    segments: HandleSpan<omega_facts::PlaceSegment>,
) -> String {
    let (machine_name, state_name) = backend_plan
        .control_flow
        .state_names_by_key(source_key)
        .map(|(machine, state)| (machine.to_string(), state.to_string()))
        .unwrap_or_else(|| ("<unknown>".to_owned(), "<unknown>".to_owned()));

    format!(
        "- {action} `{place}` in machine `{machine_name}` state `{state_name}` at {source}\n",
        place = ownership_place_text(backend_plan, source_key, root, segments),
        source = ownership_source_text(source),
    )
}

fn ownership_source_text(source: AbstractOwnershipEventSource) -> String {
    match source {
        AbstractOwnershipEventSource::Statement { statement_index } => {
            format!("statement {statement_index}")
        }
        AbstractOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            ..
        } => format!("call ordinal {call_ordinal} in statement {statement_index}"),
        AbstractOwnershipEventSource::StateExit => "state exit".to_owned(),
    }
}

fn ownership_place_text(
    backend_plan: &BackendReportInput<'_>,
    source_key: omega_control_flow::StateKey,
    root: omega_facts::PlaceRoot,
    segments: HandleSpan<omega_facts::PlaceSegment>,
) -> String {
    let mut text = match root {
        omega_facts::PlaceRoot::Symbol(symbol) => {
            ownership_symbol_name(backend_plan, source_key, symbol)
                .unwrap_or_else(|| "<unnamed>".to_owned())
        }
        omega_facts::PlaceRoot::Unknown
        | omega_facts::PlaceRoot::Expression(_)
        | omega_facts::PlaceRoot::TypeReference(_) => "<unnamed>".to_owned(),
    };

    for segment in backend_plan
        .ownership_summary()
        .segments
        .span_or_empty(segments)
    {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                text.push('.');
                text.push_str(
                    &ownership_symbol_name(backend_plan, source_key, *symbol)
                        .unwrap_or_else(|| "<field>".to_owned()),
                );
            }
            omega_facts::PlaceSegment::Index { .. } => text.push_str("[..]"),
        }
    }

    text
}

/// Resolve a place-root or field symbol to its source name through the plans
/// the backend still carries: the source state's parameters, planned state
/// locals, and the machine's owned/contained data.
fn ownership_symbol_name(
    backend_plan: &BackendReportInput<'_>,
    source_key: omega_control_flow::StateKey,
    symbol: SymbolHandle,
) -> Option<String> {
    if !symbol.is_valid() {
        return None;
    }

    if let Some(state) = backend_plan.control_flow.state_by_key(source_key) {
        if let Some(parameter) = backend_plan
            .control_flow
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == symbol)
        {
            return Some(parameter.name.to_string());
        }
    }

    if let Some((_, local)) = backend_plan
        .state_storage
        .locals
        .iter()
        .find(|(_, local)| local.symbol == symbol)
    {
        return Some(local.name.to_string());
    }

    if let Some(machine) = backend_plan
        .control_flow
        .machine_by_symbol(source_key.machine)
    {
        if let Some(owned) = backend_plan
            .control_flow
            .machine_owned_data(machine)
            .iter()
            .find(|owned| owned.symbol == symbol)
        {
            return Some(owned.name.to_string());
        }
        if let Some(contained) = backend_plan
            .control_flow
            .machine_contains(machine)
            .iter()
            .find(|contained| contained.symbol == symbol)
        {
            return Some(contained.name.to_string());
        }
    }

    // Attached-data field symbols (the `seed` of a `self.seed` place) live in
    // the layout plan's field entries, not in the control-flow topology.
    if let Some((_, field)) = backend_plan
        .layouts
        .fields
        .iter()
        .find(|(_, field)| field.symbol == symbol)
    {
        return Some(field.name.to_string());
    }

    // A `self.field` place roots at the machine (or its attached data): the
    // only runtime value either symbol can name in a place is `self`.
    if backend_plan
        .control_flow
        .machine_by_symbol(symbol)
        .is_some()
        || backend_plan
            .layouts
            .data_layouts
            .iter()
            .any(|(_, data)| data.symbol == symbol)
    {
        return Some("self".to_owned());
    }

    None
}

fn write_boundary_policy_checks(output: &mut String, boundaries: &EncodedMachineBoundarySummary) {
    output.push_str(&format!(
        "boundary policy checks: {}\n",
        boundaries.policy_checks.len()
    ));
    if boundaries.policy_checks.is_empty() {
        output.push_str("none\n");
        return;
    }

    for (_, check) in boundaries.policy_checks.iter() {
        output.push_str(&format!(
            "- {:?} `{}` operation {:?}\n",
            check.verdict, check.boundary_policy, check.operation_key
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_abstract_operations::{AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict};
    use omega_machine_bytes::EncodedMachineBoundarySummary;
    use std::sync::Arc;

    #[test]
    fn writes_boundary_policy_checks_from_preserved_semantic_spine() {
        let mut boundaries = EncodedMachineBoundarySummary::default();
        boundaries
            .policy_checks
            .insert(AbstractBoundaryPolicyCheck {
                boundary_policy: Arc::from("omega::core::Slice::Index"),
                verdict: AbstractBoundaryPolicyVerdict::DisallowedBoundaryPolicy,
                ..AbstractBoundaryPolicyCheck::default()
            });

        let mut output = String::new();
        write_boundary_policy_checks(&mut output, &boundaries);

        assert!(output.contains("boundary policy checks: 1"));
        assert!(output.contains("DisallowedBoundaryPolicy"));
        assert!(output.contains("omega::core::Slice::Index"));
    }

    #[test]
    fn renders_ownership_event_sources() {
        assert_eq!(
            ownership_source_text(AbstractOwnershipEventSource::Statement { statement_index: 2 }),
            "statement 2"
        );
        assert_eq!(
            ownership_source_text(AbstractOwnershipEventSource::Call {
                statement_index: 1,
                call_ordinal: 0,
                target_symbol: omega_core::symbols::SymbolHandle::invalid(),
            }),
            "call ordinal 0 in statement 1"
        );
        assert_eq!(
            ownership_source_text(AbstractOwnershipEventSource::StateExit),
            "state exit"
        );
    }
}
