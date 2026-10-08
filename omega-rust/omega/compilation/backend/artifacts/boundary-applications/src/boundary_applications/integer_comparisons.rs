//! Selected integer comparison meaning, independent of source lowering records.

use effects::{CompilerNumericType, CompilerPrimitiveIntegerComparisonOperation};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};

/// The selected authored operation and its exact target plan. Terminal already
/// owns the emitted operation; repeating its source emission triple here would
/// create another representation to keep synchronized with lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalIntegerComparisonOccurrenceProposal {
    pub terminal_machine: semantic_vocabulary::MachineId,
    pub terminal_operation: semantic_vocabulary::OperationId,
    pub provider_plan_index: usize,
    pub provider_plan_commitment: effects::provider_plan::ProviderPlanDigest,
    pub comparison: CompilerPrimitiveIntegerComparisonOperation,
    pub integer_type: CompilerNumericType,
}

impl TerminalIntegerComparisonOccurrenceProposal {
    pub fn execution_identity(&self) -> Option<effects::CompilerIntrinsicExecutionIdentity> {
        self.scalar_type()?;
        Some(
            effects::CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                operation: self.comparison,
                integer_type: self.integer_type,
            },
        )
    }

    fn scalar_type(&self) -> Option<ScalarType> {
        use CompilerNumericType::*;
        let (sign, bits) = match self.integer_type {
            I8 => (IntegerSign::Signed, 8),
            I16 => (IntegerSign::Signed, 16),
            I32 => (IntegerSign::Signed, 32),
            I64 => (IntegerSign::Signed, 64),
            U8 => (IntegerSign::Unsigned, 8),
            U16 => (IntegerSign::Unsigned, 16),
            U32 => (IntegerSign::Unsigned, 32),
            U64 => (IntegerSign::Unsigned, 64),
            F32 | F64 => return None,
        };
        IntegerType::new(sign, bits).ok().map(ScalarType::Integer)
    }

    /// Check supplied selections against Terminal operations. Ordinary builtin
    /// integer comparisons have no boundary demand and need no selection row.
    pub fn validate_roster(
        module: &terminal_psi::TerminalModule,
        plans: &[effects::provider_plan::ProviderPlan],
        occurrences: &[Self],
    ) -> Result<(), &'static str> {
        let mut seen = std::collections::BTreeSet::new();
        for occurrence in occurrences {
            if !seen.insert((occurrence.terminal_machine, occurrence.terminal_operation)) {
                return Err("Terminal proposal repeats an integer comparison occurrence");
            }
            let plan = plans
                .get(occurrence.provider_plan_index)
                .ok_or("Terminal integer comparison names an absent selected plan")?;
            if plan.identity_digest() != occurrence.provider_plan_commitment {
                return Err("Terminal integer comparison changed its selected plan");
            }
            let mut matching = module
                .machines
                .iter()
                .flat_map(|machine| {
                    machine.blocks.iter().flat_map(move |block| {
                        block
                            .operations
                            .iter()
                            .map(move |operation| (machine, operation))
                    })
                })
                .filter(|(machine, operation)| {
                    machine.id == occurrence.terminal_machine
                        && operation.id == occurrence.terminal_operation
                });
            let Some((machine, operation)) = matching.next() else {
                return Err("Terminal integer comparison does not name one exact operation");
            };
            if matching.next().is_some() {
                return Err("Terminal integer comparison does not name one exact operation");
            }
            use CompilerPrimitiveIntegerComparisonOperation as Comparison;
            use terminal_psi::OperationKind;
            let (left, right) = match (occurrence.comparison, &operation.kind) {
                (
                    Comparison::Equal | Comparison::NotEqual,
                    OperationKind::IntegerEqual { left, right },
                )
                | (
                    Comparison::Less | Comparison::Greater,
                    OperationKind::IntegerLessThan { left, right },
                )
                | (
                    Comparison::LessOrEqual | Comparison::GreaterOrEqual,
                    OperationKind::IntegerLessOrEqual { left, right },
                ) => (*left, *right),
                _ => return Err("Terminal integer comparison changed operation kind"),
            };
            let Some(result) = operation.result.scalar() else {
                return Err("Terminal integer comparison has no scalar result");
            };
            if result.scalar_type != ScalarType::Boolean {
                return Err("Terminal integer comparison changed its result type");
            }
            if occurrence.comparison == Comparison::NotEqual {
                let negations = machine.operations()
                    .filter(|candidate| matches!(candidate.kind, OperationKind::BooleanNot { operand } if operand == result.id))
                    .count();
                if negations != 1 {
                    return Err("Terminal integer comparison changed its authored negation");
                }
            }
            let scalar_type = occurrence
                .scalar_type()
                .ok_or("Terminal integer comparison has a noninteger selected type")?;
            for operand in [left, right] {
                let mut declarations = machine
                    .parameters
                    .iter()
                    .copied()
                    .chain(machine.block_parameters().copied())
                    .chain(
                        machine
                            .operations()
                            .filter_map(|operation| operation.result.scalar()),
                    )
                    .filter(|declaration| declaration.id == operand);
                if declarations
                    .next()
                    .is_none_or(|declaration| declaration.scalar_type != scalar_type)
                    || declarations.next().is_some()
                {
                    return Err("Terminal integer comparison changed operand type or identity");
                }
            }
        }
        Ok(())
    }
}
