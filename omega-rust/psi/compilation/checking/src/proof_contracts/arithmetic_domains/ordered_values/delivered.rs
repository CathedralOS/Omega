//! Orderings a delivered integer keeps from the expression that produced it.
//!
//! A proved `ensures` compares `result` with the callee's immutable formals,
//! including a field of an immutable formal. Substituting this call's actual
//! arguments makes that comparison a fact about the call value. The primitive
//! value-call gate does not admit a field read, so this identity is not that
//! denotational equality: it names one occurrence while its `ensures` is
//! applied, and a stored fact keeps the destination instead of the call. A
//! match delivers only an ordering every arm's value has. The caller does not
//! replay the callee body or an arm. A mutable formal is not the caller's
//! value after return.

use super::super::dispatch_arm_environments;
use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, Machine, Operand, Relation,
    RelationConstraint, State, ValueEnvironment, congruence, operand,
};
use crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning;
use crate::value_custody::places::declared_member_field_symbol;
use typed_trees::signature::{SignatureContractKind, state_parameter_position};

/// Orderings of `destination` implied by initializing it from `expression`.
pub(crate) fn delivered_value_orderings(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    destination: &Operand,
    written: &[String],
    environment: &ValueEnvironment,
) -> Vec<Relation> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(_) => match_join_orderings(
            program,
            machine,
            state,
            expression,
            destination,
            written,
            environment,
        ),
        ExpressionNode::Call(_) => {
            rebound_call_orderings(program, machine, state, expression, destination)
        }
        _ => Vec::new(),
    }
}

/// `ensures` orderings of this call value, in the caller's operand space.
pub(super) fn call_result_orderings(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Vec<Relation> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return Vec::new();
    };
    let Some((callee, entry)) = integer_result_call(program, call) else {
        return Vec::new();
    };
    let Some(call_value) = ensured_call_operand(program, machine, state, expression) else {
        return Vec::new();
    };
    let parameters = program.state_parameters(entry);
    let arguments = program.expression_table.expression_handles(call.arguments);
    let mut orderings = Vec::new();
    let facts = program
        .contract_expressions(
            program
                .machine_contracts(callee)
                .iter()
                .chain(program.state_contracts(entry)),
            SignatureContractKind::Ensures,
        )
        .collect::<Vec<_>>();
    for fact in facts.iter().copied() {
        push_ensured_orderings(
            program,
            machine,
            state,
            callee,
            entry,
            parameters,
            &arguments,
            &call_value,
            fact,
            &mut orderings,
        );
    }
    orderings
}

/// One concrete integer-returning call whose published `ensures` may be read.
///
/// Parameters may be records. Mutable formals stay in the signature and are
/// simply not substituted: their post-state is not the actual argument.
fn integer_result_call<'program>(
    program: &'program dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<(&'program Machine, &'program State)> {
    if !call.target_symbol.is_valid()
        || call.receiver.is_valid()
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
    {
        return None;
    }
    let mut targets = program.machine_states_named(call.target_symbol);
    let (machine, state) = targets.next()?;
    if targets.next().is_some()
        || program.machine_states(machine).first()?.symbol != state.symbol
        || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
        || machine.attached_data.is_some()
        || !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
    {
        return None;
    }
    if !program
        .primitive_type_reference(state.return_type)
        .is_some_and(|primitive| primitive.is_fixed_integer())
    {
        return None;
    }
    let parameters = program.state_parameters(state);
    let arguments = program.expression_table.expression_handles(call.arguments);
    if parameters.len() != arguments.len()
        || parameters
            .iter()
            .any(|parameter| !parameter.symbol.is_valid())
    {
        return None;
    }
    Some((machine, state))
}

/// Identity of one integer call occurrence while its `ensures` is applied.
///
/// Argument operands use the ordinary place identity. The call itself is not
/// required to be a primitive value closure: a field read is a real result,
/// and this operand is not reused as proof that two such calls are equal.
pub(super) fn ensured_call_operand(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<Operand> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    integer_result_call(program, call)?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    Some(Operand::Call {
        target: call.target_symbol,
        arguments: arguments
            .iter()
            .map(|argument| operand(program, machine, state, *argument, None))
            .collect::<Option<Vec<_>>>()?,
    })
}

