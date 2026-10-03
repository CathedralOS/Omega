//! Independent expression correspondence under source successor bindings.
use super::super::ScalarType;
use super::{AbstractOperation, ValueId};
use crate::legalization::scalar_graph_input::target::Boolean;
use crate::legalization::scalar_graph_input::target::Checker;
use crate::legalization::scalar_graph_input::target::Expression;
use crate::legalization::scalar_graph_input::target::location_matches;
use crate::legalization::scalar_graph_input::target::resolve;
use crate::legalization::scalar_graph_input::value_type;
use crate::legalization::scalar_graph_input::{
    saturating_carrier, supports_wrapping_division, trapping_form,
};
use abstract_operations::{IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy};
use semantic_vocabulary::IntegerType;
impl Checker<'_> {
    // Operation operands refer to established values; only the definition root
    // carries an operation. Never reconstruct an already-produced expression tree.
    pub(super) fn integer_source(
        &self,
        expression: &Expression,
        value: ValueId,
        aliases: &[(ValueId, ValueId)],
    ) -> bool {
        matches!(
            expression,
            Expression::Immediate { .. }
                | Expression::Parameter { .. }
                | Expression::ScalarHome(_)
                | Expression::BlockParameter(_)
        ) && self.expression(expression, value, aliases)
    }

    pub(super) fn boolean_source(
        &self,
        expression: &Boolean,
        value: ValueId,
        aliases: &[(ValueId, ValueId)],
    ) -> bool {
        matches!(
            expression,
            Boolean::Immediate { .. }
                | Boolean::Parameter { .. }
                | Boolean::ScalarHome(_)
                | Boolean::BlockParameter(_)
        ) && self.boolean(expression, value, aliases)
    }

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
            &self.function.graph.scalar_parameters
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
            Expression::ByteSequenceRead { psi_operation, source_value, source, view, index, length, obligation } => {
                *source_value == resolved
                    && self.byte_view(view, *source, aliases)
                    && self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                        AbstractOperation::ByteSequenceRead { psi_operation: operation, result, source: expected, index: expected_index, length: expected_length, obligation: expected_obligation }
                        if operation == psi_operation && result.value == resolved && expected == source
                        && length == expected_length && obligation == expected_obligation && self.integer_source(index, *expected_index, aliases)))
            }
            Expression::ByteSequenceLength { psi_operation, source_value, source, view, length_byte_offset } => {
                *length_byte_offset == if super::super::byte_views::reads_owner_layout(self.optimized, *source, self.types) { 0 } else { 8 } && *source_value == resolved
                    && (self.byte_view(view, *source, aliases) || self.mutable_byte_view(view, *source))
                    && self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                        AbstractOperation::ByteSequenceLength { psi_operation: operation, result, source: expected }
                        if operation == psi_operation && result.value == resolved && expected == source))
            }
            Expression::ElementViewRead { psi_operation, source_value, source, view, index, length, obligation, path, element_byte_offset } => {
                *source_value == resolved
                    && self.element_view(view, *source, aliases)
                    && self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                        AbstractOperation::ElementViewRead { psi_operation: operation, result, source: expected, index: expected_index, length: expected_length, obligation: expected_obligation, path: expected_path }
                        if operation == psi_operation && result.value == resolved && expected == source
                        && path == expected_path
                        && super::super::byte_views::element_type(self.optimized, *source, self.types)
                            .and_then(|element| crate::structural_inputs::structural_reference_input::element_leaf_geometry(
                                element, path, result.scalar_type, self.types))
                            .is_some_and(|(offset, _)| offset == *element_byte_offset)
                        && length == expected_length && obligation == expected_obligation && self.integer_source(index, *expected_index, aliases)))
            }
            Expression::ElementViewLength { psi_operation, source_value, source, view, length_byte_offset } => {
                *length_byte_offset == 8 && *source_value == resolved
                    && self.element_view(view, *source, aliases)
                    && self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                        AbstractOperation::ElementViewLength { psi_operation: operation, result, source: expected }
                        if operation == psi_operation && result.value == resolved && expected == source))
            }
            Expression::Immediate {source_value,value:literal} => *source_value == value && self.optimized.blocks.iter().flat_map(|block|&block.nodes).any(|node|
                matches!(&node.operation,AbstractOperation::IntegerConstant {result,value:actual,..} if *result == resolved && actual == literal)),
            Expression::Parameter {source_value,parameter_index,location} => {
                let Some(parameter) = self.scalar_parameters().get(*parameter_index) else {return false;};
                *source_value == value && parameter.value == resolved && location_matches(*location,&parameter.placement)
            }
            Expression::BitwiseAnd { psi_operation, left, right } => {
                self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                    AbstractOperation::IntegerBitwiseAnd { psi_operation: operation, result, left: source_left, right: source_right, .. }
                    if operation == psi_operation && *result == resolved
                        && self.integer_source(left, *source_left, aliases)
                        && self.integer_source(right, *source_right, aliases)))
            }
            Expression::BitwiseOr { psi_operation, left, right } => {
                self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                    AbstractOperation::IntegerBitwiseOr { psi_operation: operation, result, left: source_left, right: source_right, .. }
                    if operation == psi_operation && *result == resolved
                        && self.integer_source(left, *source_left, aliases)
                        && self.integer_source(right, *source_right, aliases)))
            }
            Expression::BitwiseNot { psi_operation, operand } => {
                self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                    AbstractOperation::IntegerBitwiseNot { psi_operation: operation, result, operand: source, .. }
                    if operation == psi_operation && *result == resolved
                        && self.integer_source(operand, *source, aliases)))
            }
            Expression::BitwiseXor { psi_operation, left, right } => {
                self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                    AbstractOperation::IntegerBitwiseXor { psi_operation: operation, result, left: source_left, right: source_right, .. }
                    if operation == psi_operation && *result == resolved
                        && self.integer_source(left, *source_left, aliases)
                        && self.integer_source(right, *source_right, aliases)))
            }
            // An exact pair keeps the first exact node of its operation and
            // must agree on operator and obligation. Every other pair names
            // one node with its exact pair, obligation and admitted carrier.
            Expression::Arithmetic { psi_operation, arithmetic, left, right } => {
                if arithmetic.policy() == IntegerOverflowPolicy::Exact {
                    let Some(node) = self.optimized.blocks.iter().flat_map(|block|&block.nodes).find(|node| matches!(&node.operation,
                        AbstractOperation::IntegerArithmetic {psi_operation:operation,arithmetic:source,..} if operation == psi_operation && source.policy() == IntegerOverflowPolicy::Exact)) else {return false;};
                    let AbstractOperation::IntegerArithmetic {result,arithmetic:source,left:source_left,right:source_right,..} = &node.operation else {return false;};
                    return source.operator() == arithmetic.operator() && *result == resolved && source.obligation() == arithmetic.obligation()
                        && self.integer_source(left,*source_left,aliases) && self.integer_source(right,*source_right,aliases);
                }
                self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                    AbstractOperation::IntegerArithmetic { psi_operation: operation, result, scalar_type, arithmetic: source, left: source_left, right: source_right }
                    if operation == psi_operation && source == arithmetic && *result == resolved
                        && arithmetic_carrier_admitted(*arithmetic, *scalar_type)
                        && self.integer_source(left, *source_left, aliases)
                        && self.integer_source(right, *source_right, aliases)))
            }
            Expression::Shift { psi_operation, policy, direction, count_type, value, count } => {
                self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                    AbstractOperation::IntegerShift { psi_operation: operation, result, count_type: expected_count_type, policy: source_policy, direction: source_direction, value: source_value, count: source_count, .. }
                    if operation == psi_operation && *result == resolved
                        && source_policy == policy && source_direction == direction
                        && expected_count_type == count_type
                        && self.integer_source(value, *source_value, aliases)
                        && self.integer_source(count, *source_count, aliases)))
            }
            // The Trapping root names the exact abstract primitive, both
            // carrier axes and every operand in evaluation order; a
            // conversion alone has no second operand.
            Expression::Trapping { psi_operation, primitive, operand_type, left, right } => {
                self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                    AbstractOperation::TrappingInteger { psi_operation: operation, result, scalar_type, operand_type: expected_operand_type, operation: source }
                    if operation == psi_operation && *result == resolved
                        && source.primitive() == *primitive
                        && expected_operand_type == operand_type
                        && trapping_form(*scalar_type, *operand_type, *primitive).is_some()
                        && match (source.operands().as_slice(), right) {
                            ([source_left], None) => self.integer_source(left, *source_left, aliases),
                            ([source_left, source_right], Some(right)) => {
                                self.integer_source(left, *source_left, aliases)
                                    && self.integer_source(right, *source_right, aliases)
                            }
                            _ => false,
                        }))
            }
            Expression::IntegerWiden { psi_operation, source_type, operand } => {
                self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                    AbstractOperation::IntegerWiden { psi_operation: operation, result, source_type: actual_type, operand: source, .. }
                    if operation == psi_operation && *result == resolved && actual_type == source_type && self.integer_source(operand, *source, aliases)))
            }
            Expression::IntegerExactCast { psi_operation, obligation, source_type, operand } => {
                self.optimized.blocks.iter().flat_map(|block| &block.nodes).any(|node| matches!(&node.operation,
                    AbstractOperation::IntegerExactCast { psi_operation: operation, obligation: expected_obligation, result, source_type: actual_type, operand: source, .. }
                    if operation == psi_operation && expected_obligation == obligation && *result == resolved && actual_type == source_type && self.integer_source(operand, *source, aliases)))
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
                    && self.boolean_source(left, *source_left, aliases)
                    && self.boolean_source(right, *source_right, aliases))
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
                if operation == psi_operation && *result == resolve(value, aliases) && self.boolean_source(operand, *source, aliases)));
        }
        if let Boolean::ByteSequencePredicate {
            psi_operation,
            source_value,
            operand,
            predicate,
        } = expression
        {
            return *source_value == resolve(value, aliases)
                && self
                    .optimized
                    .blocks
                    .iter()
                    .flat_map(|block| &block.nodes)
                    .any(|node| {
                        let AbstractOperation::ByteSequencePredicate {
                            psi_operation: operation,
                            result,
                            operand: expected_operand,
                            predicate: expected_predicate,
                        } = &node.operation
                        else {
                            return false;
                        };
                        operation == psi_operation
                            && result.value == resolve(value, aliases)
                            && expected_predicate == predicate
                            && operands_agree(operand, expected_operand, aliases)
                    });
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
        let (
            operation,
            source_left,
            source_right,
            scalar_type,
            right_scalar_type,
            left,
            right,
            target_operation,
        ) = match (&node.operation, expression) {
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
                    right_scalar_type,
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
                    right_scalar_type,
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
                    right_scalar_type,
                    left,
                    right,
                },
            ) => (
                *psi_operation,
                *source_left,
                *source_right,
                *scalar_type,
                *right_scalar_type,
                left,
                right,
                *target_operation,
            ),
            _ => return false,
        };
        operation == target_operation
            && value_type(self.optimized, source_left) == Some(ScalarType::Integer(scalar_type))
            && value_type(self.optimized, source_right)
                == Some(ScalarType::Integer(right_scalar_type))
            && self.integer_source(left, source_left, aliases)
            && self.integer_source(right, source_right, aliases)
    }
}

