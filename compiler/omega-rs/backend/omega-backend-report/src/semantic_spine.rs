use crate::BackendReportInput;
use omega_machine_bytes::EncodedMachineBoundarySummary;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

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

/// Render the semantic permission ledger preserved through the backend spine.
/// This is the canonical ownership carrier: establishment, transfer,
/// consumption, and affine discard remain distinguishable instead of being
/// collapsed back into the retired move/drop compatibility summaries.
fn write_ownership_events(output: &mut String, backend_plan: &BackendReportInput<'_>) {
    let ownership = backend_plan.ownership_summary();

    output.push_str(&format!("permissions: {}\n", ownership.permissions.len()));
    let realization_state = if ownership.realizations.len() == ownership.permissions.len() {
        "complete"
    } else {
        "INCOMPLETE"
    };
    output.push_str(&format!(
        "permission realizations: {} ({realization_state})\n",
        ownership.realizations.len()
    ));
    for (event_handle, event) in ownership.permissions.iter() {
        let (machine_name, state_name) = backend_plan
            .control_flow
            .state_names_by_key(event.source_key)
            .map(|(machine, state)| (machine.to_string(), state.to_string()))
            .unwrap_or_else(|| ("<unknown>".to_owned(), "<unknown>".to_owned()));
        let realization = ownership
            .realizations
            .iter()
            .find(|(_, realization)| realization.event == event_handle)
            .map(|(_, realization)| permission_realization_text(ownership, realization.kind))
            .unwrap_or_else(|| "UNLINKED".to_owned());
        output.push_str(&format!(
            "- {kind:?} `{place}` in machine `{machine_name}` state `{state_name}` at {source} (multiplicity={multiplicity:?}, access={access:?}, claim={claim}, provenance={provenance}, obligation_live={live}) realization={realization}\n",
            kind = event.kind,
            place = ownership_place_text(
                backend_plan,
                event.source_key,
                event.root,
                event.segments,
            ),
            source = permission_source_text(event.source),
            multiplicity = event.multiplicity,
            access = event.access,
            claim = permission_claim_identity_text(backend_plan, event.claim_identity),
            provenance = permission_provenance_text(backend_plan, event.provenance),
            live = event.obligation_live,
        ));
    }
}

fn permission_claim_identity_text(
    backend_plan: &BackendReportInput<'_>,
    identity: psi_language_semantics::PermissionClaimIdentity,
) -> String {
    use psi_language_semantics::PermissionClaimIdentity;
    match identity {
        PermissionClaimIdentity::Unknown => "unknown".to_owned(),
        PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source,
            ordinal,
        } => {
            let names = backend_plan
                .control_flow
                .state_key_by_symbols(machine_symbol, state_symbol)
                .and_then(|key| backend_plan.control_flow.state_names_by_key(key))
                .map(|(machine, state)| format!("{machine}::{state}"))
                .unwrap_or_else(|| "<unknown>".to_owned());
            format!("{names} at {} #{ordinal}", permission_source_text(source))
        }
    }
}

fn permission_realization_text(
    ownership: &omega_abstract_operations::AbstractOwnershipSummary,
    kind: omega_abstract_operations::AbstractPermissionRealizationKind,
) -> String {
    match kind {
        omega_abstract_operations::AbstractPermissionRealizationKind::SelectedInstructions {
            instruction_indices,
        } => format!(
            "selected-instructions{:?}",
            ownership
                .realization_instruction_indices
                .span_or_empty(instruction_indices)
        ),
        omega_abstract_operations::AbstractPermissionRealizationKind::CheckedNoCode { reason } => {
            let reason = match reason {
                omega_abstract_operations::CheckedNoCodePermissionReason::ExplicitZeroCodeConsume => {
                    "explicit-zero-code-consume"
                }
                omega_abstract_operations::CheckedNoCodePermissionReason::ElidedNoDebt => {
                    "elided-no-debt"
                }
                omega_abstract_operations::CheckedNoCodePermissionReason::TrivialAffineDrop => {
                    "trivial-affine-drop"
                }
            };
            format!("checked-no-code({reason})")
        }
    }
}

