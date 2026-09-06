//! Declared Requires and crash ceilings describe invocation-entry operands.
//! Body reads continue to use the independent current-storage namespace.

use super::*;

pub(crate) fn lower_machine_entry_boolean_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    expression: ExpressionHandle,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    let mut predicate = lower_machine_parameter_boolean_expression(
        program,
        operators,
        machine,
        expression,
        exact_integer_casts,
    )?;
    let entry = program.machine_states(machine).first()?;
    EntryOperands {
        program,
        parameters: program.state_parameters(entry),
    }
    .boolean(&mut predicate)?;
    Some(predicate)
}

struct EntryOperands<'program> {
    program: &'program TypedTrees,
    parameters: &'program [StateParameter],
}

impl EntryOperands<'_> {
    fn position(&self, symbol: symbols::SymbolHandle, primitive: PrimitiveType) -> Option<usize> {
        let mut matches = self
            .parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| symbol.is_valid() && parameter.symbol == symbol);
        let (position, parameter) = matches.next()?;
        if matches.next().is_some()
            || crate::values::mutable_scalar_parameter_type(self.program, parameter)
                != Some(primitive)
        {
            return None;
        }
        // Structural entry operands keep their separate authored positions.
        // Scalar Parameter uses the dense primitive namespace at this boundary.
        Some(
            self.parameters[..position]
                .iter()
                .filter(|parameter| {
                    self.program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
                })
                .count(),
        )
    }

    fn scalar(&self, expression: &mut CheckedScalarExpression) -> Option<()> {
        match expression {
            CheckedScalarExpression::StorageRead {
                symbol,
                primitive_type,
            } => {
                *expression = CheckedScalarExpression::Parameter {
                    position: self.position(*symbol, *primitive_type)?,
                    primitive_type: *primitive_type,
                };
            }
            CheckedScalarExpression::Local { .. } => return None,
            CheckedScalarExpression::IntegerBinary { left, right, .. } => {
                self.scalar(left)?;
                self.scalar(right)?;
            }
            CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
            | CheckedScalarExpression::IntegerWiden { operand, .. }
            | CheckedScalarExpression::IntegerExactCast { operand, .. } => self.scalar(operand)?,
            CheckedScalarExpression::Boolean(expression) => self.boolean(expression)?,
            CheckedScalarExpression::Parameter { .. }
            | CheckedScalarExpression::StructuralParameterField { .. }
            | CheckedScalarExpression::IntegerLiteral { .. }
            | CheckedScalarExpression::IeeeFloatLiteral { .. } => {}
        }
        Some(())
    }

    fn boolean(&self, expression: &mut CheckedBooleanExpression) -> Option<()> {
        match expression {
            CheckedBooleanExpression::StorageRead { symbol } => {
                *expression = CheckedBooleanExpression::Parameter {
                    position: self.position(*symbol, PrimitiveType::Bool)?,
                };
            }
            CheckedBooleanExpression::Local { .. } => return None,
            CheckedBooleanExpression::Not(operand) => self.boolean(operand)?,
            CheckedBooleanExpression::And { left, right }
            | CheckedBooleanExpression::Or { left, right }
            | CheckedBooleanExpression::Equal { left, right } => {
                self.boolean(left)?;
                self.boolean(right)?;
            }
            CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
                self.scalar(left)?;
                self.scalar(right)?;
            }
            CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::StructuralParameterField { .. }
            | CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::ByteSequenceEqual { .. }
            | CheckedBooleanExpression::PayloadlessSumEqual { .. }
            | CheckedBooleanExpression::StructuralCaseMembership { .. } => {}
        }
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_snapshot_rejects_foreign_stale_duplicate_and_wrong_typed_storage() {
        let source = "machine value(first: bool, mut input: bool) -> bool { let mut other: bool = false; input }";
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let state = &program.machine_states(&program.machines()[0])[0];
        let parameters = program.state_parameters(state);
        let entry = EntryOperands {
            program: &program,
            parameters,
        };
        let symbol = parameters[1].symbol;
        let mut valid = CheckedBooleanExpression::StorageRead { symbol };
        assert_eq!(entry.boolean(&mut valid), Some(()));
        assert_eq!(valid, CheckedBooleanExpression::Parameter { position: 1 });
        let local = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| {
                if let StatementNode::LocalData(local) = statement {
                    Some(local.symbol)
                } else {
                    None
                }
            })
            .unwrap();
        for wrong in [
            symbols::SymbolHandle::invalid(),
            symbols::SymbolHandle::from_parts(symbol.arena_index(), symbol.generation() + 1),
            parameters[0].symbol,
            local,
        ] {
            assert_eq!(
                entry.boolean(&mut CheckedBooleanExpression::StorageRead { symbol: wrong }),
                None
            );
        }
        assert_eq!(
            entry.scalar(&mut CheckedScalarExpression::StorageRead {
                symbol,
                primitive_type: PrimitiveType::I32
            }),
            None
        );
        assert_eq!(
            entry.boolean(&mut CheckedBooleanExpression::Local { position: 1 }),
            None
        );
        let duplicate_parameters = [parameters[1].clone(), parameters[1].clone()];
        let duplicate = EntryOperands {
            program: &program,
            parameters: &duplicate_parameters,
        };
        assert_eq!(
            duplicate.boolean(&mut CheckedBooleanExpression::StorageRead { symbol }),
            None
        );
    }
}
