//! Independent expression correspondence under source successor bindings.
use super::*;
impl Checker<'_> {
    pub(super) fn expression(
        &self,
        expression: &Expression,
        value: ValueId,
        aliases: &[(ValueId, ValueId)],
    ) -> bool {
        let resolved = resolve(value, aliases);
        match expression {
            Expression::Immediate {source_value,value:literal} => *source_value == value && self.optimized.blocks.iter().flat_map(|block|&block.nodes).any(|node|
                matches!(&node.operation,AbstractOperation::IntegerConstant {result,value:actual,..} if *result == resolved && actual == literal)),
            Expression::Parameter {source_value,parameter_index,location} => {
                let Some(parameter) = self.function.fixed_integer_scalar_abi.as_ref().and_then(|abi|abi.parameters.get(*parameter_index)) else {return false;};
                *source_value == value && parameter.value == resolved && location_matches(*location,&parameter.placement)
            }
            Expression::Call {psi_operation,source_value,callee,arguments,requirement_obligations,crash_continuations} => {
                let Some(node) = self.optimized.blocks.iter().flat_map(|block|&block.nodes).find(|node|matches!(&node.operation,AbstractOperation::Call {psi_operation:operation,..} if operation == psi_operation)) else {return false;};
                let AbstractOperation::Call {result,callee:actual,arguments:sources,requirement_obligations:requirements,crash_continuations:crashes,..} = &node.operation else {return false;};
                let Ok(call) = callee_plan(*callee,self.native,self.plan,self.unit) else {return false;};
                if *source_value != resolved || *result != resolved || actual != callee || requirement_obligations != requirements || crash_continuations != crashes || arguments.len() != sources.len() || arguments.len() != call.parameters.len() {return false;}
                arguments.iter().zip(sources).zip(&call.parameters).all(|((argument,source),placement)| {
                    let TargetScalarExpression::Integer {scalar_type,expression} = &argument.expression else {return false;};
                    argument.scalar_type == ScalarType::Integer(u64_type()) && *scalar_type == u64_type() && location_matches(argument.location,placement) && self.expression(expression,*source,aliases)
                })
            }
            Expression::ExactAdd {psi_operation,obligation,left,right} | Expression::ExactSubtract {psi_operation,obligation,left,right} => {
                let Some(node) = self.optimized.blocks.iter().flat_map(|block|&block.nodes).find(|node| matches!(&node.operation,
                    AbstractOperation::ExactIntegerAdd {psi_operation:operation,..} | AbstractOperation::ExactIntegerSubtract {psi_operation:operation,..} if operation == psi_operation)) else {return false;};
                let (result,source_obligation,source_left,source_right) = match (&node.operation,expression) {
                    (AbstractOperation::ExactIntegerAdd {result,obligation,left,right,..},Expression::ExactAdd {..})
                    | (AbstractOperation::ExactIntegerSubtract {result,obligation,left,right,..},Expression::ExactSubtract {..}) => (*result,*obligation,*left,*right),
                    _ => return false,
                };
                result == resolved && source_obligation == *obligation && self.expression(left,source_left,aliases) && self.expression(right,source_right,aliases)
            }
            _ => false,
        }
    }
    pub(super) fn boolean(
        &self,
        expression: &Boolean,
        value: ValueId,
        aliases: &[(ValueId, ValueId)],
    ) -> bool {
        let Some(node) = self
            .optimized
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .find(|node| {
                node.definitions
                    .iter()
                    .any(|definition| definition.value == value)
            })
        else {
            return false;
        };
        let (operation, source_left, source_right, scalar_type, left, right, target_operation) =
            match (&node.operation, expression) {
                (
                    AbstractOperation::IntegerEqual {
                        psi_operation,
                        left: source_left,
                        right: source_right,
                        ..
                    },
                    Boolean::IntegerEqual {
                        psi_operation: target_operation,
                        scalar_type,
                        left,
                        right,
                    },
                )
                | (
                    AbstractOperation::IntegerLessThan {
                        psi_operation,
                        left: source_left,
                        right: source_right,
                        ..
                    },
                    Boolean::IntegerLessThan {
                        psi_operation: target_operation,
                        scalar_type,
                        left,
                        right,
                    },
                )
                | (
                    AbstractOperation::IntegerLessOrEqual {
                        psi_operation,
                        left: source_left,
                        right: source_right,
                        ..
                    },
                    Boolean::IntegerLessOrEqual {
                        psi_operation: target_operation,
                        scalar_type,
                        left,
                        right,
                    },
                ) => (
                    *psi_operation,
                    *source_left,
                    *source_right,
                    *scalar_type,
                    left,
                    right,
                    *target_operation,
                ),
                _ => return false,
            };
        operation == target_operation
            && value_type(self.optimized, source_left) == Some(ScalarType::Integer(scalar_type))
            && self.expression(left, source_left, aliases)
            && self.expression(right, source_right, aliases)
    }
}
