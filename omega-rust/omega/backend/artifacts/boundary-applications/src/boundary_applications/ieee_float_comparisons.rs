//! Retained IEEE comparison association and independent Terminal replay.

use semantic_vocabulary::ScalarType;

/// Source-free custody for one IEEE comparison and its exact selected plan.
/// This records semantic association, not native realization authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalIeeeFloatComparisonOccurrenceProposal {
    pub terminal_machine: semantic_vocabulary::MachineId,
    pub terminal_operation: semantic_vocabulary::OperationId,
    pub provider_plan_index: usize,
    pub provider_plan_commitment: effects::provider_plan::ProviderPlanDigest,
    pub comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
    pub format: semantic_vocabulary::IeeeFloatFormat,
}

impl TerminalIeeeFloatComparisonOccurrenceProposal {
    /// The exact compiler execution identity required by this semantic occurrence.
    pub fn execution_identity(&self) -> effects::CompilerIntrinsicExecutionIdentity {
        use effects::CompilerPrimitiveFloatBinaryOperation as Execution;
        use semantic_vocabulary::IeeeFloatComparisonOperation as Comparison;
        effects::CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
            operation: match self.comparison {
                Comparison::Equal => Execution::Equal,
                Comparison::NotEqual => Execution::NotEqual,
                Comparison::Less => Execution::Less,
                Comparison::LessOrEqual => Execution::LessOrEqual,
                Comparison::Greater => Execution::Greater,
                Comparison::GreaterOrEqual => Execution::GreaterOrEqual,
            },
            format: match self.format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => {
                    numerics::literals::FloatFormat::F32
                }
                semantic_vocabulary::IeeeFloatFormat::Binary64 => {
                    numerics::literals::FloatFormat::F64
                }
            },
        }
    }

    /// Replay the complete Terminal roster and each exact realization companion.
    /// The caller separately validates the companion's artifact and demand custody.
    pub fn validate_coverage(
        module: &terminal_psi::TerminalModule,
        plans: &[effects::provider_plan::ProviderPlan],
        occurrences: &[Self],
        realizations: &crate::TerminalBoundaryApplicationRealizations,
    ) -> Result<(), &'static str> {
        Self::validate_roster(module, plans, occurrences)?;
        for occurrence in occurrences {
            let matching = realizations
                .rows()
                .iter()
                .filter(|row| row.terminal_operation() == occurrence.terminal_operation)
                .collect::<Vec<_>>();
            let [realization] = matching.as_slice() else {
                return Err(
                    "Terminal IEEE comparison requires one exact boundary realization companion",
                );
            };
            if realization.selected_plan_digest() != occurrence.provider_plan_commitment.as_bytes()
                || !matches!(realization.realization(),
                    crate::BoundaryApplicationRealization::ExactCompilerIntrinsic { execution }
                    if *execution == occurrence.execution_identity())
            {
                return Err("Terminal IEEE comparison changed its selected boundary realization");
            }
        }
        Ok(())
    }

    /// Check complete one-to-one association with the independently verified
    /// Terminal operation roster. This does not establish source custody.
    pub fn validate_roster(
        module: &terminal_psi::TerminalModule,
        plans: &[effects::provider_plan::ProviderPlan],
        occurrences: &[Self],
    ) -> Result<(), &'static str> {
        let comparisons = module
            .machines
            .iter()
            .flat_map(|machine| {
                machine.blocks.iter().flat_map(move |block| {
                    block.operations.iter().filter_map(move |operation| {
                        matches!(
                            operation.kind,
                            terminal_psi::OperationKind::IeeeFloatCompare { .. }
                        )
                        .then_some((machine, operation))
                    })
                })
            })
            .collect::<Vec<_>>();
        if comparisons.len() != occurrences.len() {
            return Err(
                "Terminal proposal must retain every IEEE comparison occurrence exactly once",
            );
        }
        let mut seen = std::collections::BTreeSet::new();
        for occurrence in occurrences {
            if !seen.insert((occurrence.terminal_machine, occurrence.terminal_operation)) {
                return Err("Terminal proposal repeats an IEEE comparison occurrence");
            }
            let plan = plans
                .get(occurrence.provider_plan_index)
                .ok_or("Terminal IEEE comparison names an absent selected plan")?;
            if plan.identity_digest() != occurrence.provider_plan_commitment {
                return Err("Terminal IEEE comparison changed its exact selected plan");
            }
            let matching = comparisons
                .iter()
                .filter(|(machine, operation)| {
                    machine.id == occurrence.terminal_machine
                        && operation.id == occurrence.terminal_operation
                })
                .collect::<Vec<_>>();
            let [(machine, operation)] = matching.as_slice() else {
                return Err("Terminal IEEE comparison does not name one exact operation");
            };
            let terminal_psi::OperationKind::IeeeFloatCompare {
                comparison,
                left,
                right,
            } = operation.kind
            else {
                return Err("Terminal IEEE comparison changed operation kind");
            };
            if comparison != occurrence.comparison
                || operation
                    .result
                    .scalar()
                    .is_none_or(|result| result.scalar_type != ScalarType::Boolean)
            {
                return Err("Terminal IEEE comparison changed operation or result type");
            }
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
                    declaration.scalar_type != ScalarType::IeeeFloat(occurrence.format)
                }) || declarations.next().is_some()
                {
                    return Err("Terminal IEEE comparison changed operand format or identity");
                }
            }
        }
        Ok(())
    }
}
