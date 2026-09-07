//! Exact integer actuals admitted to mathematical call-requirement substitution.
//!
//! This is meaning custody, not an overflow proof. Normal source formation and
//! Terminal operation obligations still independently discharge Exact arithmetic.

use super::*;
use numerics::arithmetic::ArithmeticDomain;

pub(super) fn primitive_type(
    program: &TypedTrees,
    facts: &CheckFacts,
    owner: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<PrimitiveType> {
    meaning(program, facts, owner, parameters, expression).map(|meaning| meaning.primitive)
}

/// A numeric actual is evaluated before later operands. Current call-entry
/// atoms represent that captured value only if those operands preserve it.
pub(super) fn capture_is_current<'program>(
    program: &'program TypedTrees,
    facts: &CheckFacts,
    machine: &'program typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    argument: ExpressionHandle,
    later_arguments: &[ExpressionHandle],
    frames: Option<&validation::CallFrameResolver<'program>>,
) -> bool {
    let parameters = program.state_parameters(state);
    let mut occurrences = Vec::new();
    crate::contract_occurrences::append_expression_occurrences(program, argument, &mut occurrences);
    let captured = occurrences
        .iter()
        .filter_map(|expression| {
            direct_parameter(program, parameters, *expression)
                .filter(|parameter| parameter.is_mutable)
                .map(|parameter| facts::PlaceRoot::Symbol(parameter.symbol))
        })
        .collect::<Vec<_>>();
    if captured.is_empty() || later_arguments.is_empty() {
        return true;
    }
    let Some(frames) = frames else {
        return false;
    };
    later_arguments.iter().all(|expression| {
        if !operand_effects_are_known(program, facts, machine.symbol, parameters, *expression) {
            return false;
        }
        let frame = frames.expression_write_frame(machine, *expression);
        crate::flow::frame_storage_writes(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            &frame,
        )
        .is_some_and(|writes| {
            writes.iter().all(|write| {
                let root = crate::flow::normalized_event_place_root(program, write.root);
                !captured.iter().any(|captured| {
                    root == crate::flow::normalized_event_place_root(program, *captured)
                })
            })
        })
    })
}

/// Call write frames do not describe implicit selected-operator effects.
/// Admit ordinary calls and independently builtin scalar operands only;
/// other forms need their own complete expression-effect interpretation.
fn operand_effects_are_known(
    program: &TypedTrees,
    facts: &CheckFacts,
    owner: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::String(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::ZeroValue(_) => true,
        ExpressionNode::Borrow(borrow) => {
            operand_effects_are_known(program, facts, owner, parameters, borrow.target)
        }
        ExpressionNode::Call(call) => {
            (!call.receiver.is_valid()
                || operand_effects_are_known(program, facts, owner, parameters, call.receiver))
                && program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .all(|argument| {
                        operand_effects_are_known(program, facts, owner, parameters, *argument)
                    })
        }
        ExpressionNode::Binary(_) => {
            primitive_type(program, facts, owner, parameters, expression).is_some()
                || comparison_is_supported(program, facts, owner, parameters, expression)
        }
        _ => false,
    }
}

struct Meaning {
    primitive: PrimitiveType,
    domain: ArithmeticDomain,
    type_reference: Option<TypeReferenceHandle>,
}

fn meaning(
    program: &TypedTrees,
    facts: &CheckFacts,
    owner: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<Meaning> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => {
            let parameter = direct_parameter(program, parameters, expression)?;
            Some(Meaning {
                primitive: fixed_parameter_type(program, parameter)?,
                domain: program.arithmetic_domain_for_type_reference(parameter.type_reference),
                type_reference: Some(parameter.type_reference),
            })
        }
        ExpressionNode::Integer(literal) => {
            let primitive = crate::values::scalar_expression_type(
                &checked_trees::CheckedScalarExpression::IntegerLiteral {
                    literal: literal.clone(),
                },
            )?;
            fixed_integer(primitive).then_some(Meaning {
                primitive,
                domain: literal.landing()?.domain,
                type_reference: None,
            })
        }
        ExpressionNode::Binary(binary) => {
            let spelling = match binary.operator {
                BinaryOperator::Add => OperatorSpelling::Add,
                BinaryOperator::Subtract => OperatorSpelling::Subtract,
                BinaryOperator::Multiply => OperatorSpelling::Multiply,
                _ => return None,
            };
            let left = meaning(program, facts, owner, parameters, binary.left);
            let right = meaning(program, facts, owner, parameters, binary.right);
            // Anonymous integer literals take the other operand's carrier;
            // failed normalization of an arbitrary expression never does.
            let anonymous = |primitive| Meaning {
                primitive,
                domain: ArithmeticDomain::Exact,
                type_reference: None,
            };
            let (left, right) = match (left, right) {
                (Some(left), Some(right)) => (left, right),
                (Some(left), None) if is_unlanded_integer(program, binary.right) => {
                    let right = anonymous(left.primitive);
                    (left, right)
                }
                (None, Some(right)) if is_unlanded_integer(program, binary.left) => {
                    let left = anonymous(right.primitive);
                    (left, right)
                }
                _ => return None,
            };
            if left.primitive != right.primitive
                || left.domain != ArithmeticDomain::Exact
                || right.domain != ArithmeticDomain::Exact
                // A missing checked row is not builtin authority: the typed
                // query below independently joins declared/trait meanings and
                // the retained authored selection. Existing checked rows veto
                // that reading when they selected anything non-builtin.
                // A compound operand has a plain builtin result, not a retained
                // source parameter's constrained/custom type reference.
                || !crate::checks::contracts::prover::has_builtin_operators(
                    program,
                    &facts.operators,
                    expression,
                )
                || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    owner,
                    expression,
                    spelling,
                    &[left.type_reference, right.type_reference],
                )
            {
                return None;
            }
            Some(Meaning {
                primitive: left.primitive,
                domain: ArithmeticDomain::Exact,
                type_reference: None,
            })
        }
        _ => None,
    }
}

pub(super) fn is_unlanded_integer(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    program.expression_table.expression_is_valid(expression)
        && matches!(program.expression_table.expression(expression),
            ExpressionNode::Integer(literal) if literal.landing().is_none())
}
