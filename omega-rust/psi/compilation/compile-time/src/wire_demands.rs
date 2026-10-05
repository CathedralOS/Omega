//! Collect explicit Build wire demands before directional compatibility checking.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::BuildOperation;
use language_semantics::wire::{WireCompatibilityDemand, WireCompatibilityFact};
use typed_trees::TypedTrees;

/// Only a classified operation on the toolchain Build namespace contributes a
/// demand. Ordinary static operands name product declarations and requested
/// facts; no parser-generated target string or implicit receiver grants it.
/// Consume execution coordinates rather than scanning the entry body: helpers
/// participate, and an unexecuted declaration contributes no demand.
pub fn harvest_wire_compatibility_demands(
    typed: &TypedTrees,
    executed_calls: &[typed_trees::expression::ExpressionHandle],
) -> Result<Vec<WireCompatibilityDemand>, Vec<Diagnostic>> {
    let mut demands = Vec::new();
    let mut diagnostics = Vec::new();
    for expression in executed_calls {
        if typed_trees_to_checked_trees::typed_build_operation(typed, *expression)
            != Some(BuildOperation::WireCompatibilityRequest)
        {
            diagnostics.push(Diagnostic::error(
                "executed wire demand does not identify a checked Build operation",
            ));
            continue;
        }
        let typed_trees::expression::ExpressionNode::Call(call) =
            typed.expression_table.expression(*expression)
        else {
            continue;
        };
        let parts = call
            .machine_arguments
            .iter()
            .map(|argument| {
                if argument.path.is_empty()
                    || argument.type_reference.is_valid()
                    || argument.application.is_some()
                    || argument.const_literal.is_some()
                    || argument.evidence_projection.is_some()
                {
                    return None;
                }
                Some(
                    argument
                        .path
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::"),
                )
            })
            .collect::<Option<Vec<_>>>();
        let Some(parts) = parts.filter(|parts| parts.len() >= 5) else {
            diagnostics.push(Diagnostic::error(
                    "wire compatibility requires Edge, Lineage, Local, Peer and at least one named fact",
                ));
            continue;
        };
        let mut demand = WireCompatibilityDemand {
            edge: parts[0].clone(),
            lineage: parts[1].clone(),
            local_schema: parts[2].clone(),
            peer_schema: parts[3].clone(),
            require_readable: false,
            require_writable: false,
            require_unknown_preservation: false,
            require_canonical: false,
            require_complete_migration: false,
        };
        for fact in &parts[4..] {
            let selected = match WireCompatibilityFact::from_name(fact) {
                Some(WireCompatibilityFact::Readable) => &mut demand.require_readable,
                Some(WireCompatibilityFact::Writable) => &mut demand.require_writable,
                Some(WireCompatibilityFact::PreserveUnknown) => {
                    &mut demand.require_unknown_preservation
                }
                Some(WireCompatibilityFact::Canonical) => &mut demand.require_canonical,
                Some(WireCompatibilityFact::CompleteMigration) => {
                    &mut demand.require_complete_migration
                }
                None => {
                    diagnostics.push(Diagnostic::error(format!(
                        "unknown wire compatibility fact `{fact}`"
                    )));
                    continue;
                }
            };
            if *selected {
                diagnostics.push(Diagnostic::error(format!(
                    "wire compatibility fact `{fact}` is requested twice"
                )));
            }
            *selected = true;
        }
        if demands.iter().any(|existing: &WireCompatibilityDemand| {
            existing.edge == demand.edge
                && existing.lineage == demand.lineage
                && existing.local_schema == demand.local_schema
                && existing.peer_schema == demand.peer_schema
        }) {
            diagnostics.push(Diagnostic::error(format!(
                    "wire compatibility demand for edge `{}`, lineage `{}`, local schema `{}`, and peer schema `{}` is declared twice",
                    demand.edge, demand.lineage, demand.local_schema, demand.peer_schema,
                )));
        } else {
            demands.push(demand);
        }
    }
    if diagnostics.is_empty() {
        Ok(demands)
    } else {
        Err(diagnostics)
    }
}
