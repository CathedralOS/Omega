//! Ordered source projection; target trees are checked, never used as the program.
use super::super::shared::*;
use super::{DerivedValue, LeafContext};
use crate::legalization::integer_sequence_input;
use legalized_operations::{
    LegalizedExactIntegerBinary, LegalizedExactIntegerOperator as Operator,
    LegalizedExactIntegerSequence, LegalizedIntegerStep,
};

pub(super) fn derive<'a>(
    context: &LeafContext<'a>,
    expression: &TargetIntegerExpression,
) -> Result<DerivedValue<'a>, LegalizationError> {
    if !integer_sequence_input::validate(
        expression,
        context.source_value,
        context.nodes,
        &context.optimized.parameters,
    ) {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let (returned, nodes) = context
        .nodes
        .split_last()
        .ok_or(Error::UnsupportedSourceShape {
            function: context.function,
        })?;
    let mut steps = Vec::new();
    for node in nodes {
        let site = node.definitions[0].site;
        let step = match &node.operation {
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                value,
                ..
            } => LegalizedIntegerStep::Immediate(SourceImmediate {
                source_value: *result,
                value: *value,
                constant_operation: *psi_operation,
                definition_site: site,
                fuel: super::fuel::exact_operation_fuel(node, *psi_operation, context.function)?,
            }),
            AbstractOperation::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                left,
                right,
                ..
            }
            | AbstractOperation::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                left,
                right,
                ..
            } => {
                let fact = context
                    .accepted_obligation_facts
                    .iter()
                    .find(|fact| {
                        fact.machine == context.optimized.machine
                            && fact.operation == *psi_operation
                            && fact.obligation == *obligation
                    })
                    .ok_or(Error::SourceCustodyMismatch)?;
                if !context.optimized.facts.iter().any(|fact| matches!(fact,
                    OptimizationFact::OperationObligationReference { obligation: referenced, support }
                    if referenced == obligation && support == psi_operation)) { return Err(Error::SourceCustodyMismatch); }
                LegalizedIntegerStep::ExactBinary(LegalizedExactIntegerBinary {
                    operator: if matches!(node.operation, AbstractOperation::ExactIntegerAdd { .. })
                    {
                        Operator::Add
                    } else {
                        Operator::Subtract
                    },
                    source_value: *result,
                    obligation: *obligation,
                    accepted_fact: fact.identity,
                    operation: *psi_operation,
                    definition_site: site,
                    fuel: super::fuel::exact_operation_fuel(
                        node,
                        *psi_operation,
                        context.function,
                    )?,
                    left: *left,
                    right: *right,
                })
            }
            _ => {
                return Err(Error::UnsupportedSourceShape {
                    function: context.function,
                });
            }
        };
        steps.push(step);
    }
    Ok((
        returned,
        SourceLeafValue::ExactIntegerSequence(LegalizedExactIntegerSequence { steps }),
    ))
}
