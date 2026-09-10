//! Independent expression correspondence under source successor bindings.
use super::*;
impl Checker<'_> {
    fn available_block_value(&self, parameter: &target_operations::TargetScalarBlockValue) -> bool {
        let Some(sources) = self.available else {
            return false;
        };
        let mut definitions = sources
            .iter()
            .filter(|(value, _)| *value == parameter.value);
        matches!(definitions.next(),
            Some((_, target_operations::TargetUnitScalarArgumentSource::BlockParameter(expected)))
                if expected == parameter)
            && definitions.next().is_none()
    }

    pub(super) fn scalar_parameters(&self) -> &[target_operations::ScalarAbiValue] {
        if let Some(abi) = &self.function.scalar_abi {
            &abi.parameters
        } else if let Some(abi) = &self.function.mixed_structural_scalar_abi {
            &abi.scalar_parameters
        } else {
            match &self.function.operation {
                TargetOperation::UnitBody(body) => &body.scalar_parameters,
                TargetOperation::ControlGraph(graph) => &graph.scalar_parameters,
                _ => &[],
            }
        }
    }

    fn available_home(&self, home: &target_operations::TargetUnitScalarHomeRequirement) -> bool {
        let Some(sources) = self.available else {
            return false;
        };
        let mut definitions = sources
            .iter()
            .filter(|(value, _)| *value == home.source_value);
        let Some((_, target_operations::TargetUnitScalarArgumentSource::Home(expected))) =
            definitions.next()
        else {
            return false;
        };
        home == expected && definitions.next().is_none()
    }

    pub(super) fn expression(
        &self,
        expression: &Expression,
        value: ValueId,
        aliases: &[(ValueId, ValueId)],
    ) -> bool {
        let resolved = resolve(value, aliases);
        match expression {
            Expression::BlockParameter(parameter) => parameter.value == resolved
                && matches!(parameter.scalar_type, ScalarType::Integer(_))
                && self.available_block_value(parameter),
            Expression::ScalarHome(home) => home.source_value == resolved
                && matches!(home.scalar_type, ScalarType::Integer(_))
                && self.available_home(home),
            Expression::StructuralCall { .. } => self.structural_call(expression, resolved, aliases),
            Expression::ByteSequenceRead { psi_operation, source_value, source, view, index, length, obligation } => {
                *source_value == resolved
                    && self.byte_view(view, *source, aliases)
                    && self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                        AbstractOperation::ByteSequenceRead { psi_operation: operation, result, source: expected, index: expected_index, length: expected_length, obligation: expected_obligation }
                        if operation == psi_operation && result.value == resolved && expected == source
                        && length == expected_length && obligation == expected_obligation && self.expression(index, *expected_index, aliases)))
            }
            Expression::ByteSequenceLength { psi_operation, source_value, source, view, length_byte_offset } => {
                *length_byte_offset == 8 && *source_value == resolved
                    && (self.byte_view(view, *source, aliases) || self.mutable_byte_view(view, *source))
                    && self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                        AbstractOperation::ByteSequenceLength { psi_operation: operation, result, source: expected }
                        if operation == psi_operation && result.value == resolved && expected == source))
            }
            Expression::Immediate {source_value,value:literal} => *source_value == value && self.optimized.blocks.iter().flat_map(|block|&block.nodes).any(|node|
                matches!(&node.operation,AbstractOperation::IntegerConstant {result,value:actual,..} if *result == resolved && actual == literal)),
            Expression::Parameter {source_value,parameter_index,location} => {
                let Some(parameter) = self.scalar_parameters().get(*parameter_index) else {return false;};
                *source_value == value && parameter.value == resolved && location_matches(*location,&parameter.placement)
            }
            Expression::Call {psi_operation,source_value,callee,arguments,requirement_obligations,crash_continuations} => {
                let Some(carrier @ ScalarType::Integer(_)) = value_type(self.optimized, resolved) else { return false; };
                self.call(*psi_operation, *source_value, *callee, arguments, requirement_obligations, crash_continuations, resolved, carrier, aliases)
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
            Expression::IntegerWiden { psi_operation, source_type, operand } => {
                self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                    AbstractOperation::IntegerWiden { psi_operation: operation, result, source_type: actual_type, operand: source, .. }
                    if operation == psi_operation && *result == resolved && actual_type == source_type && self.expression(operand, *source, aliases)))
            }
            Expression::IntegerExactCast { psi_operation, obligation, source_type, operand } => {
                self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                    AbstractOperation::IntegerExactCast { psi_operation: operation, obligation: expected_obligation, result, source_type: actual_type, operand: source, .. }
                    if operation == psi_operation && expected_obligation == obligation && *result == resolved && actual_type == source_type && self.expression(operand, *source, aliases)))
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
        if let Boolean::Call {
            psi_operation,
            source_value,
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
        } = expression
        {
            return self.call(
                *psi_operation,
                *source_value,
                *callee,
                arguments,
                requirement_obligations,
                crash_continuations,
                resolve(value, aliases),
                ScalarType::Boolean,
                aliases,
            );
        }
        if let Boolean::BlockParameter(parameter) = expression {
            return parameter.value == resolve(value, aliases)
                && parameter.scalar_type == ScalarType::Boolean
                && self.available_block_value(parameter);
        }
        if let Boolean::Equal {
            psi_operation,
            left,
            right,
        } = expression
        {
            return self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| {
                matches!(&node.operation,
                    AbstractOperation::BooleanEqual { psi_operation: operation, result, left: source_left, right: source_right }
                    if operation == psi_operation && *result == resolve(value, aliases)
                    && self.boolean(left, *source_left, aliases)
                    && self.boolean(right, *source_right, aliases))
            });
        }
        if let Boolean::ScalarHome(home) = expression {
            return home.source_value == resolve(value, aliases)
                && home.scalar_type == ScalarType::Boolean
                && self.available_home(home);
        }
        if let Boolean::Immediate {
            source_value,
            value: literal,
        } = expression
        {
            return *source_value == value
                && self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node|
                    matches!(&node.operation, AbstractOperation::BooleanConstant { result, value: actual, .. }
                        if *result == resolve(value, aliases) && actual == literal))
                && self.available.is_none_or(|sources| sources.iter().any(|(source, definition)|
                    *source == value && matches!(definition,
                        target_operations::TargetUnitScalarArgumentSource::BooleanImmediate { value: actual, .. } if actual == literal)));
        }
        if let Boolean::Parameter {
            source_value,
            parameter_index,
            location,
        } = expression
        {
            return *source_value == value
                && self
                    .scalar_parameters()
                    .get(*parameter_index)
                    .is_some_and(|parameter| {
                        parameter.value == resolve(value, aliases)
                            && parameter.scalar_type == ScalarType::Boolean
                            && location_matches(*location, &parameter.placement)
                    });
        }
        if let Boolean::Not {
            psi_operation,
            operand,
        } = expression
        {
            return self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                AbstractOperation::BooleanNot { psi_operation: operation, result, operand: source }
                if operation == psi_operation && *result == resolve(value, aliases) && self.boolean(operand, *source, aliases)));
        }
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
    #[allow(clippy::too_many_arguments)]
    fn call(
        &self,
        operation: semantic_vocabulary::OperationId,
        source: ValueId,
        target: MachineId,
        arguments: &[target_operations::TargetCallArgument],
        requirement_obligations: &[semantic_vocabulary::ObligationId],
        crash_continuations: &[terminal_psi::CrashRouteBucket],
        resolved: ValueId,
        carrier: ScalarType,
        aliases: &[(ValueId, ValueId)],
    ) -> bool {
        let psi_operation = &operation;
        let source_value = &source;
        let callee = &target;
        let Some(node) = self.optimized.blocks.iter().flat_map(|block|&block.nodes).find(|node|matches!(&node.operation,AbstractOperation::Call {psi_operation:operation,..} if operation == psi_operation)) else {return false;};
        let AbstractOperation::Call {
            result,
            callee: actual,
            arguments: sources,
            requirement_obligations: requirements,
            crash_continuations: crashes,
            ..
        } = &node.operation
        else {
            return false;
        };
        let Ok(call) = callee_plan(*callee, self.native, self.plan, self.unit) else {
            return false;
        };
        if value_type(self.optimized, resolved) != Some(carrier)
            || *source_value != resolved
            || *result != resolved
            || actual != callee
            || requirement_obligations != requirements
            || crash_continuations != crashes
            || arguments.len() != sources.len()
            || arguments.len() != call.parameters.len()
        {
            return false;
        }
        arguments.iter().zip(sources).zip(&call.parameters).all(
            |((argument, source), placement)| {
                value_type(self.optimized, *source) == Some(argument.scalar_type)
                    && scalar_shape(argument.scalar_type) == Some(placement.shape)
                    && location_matches(argument.location, placement)
                    && match &argument.expression {
                        TargetScalarExpression::IeeeFloat(_) => false,
                        TargetScalarExpression::Integer {
                            scalar_type,
                            expression,
                        } => {
                            argument.scalar_type == ScalarType::Integer(*scalar_type)
                                && self.expression(expression, *source, aliases)
                        }
                        TargetScalarExpression::Boolean(expression) => {
                            argument.scalar_type == ScalarType::Boolean
                                && self.boolean(expression, *source, aliases)
                        }
                    }
            },
        )
    }
}
