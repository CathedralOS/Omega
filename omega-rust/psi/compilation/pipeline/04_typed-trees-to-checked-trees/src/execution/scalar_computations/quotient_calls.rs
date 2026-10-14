//! Explicit checked expression graph for sealed quotient requests. This does
//! not admit a request: final publication consumes its separately checked row.
use super::Builder;
use checked_trees::{
    CheckedQuotientAdaptationLicense, CheckedStructuralValueHandle, CheckedStructuralValueKind,
};
use typed_trees::{
    expression::{ExpressionHandle, ExpressionNode},
    types::TypeReferenceHandle,
};

impl Builder<'_, '_> {
    pub(super) fn quotient_scalar_call(
        &mut self,
        expression: ExpressionHandle,
        expected: typed_trees::types::PrimitiveType,
    ) -> Option<checked_trees::CheckedScalarComputationHandle> {
        let ExpressionNode::Call(call) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        let request = call.quotient_operation.as_ref()?;
        let (_, representative) = validation::find_machine_by_entry_state(
            self.program,
            request.representative_operation.symbol,
        )?;
        let result_type = representative.return_type;
        if self.program.primitive_type_reference(result_type) != Some(expected) {
            return None;
        }
        let receiver_value = self.quotient_call_value(expression, result_type)?;
        self.structural_values
            .roots
            .append(checked_trees::CheckedStructuralValueRoot {
                machine: self.machine,
                state: self.state,
                statement_ordinal: u32::try_from(self.statement_index).ok()?,
                expression,
                type_reference: result_type,
                root: receiver_value,
            });
        Some(self.insert(expected, checked_trees::CheckedScalarComputationKind::StructuralField {
            source_expression: expression, receiver_expression: expression, source_type: result_type, receiver_value,
            subject: checked_trees::CheckedUnitStructuralArgumentPlan {
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression },
                path: Vec::new(), type_identity: validation::stored_structural_type_identity(self.program, result_type, &[], &[]),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            },
            field: symbols::SymbolHandle::invalid(), field_identity: Default::default(),
        }))
    }

    pub(super) fn quotient_call_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueHandle> {
        let ExpressionNode::Call(call) =
            self.program.expression_table.expression(expression).clone()
        else {
            return None;
        };
        let request = call.quotient_operation.as_ref()?;
        let (_, representative) = validation::find_machine_by_entry_state(
            self.program,
            request.representative_operation.symbol,
        )?;
        let parameters = self.program.state_parameters(representative).to_vec();
        let actuals = self
            .program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec();
        if parameters.len() != actuals.len() || !call.carries_only_positional_arguments() {
            return None;
        }
        let mut arguments = Vec::new();
        for (position, (actual, formal)) in actuals.into_iter().zip(&parameters).enumerate() {
            // Scalar actuals retain their scalar evaluation lane; until that
            // producer is installed, decline rather than invent structural ABI.
            if self
                .program
                .primitive_type_reference(formal.type_reference)
                .is_some()
            {
                return None;
            }
            let actual_type = validation::expression_type_reference_in_state(
                self.program,
                self.state,
                self.statement_index,
                actual,
            )?;
            let value = self.structural_value(actual, actual_type)?;
            let value = if let Some(data) = self.program.closed_data_definition(actual_type)
                && let Some(quotient) = data.quotient.as_ref()
            {
                if self.program.normalized_type_identity(quotient.carrier)
                    != self.program.normalized_type_identity(formal.type_reference)
                    || self.program.type_multiplicity(actual_type)
                        != language_semantics::Multiplicity::Unrestricted
                {
                    return None;
                }
                self.publish_structural_value(
                    actual,
                    formal.type_reference,
                    CheckedStructuralValueKind::QuotientAdaptation {
                        source: value,
                        quotient: actual_type,
                        license: CheckedQuotientAdaptationLicense::Input {
                            request: expression,
                            position: u32::try_from(position).ok()?,
                        },
                    },
                )?
            } else {
                if self.program.normalized_type_identity(actual_type)
                    != self.program.normalized_type_identity(formal.type_reference)
                {
                    return None;
                }
                value
            };
            arguments.push(value);
        }
        let value = self.publish_structural_value(
            expression,
            representative.return_type,
            CheckedStructuralValueKind::QuotientCall {
                request: expression,
                representative_state: representative.symbol,
                arguments,
            },
        )?;
        if let Some(data) = self.program.closed_data_definition(expected)
            && let Some(quotient) = data.quotient.as_ref()
        {
            if self.program.normalized_type_identity(quotient.carrier)
                != self
                    .program
                    .normalized_type_identity(representative.return_type)
                || self.program.type_multiplicity(expected)
                    != language_semantics::Multiplicity::Unrestricted
            {
                return None;
            }
            self.publish_structural_value(
                expression,
                expected,
                CheckedStructuralValueKind::QuotientAdaptation {
                    source: value,
                    quotient: expected,
                    license: CheckedQuotientAdaptationLicense::Result {
                        request: expression,
                    },
                },
            )
        } else {
            (self.program.normalized_type_identity(expected)
                == self
                    .program
                    .normalized_type_identity(representative.return_type))
            .then_some(value)
        }
    }
}
