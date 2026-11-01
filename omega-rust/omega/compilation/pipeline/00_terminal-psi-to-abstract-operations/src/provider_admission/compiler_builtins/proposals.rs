//! Target-constrained proposals for compiler-owned native builtins.
//!
//! Provider selection and checked intrinsic review establish the exact
//! declaration/target identity. This module performs the later executable
//! projection: only compiler-intrinsic requirements actually called by the
//! canonical Terminal artifact receive a structural proposal, bound here to
//! its selected plan and Terminal boundary. The consuming lowerer admits it
//! through its local target catalog.

use super::external_satisfaction::reconcile_numeric_meaning;
use diagnostics::Diagnostic;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::ServiceSchema;
use provider_selection::selected_external_execution;
use std::collections::BTreeSet;

pub use provider_selection::CompilerIntrinsicPlanProvenance;

struct SelectedIntrinsicRow<'a> {
    requirement_identity: &'a str,
    plan_index: usize,
    row: &'a effects::provider_plan::ProviderPlanRow,
    semantic_schema: &'a ServiceSchema,
    execution: Option<CompilerIntrinsicExecutionIdentity>,
    external_realization: Option<&'a str>,
}

/// Project called intrinsic rows into native catalog proposals, each bound
/// once to its selected plan and the one Terminal boundary it settles.
/// Provenance stays borrowed and source-free; every plan is checked even
/// without demand. The receiving target catalog admits these proposals later.
pub(crate) fn derive_selected_intrinsic_settlement_proposals<'a>(
    selected: &'a effects::SelectedProviderPlanFacts,
    provenance: impl ExactSizeIterator<Item = CompilerIntrinsicPlanProvenance<'a>>,
    demanded_boundaries: &BTreeSet<String>,
    boundaries: &[terminal_psi::BoundaryMachineDeclaration],
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<Vec<super::TerminalCompilerBuiltinProposal>, Vec<Diagnostic>> {
    let plans = selected.plans();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected provider plans are not aligned with compiler-intrinsic settlement provenance",
        )]);
    }
    let mut selected_rows = Vec::new();
    for (plan_index, (plan, retained)) in plans.iter().zip(provenance).enumerate() {
        if retained.plan != plan
            || retained.row_executions.len() != plan.rows.len()
            || retained.row_numeric_realizations.len() != plan.rows.len()
        {
            return Err(vec![Diagnostic::error(format!(
                "selected compiler-intrinsic plan `{}` has misaligned retained settlement provenance",
                plan.name,
            ))]);
        }
        if demanded_boundaries.is_empty() {
            continue;
        }
        for ((row, execution), realization) in plan
            .rows
            .iter()
            .zip(retained.row_executions)
            .zip(retained.row_numeric_realizations)
        {
            if matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
            ) {
                selected_rows.push(SelectedIntrinsicRow {
                    requirement_identity: plan.identities.text(row.requirement_identity),
                    plan_index,
                    row,
                    semantic_schema: retained.semantic_schema,
                    execution: *execution,
                    external_realization: realization.as_deref(),
                });
            }
        }
    }
    // Keep the row/execution association from its producing plan. Sorting only
    // this borrowed selection leaves plan identity and row ownership unchanged.
    selected_rows
        .sort_unstable_by(|left, right| left.requirement_identity.cmp(right.requirement_identity));

    // Each demanded callable retains its own occurrence. A selected row may
    // realize both a portable family head and a direct requirement call.
    let mut evidence = Vec::new();
    let mut diagnostics = Vec::new();
    for requirement in demanded_boundaries {
        let actual_requirement = selected.selected_requirement_identity(requirement);
        let first =
            selected_rows.partition_point(|row| row.requirement_identity < actual_requirement);
        let matches = selected_rows[first..]
            .iter()
            .take_while(|row| row.requirement_identity == actual_requirement)
            .filter(|row| {
                selected.selected_row_matches_requirement(
                    requirement,
                    row.requirement_identity,
                    plans[row.plan_index].report_fingerprint(),
                )
            })
            .collect::<Vec<_>>();
        let [selected_row] = matches.as_slice() else {
            if !matches.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "Terminal boundary `{requirement}` resolves to {} selected compiler-intrinsic rows", matches.len(),
                )));
            }
            continue;
        };
        let mut matching_boundaries = boundaries
            .iter()
            .filter(|boundary| identities.text(boundary.identity) == Some(requirement.as_str()));
        let (Some(boundary), None) = (matching_boundaries.next(), matching_boundaries.next())
        else {
            diagnostics.push(Diagnostic::error(format!(
                "compiler-builtin proposal for `{requirement}` does not rejoin one Terminal boundary"
            )));
            continue;
        };
        // A row this compile could not close is retained with its reason, not
        // refused here: whether the boundary is executed is each entry's
        // execution-scope question, answered at admission.
        let identity = match selected_external_execution(
            external_providers,
            identities,
            &boundary.owner,
            &plans[selected_row.plan_index],
            selected_row.row,
            selected_row.execution,
            selected_row.external_realization,
        ) {
            Ok(identity) => identity,
            Err(reason) => {
                diagnostics.push(Diagnostic::error(format!(
                    "Terminal boundary `{requirement}` {reason}"
                )));
                continue;
            }
        };
        if let Err(reason) = reconcile_numeric_meaning(boundary.numeric_intrinsic, identity) {
            diagnostics.push(Diagnostic::error(format!(
                "Terminal boundary `{requirement}` {reason}"
            )));
            continue;
        }
        let format = boundary
            .numeric_intrinsic
            .and_then(|meaning| meaning.builtin_format());
        let settlement = match identity {
            None => super::TerminalCompilerBuiltinSettlement::NoClosedCatalogIdentity,
            // A private compiler operation is not a selectable implementation of
            // an authored provider requirement. It is bound from its own retained
            // declaration and lifecycle contract, never from a Build intrinsic row.
            Some(CompilerIntrinsicExecutionIdentity::AccessLifecycle(_)) => {
                super::TerminalCompilerBuiltinSettlement::NoNativeBoundaryRealization
            }
            Some(identity) => {
                match super::SettledCompilerBuiltin::from_identity(identity, boundary.id, format) {
                    Some(execution) => super::TerminalCompilerBuiltinSettlement::Execution {
                        identity,
                        execution,
                    },
                    None => super::TerminalCompilerBuiltinSettlement::NoNativeBoundaryRealization,
                }
            }
        };
        evidence.push(super::TerminalCompilerBuiltinProposal::new(
            selected_row.requirement_identity.to_owned(),
            selected_row.plan_index,
            boundary.id,
            settlement,
            selected_row.semantic_schema,
            &plans[selected_row.plan_index].schema,
        ));
    }
    if diagnostics.is_empty() {
        Ok(evidence)
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use language_semantics::numeric_intrinsics::NumericIntrinsic;
    use target_operations::CompilerBuiltinExecution;

    #[test]
    fn portable_numeric_meaning_cannot_authorize_or_replace_provider_custody() {
        let meaning = NumericIntrinsic::from_canonical_encoding([0, 5, 1, 0]).unwrap();
        let selected = CompilerIntrinsicExecutionIdentity::from(meaning);
        assert!(reconcile_numeric_meaning(Some(meaning), Some(selected)).is_ok());
        assert!(reconcile_numeric_meaning(Some(meaning), None).is_err());
        assert!(reconcile_numeric_meaning(None, Some(selected)).is_err());
        let other = NumericIntrinsic::from_canonical_encoding([0, 6, 1, 0]).unwrap();
        assert!(reconcile_numeric_meaning(Some(other), Some(selected)).is_err());
        assert!(
            reconcile_numeric_meaning(
                Some(meaning),
                Some(CompilerIntrinsicExecutionIdentity::HostedExitProcessI32)
            )
            .is_err()
        );
        assert!(
            reconcile_numeric_meaning(
                None,
                Some(CompilerIntrinsicExecutionIdentity::HostedExitProcessI32)
            )
            .is_ok()
        );
        assert!(reconcile_numeric_meaning(None, None).is_ok());
        let format = meaning.builtin_format().unwrap();
        assert_eq!(format, semantic_vocabulary::IeeeFloatFormat::Binary64);
        assert!(matches!(
            super::super::SettledCompilerBuiltin::from_identity(
                selected,
                semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
                Some(format)
            )
            .map(super::super::SettledCompilerBuiltin::execution),
            Some(CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                operation: target_operations::IeeeFloatArithmeticOperation::IsNan,
                format: semantic_vocabulary::IeeeFloatFormat::Binary64,
            })
        ));
    }
}
