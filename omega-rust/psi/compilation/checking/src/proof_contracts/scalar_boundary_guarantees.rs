//! Scalar consequences of authenticated boundary normal-return contracts.
//!
//! A meaning equation can publish a weaker runtime predicate without pretending
//! that a policy adapter is an arithmetic producer. Only sealed mathematical
//! applications with exact result and formal custody contribute such facts.

use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedFloatProjectionSource as Source,
    CheckedFloatSemanticApplicationOperand as Operand, CheckedIeeeFloatComparisonKind as Compare,
    CheckedScalarExpression as Scalar, ClosedScalarContractValue,
};
use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue};
use typed_trees::{TypedTrees, machine::Machine, types::PrimitiveType};

pub fn derive_scalar_boundary_guarantees(
    program: &TypedTrees,
    facts: &checked_trees::CheckFacts,
    machine: &Machine,
    contract: &checked_trees::MachineContractPlan,
) -> Vec<ClosedScalarContractValue> {
    contract
        .closed_scalar_values
        .ensures()
        .iter()
        .flatten()
        .filter_map(|clause| {
            match clause {
                ClosedScalarContractValue::Predicate(_) => Some(clause.clone()),
                ClosedScalarContractValue::FloatMeaningEquality { expression, .. } => {
                    meaning_consequence(program, facts, machine, *expression)
                        .map(ClosedScalarContractValue::Predicate)
                }
                // Content and other proof-only contracts have separate owners. A
                // consequence this runtime vocabulary cannot express grants no fact.
                _ => None,
            }
        })
        .collect()
}

fn meaning_consequence(
    program: &TypedTrees,
    facts: &checked_trees::CheckFacts,
    machine: &Machine,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<Boolean> {
    let mut equalities = facts
        .proof
        .float_meaning_equalities
        .iter()
        .filter(|row| row.use_site.is_none() && row.source_expression == expression);
    let equality = equalities.next()?;
    if equalities.next().is_some() {
        return None;
    }
    let projections = &facts.proof.float_meaning_projections;
    let projection = |id| projections.iter().find(|row| row.result.id == id);
    let (result, application) = [
        (equality.left, equality.right),
        (equality.right, equality.left),
    ]
    .into_iter()
    .find_map(|(result, application)| {
        let Source::DirectMachineResult(result) = projection(result)?.source else {
            return None;
        };
        if result.owner_machine != machine.symbol {
            return None;
        }
        let application = facts
            .proof
            .float_semantic_applications
            .iter()
            .find(|row| row.result == application)?;
        Some((result, application))
    })?;
    application.validate().ok()?;
    if !source_equation_matches(program, facts, machine, expression, application) {
        return None;
    }
    let row = numerics::float_semantics_catalog::FloatSemanticOperation::for_contract_identity(
        &application.contract,
    )?;
    // These are names in the sealed mathematical catalog after complete
    // identity replay, not the runtime callee's namespace or spelling.
    let division = match row.name {
        "add" | "subtract" | "multiply" => false,
        "divide" => true,
        _ => return None,
    };
    let primitive_type = match application.format {
        IeeeFloatFormat::Binary32 => PrimitiveType::F32,
        IeeeFloatFormat::Binary64 => PrimitiveType::F64,
    };
    if result.fallback.primitive != primitive_type {
        return None;
    }
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if program.primitive_type_reference(state.return_type) != Some(primitive_type)
        || !language_core::signature_reserves_result(
            program
                .state_parameters(state)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .map(|parameter| program.symbols.name(parameter.symbol)),
        )
    {
        return None;
    }
    let parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .collect::<Vec<_>>();
    let [
        Operand::Format(format),
        Operand::Meaning(left),
        Operand::Meaning(right),
    ] = application.operands.as_slice()
    else {
        return None;
    };
    if *format != application.format {
        return None;
    }
    let formal = |id| {
        let Source::DirectMachineParameter(source) = projection(id)?.source else {
            return None;
        };
        if source.owner_machine != machine.symbol || source.fallback.primitive != primitive_type {
            return None;
        }
        let position = parameters.iter().position(|parameter| {
            parameter.symbol == source.parameter
                && program.primitive_type_reference(parameter.type_reference)
                    == Some(primitive_type)
        })?;
        Some(Scalar::Parameter {
            position,
            primitive_type,
        })
    };
    let left = formal(*left)?;
    let right = formal(*right)?;
    let result = Scalar::Parameter {
        position: parameters.len(),
        primitive_type,
    };
    let mut condition = and(
        finite(left, application.format),
        finite(right.clone(), application.format),
    );
    if division {
        let zero = Scalar::IeeeFloatLiteral {
            value: match application.format {
                IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0),
                IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(0),
            },
        };
        condition = and(
            condition,
            Boolean::Not(Box::new(compare(Compare::Equal, right, zero))),
        );
    }
    Some(Boolean::Or {
        left: Box::new(Boolean::Not(Box::new(condition))),
        right: Box::new(compare(Compare::Equal, result.clone(), result)),
    })
}

