//! Boolean alternatives and predicates over completed value meanings.
//!
//! A false Boolean alternative narrows only its detached proof path. IEEE
//! comparison consumes exact sealed projections and a non-NaN fact; it never
//! enters the value-identity substitutions used for structural equality.
//! Primitive scalar predicates and selected semantic applications enter the
//! same completed-meaning reader. Reading only applications loses false scalar
//! guards, such as the zero exclusion on an absolute-value fallback path.

use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use numerics::float_projection::FloatProjectionOperation;
use numerics::float_semantics_catalog::{
    FloatSemanticComparison, FloatSemanticKernel, FloatSemanticPredicate,
};
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::PrimitiveType;

impl StructuralJudge<'_> {
    /// An observed Boolean equality against a literal determines the other
    /// operand. This is the inverse of Boolean folding, not an equality law
    /// for the values an operand's predicate may compare.
    pub(super) fn boolean_equality_operand(
        &self,
        predicate: &StructuralTerm,
        truth: &StructuralTerm,
    ) -> Option<(StructuralTerm, StructuralTerm)> {
        let StructuralTerm::ScalarBinary {
            references,
            operator,
            meaning,
            left,
            right,
        } = predicate
        else {
            return None;
        };
        if !meaning
            .iter()
            .all(|(primitive, _)| *primitive == PrimitiveType::Bool)
            || !matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
            || !super::super::structural_terms::scalar_operand_is_observed(
                self.program,
                references[0],
                left,
            )
            || !super::super::structural_terms::scalar_operand_is_observed(
                self.program,
                references[1],
                right,
            )
        {
            return None;
        }
        let truth = boolean_value(truth)?;
        let (operand, known) = if let Some(known) = boolean_value(right) {
            (left.as_ref(), known)
        } else {
            (right.as_ref(), boolean_value(left)?)
        };
        let equal = truth == (*operator == BinaryOperator::Equal);
        Some((
            operand.clone(),
            boolean_term(if equal { known } else { !known }),
        ))
    }

    /// Logical conjunction/disjunction have no overloadable operator spelling.
    /// The retained connective decomposes predicates; every leaf still owes
    /// its own exact selected meaning and truth judgment. A scalar result-type
    /// query must not exclude a nominal predicate from that decomposition.
    pub(super) fn has_boolean_connective_meaning(&self, expression: ExpressionHandle) -> bool {
        let Some(binary) = self.program.expression_table.binary_node(expression) else {
            return false;
        };
        matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
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
        let sealed_float = super::super::structural_terms::sealed_primitive_float_operator(
            program,
            expression,
            spelling,
            &references,
        )
        .is_some();
        if !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
            || program.primitive_type_reference(right_type) != Some(primitive)
            || (!typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                self.machine_symbol,
                expression,
                spelling,
                &references,
            ) && !sealed_float)
        {
            return None;
        }
        let project = |operand| {
            let term = self.declaration_term(operand)?;
            Some(self.resolve(projected_meaning(program, primitive, term.clone()).unwrap_or(term)))
        };
        let left = project(binary.left)?;
        let right = project(binary.right)?;
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
        match term.semantic_value() {
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
        if depth >= 32 {
            return None;
        }
        let (comparison, left, right) = self.selected_float_comparison(value)?;
        self.float_comparison_truth(comparison, &left, &right, depth + 1)
            .map(boolean_term)
    }

    /// Read the selected predicate without replacing it by its recorded truth.
    /// Scalar operands owe their original observation/carrier checks and exact
    /// format projection; semantic applications already supply meanings.
    fn selected_float_comparison(
        &self,
        value: &StructuralTerm,
    ) -> Option<(FloatSemanticComparison, StructuralTerm, StructuralTerm)> {
        if let StructuralTerm::ScalarBinary {
            references,
            operator,
            meaning,
            left,
            right,
        } = value.semantic_value()
        {
            let primitive = meaning[0].0;
            if meaning[1].0 != primitive
                || !matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
            {
                return None;
            }
            let comparison = match operator {
                BinaryOperator::Equal => FloatSemanticComparison::Equal,
                BinaryOperator::NotEqual => FloatSemanticComparison::NotEqual,
                BinaryOperator::Less => FloatSemanticComparison::Less,
                BinaryOperator::LessOrEqual => FloatSemanticComparison::LessOrEqual,
                BinaryOperator::Greater => FloatSemanticComparison::Greater,
                BinaryOperator::GreaterOrEqual => FloatSemanticComparison::GreaterOrEqual,
                _ => return None,
            };
            let operands = [left, right];
            for position in 0..2 {
                if super::super::structural_terms::scalar_operand_meaning(
                    self.program,
                    references[position],
                    operands[position],
                ) != Some(meaning[position])
                {
                    return None;
                }
            }
            return Some((
                comparison,
                projected_meaning(self.program, primitive, left.as_ref().clone())?,
                projected_meaning(self.program, primitive, right.as_ref().clone())?,
            ));
        }
        let StructuralTerm::Application {
            target,
            result_type,
            arguments,
            selections,
            ..
        } = value.semantic_value()
        else {
            return None;
        };
        if !selections.is_empty() {
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
        Some((comparison, left.clone(), right.clone()))
    }

    /// Inspect a retained predicate's selected meaning without resolving the
    /// predicate through its own truth rewrite. Only its completed operands
    /// are normalized; bounded public contract equations expose sealed calls.
    fn completed_float_comparison(
        &self,
        predicate: &StructuralTerm,
        depth: usize,
    ) -> Option<(FloatSemanticComparison, StructuralTerm, StructuralTerm)> {
        let mut predicate = predicate.clone();
        for depth in depth..32 {
            if let Some((comparison, left, right)) = self.selected_float_comparison(&predicate) {
                return Some((
                    comparison,
                    self.resolve_at(left, depth + 1),
                    self.resolve_at(right, depth + 1),
                ));
            }
            predicate = self.selected_contract_normal_form(&predicate, depth + 1)?;
        }
        None
    }

    /// One exact sealed projection keeps its format and completed scalar.
    /// Similar type names or literal spellings supply no projection law.
    fn float_projection<'term>(
        &self,
        term: &'term StructuralTerm,
    ) -> Option<(numerics::literals::FloatFormat, &'term StructuralTerm)> {
        let StructuralTerm::Application {
            target,
            result_type,
            selections,
            arguments,
            ..
        } = term
        else {
            return None;
        };
        if !selections.is_empty() {
            return None;
        }
        let [value] = arguments.as_slice() else {
            return None;
        };
        let operator = self.program.operator_definition_by_symbol(*target)?;
        if !crate::declared_carriers_match(self.program, *result_type, operator.return_type) {
            return None;
        }
        let [namespace, name] = self.program.operator_path_members(operator.name) else {
            return None;
        };
        let operation =
            FloatProjectionOperation::from_source_identity(namespace.as_str(), name.as_str())?;
        let primitive = crate::proof_contracts::float_projection_bindings::exact_toolchain_float_projection_primitive(
            self.program, operator, operation)?;
        let [parameter] = self.program.operator_parameters(operator) else {
            return None;
        };
        if !super::super::structural_terms::scalar_operand_is_observed(
            self.program,
            parameter.type_reference,
            value,
        ) {
            return None;
        }
        let format = match primitive {
            PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
            PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
            _ => return None,
        };
        // A previously landed literal cannot silently change format here.
        if let StructuralTerm::Float(literal) = value.semantic_value()
            && literal.landing().is_some_and(|landing| landing != format)
        {
            return None;
        }
        Some((format, value))
    }

    fn float_literal_endpoint(
        &self,
        term: &StructuralTerm,
    ) -> Option<(
        numerics::literals::FloatFormat,
        numerics::bignum::ExactFloat,
    )> {
        // A completed literal already carries its format. Two such literals
        // compare by IEEE value here; a projection application is the other
        // way a sealed meaning supplies that same literal.
        let (format, value) = if let StructuralTerm::Float(literal) = term.semantic_value() {
            (literal.landing()?, term)
        } else {
            self.float_projection(term)?
        };
        let StructuralTerm::Float(literal) = value.semantic_value() else {
            return None;
        };
        // Read the decimal directly in the authenticated format, once.
        Some((
            format,
            match format {
                numerics::literals::FloatFormat::F32 => {
                    numerics::bignum::ExactFloat::from_f32(literal.value_f32())
                }
                numerics::literals::FloatFormat::F64 => {
                    numerics::bignum::ExactFloat::from_f64(literal.value_f64())
                }
            },
        ))
    }

    fn float_bound_observation(
        &self,
        comparison: FloatSemanticComparison,
        left: &StructuralTerm,
        right: &StructuralTerm,
    ) -> Option<(StructuralTerm, facts::FloatPredicate)> {
        let operator = float_binary_operator(comparison);
        let (subject, format, operator, bound) =
            if let Some((format, bound)) = self.float_literal_endpoint(right) {
                (left, format, operator, bound)
            } else {
                let (format, bound) = self.float_literal_endpoint(left)?;
                (right, format, reverse_comparison(operator), bound)
            };
        if self.float_projection(subject)?.0 != format {
            return None;
        }
        Some((
            subject.clone(),
            facts::FloatPredicate::comparison(format, operator, &bound)?,
        ))
    }

    fn observed_float_comparison_truth(
        &self,
        comparison: FloatSemanticComparison,
        left: &StructuralTerm,
        right: &StructuralTerm,
        depth: usize,
    ) -> Option<bool> {
        if let Some(((left_format, left), (right_format, right))) = self
            .float_literal_endpoint(left)
            .zip(self.float_literal_endpoint(right))
            && left_format == right_format
        {
            use numerics::float_semantics::{FloatMeaning, FloatSemantics};
            let left = FloatMeaning::from_f64(left.to_f64());
            let right = FloatMeaning::from_f64(right.to_f64());
            return Some(match comparison {
                FloatSemanticComparison::Equal => FloatSemantics::equal(&left, &right),
                FloatSemanticComparison::NotEqual => FloatSemantics::not_equal(&left, &right),
                FloatSemanticComparison::Less => FloatSemantics::less(&left, &right),
                FloatSemanticComparison::LessOrEqual => {
                    FloatSemantics::less_or_equal(&left, &right)
                }
                FloatSemanticComparison::Greater => FloatSemantics::greater(&left, &right),
                FloatSemanticComparison::GreaterOrEqual => {
                    FloatSemantics::greater_or_equal(&left, &right)
                }
            });
        }
        let (subject, required) = self.float_bound_observation(comparison, left, right)?;
        for (predicate, truth) in self.rewrites.iter() {
            // Only positive observed IEEE predicates exclude NaN. Inverting a
            // false ordered predicate would grant an invalid integer law.
            if boolean_value(truth) != Some(true) {
                continue;
            }
            let Some((comparison, left, right)) =
                self.completed_float_comparison(predicate, depth + 1)
            else {
                continue;
            };
            let Some((observed_subject, observed)) =
                self.float_bound_observation(comparison, &left, &right)
            else {
                continue;
            };
            if observed_subject == subject && observed.implies(required) {
                return Some(true);
            }
        }
        None
    }

    pub(in crate::proof_contracts::contract_entailment) fn observed_float_predicate_judgment(
        &self,
        predicate: &StructuralTerm,
        depth: usize,
    ) -> Option<StructuralJudgment> {
        let (comparison, left, right) = self.completed_float_comparison(predicate, depth)?;
        match self.float_comparison_truth(comparison, &left, &right, depth + 1) {
            Some(true) => Some(StructuralJudgment::Proven),
            Some(false) => Some(StructuralJudgment::Refuted),
            None => None,
        }
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
        if let Some(truth) =
            self.observed_float_comparison_truth(comparison, left, right, depth + 1)
        {
            return Some(truth);
        }
        // IEEE equality and inequality are exact Boolean complements even
        // for NaNs and signed zeros. Use only a completed predicate over the
        // same meanings and an authenticated core comparison declaration.
        // Ordered comparisons deliberately gain no complement law: !(a < b)
        // does not establish a >= b when either operand may be NaN.
        let complement = match comparison {
            FloatSemanticComparison::Equal => Some(FloatSemanticComparison::NotEqual),
            FloatSemanticComparison::NotEqual => Some(FloatSemanticComparison::Equal),
            _ => None,
        };
        let same_meaning = left == right;
        let mut observed_non_nan = false;
        for (predicate, truth) in self.rewrites.iter() {
            let Some((observed, observed_left, observed_right)) =
                self.completed_float_comparison(predicate, depth + 1)
            else {
                continue;
            };
            let same_order = observed_left == *left && observed_right == *right;
            let swapped = matches!(
                comparison,
                FloatSemanticComparison::Equal | FloatSemanticComparison::NotEqual
            ) && observed_left == *right
                && observed_right == *left;
            let shares_subject =
                same_meaning && (observed_left == *left || observed_right == *left);
            if !same_order && !swapped && !shares_subject {
                continue;
            }
            let Some(truth) = boolean_value(&self.resolve_at(truth.clone(), depth + 1)) else {
                continue;
            };
            // A true equality or ordered comparison excludes NaN for both
            // exact operands, as does a false inequality. This also keeps a
            // saved float reflexive across a sibling write, without replacing
            // it with a literal or treating IEEE equality as operand identity.
            // False ordered predicates and true inequality classify neither.
            if shares_subject {
                observed_non_nan |= if observed == FloatSemanticComparison::NotEqual {
                    !truth
                } else {
                    truth
                };
            }
            if !same_order && !swapped {
                continue;
            }
            // A recorded Boolean is a predicate fact, never a substitution
            // between its operands. Both polarities survive this read.
            if observed == comparison {
                return Some(truth);
            }
            if Some(observed) == complement {
                return Some(!truth);
            }
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
        let classified = left_nan.or(right_nan).or(observed_non_nan.then_some(false));
        if super::super::struct_trace() {
            eprintln!(
                "STRUCT ieee completed comparison={comparison:?} same_meaning={same_meaning} left_nan={left_nan:?} right_nan={right_nan:?} observed_non_nan={observed_non_nan}",
            );
        }
        if left_nan == Some(true) || right_nan == Some(true) {
            Some(comparison.unordered_truth())
        } else if same_meaning {
            comparison.same_meaning_truth(classified)
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

fn boolean_value(value: &StructuralTerm) -> Option<bool> {
    let StructuralTerm::Constructor { data, case, fields } = value.semantic_value() else {
        return None;
    };
    if data != "bool" || !fields.is_empty() {
        return None;
    }
    language_semantics::const_value::boolean_literal_spelling(case)
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

fn float_binary_operator(comparison: FloatSemanticComparison) -> BinaryOperator {
    match comparison {
        FloatSemanticComparison::Equal => BinaryOperator::Equal,
        FloatSemanticComparison::NotEqual => BinaryOperator::NotEqual,
        FloatSemanticComparison::Less => BinaryOperator::Less,
        FloatSemanticComparison::LessOrEqual => BinaryOperator::LessOrEqual,
        FloatSemanticComparison::Greater => BinaryOperator::Greater,
        FloatSemanticComparison::GreaterOrEqual => BinaryOperator::GreaterOrEqual,
    }
}

fn reverse_comparison(operator: BinaryOperator) -> BinaryOperator {
    match operator {
        BinaryOperator::Less => BinaryOperator::Greater,
        BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
        BinaryOperator::Greater => BinaryOperator::Less,
        BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
        operator => operator,
    }
}
