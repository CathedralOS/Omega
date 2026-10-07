//! Reconstruct boundary-operator applications and bind their custody to the published artifact.

mod float_comparisons;
mod integer_comparisons;
mod result_policies;

use checked_trees::CheckedTrees;
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
    result_policies::replay(checked, lowered)?;
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
    float_comparisons::replay(checked, lowered, &mut occurrences)?;
    integer_comparisons::replay(checked, lowered, &mut occurrences)?;
    // Boundary calls already carry ordinary source-call custody. Rejoin that
    // custody to the operator application instead of inferring coverage from
    // a selected provider or treating a missing row as an empty demand scope.
    for operation in lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
            continue;
        };
        let declaration = lowered
            .semantic_module
            .boundary_machines
            .iter()
            .find(|declaration| declaration.id == boundary)
            .ok_or("boundary application has no Terminal declaration")?;
        if declaration.kind != terminal_psi::BoundaryRequirementKind::Operator {
            continue;
        }
        let mut sources = lowered
            .source_call_occurrences
            .iter()
            .filter(|source| source.terminal_site.operation() == Some(operation.id));
        let (Some(source), None) = (sources.next(), sources.next()) else {
            return unsupported("named boundary application has no unique source-call occurrence");
        };
        let Some(checked_trees::NominalMachineUseSite::Expression(expression)) = source.source_site
        else {
            return unsupported("named boundary application has no exact expression occurrence");
        };
        let operator = checked
            .typed
            .operators()
            .iter()
            .find(|operator| operator.symbol == source.source_target && operator.is_boundary)
            .ok_or("named boundary application lost its authored requirement")?;
        if !checked.typed.operator_type_parameters(operator).is_empty()
            || typed_trees::operator::boundary_operator_requirement_identity(
                &checked.typed,
                operator,
            ) != declaration.identity
        {
            return unsupported(
                "named boundary application changed its closed requirement identity",
            );
        }
        let control = &checked.facts.flow.control;
        let flow_calls = control
            .states
            .iter()
            .filter(|(_, state)| state.state_symbol == source.source_state)
            .flat_map(|(_, state)| control.calls.span_or_empty(state.calls))
            .filter(|call| {
                call.statement_index == source.statement_index
                    && call.call_ordinal == source.call_ordinal
                    && call.authored_expression == expression
                    && call.target_symbol == source.source_target
            })
            .count();
        if flow_calls != 1 {
            return unsupported("named boundary application changed its checked call coordinate");
        }
        let mut uses = checked
            .facts
            .operators
            .named_uses
            .iter()
            .filter(|(_, use_fact)| {
                use_fact.expression == expression
                    && use_fact.selected_operator_symbol == source.source_target
                    && named_call_origin_matches(checked, use_fact.origin, source)
            });
        let (Some((_, use_fact)), None) = (uses.next(), uses.next()) else {
            return unsupported("named boundary application has no unique checked operator use");
        };
        let site = checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
            expression,
            origin: use_fact.origin,
        };
        let mut applications = checked
            .facts
            .operators
            .boundary_applications
            .iter()
            .enumerate()
            .filter(|(_, application)| {
                application.site == site && application.requirement_symbol == source.source_target
            });
        let (Some((application_index, application)), None) =
            (applications.next(), applications.next())
        else {
            return unsupported(
                "named boundary call does not rejoin one exact checked application",
            );
        };
        if !application.arguments.is_empty() {
            return unsupported(
                "nongeneric boundary operator lost its canonical empty application",
            );
        }
        occurrences.push(CheckedBoundaryOperatorApplicationOccurrence {
            application_index,
            terminal_operation: operation.id,
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
    Ok(occurrences)
}

fn named_call_origin_matches(
    checked: &CheckedTrees,
    mut origin: checked_trees::CheckedValueOrigin,
    source: &lowered_psi::LoweredSourceCallOccurrence,
) -> bool {
    let mut parents = Vec::new();
    while let checked_trees::CheckedValueOrigin::NestedExpression { parent } = origin {
        if parents.contains(&parent) {
            return false;
        }
        parents.push(parent);
        let mut origins = checked
            .facts
            .values
            .expression_values(parent)
            .map(|(_, value)| value.origin);
        let Some(parent_origin) = origins.next() else {
            return false;
        };
        if origins.any(|candidate| candidate != parent_origin) {
            return false;
        }
        origin = parent_origin;
    }
    matches!(origin, checked_trees::CheckedValueOrigin::StateStatement {
        state_symbol, statement_index, ..
    } if state_symbol == source.source_state && statement_index == source.statement_index)
}
