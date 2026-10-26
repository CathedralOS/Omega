//! Optimizer module role: stage group. Identities a rewrite must not erase.
//!
//! Proof, custody and ranking sidecars name values and coordinates that no
//! direct use in the module mentions, so a pass that judges liveness or
//! equivalence from uses alone would delete or renumber them. `proof_values`
//! answers which values those sidecars name; `ranked_coverage` answers which
//! coordinates one machine's retained ranking evidence covers; `record_named_rows`
//! reads the carriers every low-level rewrite and the cleanup's evidence
//! inventory share. The three rewrites that can drop or merge a value —
//! `copy_propagation`, `global_value_numbering` and `dead_scalar_elimination`
//! — consult these before acting; nothing here rewrites anything itself.

pub(crate) mod proof_values;
pub(crate) mod ranked_coverage;

use semantic_vocabulary::{BlockId, OperationId, ValueId};
use std::collections::BTreeSet;

/// Rows a module's semantic record names by identity rather than through an
/// executable use: the block, operation and value rows a rewrite may not drop
/// or renumber. Every Terminal identity is module-unique, so a carrier that
/// names a row pins it wherever the row lives.
#[derive(Debug, Default)]
pub(crate) struct RecordNamedRows {
    pub(crate) blocks: BTreeSet<BlockId>,
    pub(crate) operations: BTreeSet<OperationId>,
    pub(crate) values: BTreeSet<ValueId>,
}

/// Read the record carriers the low-level rewrites and the cleanup's evidence
/// inventory share: quotient rows and their adaptations, boundary application
/// demands, suspension call sites, suspension plans with their loan
/// formations, and float-meaning projections. The cleanup keeps every named
/// row; each rewrite keeps the named value identities exact. One read serves
/// all of them, so a carrier added here reaches every consumer at once.
pub(crate) fn record_named_rows(module: &terminal_psi::TerminalModule) -> RecordNamedRows {
    let mut rows = RecordNamedRows::default();
    rows.operations
        .extend(module.quotients.operations.iter().map(|row| row.call));
    rows.operations
        .extend(module.quotients.adaptations.iter().map(|row| row.copy));
    rows.values.extend(quotient_call_results(module));
    rows.operations.extend(
        module
            .boundary_application_demands
            .iter()
            .map(|demand| demand.terminal_operation()),
    );
    for site in &module.suspension_call_sites {
        rows.operations.insert(site.operation);
    }
    for plan in &module.suspension_call_plans {
        rows.operations.insert(plan.operation);
        plan.visit_value_ids(|value| {
            rows.values.insert(value);
        });
        for (_, loan) in plan.loans.occurrences.iter() {
            if let terminal_psi::TerminalSuspensionLoanCapture::Reference { formation, .. } =
                loan.capture
            {
                rows.operations.insert(formation);
            }
        }
    }
    for projection in &module.float_meaning_projections {
        match &projection.source {
            terminal_psi::FloatMeaningSource::DirectMachineParameter(row) => {
                rows.values.insert(row.parameter);
            }
            terminal_psi::FloatMeaningSource::DirectMachineResult(row) => {
                rows.values.insert(row.result);
            }
            terminal_psi::FloatMeaningSource::DirectBlockParameter(row) => {
                rows.blocks.insert(row.block);
                rows.values.insert(row.parameter);
            }
            terminal_psi::FloatMeaningSource::DirectOperationResult(row) => {
                rows.operations.insert(row.producer);
                rows.values.insert(row.result);
            }
            terminal_psi::FloatMeaningSource::DirectCallResult(row) => {
                rows.operations.insert(row.producer);
                rows.values.insert(row.result);
            }
            terminal_psi::FloatMeaningSource::TransitionalInput(_)
            | terminal_psi::FloatMeaningSource::DirectStructuralLeaf(_)
            | terminal_psi::FloatMeaningSource::ExactBinary32Literal(_)
            | terminal_psi::FloatMeaningSource::ExactBinary64Literal(_)
            | terminal_psi::FloatMeaningSource::SemanticApplication(_) => {}
        }
    }
    rows
}

