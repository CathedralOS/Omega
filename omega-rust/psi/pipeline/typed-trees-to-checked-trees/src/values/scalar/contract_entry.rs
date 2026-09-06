//! Declared Requires and crash ceilings describe invocation-entry operands.
//! Body reads continue to use the independent current-storage namespace.

use super::*;

/// Requires-only fallback for scalar Boolean formals in an exact entry namespace.
/// Unlike the shared structural crash reader, this boundary cannot recover a source name
/// from its spelling or introduce a result/body-local namespace.
pub(crate) fn lower_machine_entry_scalar_contract_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    expression: ExpressionHandle,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    if !machine.symbol.is_valid()
        || program.symbols.get(machine.symbol).kind != symbols::SymbolKind::Machine
        || !program.machine_type_parameters(machine).is_empty()
        || !machine.lifetime_parameters.is_empty()
        || !machine.conformance_bounds.is_empty()
        || program
            .machines()
            .iter()
            .filter(|candidate| candidate.symbol == machine.symbol)
            .count()
            != 1
    {
        return None;
    }
    match &machine.attached_data {
        None if machine.attached_data_symbol.is_valid() => return None,
        None => {}
        Some(name) => {
            let mut owners = program
                .data_definitions()
                .iter()
                .filter(|data| data.symbol == machine.attached_data_symbol);
            let owner = owners.next()?;
            if owners.next().is_some()
                || program.symbols.get(owner.symbol).kind != symbols::SymbolKind::Data
                || owner.name.as_str() != name.as_str()
                || !program.data_type_parameters(owner).is_empty()
                || !owner.lifetime_parameters.is_empty()
            {
                return None;
            }
        }
    }
    let entry = program.machine_states(machine).first()?;
    let entry_symbol = program.symbols.get(entry.symbol);
    if entry_symbol.kind != symbols::SymbolKind::State || entry_symbol.parent != machine.symbol {
        return None;
    }
    let parameters = program.state_parameters(entry);
    // Unread structural operands and receivers retain their authored slots;
    // they do not make an exact owned Boolean formal a structural predicate.
    for (position, parameter) in parameters.iter().enumerate() {
        let symbol = program.symbols.get(parameter.symbol);
        if symbol.kind != symbols::SymbolKind::Parameter
            || symbol.parent != entry.symbol
            || parameters[..position]
                .iter()
                .any(|prior| prior.symbol == parameter.symbol || prior.name == parameter.name)
        {
            return None;
        }
    }
    // Check the handle graph before invoking recursive semantic queries.
    let mut pending = vec![(expression, false)];
    let mut active = Vec::new();
    let mut complete = Vec::new();
    let mut operations = Vec::new();
    while let Some((expression, leaving)) = pending.pop() {
        if leaving {
            active.pop();
            complete.push(expression);
            continue;
        }
        if !program.expression_table.expression_is_valid(expression) || active.contains(&expression)
        {
            return None;
        }
        if complete.contains(&expression) {
            continue;
        }
        active.push(expression);
        pending.push((expression, true));
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                if !path.symbol.is_valid()
                    || path.head_symbol != path.symbol
                    || members.len() != 1
                    || parameters
                        .iter()
                        .filter(|parameter| parameter.symbol == path.symbol)
                        .count()
                        != 1
                    || !parameters.iter().any(|parameter| {
                        parameter.symbol == path.symbol
                            && !parameter.is_self
                            && !parameter.is_const
                            && parameter.name.as_str() == members[0].as_str()
                            && program.primitive_type_reference(parameter.type_reference)
                                == Some(PrimitiveType::Bool)
                    })
                {
                    return None;
                }
            }
            // Numeric clauses keep their existing landing, totality and
            // operator-custody reader. This fallback only adds Boolean facts.
            ExpressionNode::Boolean(_) => {}
            ExpressionNode::Binary(binary) => {
                operations.push(expression);
                pending.extend([(binary.right, false), (binary.left, false)]);
            }
            ExpressionNode::Unary(unary) => {
                operations.push(expression);
                pending.push((unary.operand, false));
            }
            _ => return None,
        }
    }
    if operations
        .iter()
        .any(|expression| !operator_is_builtin(operators, *expression))
        || !validation::has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(entry),
            expression,
        )
    {
        return None;
    }
    lower_machine_entry_boolean_expression(
        program,
        operators,
        machine,
        expression,
        exact_integer_casts,
    )
}

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

    fn typed(source: &str) -> TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
    }

    fn requirement(program: &TypedTrees) -> ExpressionHandle {
        program
            .machine_contracts(&program.machines()[0])
            .iter()
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .find_map(|fact| {
                if let typed_trees::domain::ProofFact::Expression(expression) = fact {
                    Some(*expression)
                } else {
                    None
                }
            })
            .expect("fixture has one entry requirement")
    }

    fn scalar_requirement(
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Option<CheckedBooleanExpression> {
        lower_machine_entry_scalar_contract_expression(
            program,
            &CheckedOperatorFacts::default(),
            &program.machines()[0],
            expression,
            &[],
        )
    }

    #[test]
    fn scalar_entry_requirement_uses_exact_formals_and_preserves_mutable_snapshots() {
        for requirement_text in ["flag", "!flag", "flag && !other", "flag == other"] {
            let program = typed(&format!(
                "machine value(mut flag: bool, other: bool) -> bool requires {requirement_text} {{ flag }}"
            ));
            assert!(
                scalar_requirement(&program, requirement(&program)).is_some(),
                "{requirement_text}"
            );
        }
    }

    #[test]
    fn scalar_entry_requirement_accepts_attached_and_mixed_exact_namespaces() {
        for requirement_text in ["right", "!right", "left && right", "left == right"] {
            let ordinary = typed(&format!(
                "machine value(left: bool, mut right: bool) -> bool requires {requirement_text} {{ true }}"
            ));
            let expected = scalar_requirement(&ordinary, requirement(&ordinary));
            assert!(expected.is_some());
            for signature in [
                "machine Box::value(left: bool, mut right: bool)",
                "machine value(left: bool, record: Box, mut right: bool)",
                "machine Box::value(&self, left: bool, record: Box, mut right: bool)",
            ] {
                let program = typed(&format!(
                    "data Box {{ flag: bool; }} {signature} -> bool requires {requirement_text} {{ true }}"
                ));
                assert_eq!(
                    scalar_requirement(&program, requirement(&program)),
                    expected,
                    "{signature}: {requirement_text}"
                );
            }
        }
    }

    #[test]
    fn scalar_entry_requirement_rejects_symbol_and_spelling_disagreement() {
        let mut program =
            typed("machine value(left: bool, right: bool) -> bool requires left { true }");
        let root = requirement(&program);
        let parameters =
            program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
        let left = parameters[0].symbol;
        let right = parameters[1].symbol;
        let mut pending = vec![root];
        while let Some(expression) = pending.pop() {
            match program.expression_table.expression_mut(expression) {
                ExpressionNode::Name(path) if path.symbol == left => {
                    path.symbol = right;
                    path.head_symbol = right;
                    assert!(scalar_requirement(&program, root).is_none());
                    return;
                }
                ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
                ExpressionNode::Unary(unary) => pending.push(unary.operand),
                _ => {}
            }
        }
        panic!("requirement has the left formal occurrence");
    }

    #[test]
    fn scalar_entry_requirement_rejects_ambiguous_retained_formal_names() {
        let mut program =
            typed("machine value(left: bool, right: bool) -> bool requires right { true }");
        let root = requirement(&program);
        let parameters = program.machine_states(&program.machines()[0])[0].parameters;
        let names = program
            .tables
            .state_parameters
            .span_mut_or_empty(parameters);
        names[0].name = names[1].name.clone();
        assert!(scalar_requirement(&program, root).is_none());
    }

    #[test]
    fn scalar_entry_requirement_requires_one_live_consistent_attachment_owner() {
        let program = typed(
            "data Box {} data Other {} machine Box::value(flag: bool) -> bool requires flag { flag }",
        );
        let root = requirement(&program);
        assert!(scalar_requirement(&program, root).is_some());
        let owner = program.machines()[0].attached_data_symbol;
        for wrong in [
            symbols::SymbolHandle::invalid(),
            symbols::SymbolHandle::from_parts(owner.arena_index(), owner.generation() + 1),
            program.data_definitions()[1].symbol,
            program.machines()[0].symbol,
        ] {
            let mut invalid = program.clone();
            invalid.machines_mut()[0].attached_data_symbol = wrong;
            assert!(scalar_requirement(&invalid, root).is_none());
        }
        let mut detached = program.clone();
        detached.machines_mut()[0].attached_data = None;
        assert!(scalar_requirement(&detached, root).is_none());
        let mut duplicate = program.clone();
        let definitions = duplicate.roots.data_definitions;
        duplicate
            .tables
            .data_definitions
            .span_mut_or_empty(definitions)[1] = program.data_definitions()[0].clone();
        assert!(scalar_requirement(&duplicate, root).is_none());
        let mut stale = program.clone();
        let definitions = stale.roots.data_definitions;
        let stale_owner =
            symbols::SymbolHandle::from_parts(owner.arena_index(), owner.generation() + 1);
        stale.tables.data_definitions.span_mut_or_empty(definitions)[0].symbol = stale_owner;
        stale.machines_mut()[0].attached_data_symbol = stale_owner;
        assert!(scalar_requirement(&stale, root).is_none());
    }

    #[test]
    fn scalar_entry_requirement_rejects_unresolved_or_non_owned_boolean_leaves() {
        for source in [
            "data Box<T> {} machine Box::value(flag: bool) -> bool requires flag { flag }",
            "machine value<T>(flag: bool) -> bool requires flag { flag }",
            "machine value(flag: &bool) -> bool requires flag { true }",
            "data Box { flag: bool; } machine Box::value(&self) -> bool requires self.flag { true }",
        ] {
            let program = typed(source);
            assert!(
                scalar_requirement(&program, requirement(&program)).is_none(),
                "{source}"
            );
        }
    }

    #[test]
    fn scalar_entry_requirement_rejects_corrupted_formal_namespace_rows() {
        let program = typed(
            "machine value(left: bool, right: bool) -> bool requires left { true } machine foreign(right: bool) -> bool { right }",
        );
        let root = requirement(&program);
        let entry = &program.machine_states(&program.machines()[0])[0];
        let parameters = program.state_parameters(entry);
        let right = parameters[1].symbol;
        for wrong in [
            symbols::SymbolHandle::invalid(),
            symbols::SymbolHandle::from_parts(right.arena_index(), right.generation() + 1),
            parameters[0].symbol,
            program.state_parameters(&program.machine_states(&program.machines()[1])[0])[0].symbol,
        ] {
            let mut invalid = program.clone();
            invalid
                .tables
                .state_parameters
                .span_mut_or_empty(entry.parameters)[1]
                .symbol = wrong;
            assert!(scalar_requirement(&invalid, root).is_none());
        }
    }

    #[test]
    fn scalar_entry_requirement_does_not_recover_missing_or_foreign_name_symbols() {
        let program = typed(
            "machine value(flag: bool) -> bool requires flag { let local: bool = false; flag } machine foreign(flag: bool) -> bool { flag }",
        );
        let root = requirement(&program);
        let symbol =
            program.state_parameters(&program.machine_states(&program.machines()[0])[0])[0].symbol;
        let mut pending = vec![root];
        let name = loop {
            let expression = pending
                .pop()
                .expect("requirement has its exact flag occurrence");
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(path) if path.symbol == symbol => break expression,
                ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
                ExpressionNode::Unary(unary) => pending.push(unary.operand),
                _ => {}
            }
        };
        let foreign =
            program.state_parameters(&program.machine_states(&program.machines()[1])[0])[0].symbol;
        let local = program
            .statement_table
            .statements(program.machine_states(&program.machines()[0])[0].statement_nodes)
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
            foreign,
            local,
        ] {
            let mut invalid = program.clone();
            let ExpressionNode::Name(path) = invalid.expression_table.expression_mut(name) else {
                unreachable!()
            };
            path.symbol = wrong;
            path.head_symbol = wrong;
            assert!(scalar_requirement(&invalid, root).is_none());
        }
        let mut invalid = program.clone();
        let ExpressionNode::Name(path) = invalid.expression_table.expression_mut(name) else {
            unreachable!()
        };
        path.head_symbol = foreign;
        assert!(scalar_requirement(&invalid, root).is_none());
    }

    #[test]
    fn scalar_entry_requirement_rejects_cycles_structural_parameters_and_authored_operators() {
        let program = typed("machine value(flag: bool) -> bool requires flag && true { flag }");
        let root = requirement(&program);
        let mut cyclic = program.clone();
        let ExpressionNode::Binary(binary) = cyclic.expression_table.expression_mut(root) else {
            panic!("normalized Boolean fact")
        };
        binary.left = root;
        assert!(scalar_requirement(&cyclic, root).is_none());
        assert!(
            scalar_requirement(
                &program,
                ExpressionHandle::from_parts(root.arena_index(), root.generation() + 1)
            )
            .is_none()
        );
        let structural = typed(
            "data Box { flag: bool; } machine value(input: Box) -> bool requires input.flag { true }",
        );
        assert!(scalar_requirement(&structural, requirement(&structural)).is_none());
        let authored = typed(
            "boundary operator == bool::custom(left: bool, right: bool) -> bool; machine value(flag: bool) -> bool requires flag == true { flag }",
        );
        assert!(scalar_requirement(&authored, requirement(&authored)).is_none());
    }

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
