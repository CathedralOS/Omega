//! Semantic records which keep scalar identities even without an executable read.

use semantic_vocabulary::{BlockId, OperationId, ValueId};
use terminal_psi::{CrashRouteGuard, TerminalSuspensionLoanCapture};

#[derive(Default)]
pub struct RecordNamedRows {
    pub blocks: Vec<BlockId>,
    pub operations: Vec<OperationId>,
    pub values: Vec<ValueId>,
}

/// Visit the evidence lanes independently of ordinary runtime call arguments.
pub fn visit_invocation_evidence_values(
    inputs: terminal_psi::ModuleInvocationInputs<'_>,
    mut retain: impl FnMut(ValueId),
) {
    visit_erased_value_arguments(
        inputs.erased_arguments,
        inputs.erased_proof_arguments,
        &mut retain,
    );
    for bucket in inputs.crash_continuations {
        for alternative in &bucket.alternatives {
            if let CrashRouteGuard::Predicate(term) = alternative {
                term.proposition().visit_value_ids(&mut retain);
            }
        }
    }
}

/// Edge proof lanes retain values independently of runtime argument demand.
pub(super) fn visit_erased_value_arguments(
    scalars: &[semantic_vocabulary::ScalarTerm],
    proofs: &[semantic_vocabulary::ProofTerm],
    mut retain: impl FnMut(ValueId),
) {
    for term in scalars {
        term.visit_value_ids(|value| {
            retain(value);
            true
        });
    }
    for term in proofs {
        term.visit_value_ids(|value| {
            retain(value);
            true
        });
    }
}

pub fn record_named_rows(module: &terminal_psi::TerminalModule) -> RecordNamedRows {
    let mut rows = RecordNamedRows::default();
    for observation in module.scalar_applications.conditions.observations() {
        if let terminal_psi::ScalarRangeObservation::EntryResult { result, .. } = observation {
            rows.values.push(*result);
        }
    }
    for machine in &module.machines {
        for invocation in machine.module_invocations() {
            visit_invocation_evidence_values(invocation.inputs, |value| rows.values.push(value));
        }
    }
    rows.operations
        .extend(module.quotients.operations.iter().map(|row| row.call));
    rows.operations
        .extend(module.quotients.adaptations.iter().map(|row| row.copy));
    rows.values.extend(
        module
            .operations()
            .filter(|operation| {
                module
                    .quotients
                    .operations
                    .iter()
                    .any(|row| row.call == operation.id)
            })
            .filter_map(|operation| operation.result.scalar().map(|value| value.id)),
    );
    rows.operations.extend(
        module
            .boundary_application_demands
            .iter()
            .map(|demand| demand.terminal_operation()),
    );
    for site in &module.suspension_call_sites {
        rows.operations.push(site.operation);
    }
    for plan in &module.suspension_call_plans {
        rows.operations.push(plan.operation);
        plan.visit_value_ids(|value| {
            rows.values.push(value);
        });
        for (_, loan) in plan.loans.occurrences.iter() {
            match loan.capture {
                TerminalSuspensionLoanCapture::Reference { formation, .. } => {
                    rows.operations.push(formation);
                }
                TerminalSuspensionLoanCapture::CompletedOutput { call, .. }
                | TerminalSuspensionLoanCapture::CallInput { call, .. } => {
                    rows.operations.push(call);
                }
                _ => {}
            }
        }
    }
    for projection in &module.float_meaning_projections {
        match &projection.source {
            terminal_psi::FloatMeaningSource::DirectMachineParameter(row) => {
                rows.values.push(row.parameter);
            }
            terminal_psi::FloatMeaningSource::DirectMachineResult(row) => {
                rows.values.push(row.result);
            }
            terminal_psi::FloatMeaningSource::DirectBlockParameter(row) => {
                rows.blocks.push(row.block);
                rows.values.push(row.parameter);
            }
            terminal_psi::FloatMeaningSource::DirectOperationResult(row) => {
                rows.operations.push(row.producer);
                rows.values.push(row.result);
            }
            terminal_psi::FloatMeaningSource::DirectCallResult(row) => {
                rows.operations.push(row.producer);
                rows.values.push(row.result);
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