fn rebound_call_orderings(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    destination: &Operand,
) -> Vec<Relation> {
    let Some(call_value) = ensured_call_operand(program, machine, state, expression) else {
        return Vec::new();
    };
    call_result_orderings(program, machine, state, expression)
        .into_iter()
        .filter_map(|relation| {
            let relation = Relation {
                left: replace_operand(relation.left, &call_value, destination),
                right: replace_operand(relation.right, &call_value, destination),
                constraint: relation.constraint,
            };
            (&relation.left == destination || &relation.right == destination).then_some(relation)
        })
        .collect()
}

fn push_ensured_orderings(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    callee: &Machine,
    entry: &State,
    parameters: &[typed_trees::signature::StateParameter],
    arguments: &[ExpressionHandle],
    call_value: &Operand,
    fact: ExpressionHandle,
    orderings: &mut Vec<Relation>,
) {
    let Some(comparison) = program.expression_table.binary_node(fact) else {
        return;
    };
    if comparison.operator == BinaryOperator::And {
        push_ensured_orderings(
            program,
            machine,
            state,
            callee,
            entry,
            parameters,
            arguments,
            call_value,
            comparison.left,
            orderings,
        );
        push_ensured_orderings(
            program,
            machine,
            state,
            callee,
            entry,
            parameters,
            arguments,
            call_value,
            comparison.right,
            orderings,
        );
        return;
    }
    if !matches!(
        comparison.operator,
        BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Equal
    ) || !has_builtin_binary_expression_meaning(program, callee, Some(entry), fact)
    {
        return;
    }
    let Some(left) = translate_ensured_operand(
        program,
        machine,
        state,
        callee,
        entry,
        parameters,
        arguments,
        call_value,
        comparison.left,
    ) else {
        return;
    };
    let Some(right) = translate_ensured_operand(
        program,
        machine,
        state,
        callee,
        entry,
        parameters,
        arguments,
        call_value,
        comparison.right,
    ) else {
        return;
    };
    orderings.extend(ordering_relations(comparison.operator, left, right));
}

fn translate_ensured_operand(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    callee: &Machine,
    entry: &State,
    parameters: &[typed_trees::signature::StateParameter],
    arguments: &[ExpressionHandle],
    call_value: &Operand,
    expression: ExpressionHandle,
) -> Option<Operand> {
    if !expression.is_valid() {
        return None;
    }
    if crate::proof_contracts::contract_results::reserved_result_owner(program, expression)
        .is_some()
    {
        return Some(call_value.clone());
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => {
            Some(Operand::Integer(literal.clone(), literal.landing()))
        }
        ExpressionNode::Borrow(borrow) => translate_ensured_operand(
            program,
            machine,
            state,
            callee,
            entry,
            parameters,
            arguments,
            call_value,
            borrow.target,
        ),
        ExpressionNode::Name(path) => {
            let position = state_parameter_position(parameters, path.symbol)?;
            let parameter = &parameters[position];
            // An immutable formal still names the actual's value. A mutable
            // formal's post-state is not that argument.
            if parameter.is_mutable || parameter.is_const {
                return None;
            }
            operand(program, machine, state, arguments[position], None)
        }
        ExpressionNode::Member(member) => {
            let receiver = translate_ensured_operand(
                program,
                machine,
                state,
                callee,
                entry,
                parameters,
                arguments,
                call_value,
                member.receiver,
            )?;
            let field = declared_member_field_symbol(program, callee, Some(entry), expression)?;
            let reference =
                super::super::declared_place_type_raw(program, callee, Some(entry), expression)?;
            Some(Operand::Field {
                receiver: Box::new(receiver),
                field,
                name: member.member.as_str().to_owned(),
                primitive: program.primitive_type_reference(reference),
            })
        }
        _ => None,
    }
}