/// Rejoin the retained proof coordinates to this declaration's authored
/// equation. Changing both the retained consequence and its plan must not
/// replace the mathematical producer, operand order, or result source.
fn source_equation_matches(
    program: &TypedTrees,
    facts: &checked_trees::CheckFacts,
    machine: &Machine,
    expression: typed_trees::expression::ExpressionHandle,
    application: &checked_trees::CheckedFloatSemanticApplication,
) -> bool {
    use typed_trees::expression::{BinaryOperator, ExpressionNode};
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    if binary.operator != BinaryOperator::Equal {
        return false;
    }
    let projection_source = |expression, expected: Option<symbols::SymbolHandle>| {
        let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
            return false;
        };
        let Some((_, primitive, _, _)) =
            super::float_projection_invocations::exact_projection_operation(program, call)
        else {
            return false;
        };
        if primitive
            != match application.format {
                IeeeFloatFormat::Binary32 => PrimitiveType::F32,
                IeeeFloatFormat::Binary64 => PrimitiveType::F64,
            }
        {
            return false;
        }
        let [operand] = program.expression_table.expression_handles(call.arguments) else {
            return false;
        };
        let ExpressionNode::Name(path) = program.expression_table.expression(*operand) else {
            return false;
        };
        match expected {
            Some(symbol) => path.symbol == symbol,
            None => matches!(program.expression_table.name_path_members(path.members), [name]
                if name.as_str() == "result"),
        }
    };
    [(binary.left, binary.right), (binary.right, binary.left)]
        .into_iter()
        .any(|(result, meaning)| {
            if !projection_source(result, None) {
                return false;
            }
            let ExpressionNode::Call(call) = program.expression_table.expression(meaning) else {
                return false;
            };
            let Some(operator) =
                typed_trees::operator::resolve_named_expression_call(program, call)
            else {
                return false;
            };
            let Some((_, contract)) =
                crate::exact_toolchain_float_semantic_contract(program, operator)
            else {
                return false;
            };
            if contract != application.contract {
                return false;
            }
            let arguments = program.expression_table.expression_handles(call.arguments);
            if arguments.len() != application.operands.len() {
                return false;
            }
            arguments
                .iter()
                .zip(&application.operands)
                .all(|(expression, operand)| match operand {
                    Operand::Format(format) => {
                        crate::exact_toolchain_float_format_const(program, *expression)
                            == Some(*format)
                    }
                    Operand::Meaning(value) => {
                        let Some(projection) = facts
                            .proof
                            .float_meaning_projections
                            .iter()
                            .find(|projection| projection.result.id == *value)
                        else {
                            return false;
                        };
                        let Source::DirectMachineParameter(source) = projection.source else {
                            return false;
                        };
                        source.owner_machine == machine.symbol
                            && projection_source(*expression, Some(source.parameter))
                    }
                })
        })
}

fn compare(kind: Compare, left: Scalar, right: Scalar) -> Boolean {
    Boolean::ScalarIeeeFloatComparison {
        kind,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn and(left: Boolean, right: Boolean) -> Boolean {
    Boolean::And {
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn finite(value: Scalar, format: IeeeFloatFormat) -> Boolean {
    let (minimum, maximum) = match format {
        IeeeFloatFormat::Binary32 => (
            IeeeFloatValue::Binary32(0xff7f_ffff),
            IeeeFloatValue::Binary32(0x7f7f_ffff),
        ),
        IeeeFloatFormat::Binary64 => (
            IeeeFloatValue::Binary64(0xffef_ffff_ffff_ffff),
            IeeeFloatValue::Binary64(0x7fef_ffff_ffff_ffff),
        ),
    };
    and(
        compare(
            Compare::LessOrEqual,
            Scalar::IeeeFloatLiteral { value: minimum },
            value.clone(),
        ),
        compare(
            Compare::LessOrEqual,
            value,
            Scalar::IeeeFloatLiteral { value: maximum },
        ),
    )
}
