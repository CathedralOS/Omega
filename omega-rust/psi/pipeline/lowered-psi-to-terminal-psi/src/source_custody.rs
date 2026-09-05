//! Source-companion custody checked against the exact published artifact.

use checked_trees::{CheckedTrees, CheckedUnitEffectOperationPlan};
use lowered_psi::LoweredPsi;
use semantic_vocabulary::OperationId;
use std::collections::BTreeSet;

fn unsupported<T>(message: &'static str) -> Result<T, &'static str> {
    Err(message)
}
/// Checked source demand scope retained beside one exact Terminal artifact.
///
/// The fields are private so downstream realization cannot replace checked
/// D29 custody with a caller-authored count or Boolean. The receipt retains the
/// complete checked demand roster and is useful only for the canonical artifact
/// produced by the same lowering operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoundaryOperatorApplicationScope {
    terminal_artifact_identity: terminal_codec::TerminalArtifactIdentity,
    applications: Vec<checked_trees::CheckedBoundaryOperatorApplicationDemand>,
    occurrences: Vec<CheckedBoundaryOperatorApplicationOccurrence>,
}

impl CheckedBoundaryOperatorApplicationScope {
    pub fn validate_for_artifact(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
    ) -> Result<(), &'static str> {
        if self.terminal_artifact_identity != artifact.manifest().identity() {
            return Err("checked boundary-operator scope belongs to a different Terminal artifact");
        }
        Ok(())
    }

    pub fn applications(&self) -> &[checked_trees::CheckedBoundaryOperatorApplicationDemand] {
        &self.applications
    }

    pub fn is_empty(&self) -> bool {
        self.applications.is_empty()
    }

    pub fn occurrences(&self) -> &[CheckedBoundaryOperatorApplicationOccurrence] {
        &self.occurrences
    }
}

/// Compiler-private join from one retained checked D29 demand to the exact
/// Terminal operation produced for it. The application index addresses the
/// immutable roster in the checked boundary-operator application scope.
///
/// This is source-to-Terminal custody, not canonical D29 coverage: public
/// source-free application identity and the role-specific realization
/// companion remain later compiler-owned projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedBoundaryOperatorApplicationOccurrence {
    application_index: usize,
    terminal_operation: OperationId,
}

impl CheckedBoundaryOperatorApplicationOccurrence {
    pub const fn application_index(self) -> usize {
        self.application_index
    }

    pub const fn terminal_operation(self) -> OperationId {
        self.terminal_operation
    }
}

pub fn checked_boundary_operator_scope(
    checked: &CheckedTrees,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    lowered: &LoweredPsi,
) -> Result<CheckedBoundaryOperatorApplicationScope, &'static str> {
    let semantic = terminal_codec::terminal_psi_identity(&lowered.semantic_module)
        .map_err(|_| "checked boundary-operator scope has invalid semantics")?;
    if semantic != artifact.manifest().semantic() {
        return Err("checked boundary-operator scope semantics differ from the published artifact");
    }
    Ok(CheckedBoundaryOperatorApplicationScope {
        terminal_artifact_identity: artifact.manifest().identity(),
        applications: checked.facts.operators.boundary_applications.clone(),
        occurrences: checked_boundary_operator_occurrences(checked, lowered)?,
    })
}

