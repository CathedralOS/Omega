//! Form a dispatch in authored order over one completed subject. Selected
//! predicates remain on their branches; the enclosing engine receives only
//! operation definitions and an exact guarded value, never branch hypotheses.

use super::super::conditional_values::{Comparison, ConditionalValueArm};
use super::{ApplicationDecisions, Engine, Polynomial, formed_value};
use crate::frozen_program::expression_table;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern, TableMatchExpression,
    UnaryOperator,
};
use typed_trees::types::PrimitiveType;

#[derive(Clone)]
struct Path<'program> {
    engine: Engine<'program>,
    conditions: Vec<Comparison>,
}

impl Path<'_> {
    fn assume(mut self, comparison: Comparison) -> Option<Self> {
        // Freeze completed operands before importing the selection itself.
        // Substituting afterward could erase x == 4 into 4 == 4 and export an
        // unguarded arm value, or lose a callee's independently proved result.
        let comparison = (
            comparison.0,
            self.engine.substituted(&comparison.1),
            self.engine.substituted(&comparison.2),
        );
        if self
            .engine
            .integer_comparisons_unsatisfiable(vec![comparison.clone()])
        {
            return None;
        }
        self.conditions.push(comparison);
        Some(self)
    }
}

#[derive(Clone)]
enum Subject {
    Integer(Polynomial),
    Boolean(bool),
}

pub(super) fn form(
    engine: &mut Engine<'_>,
    expression: ExpressionHandle,
    dispatch: &TableMatchExpression,
    depth: usize,
    decisions: &mut ApplicationDecisions,
) -> Option<Polynomial> {
    engine.charge_conditional_work()?;
    let program = engine.program;
    let path = Path {
        engine: engine.clone(),
        conditions: Vec::new(),
    };
    let boolean = matches!(
        expression_table(program).expression(dispatch.subject),
        ExpressionNode::Boolean(_)
    ) || engine
        .source_result_type(dispatch.subject)
        .and_then(|reference| program.primitive_type_reference(reference))
        == Some(PrimitiveType::Bool);
    if crate::proof_contracts::contract_entailment::struct_trace() {
        eprintln!(
            "STRUCT declaration-dispatch begin expression={expression:?} boolean={boolean} subject={:?} type={:?}",
            expression_table(program).expression(dispatch.subject),
            engine.source_result_type(dispatch.subject)
        );
    }
    let mut remaining = if boolean {
        condition_paths(path, dispatch.subject, depth + 1, decisions)?
            .into_iter()
            .map(|(value, path)| (path, Subject::Boolean(value)))
            .collect::<Vec<_>>()
    } else {
        let mut path = path;
        let value = formed_value(
            &mut path.engine,
            program,
            dispatch.subject,
            depth + 1,
            decisions,
        )?;
        vec![(path, Subject::Integer(value))]
    };
    let mut values = Vec::new();
    for arm in expression_table(program).match_arms(dispatch.arms) {
        let mut selected = Vec::new();
        let mut failed = Vec::new();
        for (path, subject) in remaining {
            path.engine.charge_conditional_work()?;
            match arm.pattern {
                MatchPattern::Wildcard => selected.push(path),
                MatchPattern::Destructure(pattern) => {
                    for (matched, path) in
                        condition_paths(path, pattern.condition, depth + 1, decisions)?
                    {
                        if matched {
                            selected.push(path);
                        } else {
                            failed.push((path, subject.clone()));
                        }
                    }
                }
                MatchPattern::Value(pattern) => {
                    let operands = [dispatch.subject, pattern]
                        .map(|operand| path.engine.source_result_type(operand));
                    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        path.engine.source_owner,
                        expression,
                        language_core::OperatorSpelling::Equal,
                        &operands,
                    ) {
                        return None;
                    }
                    match subject {
                        Subject::Boolean(value) => {
                            for (pattern, path) in
                                condition_paths(path, pattern, depth + 1, decisions)?
                            {
                                if value == pattern {
                                    selected.push(path);
                                } else {
                                    failed.push((path, Subject::Boolean(value)));
                                }
                            }
                        }
                        Subject::Integer(value) => {
                            let mut path = path;
                            let pattern = formed_value(
                                &mut path.engine,
                                program,
                                pattern,
                                depth + 1,
                                decisions,
                            )?;
                            if let Some(matched) = path.clone().assume((
                                BinaryOperator::Equal,
                                value.clone(),
                                pattern.clone(),
                            )) {
                                selected.push(matched);
                            }
                            if let Some(path) =
                                path.assume((BinaryOperator::NotEqual, value.clone(), pattern))
                            {
                                failed.push((path, Subject::Integer(value)));
                            }
                        }
                    }
                }
            }
        }
        if crate::proof_contracts::contract_entailment::struct_trace() {
            eprintln!(
                "STRUCT declaration-dispatch arm value={:?} selected={} remaining={}",
                arm.value,
                selected.len(),
                failed.len()
            );
        }
        for mut path in selected {
            let value = formed_value(&mut path.engine, program, arm.value, depth + 1, decisions)
                .or_else(|| {
                    if crate::proof_contracts::contract_entailment::struct_trace() {
                        eprintln!(
                            "STRUCT declaration-dispatch arm-formation-declined value={:?}",
                            arm.value
                        );
                    }
                    None
                })?;
            let value = path.engine.substituted(&value);
            engine.retain_operation_definitions(&path.engine)?;
            values.push(ConditionalValueArm {
                conditions: path.conditions,
                value,
            });
        }
        remaining = failed;
    }
    // Coverage belongs to the admitted subject, not a convenient subset of
    // alternatives. A missing reachable arm cannot donate a result value.
    if !remaining.is_empty() {
        return None;
    }
    engine.conditional_value(values)
}

