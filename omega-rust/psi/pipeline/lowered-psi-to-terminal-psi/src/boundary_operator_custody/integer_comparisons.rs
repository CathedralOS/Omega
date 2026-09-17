//! Replay of the selected integer comparison roster: the integer counterpart
//! of `float_comparisons`, keyed by the same checked `operator_use` identity.

use super::{CheckedBoundaryOperatorApplicationOccurrence, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::types::PrimitiveType;
use lowered_psi::{LoweredPsi, LoweredSelectedIntegerComparisonOperation};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};

pub(super) fn replay(
    checked: &CheckedTrees,
    lowered: &LoweredPsi,
    occurrences: &mut Vec<CheckedBoundaryOperatorApplicationOccurrence>,
) -> Result<(), &'static str> {
    // Unlike `IeeeFloatCompare`, the three authored-order integer comparison
    // operations are also emitted for builtin comparisons, so the Terminal
    // roster cannot count which of them were selected: a builtin `==` and an
    // unrecorded selected one look the same from the artifact. Every recorded
    // occurrence must therefore rejoin one exact checked use, one exact
    // checked application and one exact Terminal operation; a crash-qualified
    // use that reached emission without a row is the producer's fail-closed
    // rejection (`retention/operation_crash_contracts.rs`), not this replay's.
    for comparison in &lowered.selected_integer_comparison_occurrences {
        if !checked
            .facts
            .operators
            .uses
            .is_valid(comparison.operator_use)
        {
            return unsupported("integer comparison lost its checked operator occurrence");
        }
        let operator_use = checked.facts.operators.uses.get(comparison.operator_use);
        let selected_meaning = checked
            .facts
            .operators
            .selected_integer_comparison(&checked.typed, comparison.operator_use)
            .and_then(|(operation, primitive)| {
                Some((
                    authored_order_operation(operation)?,
                    integer_scalar_type(primitive)?,
                ))
            });
        let expected = ScalarType::Integer(comparison.integer_type);
        if operator_use.application_site() != comparison.application_site
            || operator_use.selected_operator_symbol != comparison.requirement_operator
            || operator_use.provider_plan_commitment != comparison.provider_plan_commitment
            || comparison.provider_plan_commitment.is_empty()
            || operator_use.operands(&checked.typed).is_none()
            || selected_meaning != Some((comparison.comparison, expected))
        {
            return unsupported("integer comparison changed its exact selected application");
        }
        let mut matching_operations = 0;
        for machine in &lowered.semantic_module.machines {
            if machine.id != comparison.terminal_machine {
                continue;
            }
            for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
                let Some((left, right)) =
                    authored_order_operands(&operation.kind, comparison.comparison)
                else {
                    continue;
                };
                if operation.id != comparison.terminal_operation {
                    continue;
                }
                let values = machine
                    .parameters
                    .iter()
                    .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
                    .copied()
                    .chain(
                        machine
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .filter_map(|operation| operation.result.scalar()),
                    );
                if [left, right].into_iter().all(|operand| {
                    values
                        .clone()
                        .any(|value| value.id == operand && value.scalar_type == expected)
                }) {
                    matching_operations += 1;
                }
            }
        }
        if matching_operations != 1 {
            return unsupported("integer comparison does not name one exact Terminal operation");
        }
        let mut applications = checked
            .facts
            .operators
            .boundary_applications
            .iter()
            .enumerate()
            .filter(|(_, application)| {
                application.site == comparison.application_site
                    && application.requirement_symbol == comparison.requirement_operator
            });
        let application = applications.next();
        let (Some((application_index, _)), None) = (application, applications.next()) else {
            return unsupported("integer comparison does not rejoin one exact checked application");
        };
        occurrences.push(CheckedBoundaryOperatorApplicationOccurrence {
            application_index,
            terminal_operation: comparison.terminal_operation,
        });
    }
    Ok(())
}

/// The one Terminal operation whose operand order is the authored operand
/// order, mirroring the producer's admission: `!=`, `>` and `>=` record no
/// occurrence, so a row claiming one is foreign.
const fn authored_order_operation(
    operation: checked_trees::expression::BinaryOperator,
) -> Option<LoweredSelectedIntegerComparisonOperation> {
    use checked_trees::expression::BinaryOperator;
    match operation {
        BinaryOperator::Equal => Some(LoweredSelectedIntegerComparisonOperation::Equal),
        BinaryOperator::Less => Some(LoweredSelectedIntegerComparisonOperation::LessThan),
        BinaryOperator::LessOrEqual => Some(LoweredSelectedIntegerComparisonOperation::LessOrEqual),
        _ => None,
    }
}

const fn authored_order_operands(
    kind: &terminal_psi::OperationKind,
    comparison: LoweredSelectedIntegerComparisonOperation,
) -> Option<(semantic_vocabulary::ValueId, semantic_vocabulary::ValueId)> {
    use terminal_psi::OperationKind;
    match (kind, comparison) {
        (
            OperationKind::IntegerEqual { left, right },
            LoweredSelectedIntegerComparisonOperation::Equal,
        )
        | (
            OperationKind::IntegerLessThan { left, right },
            LoweredSelectedIntegerComparisonOperation::LessThan,
        )
        | (
            OperationKind::IntegerLessOrEqual { left, right },
            LoweredSelectedIntegerComparisonOperation::LessOrEqual,
        ) => Some((*left, *right)),
        _ => None,
    }
}

/// The Terminal scalar type of a checked integer primitive, the same mapping
/// the producer's `emission/scalar_types.rs` applies when it records the
/// occurrence, so a row whose `integer_type` drifted from its checked operand
/// type does not rejoin.
fn integer_scalar_type(primitive: PrimitiveType) -> Option<ScalarType> {
    let (sign, bits) = match primitive {
        PrimitiveType::Addr => return IntegerType::address(64).ok().map(ScalarType::Integer),
        PrimitiveType::I8 => (IntegerSign::Signed, 8),
        PrimitiveType::I16 => (IntegerSign::Signed, 16),
        PrimitiveType::I32 => (IntegerSign::Signed, 32),
        PrimitiveType::I64 => (IntegerSign::Signed, 64),
        PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
        PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
        PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
        PrimitiveType::U64 => (IntegerSign::Unsigned, 64),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => return None,
    };
    IntegerType::new(sign, bits).ok().map(ScalarType::Integer)
}
