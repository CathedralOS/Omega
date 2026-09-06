use super::super::shared::*;
use super::fuel::exact_operation_fuel;
use super::immediate::derive_operand;
use super::{DerivedValue, LeafContext};

pub(super) fn derive_add<'a>(
    context: &LeafContext<'a>,
    widen_operation: OperationId,
    source_type: semantic_vocabulary::IntegerType,
    operand: &TargetIntegerExpression,
) -> Result<DerivedValue<'a>, LegalizationError> {
    derive(context, widen_operation, source_type, operand, false)
}

pub(super) fn derive_subtract<'a>(
    context: &LeafContext<'a>,
    widen_operation: OperationId,
    source_type: semantic_vocabulary::IntegerType,
    operand: &TargetIntegerExpression,
) -> Result<DerivedValue<'a>, LegalizationError> {
    derive(context, widen_operation, source_type, operand, true)
}

fn derive<'a>(
    context: &LeafContext<'a>,
    widen_operation: OperationId,
    source_type: semantic_vocabulary::IntegerType,
    operand: &TargetIntegerExpression,
    is_subtract: bool,
) -> Result<DerivedValue<'a>, LegalizationError> {
    let (arithmetic_operation, obligation, target_left, target_right) = match (operand, is_subtract)
    {
        (
            TargetIntegerExpression::ExactAdd {
                psi_operation,
                obligation,
                left,
                right,
            },
            false,
        )
        | (
            TargetIntegerExpression::ExactSubtract {
                psi_operation,
                obligation,
                left,
                right,
            },
            true,
        ) => (*psi_operation, *obligation, left, right),
        _ => unreachable!("widened catalog arm supplied its exact binary operand"),
    };
    let u8_integer_type =
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let u8_type = ScalarType::Integer(u8_integer_type);
    if context.nodes.len() != 5 || source_type != u8_integer_type {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let left = derive_operand(
        context.function,
        context.arm_edge,
        target_left,
        &context.nodes[0],
        u8_type,
    )?;
    let right = derive_operand(
        context.function,
        context.arm_edge,
        target_right,
        &context.nodes[1],
        u8_type,
    )?;
    let (
        abstract_operation,
        abstract_obligation,
        narrow_result,
        arithmetic_type,
        abstract_left,
        abstract_right,
    ) = match (&context.nodes[2].operation, is_subtract) {
        (
            AbstractOperation::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            false,
        )
        | (
            AbstractOperation::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            true,
        ) => (psi_operation, obligation, result, scalar_type, left, right),
        _ => {
            return Err(Error::UnsupportedSourceShape {
                function: context.function,
            });
        }
    };
    if *abstract_operation != arithmetic_operation
        || *abstract_obligation != obligation
        || *arithmetic_type != u8_integer_type
        || *abstract_left != left.source_value
        || *abstract_right != right.source_value
        || context.nodes[2].definitions.len() != 1
        || context.nodes[2].definitions[0].value != *narrow_result
        || context.nodes[2].provenance != vec![PsiProvenance::Operation(arithmetic_operation)]
    {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let AbstractOperation::IntegerWiden {
        psi_operation: abstract_widen_operation,
        result: widened_result,
        source_type: abstract_source_type,
        target_type: abstract_target_type,
        operand: abstract_operand,
    } = &context.nodes[3].operation
    else {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    };
    if *abstract_widen_operation != widen_operation
        || *widened_result != context.source_value
        || *narrow_result == context.source_value
        || *abstract_source_type != u8_integer_type
        || *abstract_target_type != context.u64_integer_type
        || *abstract_operand != *narrow_result
        || context.nodes[3].definitions.len() != 1
        || context.nodes[3].definitions[0].value != context.source_value
        || context.nodes[3].provenance != vec![PsiProvenance::Operation(widen_operation)]
    {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }

    validate_values(
        context,
        is_subtract,
        u8_integer_type,
        left.value,
        right.value,
    )?;
    let Some(accepted_fact) = context.accepted_obligation_facts.iter().find(|fact| {
        fact.machine == context.optimized.machine
            && fact.operation == arithmetic_operation
            && fact.obligation == obligation
    }) else {
        return Err(Error::SourceCustodyMismatch);
    };
    if !context.optimized.facts.iter().any(|fact| {
        matches!(fact, OptimizationFact::OperationObligationReference { obligation: referenced_obligation, support } if *referenced_obligation == obligation && *support == arithmetic_operation)
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    let arithmetic_fuel =
        exact_operation_fuel(&context.nodes[2], arithmetic_operation, context.function)?;
    let widen_fuel = exact_operation_fuel(&context.nodes[3], widen_operation, context.function)?;
    let common = (
        obligation,
        accepted_fact.identity,
        *narrow_result,
        arithmetic_fuel,
        widen_fuel,
        left,
        right,
    );
    let value = if is_subtract {
        SourceLeafValue::WidenedExactSubtract {
            source_type: u8_integer_type,
            target_type: context.u64_integer_type,
            theorem: LegalizationTheorem::UnsignedExactSubtractCommutesWithWidenV1,
            obligation: common.0,
            accepted_fact: common.1,
            subtract_operation: arithmetic_operation,
            narrow_result: common.2,
            subtract_definition_site: context.nodes[2].definitions[0].site,
            subtract_fuel: common.3,
            widen_operation,
            widen_definition_site: context.nodes[3].definitions[0].site,
            widen_fuel: common.4,
            left_temporary: context.temporaries[0],
            right_temporary: context.temporaries[1],
            left: common.5,
            right: common.6,
        }
    } else {
        SourceLeafValue::WidenedExactAdd {
            source_type: u8_integer_type,
            target_type: context.u64_integer_type,
            theorem: LegalizationTheorem::UnsignedExactAddCommutesWithWidenV1,
            obligation: common.0,
            accepted_fact: common.1,
            add_operation: arithmetic_operation,
            narrow_result: common.2,
            add_definition_site: context.nodes[2].definitions[0].site,
            add_fuel: common.3,
            widen_operation,
            widen_definition_site: context.nodes[3].definitions[0].site,
            widen_fuel: common.4,
            left_temporary: context.temporaries[0],
            right_temporary: context.temporaries[1],
            left: common.5,
            right: common.6,
        }
    };
    Ok((&context.nodes[4], value))
}

fn validate_values(
    context: &LeafContext<'_>,
    is_subtract: bool,
    source_type: semantic_vocabulary::IntegerType,
    left: semantic_vocabulary::IntegerValue,
    right: semantic_vocabulary::IntegerValue,
) -> Result<(), LegalizationError> {
    let narrow = if is_subtract {
        source_type.exact_sub(left, right)
    } else {
        source_type.exact_add(left, right)
    };
    let Some(narrow) = narrow else {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    };
    let Some(widened) = source_type.widen_value_to(context.u64_integer_type, narrow) else {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    };
    let Some(widened_left) = source_type.widen_value_to(context.u64_integer_type, left) else {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    };
    let Some(widened_right) = source_type.widen_value_to(context.u64_integer_type, right) else {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    };
    let recomputed = if is_subtract {
        context
            .u64_integer_type
            .exact_sub(widened_left, widened_right)
    } else {
        context
            .u64_integer_type
            .exact_add(widened_left, widened_right)
    };
    if recomputed != Some(widened) {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    Ok(())
}
pub(super) fn derive_expression<'a>(
    context: &LeafContext<'a>,
    expression: &TargetIntegerExpression,
) -> Result<DerivedValue<'a>, LegalizationError> {
    let TargetIntegerExpression::IntegerWiden {
        psi_operation,
        source_type,
        operand,
    } = expression
    else {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    };
    match operand.as_ref() {
        TargetIntegerExpression::ExactAdd { .. } => {
            derive_add(context, *psi_operation, *source_type, operand)
        }
        TargetIntegerExpression::ExactSubtract { .. } => {
            derive_subtract(context, *psi_operation, *source_type, operand)
        }
        _ => Err(Error::UnsupportedSourceShape {
            function: context.function,
        }),
    }
}