fn ordering_relations(operator: BinaryOperator, left: Operand, right: Operand) -> Vec<Relation> {
    let gap = |ceiling: Operand, value: Operand, strict: bool| Relation {
        left: ceiling,
        right: value,
        constraint: RelationConstraint::MinimumDifference(i64::from(strict)),
    };
    match operator {
        BinaryOperator::GreaterOrEqual => vec![gap(left, right, false)],
        BinaryOperator::Greater => vec![gap(left, right, true)],
        BinaryOperator::LessOrEqual => vec![gap(right, left, false)],
        BinaryOperator::Less => vec![gap(right, left, true)],
        BinaryOperator::Equal => vec![
            gap(left.clone(), right.clone(), false),
            gap(right, left, false),
        ],
        _ => Vec::new(),
    }
}

fn replace_operand(operand: Operand, call_value: &Operand, destination: &Operand) -> Operand {
    if &operand == call_value {
        destination.clone()
    } else {
        operand
    }
}

/// Orderings of `destination` that every arm of this match establishes.
fn match_join_orderings(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    destination: &Operand,
    written: &[String],
    environment: &ValueEnvironment,
) -> Vec<Relation> {
    let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) else {
        return Vec::new();
    };
    let arms = program.expression_table.match_arms(dispatch.arms);
    if arms.is_empty() {
        return Vec::new();
    }
    let arm_environments = dispatch_arm_environments(
        program,
        machine,
        Some(state),
        expression,
        dispatch,
        environment,
    );
    if arm_environments.len() != arms.len() {
        return Vec::new();
    }
    let mut shared: Option<Vec<Relation>> = None;
    for (arm, arm_environment) in arms.iter().zip(&arm_environments) {
        let Some(observed) = operand(program, machine, state, arm.value, None) else {
            return Vec::new();
        };
        if !observed.survives(written) {
            return Vec::new();
        }
        let inherited =
            ceilings_of_equal_value(&arm_environment.ordered_values, &observed, destination);
        shared = Some(match shared {
            None => inherited,
            Some(previous) => intersect_relations(previous, inherited),
        });
    }
    shared.unwrap_or_default()
}

/// Ceilings of `observed`, restated for `destination` once the two are equal.
fn ceilings_of_equal_value(
    relations: &[Relation],
    observed: &Operand,
    destination: &Operand,
) -> Vec<Relation> {
    let mut extended = relations.to_vec();
    extended.push(Relation {
        left: observed.clone(),
        right: destination.clone(),
        constraint: RelationConstraint::MinimumDifference(0),
    });
    extended.push(Relation {
        left: destination.clone(),
        right: observed.clone(),
        constraint: RelationConstraint::MinimumDifference(0),
    });
    let mut inherited = Vec::new();
    for relation in &extended {
        let RelationConstraint::MinimumDifference(distance) = relation.constraint else {
            continue;
        };
        if &relation.left == destination {
            continue;
        }
        if !congruence::same_value(&extended, &relation.right, destination) {
            continue;
        }
        let inherited_relation = Relation {
            left: relation.left.clone(),
            right: destination.clone(),
            constraint: RelationConstraint::MinimumDifference(distance),
        };
        if !inherited.contains(&inherited_relation) {
            inherited.push(inherited_relation);
        }
    }
    inherited
}

fn intersect_relations(previous: Vec<Relation>, inherited: Vec<Relation>) -> Vec<Relation> {
    previous
        .into_iter()
        .filter(|relation| inherited.contains(relation))
        .collect()
}

/// The greatest floor of `left - arm` that every arm of a match on `right` proves.
pub(super) fn match_subtract_floor(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    environment: &ValueEnvironment,
    left: ExpressionHandle,
    right: ExpressionHandle,
    depth: usize,
) -> Option<i64> {
    let ExpressionNode::Match(dispatch) = program.expression_table.expression(right) else {
        return None;
    };
    let arms = program.expression_table.match_arms(dispatch.arms);
    if arms.is_empty() {
        return None;
    }
    let arm_environments =
        dispatch_arm_environments(program, machine, Some(state), right, dispatch, environment);
    if arm_environments.len() != arms.len() {
        return None;
    }
    let mut floor = None;
    for (arm, arm_environment) in arms.iter().zip(&arm_environments) {
        let arm_floor = super::subtract_floor_limited(
            program,
            machine,
            Some(state),
            arm_environment,
            left,
            arm.value,
            depth + 1,
        )?;
        floor = Some(floor.map_or(arm_floor, |existing: i64| existing.min(arm_floor)));
    }
    floor
}