fn condition_paths<'program>(
    mut path: Path<'program>,
    expression: ExpressionHandle,
    depth: usize,
    decisions: &mut ApplicationDecisions,
) -> Option<Vec<(bool, Path<'program>)>> {
    if depth >= 128 {
        return None;
    }
    path.engine.charge_conditional_work()?;
    let program = path.engine.program;
    match expression_table(program).expression(expression) {
        ExpressionNode::Boolean(value) => Some(vec![(*value, path)]),
        ExpressionNode::Atomic(atomic) => condition_paths(path, atomic.value, depth + 1, decisions),
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => Some(
            condition_paths(path, unary.operand, depth + 1, decisions)?
                .into_iter()
                .map(|(value, path)| (!value, path))
                .collect(),
        ),
        ExpressionNode::Binary(binary)
            if path.engine.selected_binary_meaning(expression, binary) =>
        {
            if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                let mut cases = Vec::new();
                for (value, path) in condition_paths(path, binary.left, depth + 1, decisions)? {
                    if (binary.operator == BinaryOperator::And && !value)
                        || (binary.operator == BinaryOperator::Or && value)
                    {
                        cases.push((value, path));
                    } else {
                        cases.extend(condition_paths(path, binary.right, depth + 1, decisions)?);
                    }
                }
                return Some(cases);
            }
            let complement = binary.operator.complement()?;
            let left = formed_value(&mut path.engine, program, binary.left, depth + 1, decisions)?;
            let right = formed_value(
                &mut path.engine,
                program,
                binary.right,
                depth + 1,
                decisions,
            )?;
            let mut cases = Vec::new();
            if let Some(selected) =
                path.clone()
                    .assume((binary.operator, left.clone(), right.clone()))
            {
                cases.push((true, selected));
            }
            if let Some(selected) = path.assume((complement, left, right)) {
                cases.push((false, selected));
            }
            Some(cases)
        }
        _ => {
            if crate::proof_contracts::contract_entailment::struct_trace() {
                eprintln!(
                    "STRUCT declaration-dispatch condition-declined expression={expression:?} node={:?}",
                    expression_table(program).expression(expression)
                );
            }
            None
        }
    }
}
