//! Boolean alternatives and predicates over completed value meanings.
//!
//! A false Boolean alternative narrows only its detached proof path. IEEE
//! comparison consumes exact sealed projections and a non-NaN fact; it never
//! enters the value-identity substitutions used for structural equality.

use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use numerics::float_projection::FloatProjectionOperation;
use numerics::float_semantics_catalog::{
    FloatSemanticComparison, FloatSemanticKernel, FloatSemanticPredicate,
};
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
        if super::super::struct_trace() {
            eprintln!(
                "STRUCT ieee operands owner={:?} expression={expression:?} references={references:?}",
                self.machine_symbol
            );
        }
        let [Some(left_type), Some(right_type)] = references else {
            return None;
        };
        let primitive = program.primitive_type_reference(left_type)?;
        if super::super::struct_trace()
            && matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
        {
            eprintln!(
                "STRUCT ieee authority owner={:?} expression={expression:?} primitive={primitive:?} right_primitive={:?} builtin={}",
                self.machine_symbol,
                program.primitive_type_reference(right_type),
                typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    self.machine_symbol,
                    expression,
                    spelling,
                    &references
                )
            );
        }
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
        let comparison = match binary.operator {
            BinaryOperator::Equal => FloatSemanticComparison::Equal,
            BinaryOperator::NotEqual => FloatSemanticComparison::NotEqual,
            _ => return None,
        };
        Some(
            match self.float_comparison_truth(comparison, &left, &right, 0) {
                Some(true) => StructuralJudgment::Proven,
                Some(false) => StructuralJudgment::Refuted,
                None => StructuralJudgment::Unknown,
            },
        )
    }

    /// Selected callable comparisons enter the same application normalizer as
    /// named calls. A Boolean truth follows only from completed value terms,
    /// not from equality's source token or a matching result carrier.
    pub(super) fn judge_value_predicate(
        &self,
        expression: ExpressionHandle,
    ) -> Option<StructuralJudgment> {
        if self
            .declaration_expression_type(expression)
            .and_then(|reference| self.program.primitive_type_reference(reference))
            != Some(PrimitiveType::Bool)
        {
            return None;
        }
        let term = self.resolve(self.declaration_term(expression)?);
        match term {
            StructuralTerm::Constructor { data, case, fields }
                if data == "bool" && fields.is_empty() =>
            {
                match language_semantics::const_value::boolean_literal_spelling(&case) {
                    Some(true) => Some(StructuralJudgment::Proven),
                    Some(false) => Some(StructuralJudgment::Refuted),
                    None => None,
                }
            }
            _ => None,
        }
    }

    /// The sealed comparison row owns its law. Equal complete operands permit
    /// reflexive IEEE truth only with the appropriate NaN classification;
    /// this reduction creates a Boolean, never a value-identity substitution.
    pub(super) fn resolved_float_comparison(
        &self,
        value: &StructuralTerm,
        depth: usize,
    ) -> Option<StructuralTerm> {
        let StructuralTerm::Application {
            target,
            result_type,
            arguments,
            selections,
            ..
        } = value
        else {
            return None;
        };
        if depth >= 32 || !selections.is_empty() {
            return None;
        }
        let operator = self.program.operator_definition_by_symbol(*target)?;
        let (row, _) = crate::proof_contracts::float_projection_bindings::semantic_operations::exact_toolchain_float_semantic_contract(self.program, operator)?;
        let FloatSemanticKernel::PairBool(comparison) = row.kernel() else {
            return None;
        };
        let [left, right] = arguments.as_slice() else {
            return None;
        };
        if !crate::declared_carriers_match(self.program, *result_type, operator.return_type)
            || self
                .program
                .arithmetic_domain_for_type_reference(*result_type)
                != self
                    .program
                    .arithmetic_domain_for_type_reference(operator.return_type)
        {
            return None;
        }
        self.float_comparison_truth(comparison, left, right, depth + 1)
            .map(boolean_term)
    }

    fn float_comparison_truth(
        &self,
        comparison: FloatSemanticComparison,
        left: &StructuralTerm,
        right: &StructuralTerm,
        depth: usize,
    ) -> Option<bool> {
        if depth >= 32 {
            return None;
        }
        let nan_truth = |meaning: &StructuralTerm| {
            let predicate = nan_predicate(self.program, meaning.clone())?;
            let term = self.resolve_at(predicate, depth + 1);
            if term == boolean_term(true) {
                Some(true)
            } else if term == boolean_term(false) {
                Some(false)
            } else {
                None
            }
        };
        let left_nan = nan_truth(left);
        let right_nan = nan_truth(right);
        if super::super::struct_trace() {
            eprintln!(
                "STRUCT ieee completed comparison={comparison:?} same_meaning={} left_nan={left_nan:?} right_nan={right_nan:?}",
                left == right
            );
        }
        if left_nan == Some(true) || right_nan == Some(true) {
            Some(comparison.unordered_truth())
        } else if left == right {
            comparison.same_meaning_truth(left_nan.or(right_nan))
        } else {
            None
        }
    }
}

pub(super) fn boolean_term(value: bool) -> StructuralTerm {
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
    let operator = candidates.next();
    let duplicate = candidates.next();
    if super::super::struct_trace() {
        eprintln!(
            "STRUCT ieee projection primitive={primitive:?} target={:?} duplicate={:?}",
            operator.map(|operator| operator.symbol),
            duplicate.map(|operator| operator.symbol)
        );
    }
    let operator = operator?;
    if duplicate.is_some() {
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