fn checked_boundary_operator_occurrences(
    checked: &CheckedTrees,
    lowered: &LoweredPsi,
) -> Result<Vec<CheckedBoundaryOperatorApplicationOccurrence>, &'static str> {
    let mut occurrences = Vec::new();
    let mut matched_ieee_float_fmas = 0_usize;
    for machine in &checked.facts.flow.terminal_unit_effects.machines {
        for operation in &machine.operations {
            let (statement_index, requirement, terminal_operation) = match operation {
                CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                    coordinate,
                    requirement_operator,
                    realization_machine,
                    ..
                }
                | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                    coordinate,
                    requirement_operator,
                    realization_machine,
                    ..
                }
                | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                    coordinate,
                    requirement_operator,
                    realization_machine,
                    ..
                } => {
                    let statement_index = usize::try_from(coordinate.statement_index)
                        .map_err(|_| "selected operator statement coordinate exceeds usize")?;
                    let call_ordinal = usize::try_from(coordinate.call_ordinal)
                        .map_err(|_| "selected operator call coordinate exceeds usize")?;
                    let matching = lowered
                        .source_call_occurrences
                        .iter()
                        .filter(|occurrence| {
                            occurrence.source_state == machine.state
                                && occurrence.statement_index == statement_index
                                && occurrence.call_ordinal == call_ordinal
                                && occurrence.source_target == *realization_machine
                        })
                        .collect::<Vec<_>>();
                    if matching.is_empty() {
                        continue;
                    }
                    let [matching] = matching.as_slice() else {
                        return unsupported(
                            "selected operator application maps to duplicate Terminal call occurrences",
                        );
                    };
                    (
                        statement_index,
                        *requirement_operator,
                        matching.terminal_operation,
                    )
                }
                CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
                    coordinate,
                    requirement_operator,
                    ..
                } => {
                    let statement_index = usize::try_from(coordinate.statement_index)
                        .map_err(|_| "selected IEEE FMA statement coordinate exceeds usize")?;
                    let call_ordinal = usize::try_from(coordinate.call_ordinal)
                        .map_err(|_| "selected IEEE FMA call coordinate exceeds usize")?;
                    let matching = lowered
                        .selected_ieee_float_fma_occurrences
                        .iter()
                        .filter(|occurrence| {
                            occurrence.source_state == machine.state
                                && occurrence.statement_index == statement_index
                                && occurrence.call_ordinal == call_ordinal
                                && occurrence.requirement_operator == *requirement_operator
                        })
                        .collect::<Vec<_>>();
                    if matching.is_empty() {
                        continue;
                    }
                    let [matching] = matching.as_slice() else {
                        return unsupported(
                            "selected IEEE FMA application maps to duplicate Terminal occurrences",
                        );
                    };
                    matched_ieee_float_fmas += 1;
                    (
                        statement_index,
                        *requirement_operator,
                        matching.terminal_operation,
                    )
                }
                _ => continue,
            };
            let matching_applications = checked
                .facts
                .operators
                .boundary_applications
                .iter()
                .enumerate()
                .filter(|(_, application)| {
                    application.requirement_symbol == requirement
                        && matches!(
                            application.site,
                            checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                                origin: checked_trees::CheckedValueOrigin::StateStatement {
                                    machine_symbol,
                                    state_symbol,
                                    statement_index: application_statement,
                                    role: checked_trees::CheckedValueStatementRole::LocalInitializer,
                                },
                                ..
                            } if machine_symbol == machine.machine
                                && state_symbol == machine.state
                                && application_statement == statement_index
                        )
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let [application_index] = matching_applications.as_slice() else {
                return unsupported(
                    "lowered boundary-operator occurrence does not rejoin one exact checked application",
                );
            };
            occurrences.push(CheckedBoundaryOperatorApplicationOccurrence {
                application_index: *application_index,
                terminal_operation,
            });
        }
    }
    for plan in &checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .selected_operator_machines
    {
        let statement_index = usize::try_from(plan.return_statement_ordinal)
            .map_err(|_| "selected structural operator statement coordinate exceeds usize")?;
        let matching_occurrences = lowered
            .source_call_occurrences
            .iter()
            .filter(|occurrence| {
                occurrence.source_state == plan.state
                    && occurrence.statement_index == statement_index
                    && occurrence.call_ordinal == 0
                    && occurrence.source_target == plan.realization_machine
            })
            .collect::<Vec<_>>();
        if matching_occurrences.is_empty() {
            continue;
        }
        let [terminal_occurrence] = matching_occurrences.as_slice() else {
            return unsupported(
                "selected structural operator maps to duplicate Terminal call occurrences",
            );
        };
        let matching_applications = checked
            .facts
            .operators
            .boundary_applications
            .iter()
            .enumerate()
            .filter(|(_, application)| {
                application.requirement_symbol == plan.requirement_operator
                    && matches!(
                        application.site,
                        checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                            origin: checked_trees::CheckedValueOrigin::StateStatement {
                                machine_symbol,
                                state_symbol,
                                statement_index: application_statement,
                                role: checked_trees::CheckedValueStatementRole::Expression,
                            },
                            ..
                        } if machine_symbol == plan.machine
                            && state_symbol == plan.state
                            && application_statement == statement_index
                    )
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let [application_index] = matching_applications.as_slice() else {
            return unsupported(
                "selected structural operator does not rejoin one exact checked D29 application",
            );
        };
        occurrences.push(CheckedBoundaryOperatorApplicationOccurrence {
            application_index: *application_index,
            terminal_operation: terminal_occurrence.terminal_operation,
        });
    }
    occurrences.sort_by_key(|occurrence| occurrence.terminal_operation.get());
    let application_indices = occurrences
        .iter()
        .map(|occurrence| occurrence.application_index)
        .collect::<BTreeSet<_>>();
    let terminal_operations = occurrences
        .iter()
        .map(|occurrence| occurrence.terminal_operation)
        .collect::<BTreeSet<_>>();
    if application_indices.len() != occurrences.len()
        || terminal_operations.len() != occurrences.len()
    {
        return unsupported(
            "checked boundary-operator applications do not map one-to-one onto Terminal operations",
        );
    }
    if matched_ieee_float_fmas != lowered.selected_ieee_float_fma_occurrences.len() {
        return unsupported(
            "selected IEEE FMA Terminal occurrences do not all rejoin checked boundary applications",
        );
    }
    Ok(occurrences)
}