/// Selected boundary operations retain their meaning until Omega supplies
/// the realization. The sorted portable demand roster is the authority here,
/// not a checked-source occurrence list.
pub(crate) fn has_boundary_application(
    demands: &[terminal_psi::TerminalBoundaryApplicationDemand],
    operation: semantic_vocabulary::OperationId,
) -> bool {
    demands
        .binary_search_by_key(&operation, |demand| demand.terminal_operation())
        .is_ok()
}

pub(crate) fn boundary_application_results(
    module: &terminal_psi::TerminalModule,
) -> impl Iterator<Item = semantic_vocabulary::ValueId> + '_ {
    module
        .operations()
        .filter(|operation| {
            has_boundary_application(&module.boundary_application_demands, operation.id)
        })
        .filter_map(|operation| operation.result.scalar().map(|result| result.id))
}

/// Unsealed source custody names either an ordinary operation or a terminal
/// invocation edge. Their namespaces remain distinct, even at equal markers.
pub(crate) fn contains_source_call(
    machine: &terminal_psi::TerminalMachine,
    occurrence: &lowered_psi::LoweredSourceCallOccurrence,
) -> bool {
    match occurrence.terminal_site {
        semantic_vocabulary::BoundaryInvocationSite::Operation(operation) => machine
            .blocks
            .iter()
            .any(|block| block.operations.iter().any(|row| row.id == operation)),
        semantic_vocabulary::BoundaryInvocationSite::ExternalCompletion(edge) => {
            machine.blocks.iter().any(|block| {
                matches!(block.terminator,
                terminal_psi::Terminator::ExternalCompletion { edge: actual, .. } if actual == edge)
            })
        }
    }
}

/// Prune debug sites whose subject a rewrite removed — dropped operations,
/// their scalar results, and dropped block parameters — then restamp the
/// module identity on the surviving map.
pub(crate) fn prune_debug_map(
    before: &lowered_psi::LoweredPsi,
    after: &mut lowered_psi::LoweredPsi,
) -> Result<(), crate::PsiOptimizationStageError> {
    use std::collections::BTreeSet;
    let Some(debug) = after.debug_map.as_mut() else {
        return Ok(());
    };
    let operations = after
        .semantic_module
        .operations()
        .map(|operation| operation.id)
        .collect::<BTreeSet<_>>();
    let removed_values = before
        .semantic_module
        .machines
        .iter()
        .zip(&after.semantic_module.machines)
        .flat_map(|(old_machine, new_machine)| {
            let removed_results = old_machine
                .operations()
                .filter(|operation| !operations.contains(&operation.id))
                .filter_map(|operation| operation.result.scalar().map(|value| value.id));
            let removed_parameters = old_machine.blocks.iter().zip(&new_machine.blocks).flat_map(
                |(old_block, new_block)| {
                    let retained = new_block
                        .parameters
                        .iter()
                        .map(|parameter| parameter.id)
                        .collect::<BTreeSet<_>>();
                    old_block
                        .parameters
                        .iter()
                        .filter(move |parameter| !retained.contains(&parameter.id))
                        .map(|parameter| parameter.id)
                },
            );
            removed_results.chain(removed_parameters)
        })
        .collect::<BTreeSet<_>>();
    debug.sites.retain(|site| match site.subject {
        terminal_psi::DebugSubject::Operation(operation) => operations.contains(&operation),
        terminal_psi::DebugSubject::Value(value) => !removed_values.contains(&value),
        _ => true,
    });
    debug.semantic = terminal_codec::terminal_psi_identity(&after.semantic_module)
        .map_err(crate::PsiOptimizationStageError::InvalidSemantic)?;
    Ok(())
}

/// The catalog joins a selected representative call to its public operation.
/// Its scalar result remains an exact correspondence endpoint even when no
/// ordinary executable use survives; preserve it until catalog-aware rewriting
/// can transport that judgment along with the computation.
pub(crate) fn quotient_call_results(
    module: &terminal_psi::TerminalModule,
) -> impl Iterator<Item = semantic_vocabulary::ValueId> + '_ {
    module
        .operations()
        .filter(|operation| {
            module
                .quotients
                .operations
                .iter()
                .any(|row| row.call == operation.id)
        })
        .filter_map(|operation| operation.result.scalar().map(|result| result.id))
}
