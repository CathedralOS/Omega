//! Validated source joins for proof-only float-meaning projections.
//!
//! These facts are deliberately transient: source handles identify the exact
//! typed invocation and its landed operand only until checked plans replace
//! them with dense, plan-local identities.

use diagnostics::Diagnostic;
use numerics::float_projection::FloatProjectionContractIdentity;
use numerics::float_projection::FloatProjectionOperation;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
};
use typed_trees::operator::{resolve_named_call, resolve_named_expression_call};
use typed_trees::types::PrimitiveType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedFloatMeaningProjectionInvocation {
    pub invocation: ExpressionHandle,
    pub source: ExpressionHandle,
    pub selected_operator_symbol: SymbolHandle,
    pub source_primitive: PrimitiveType,
    pub operation: FloatProjectionOperation,
    pub contract: FloatProjectionContractIdentity,
}

/// One validated proof-position equality between two exact projection calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedFloatMeaningEqualityProposition {
    pub expression: ExpressionHandle,
    pub left: ExpressionHandle,
    pub right: ExpressionHandle,
}

pub fn exact_projection_operation(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<(
    SymbolHandle,
    PrimitiveType,
    FloatProjectionOperation,
    FloatProjectionContractIdentity,
)> {
    let operator = resolve_projection_call(program, call)?;
    let [namespace, name] = program.operator_path_members(operator.name) else {
        return None;
    };
    let operation =
        FloatProjectionOperation::from_source_identity(namespace.as_str(), name.as_str())?;
    let (primitive, contract) =
        crate::proof_contracts::float_projection_bindings::exact_toolchain_float_projection_contract(
            program, operator, operation,
        )?;
    Some((operator.symbol, primitive, operation, contract))
}

fn requested_projection_operation(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<FloatProjectionOperation> {
    let path = program.expression_table.name_path_node(call.receiver)?;
    let [namespace] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    FloatProjectionOperation::from_source_identity(namespace.as_str(), call.target.as_str())
}

fn resolve_projection_call<'program>(
    program: &'program dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<&'program typed_trees::operator::OperatorDefinition> {
    resolve_named_expression_call(program, call)
        .or_else(|| float_static_call_resolution(program, call))
}

/// The `Float` static-namespace spelling of one expression call: calls like
/// `Float::sqrt(x)` retain no receiver expression resolution, so selection
/// falls back to the ambiguity-checked named path under the literal Float
/// namespace. Shared with checked-tree replay so both stages resolve the
/// identical operator.
pub fn float_static_call_resolution<'program>(
    program: &'program dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<&'program typed_trees::operator::OperatorDefinition> {
    let path = program.expression_table.name_path_node(call.receiver)?;
    let [namespace] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if namespace.as_str() != "Float" {
        return None;
    }
    let static_receiver = [namespace.as_str()];
    resolve_named_call(
        program,
        call.target_symbol,
        Some(&static_receiver),
        call.target.as_str(),
        program
            .expression_table
            .expression_handles(call.arguments)
            .len(),
        false,
    )
}

fn walk_expression(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    visited: &mut Vec<ExpressionHandle>,
    projections: &mut Vec<ValidatedFloatMeaningProjectionInvocation>,
    equalities: &mut Vec<ValidatedFloatMeaningEqualityProposition>,
) -> Result<(), Diagnostic> {
    if !expression.is_valid() || visited.contains(&expression) {
        return Ok(());
    }
    visited.push(expression);
    let node = program.expression_table.expression(expression);
    match node {
        // An equality joins the projections its operands already recorded, so a
        // binary's subtrees are walked before the join is proposed.
        ExpressionNode::Binary(binary) => {
            walk_expression(program, binary.left, visited, projections, equalities)?;
            walk_expression(program, binary.right, visited, projections, equalities)?;
            let float_meaning_operands = (|| {
                let left = projections
                    .iter()
                    .find(|projection| projection.invocation == binary.left)?;
                let right = projections
                    .iter()
                    .find(|projection| projection.invocation == binary.right)?;
                Some((left, right))
            })();
            if binary.operator == BinaryOperator::Equal
                && let Some((left, right)) = float_meaning_operands
            {
                if left.operation != right.operation || left.contract != right.contract {
                    return Err(Diagnostic::error(
                        "FloatMeaningEqual requires two projections with the same exact format and projection contract",
                    ));
                }
                equalities.push(ValidatedFloatMeaningEqualityProposition {
                    expression,
                    left: binary.left,
                    right: binary.right,
                });
            }
            return Ok(());
        }
        // A projection call is retained after its own operands, so its receiver
        // and arguments are walked before the join it contributes.
        ExpressionNode::Call(call) => {
            walk_expression(program, call.receiver, visited, projections, equalities)?;
            for argument in program.expression_table.expression_handles(call.arguments) {
                walk_expression(program, *argument, visited, projections, equalities)?;
            }
            if let Some(requested_operation) = requested_projection_operation(program, call) {
                let Some((selected_operator_symbol, source_primitive, operation, contract)) =
                    exact_projection_operation(program, call)
                else {
                    return Err(Diagnostic::error(
                        "proof-only float-meaning projection call did not resolve one exact canonical operator signature",
                    ));
                };
                if operation != requested_operation {
                    return Err(Diagnostic::error(
                        "proof-only float-meaning projection call resolved a different canonical operation",
                    ));
                }
                let [source] = program.expression_table.expression_handles(call.arguments) else {
                    return Err(Diagnostic::error(
                        "validated float-meaning projection did not retain one source operand",
                    ));
                };
                let operator = resolve_projection_call(program, call)
                    .expect("exact projection operation resolved its operator");
                let [parameter] = program.operator_parameters(operator) else {
                    unreachable!("exact projection operation checked one parameter");
                };
                if !crate::value_custody::expression_types::argument_matches_type_reference_handle(
                    program,
                    *source,
                    parameter.type_reference,
                ) {
                    return Err(Diagnostic::error(
                        "validated float-meaning projection source no longer matches its exact format",
                    ));
                }
                projections.push(ValidatedFloatMeaningProjectionInvocation {
                    invocation: expression,
                    source: *source,
                    selected_operator_symbol,
                    source_primitive,
                    operation,
                    contract,
                });
            }
            return Ok(());
        }
        _ => {}
    }
    // Every other node contributes only its operand edges: case arm bodies,
    // array elements, record fields, range endpoints, a folded carrier and the
    // structural wrappers. The shared roster owns that list.
    let mut failure = None;
    crate::expression_traversal::children(program, node, |child| {
        if failure.is_none()
            && let Err(diagnostic) =
                walk_expression(program, child, visited, projections, equalities)
        {
            failure = Some(diagnostic);
        }
    });
    match failure {
        Some(diagnostic) => Err(diagnostic),
        None => Ok(()),
    }
}

pub(crate) fn collect_float_meaning_projection_invocations(
    program: &dyn crate::CheckingInput,
) -> Result<
    (
        Vec<ValidatedFloatMeaningProjectionInvocation>,
        Vec<ValidatedFloatMeaningEqualityProposition>,
    ),
    Vec<Diagnostic>,
> {
    let mut visited = Vec::new();
    let mut projections = Vec::new();
    let mut equalities = Vec::new();
    for (_, fact) in typed_trees::references::program_proof_facts(program) {
        let roots: &[ExpressionHandle] = match fact {
            ProofFact::Expression(expression) => std::slice::from_ref(expression),
            ProofFact::Membership(membership) => std::slice::from_ref(&membership.value),
            ProofFact::Proposition(application) => program
                .expression_table
                .expression_handles(application.arguments),
        };
        for root in roots {
            walk_expression(
                program,
                *root,
                &mut visited,
                &mut projections,
                &mut equalities,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        }
    }
    projections.sort_by_key(|projection| projection.invocation.arena_index());
    equalities.sort_by_key(|proposition| proposition.expression.arena_index());
    Ok((projections, equalities))
}