/// A verbatim-carried byte-sequence operand agrees with the node's own when
/// its length scalar is the same value modulo successor bindings.
fn operands_agree(
    expressed: &terminal_psi::control_flow::ByteSequenceOperand,
    expected: &terminal_psi::control_flow::ByteSequenceOperand,
    aliases: &[(ValueId, ValueId)],
) -> bool {
    use terminal_psi::control_flow::ByteSequenceOperand as Operand;
    match (expressed, expected) {
        (
            Operand::Whole {
                source: expressed_source,
                length: expressed_length,
            },
            Operand::Whole {
                source: expected_source,
                length: expected_length,
            },
        ) => {
            expressed_source == expected_source
                && resolve(*expressed_length, aliases) == *expected_length
        }
        (
            Operand::Field {
                source: expressed_source,
                field: expressed_field,
                length: expressed_length,
            },
            Operand::Field {
                source: expected_source,
                field: expected_field,
                length: expected_length,
            },
        ) => {
            expressed_source == expected_source
                && expressed_field == expected_field
                && resolve(*expressed_length, aliases) == *expected_length
        }
        _ => false,
    }
}

/// Saturating multiply, divide and remainder, and wrapping divide and
/// remainder, are realized only over the carriers their selections support.
fn arithmetic_carrier_admitted(arithmetic: IntegerArithmetic, scalar_type: IntegerType) -> bool {
    use IntegerArithmeticOperator as Operator;
    use IntegerOverflowPolicy as Policy;
    match (arithmetic.policy(), arithmetic.operator()) {
        (Policy::Saturating, Operator::Multiply | Operator::Divide | Operator::Remainder) => {
            saturating_carrier(scalar_type).is_some()
        }
        (Policy::Wrapping, Operator::Divide | Operator::Remainder) => {
            supports_wrapping_division(scalar_type)
        }
        _ => true,
    }
}