fn permission_source_text(source: psi_language_semantics::PermissionEventSource) -> String {
    match source {
        psi_language_semantics::PermissionEventSource::StateEntry => "state entry".to_owned(),
        psi_language_semantics::PermissionEventSource::Statement { statement_index } => {
            format!("statement {statement_index}")
        }
        psi_language_semantics::PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            ..
        } => format!("call ordinal {call_ordinal} in statement {statement_index}"),
        psi_language_semantics::PermissionEventSource::StateExit => "state exit".to_owned(),
    }
}

fn permission_provenance_text(
    backend_plan: &BackendReportInput<'_>,
    provenance: psi_language_semantics::PermissionProvenance,
) -> String {
    use psi_language_semantics::PermissionProvenance;
    match provenance {
        PermissionProvenance::Unknown => "unknown".to_owned(),
        PermissionProvenance::Established {
            machine_symbol,
            state_symbol,
            source,
        } => {
            let names = backend_plan
                .control_flow
                .state_key_by_symbols(machine_symbol, state_symbol)
                .and_then(|key| backend_plan.control_flow.state_names_by_key(key))
                .map(|(machine, state)| format!("{machine}::{state}"))
                .unwrap_or_else(|| "<unknown>".to_owned());
            format!("{names} at {}", permission_source_text(source))
        }
    }
}

fn ownership_place_text(
    backend_plan: &BackendReportInput<'_>,
    source_key: omega_control_flow::StateKey,
    root: psi_facts::PlaceRoot,
    segments: HandleSpan<psi_facts::PlaceSegment>,
) -> String {
    let mut text = match root {
        psi_facts::PlaceRoot::Symbol(symbol) => {
            ownership_symbol_name(backend_plan, source_key, symbol)
                .unwrap_or_else(|| "<unnamed>".to_owned())
        }
        psi_facts::PlaceRoot::Unknown
        | psi_facts::PlaceRoot::Expression(_)
        | psi_facts::PlaceRoot::TypeReference(_) => "<unnamed>".to_owned(),
    };

    for segment in backend_plan
        .ownership_summary()
        .segments
        .span_or_empty(segments)
    {
        match segment {
            psi_facts::PlaceSegment::Field { symbol } => {
                text.push('.');
                text.push_str(
                    &ownership_symbol_name(backend_plan, source_key, *symbol)
                        .unwrap_or_else(|| "<field>".to_owned()),
                );
            }
            psi_facts::PlaceSegment::Case { variant } => {
                text.push_str("::");
                text.push_str(
                    &ownership_symbol_name(backend_plan, source_key, *variant)
                        .unwrap_or_else(|| "<case>".to_owned()),
                );
            }
            psi_facts::PlaceSegment::FixedIndex { index } => {
                text.push('[');
                text.push_str(&index.to_string());
                text.push(']');
            }
            psi_facts::PlaceSegment::Index { .. } => text.push_str("[..]"),
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
    fn renders_permission_event_sources() {
        assert_eq!(
            permission_source_text(psi_language_semantics::PermissionEventSource::StateEntry),
            "state entry"
        );
        assert_eq!(
            permission_source_text(psi_language_semantics::PermissionEventSource::Statement {
                statement_index: 2,
            }),
            "statement 2"
        );
        assert_eq!(
            permission_source_text(psi_language_semantics::PermissionEventSource::Call {
                statement_index: 1,
                call_ordinal: 0,
                target_symbol: psi_symbols::SymbolHandle::invalid(),
            }),
            "call ordinal 0 in statement 1"
        );
        assert_eq!(
            permission_source_text(psi_language_semantics::PermissionEventSource::StateExit),
            "state exit"
        );
    }
}
