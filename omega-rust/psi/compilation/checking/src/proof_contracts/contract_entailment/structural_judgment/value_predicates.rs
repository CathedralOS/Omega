//! Boolean alternatives and predicates over completed value meanings.
//!
//! A false Boolean alternative narrows only its detached proof path. IEEE
//! comparison consumes exact sealed projections and a non-NaN fact; it never
//! enters the value-identity substitutions used for structural equality.

use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use numerics::float_projection::FloatProjectionOperation;
use numerics::float_semantics_catalog::{FloatSemanticKernel, FloatSemanticPredicate};
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::PrimitiveType;

impl StructuralJudge<'_> {
    /// Logical conjunction/disjunction have no overloadable operator spelling.
    /// Require their retained Boolean operands before decomposing the node.
    pub(super) fn has_boolean_connective_meaning(&self, expression: ExpressionHandle) -> bool {
        let Some(binary) = self.program.expression_table.binary_node(expression) else {
            return false;
        };
        if !matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
            return false;
        }
        [binary.left, binary.right].into_iter().all(|operand| {
            self.declaration_expression_type(operand)
                .and_then(|reference| self.program.primitive_type_reference(reference))
                == Some(PrimitiveType::Bool)
        })
    }

    pub(super) fn judge_disjunction(
        &self,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> StructuralJudgment {
        let left_judgment = self.judge(self.program, left);
        let right_judgment = self.judge(self.program, right);
        match (&left_judgment, &right_judgment) {
            (StructuralJudgment::Proven, _) | (_, StructuralJudgment::Proven) => {
                return StructuralJudgment::Proven;
            }
            (StructuralJudgment::Refuted, StructuralJudgment::Refuted) => {
                return StructuralJudgment::Refuted;
            }
            _ => {}
        }
        let Some(left_term) = self.declaration_term(left) else {
            return StructuralJudgment::Unknown;
        };
        let mut alternative = self.clone();
        alternative.intake_equation(left_term, boolean_term(false), 0);
        if matches!(
            alternative.judge(self.program, right),
            StructuralJudgment::Proven
        ) {
            StructuralJudgment::Proven
        } else {
            StructuralJudgment::Unknown
        }
    }

    /// Equal complete meanings imply IEEE equality only outside NaN. The
    /// selected comparison and projection source carriers must both agree;
    /// user operators and cross-format meanings supply no such law.
    pub(super) fn judge_ieee_equality(
        &self,
        expression: ExpressionHandle,
    ) -> Option<StructuralJudgment> {
        let program = self.program;
        let binary = program.expression_table.binary_node(expression)?;
        let spelling = match binary.operator {
            BinaryOperator::Equal => language_core::OperatorSpelling::Equal,
            BinaryOperator::NotEqual => language_core::OperatorSpelling::NotEqual,
            _ => return None,
        };
        let references =
            [binary.left, binary.right].map(|operand| self.declaration_expression_type(operand));
        let [Some(left_type), Some(right_type)] = references else {
            return None;
        };
        let primitive = program.primitive_type_reference(left_type)?;
        if !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
            || program.primitive_type_reference(right_type) != Some(primitive)
            || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                self.machine_symbol,
                expression,
                spelling,
                &references,
            )
        {
            return None;
        }
        let left = self.resolve(projected_meaning(
            program,
            primitive,
            self.declaration_term(binary.left)?,
        )?);
        let right = self.resolve(projected_meaning(
            program,
            primitive,
            self.declaration_term(binary.right)?,
        )?);
        let nan_truth = |meaning: &StructuralTerm| {
            let predicate = nan_predicate(program, meaning.clone())?;
            let term = self.resolve(predicate);
            if term == boolean_term(true) {
                Some(true)
            } else if term == boolean_term(false) {
                Some(false)
            } else {
                None
            }
        };
        let left_nan = nan_truth(&left);
        let right_nan = nan_truth(&right);
        let equality = if left_nan == Some(true) || right_nan == Some(true) {
            false
        } else if left == right && (left_nan == Some(false) || right_nan == Some(false)) {
            true
        } else {
            return Some(StructuralJudgment::Unknown);
        };
        let holds = equality == (binary.operator == BinaryOperator::Equal);
        Some(if holds {
            StructuralJudgment::Proven
        } else {
            StructuralJudgment::Refuted
        })
    }
}

fn boolean_term(value: bool) -> StructuralTerm {
    StructuralTerm::Constructor {
        data: "bool".to_owned(),
        case: value.to_string(),
        fields: Vec::new(),
    }
}

fn projected_meaning(
    program: &dyn crate::CheckingInput,
    primitive: PrimitiveType,
    value: StructuralTerm,
) -> Option<StructuralTerm> {
    let mut candidates = program.operators().iter().filter_map(|operator| {
        let [namespace, name] = program.operator_path_members(operator.name) else {
            return None;
        };
        let operation =
            FloatProjectionOperation::from_source_identity(namespace.as_str(), name.as_str())?;
        let source = crate::proof_contracts::float_projection_bindings::exact_toolchain_float_projection_primitive(
            program, operator, operation,
        )?;
        (source == primitive).then_some(operator)
    });
    let operator = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    operator_application(program, operator, value)
}

fn nan_predicate(
    program: &dyn crate::CheckingInput,
    meaning: StructuralTerm,
) -> Option<StructuralTerm> {
    let mut candidates = program.operators().iter().filter(|operator| {
        crate::proof_contracts::float_projection_bindings::semantic_operations::exact_toolchain_float_semantic_contract(program, operator)
            .is_some_and(|(row, _)| matches!(row.kernel(), FloatSemanticKernel::SinglePredicate(FloatSemanticPredicate::NaN)))
    });
    let operator = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    operator_application(program, operator, meaning)
}

fn operator_application(
    program: &dyn crate::CheckingInput,
    operator: &typed_trees::operator::OperatorDefinition,
    argument: StructuralTerm,
) -> Option<StructuralTerm> {
    let name = program.operator_path_members(operator.name).last()?;
    Some(StructuralTerm::Application {
        result_type: operator.return_type,
        target: operator.symbol,
        selections: Vec::new(),
        machine: super::super::structural_terms::structural_call_machine_name(
            name.as_str(),
            &[],
            &[],
        ),
        arguments: vec![argument],
    })
}
