//! Selected comparison custody, separate from ordinary integer operations.
//!
//! Builtin comparisons and generated guards use the same Terminal operations
//! without selecting a provider. Operation kinds therefore cannot supply the
//! selected-occurrence census. Final admission checks completeness against the
//! artifact-bound checked boundary scope, then checks each row's
//! operation, provider and operands. The standalone row check establishes only
//! the validity of supplied rows, not completeness of source custody.

use super::TerminalIntegerComparisonOccurrenceProposal;
use semantic_vocabulary::ScalarType;

pub(super) fn validate_integer_comparison_coverage(
    module: &terminal_psi::TerminalModule,
    plans: &[effects::provider_plan::ProviderPlan],
    occurrences: &[TerminalIntegerComparisonOccurrenceProposal],
    realizations: &boundary_applications::TerminalBoundaryApplicationRealizations,
    checked_scope: &lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
) -> Result<(), &'static str> {
    validate_integer_comparison_occurrences(module, plans, occurrences)?;
    // The caller has validated this sealed scope against the same canonical
    // artifact. Classify its exact operations, not the public companion's
    // claimed role: relabeling a selected comparison as a checked-body
    // realization must not erase the completeness obligation.
    let selected_count = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerEqual { .. }
                    | terminal_psi::OperationKind::IntegerLessThan { .. }
                    | terminal_psi::OperationKind::IntegerLessOrEqual { .. }
            ) && checked_scope
                .occurrences()
                .iter()
                .any(|occurrence| occurrence.terminal_operation() == operation.id)
        })
        .count();
    if selected_count != occurrences.len() {
        return Err(
            "Terminal proposal must retain every selected integer comparison occurrence exactly once",
        );
    }
    // Distinct occurrence coordinates plus this exact forward join and the
    // sealed-scope census establish both directions: no selected use may be
    // dropped, and no builtin comparison may acquire a selected provider row.
    for occurrence in occurrences {
        let matching = realizations
            .rows()
            .iter()
            .filter(|row| row.terminal_operation() == occurrence.terminal_operation)
            .collect::<Vec<_>>();
        let [realization] = matching.as_slice() else {
            return Err(
                "Terminal integer comparison requires one exact boundary realization companion",
            );
        };
        if realization.selected_plan_digest() != occurrence.provider_plan_commitment.as_bytes()
            || !matches!(realization.realization(),
                boundary_applications::BoundaryApplicationRealization::ExactCompilerIntrinsic { execution }
                if Some(*execution) == occurrence.execution_identity())
        {
            return Err("Terminal integer comparison changed its selected boundary realization");
        }
    }
    Ok(())
}

pub(super) fn validate_integer_comparison_occurrences(
    module: &terminal_psi::TerminalModule,
    plans: &[effects::provider_plan::ProviderPlan],
    occurrences: &[TerminalIntegerComparisonOccurrenceProposal],
) -> Result<(), &'static str> {
    let comparisons = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine.blocks.iter().flat_map(move |block| {
                block.operations.iter().filter_map(move |operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::IntegerEqual { .. }
                            | terminal_psi::OperationKind::IntegerLessThan { .. }
                            | terminal_psi::OperationKind::IntegerLessOrEqual { .. }
                    )
                    .then_some((machine, operation))
                })
            })
        })
        .collect::<Vec<_>>();
    let mut seen = std::collections::BTreeSet::new();
    for occurrence in occurrences {
        if !seen.insert((occurrence.terminal_machine, occurrence.terminal_operation)) {
            return Err("Terminal proposal repeats an integer comparison occurrence");
        }
        let plan = plans
            .get(occurrence.provider_plan_index)
            .ok_or("Terminal integer comparison names an absent selected plan")?;
        if plan.identity_digest() != occurrence.provider_plan_commitment {
            return Err("Terminal integer comparison changed its exact selected plan");
        }
        let matching = comparisons
            .iter()
            .filter(|(machine, operation)| {
                machine.id == occurrence.terminal_machine
                    && operation.id == occurrence.terminal_operation
            })
            .collect::<Vec<_>>();
        let [(machine, operation)] = matching.as_slice() else {
            return Err("Terminal integer comparison does not name one exact operation");
        };
        let (left, right) = match (occurrence.comparison, &operation.kind) {
            (
                lowered_psi::LoweredSelectedIntegerComparisonOperation::Equal,
                terminal_psi::OperationKind::IntegerEqual { left, right },
            )
            | (
                lowered_psi::LoweredSelectedIntegerComparisonOperation::LessThan,
                terminal_psi::OperationKind::IntegerLessThan { left, right },
            )
            | (
                lowered_psi::LoweredSelectedIntegerComparisonOperation::LessOrEqual,
                terminal_psi::OperationKind::IntegerLessOrEqual { left, right },
            ) => (*left, *right),
            _ => return Err("Terminal integer comparison changed operation kind"),
        };
        let Some(result) = operation.result.scalar() else {
            return Err("Terminal integer comparison has no scalar result");
        };
        if result.scalar_type != ScalarType::Boolean {
            return Err("Terminal integer comparison changed its result type");
        }
        // A negated authored comparison (`!=`) completes its meaning with one
        // `BooleanNot` over the emitted comparison's result. That not must
        // exist exactly once; an unnegated authored comparison leaves the
        // result free for any authored read, including an authored `!`.
        let negations = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|candidate| {
                matches!(
                    candidate.kind,
                    terminal_psi::OperationKind::BooleanNot { operand }
                        if operand == result.id
                )
            })
            .count();
        if occurrence.negated && negations != 1 {
            return Err("Terminal integer comparison changed its authored negation");
        }
        // The recorded operand order maps authored ordinals into the emitted
        // operation's positional operands; each emitted operand must be the
        // one exact integer scalar the occurrence claims.
        let operands = [
            occurrence
                .operand_order
                .terminal_operand_position(0, 2)
                .map(|position| [left, right][position]),
            occurrence
                .operand_order
                .terminal_operand_position(1, 2)
                .map(|position| [left, right][position]),
        ];
        let [Some(_), Some(_)] = operands else {
            return Err("Terminal integer comparison operand order cannot address its operands");
        };
        for operand in [left, right] {
            let mut declarations = machine
                .parameters
                .iter()
                .copied()
                .chain(
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.parameters)
                        .copied(),
                )
                .chain(
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .filter_map(|operation| operation.result.scalar()),
                )
                .filter(|declaration| declaration.id == operand);
            if declarations.next().is_none_or(|declaration| {
                declaration.scalar_type != ScalarType::Integer(occurrence.integer_type)
            }) || declarations.next().is_some()
            {
                return Err("Terminal integer comparison changed operand type or identity");
            }
        }
    }
    Ok(())
}
