//! Independently rejoin mixed call expressions to ordered source calls and both ABIs.
use super::*;

impl Checker<'_> {
    pub(super) fn structural_call(
        &self,
        expression: &Expression,
        value: ValueId,
        aliases: &[(ValueId, ValueId)],
    ) -> bool {
        let Expression::StructuralCall {
            psi_operation,
            source_value,
            callee,
            arguments,
            structural_arguments,
            call_plan,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } = expression
        else {
            return false;
        };
        let Some(node) = self
            .optimized
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .find(|node| {
                matches!(&node.operation,
                AbstractOperation::CallStructuralScalar { psi_operation: operation, .. }
                if operation == psi_operation)
            })
        else {
            return false;
        };
        let AbstractOperation::CallStructuralScalar {
            result,
            callee: expected_callee,
            arguments: sources,
            structural_arguments: source_structural,
            claim_transfers: claims,
            requirement_obligations: requirements,
            crash_continuations: crashes,
            ..
        } = &node.operation
        else {
            return false;
        };
        let Ok(expected) = callee_plan(*callee, self.native, self.plan, self.unit) else {
            return false;
        };
        let ([semantic], [target]) = (
            source_structural.as_slice(),
            structural_arguments.as_slice(),
        ) else {
            return false;
        };
        if *source_value != value
            || result.value != value
            || expected_callee != callee
            || call_plan != &expected
            || arguments.len() != sources.len()
            || expected.parameters.len() != arguments.len() + 1
            || claim_transfers != claims
            || requirement_obligations != requirements
            || crash_continuations != crashes
            || super::super::structural_call::validate_argument(
                semantic,
                target,
                *psi_operation,
                self.optimized,
                *callee,
                self.native,
                self.plan,
                self.unit,
            )
            .is_err()
        {
            return false;
        }
        arguments.iter().zip(sources).zip(&expected.parameters).all(
            |((argument, source), placement)| {
                let TargetScalarExpression::Integer {
                    scalar_type,
                    expression,
                } = &argument.expression
                else {
                    return false;
                };
                argument.scalar_type == ScalarType::Integer(*scalar_type)
                    && value_type(self.optimized, *source) == Some(argument.scalar_type)
                    && scalar_shape(argument.scalar_type) == Some(placement.shape)
                    && location_matches(argument.location, placement)
                    && self.expression(expression, *source, aliases)
            },
        )
    }
}
