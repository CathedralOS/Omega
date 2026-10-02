//! Content comparison observes a completed structural operand. Pure places
//! and computed descriptors share the same query; descriptor establishment
//! stays at the operand's selective evaluation point.
use super::{
    BinaryOperator, Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    ExpressionHandle, ExpressionNode, PrimitiveType, semantic_casts,
};
use crate::execution::terminal_unit::{calls, types};
use checked_trees::{
    CheckedStructuralAccess, CheckedSubsliceSite, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan as Source,
};

impl Builder<'_, '_> {
    pub(super) fn byte_literal_comparison(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<CheckedScalarComputationHandle> {
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) || !super::operator_is_builtin(self.operators, expression)
        {
            return None;
        }
        let operand = match (
            self.program.expression_table.expression(binary.left),
            self.program.expression_table.expression(binary.right),
        ) {
            (_, ExpressionNode::String(_)) => binary.left,
            (ExpressionNode::String(_), _) => binary.right,
            _ => return None,
        };
        let target = semantic_casts::result_type(self.program, self.state, operand)?;
        super::super::boolean_lowering::comparable_byte_carrier_type(self.program, target)?;
        let subject = self.byte_operand_subject(operand, target)?;
        Some(self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::ByteSequenceLiteralComparison {
                operand_type: target,
                source_expression: expression,
                subject,
            },
        ))
    }

    /// A declared byte-predicate call observes the same completed structural
    /// operand a content comparison would: a pure place or a computed
    /// descriptor admitted at the same selective evaluation point. The
    /// predicate's reads happen only after the operand's own preparation.
    pub(super) fn byte_predicate(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<CheckedScalarComputationHandle> {
        let (predicate, operand) =
            typed_trees::byte_predicates::byte_predicate_call(self.program, expression)?;
        let target = semantic_casts::result_type(self.program, self.state, operand)?;
        super::super::boolean_lowering::comparable_byte_carrier_type(self.program, target)?;
        let subject = self.byte_operand_subject(operand, target)?;
        Some(self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::ByteSequencePredicate {
                predicate,
                operand_type: target,
                source_expression: expression,
                subject,
            },
        ))
    }

    /// The byte-observation operand both content comparisons and declared
    /// predicates share: a pure place when one names the operand, else the
    /// computed descriptor admission at the expression's own site.
    fn byte_operand_subject(
        &mut self,
        operand: ExpressionHandle,
        target: typed_trees::types::TypeReferenceHandle,
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        if let Some(subject) = self.byte_comparison_place(operand, target) {
            return Some(subject);
        }
        let (machine, state) =
            crate::semantic::calls::find_state_with_machine(self.program, self.state)?;
        let parameters = if machine.attached_data.is_some() {
            calls::ambient_self_scalar_graph_signature(self.program, machine, state)?.0
        } else {
            calls::free_structural_scalar_signature(
                self.program,
                &mut types::ShapeCollector::new(self.program),
                state,
                &[],
            )?
            .0
        };
        calls::view_subslice::admit_resolved(
            self.program,
            self.operators,
            self.pure,
            machine,
            state,
            &parameters,
            target,
            operand,
            self.statement_index,
            CheckedSubsliceSite::Expression {
                expression: operand,
            },
        )
        .map(|resolution| resolution.argument())
    }
    fn byte_comparison_place(
        &self,
        operand: ExpressionHandle,
        target: typed_trees::types::TypeReferenceHandle,
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        let (machine, state) =
            crate::semantic::calls::find_state_with_machine(self.program, self.state)?;
        let place = crate::flow::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            operand,
        )?;
        if !validation::place_has_builtin_coordinates(self.program, machine, Some(state), operand) {
            return None;
        }
        let facts::PlaceRoot::Symbol(root) =
            crate::flow::normalized_event_place_root(self.program, place.root)
        else {
            return None;
        };
        let source = if let Some(position) = self.authored_parameters.iter().position(|parameter| {
            (parameter.is_self && root == self.machine)
                || crate::flow::normalized_event_place_root(
                    self.program,
                    facts::PlaceRoot::Symbol(parameter.symbol),
                ) == facts::PlaceRoot::Symbol(root)
        }) {
            Source::Parameter {
                parameter_index: self.dense_structural_parameter_index(position)?,
            }
        } else {
            // The ordinary operation sequence owns local establishment and
            // lifetime. Observing a projected field retains that local's
            // identity rather than replaying its initializer as storage.
            Source::StructuralLocal { symbol: root }
        };
        let (projected, path) = calls::projected_observed_path(
            self.program,
            self.machine,
            self.state,
            self.statement_index,
            &place,
            &|selector| self.local_selector(selector),
        )?;
        if self.program.normalized_type_identity(projected)
            != self.program.normalized_type_identity(target)
        {
            return None;
        }
        Some(CheckedUnitStructuralArgumentPlan {
            source,
            path,
            type_identity: types::byte_sequence_type_identity(self.program, target, &[], &[])?,
            access: CheckedStructuralAccess::SharedBorrow,
        })
    }
}
