//! Retained selected integer comparison meaning and Terminal row replay.

use semantic_vocabulary::ScalarType;

/// Source-free custody for one emitted integer comparison and its exact
/// selected plan. This records semantic association, not native realization
/// authority. The emitted operation is only `IntegerEqual`, `IntegerLessThan`
/// or `IntegerLessOrEqual`; `operand_order` and `negated` recover the authored
/// `==`/`!=`/`<`/`<=`/`>`/`>=` meaning the selected provider committed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalIntegerComparisonOccurrenceProposal {
    pub terminal_machine: semantic_vocabulary::MachineId,
    pub terminal_operation: semantic_vocabulary::OperationId,
    pub provider_plan_index: usize,
    pub provider_plan_commitment: effects::provider_plan::ProviderPlanDigest,
    pub comparison: lowered_psi::LoweredSelectedIntegerComparisonOperation,
    pub operand_order: lowered_psi::LoweredSelectedIntegerComparisonOperandOrder,
    pub negated: bool,
    pub integer_type: semantic_vocabulary::IntegerType,
}

impl TerminalIntegerComparisonOccurrenceProposal {
    /// The authored comparison meaning the selected provider committed to,
    /// recovered from the recorded emission triple. Combinations outside the
    /// admitted emission roster have no authored meaning and fail closed.
    pub fn authored_operation(
        &self,
    ) -> Option<effects::CompilerPrimitiveIntegerComparisonOperation> {
        use effects::CompilerPrimitiveIntegerComparisonOperation as Operation;
        use lowered_psi::LoweredSelectedIntegerComparisonOperandOrder as Order;
        use lowered_psi::LoweredSelectedIntegerComparisonOperation as Emitted;
        match (self.comparison, self.operand_order, self.negated) {
            (Emitted::Equal, Order::Authored, false) => Some(Operation::Equal),
            (Emitted::Equal, Order::Authored, true) => Some(Operation::NotEqual),
            (Emitted::LessThan, Order::Authored, false) => Some(Operation::Less),
            (Emitted::LessOrEqual, Order::Authored, false) => Some(Operation::LessOrEqual),
            (Emitted::LessThan, Order::Swapped, false) => Some(Operation::Greater),
            (Emitted::LessOrEqual, Order::Swapped, false) => Some(Operation::GreaterOrEqual),
            _ => None,
        }
    }

    /// The exact compiler execution identity required by this semantic
    /// occurrence. Address-carrier and non-fixed-width integer operands have
    /// no closed numeric identity and fail closed.
    pub fn execution_identity(&self) -> Option<effects::CompilerIntrinsicExecutionIdentity> {
        let operation = self.authored_operation()?;
        let integer_type = match (
            self.integer_type.carrier(),
            self.integer_type.sign(),
            self.integer_type.bits(),
        ) {
            (
                semantic_vocabulary::IntegerCarrier::Fixed,
                semantic_vocabulary::IntegerSign::Signed,
                8,
            ) => effects::CompilerNumericType::I8,
            (
                semantic_vocabulary::IntegerCarrier::Fixed,
                semantic_vocabulary::IntegerSign::Signed,
                16,
            ) => effects::CompilerNumericType::I16,
            (
                semantic_vocabulary::IntegerCarrier::Fixed,
                semantic_vocabulary::IntegerSign::Signed,
                32,
            ) => effects::CompilerNumericType::I32,
            (
                semantic_vocabulary::IntegerCarrier::Fixed,
                semantic_vocabulary::IntegerSign::Signed,
                64,
            ) => effects::CompilerNumericType::I64,
            (
                semantic_vocabulary::IntegerCarrier::Fixed,
                semantic_vocabulary::IntegerSign::Unsigned,
                8,
            ) => effects::CompilerNumericType::U8,
            (
                semantic_vocabulary::IntegerCarrier::Fixed,
                semantic_vocabulary::IntegerSign::Unsigned,
                16,
            ) => effects::CompilerNumericType::U16,
            (
                semantic_vocabulary::IntegerCarrier::Fixed,
                semantic_vocabulary::IntegerSign::Unsigned,
                32,
            ) => effects::CompilerNumericType::U32,
            (
                semantic_vocabulary::IntegerCarrier::Fixed,
                semantic_vocabulary::IntegerSign::Unsigned,
                64,
            ) => effects::CompilerNumericType::U64,
            _ => return None,
        };
        Some(
            effects::CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                operation,
                integer_type,
            },
        )
    }

    /// Check each supplied selected occurrence against its exact Terminal
    /// operation and plan. Builtin comparisons need no selected occurrence.
    /// Final proposal admission separately checks completeness against the
    /// artifact-bound checked boundary scope.
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
                return Err(
                    "Terminal integer comparison operand order cannot address its operands",
                );
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
}
