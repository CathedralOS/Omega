//! Modular result bounds from exact, builtin normal-return contract facts.
use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, Interval, SignatureContractKind,
    TypeReferenceHandle, ValueEnvironment, guard_narrowing, range_constraint_interval,
};
use crate::machine_calls::calls::CalleeContract;
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    literal_interval, primitive_range,
};
use symbols::SymbolHandle;

/// Observations of an immutable argument binding. A collection's descriptor
/// extent is independent of writes to its elements; it is not the integer
/// value of that formal. Keep the two coordinates separate during substitution.
#[derive(Default)]
pub(super) struct ArgumentBounds {
    pub values: Vec<(SymbolHandle, Interval)>,
    pub lengths: Vec<(SymbolHandle, Interval)>,
}

pub(super) fn normal_return_interval(
    program: &dyn crate::CheckingInput,
    callee: &CalleeContract<'_>,
    declared: Interval,
    actuals: &ArgumentBounds,
) -> Interval {
    // Only gated builtin preconditions may refine a formal used in a result
    // relation. The ordinary source guard environment is not selection evidence.
    let mut environment = ValueEnvironment::new();
    for parameter in callee.parameters {
        if parameter.is_mutable || parameter.is_const {
            continue;
        }
        let Some(mut interval) = program
            .primitive_type_reference(parameter.type_reference)
            .and_then(primitive_range)
        else {
            continue;
        };
        if let Some(actual) = crate::substitutions::actual_for(&actuals.values, parameter.symbol) {
            interval = interval.intersect(actual);
        }
        for contract in callee
            .contracts
            .iter()
            .filter(|contract| contract.kind == SignatureContractKind::Requires)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let Some(expression) = fact.as_expression() {
                    interval = interval.intersect(project(
                        program,
                        callee,
                        &environment,
                        &actuals.lengths,
                        *expression,
                        Some(parameter),
                    ));
                }
            }
        }
        environment.set(parameter.name.as_str().to_owned(), interval);
    }
    program
        .contract_expressions(callee.contracts.iter(), SignatureContractKind::Ensures)
        .fold(declared, |interval, expression| {
            interval.intersect(project(
                program,
                callee,
                &environment,
                &actuals.lengths,
                expression,
                None,
            ))
        })
}

fn project(
    program: &dyn crate::CheckingInput,
    callee: &CalleeContract<'_>,
    environment: &ValueEnvironment,
    lengths: &[(SymbolHandle, Interval)],
    expression: ExpressionHandle,
    formal: Option<&typed_trees::signature::StateParameter>,
) -> Interval {
    project_possible(program, callee, environment, lengths, expression, formal)
        .unwrap_or(Interval::UNBOUNDED)
}

/// A disjunct proven impossible contributes no possible result, rather than
/// an unbounded result that erases its live sibling. Unknown propositions still
/// contribute the full interval. Facts stay in the callee's immutable argument
/// environment; caller expressions are never evaluated again here.
fn project_possible(
    program: &dyn crate::CheckingInput,
    callee: &CalleeContract<'_>,
    environment: &ValueEnvironment,
    lengths: &[(SymbolHandle, Interval)],
    expression: ExpressionHandle,
    formal: Option<&typed_trees::signature::StateParameter>,
) -> Option<Interval> {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return Some(Interval::UNBOUNDED);
    };
    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
        // These two selective connectives have no declared operator spelling.
        let left = project_possible(program, callee, environment, lengths, binary.left, formal);
        if binary.operator == BinaryOperator::And {
            let left = left?;
            let continuing = condition_environment(program, callee, environment, binary.left);
            if continuing.intervals.values().any(|interval| {
                interval
                    .low
                    .zip(interval.high)
                    .is_some_and(|(low, high)| low > high)
            }) {
                return None;
            }
            return project_possible(program, callee, &continuing, lengths, binary.right, formal)
                .map(|right| left.intersect(right));
        }
        let right = project_possible(program, callee, environment, lengths, binary.right, formal);
        return match (left, right) {
            (Some(left), Some(right)) => Some(left.union(right)),
            (left, right) => left.or(right),
        };
    }
    let result = |expression| {
        if let Some(parameter) = formal {
            matches!(program.expression_table.expression(expression), ExpressionNode::Name(path)
            if parameter.symbol.is_valid() && path.symbol == parameter.symbol && path.head_symbol == parameter.symbol)
        } else {
            crate::reserved_result_owner(program, expression).is_some_and(|(owner, return_type)| {
                owner == callee.callable && return_type == callee.return_type
            })
        }
    };
    let (other, subject_on_left) = if result(binary.left) {
        (binary.right, true)
    } else if result(binary.right) {
        (binary.left, false)
    } else {
        return Some(Interval::UNBOUNDED);
    };
    let Some((operand, operand_type)) = operand_interval(
        program,
        callee,
        environment,
        lengths,
        other,
        formal.is_none(),
    ) else {
        return Some(Interval::UNBOUNDED);
    };
    let subject_type = formal.map_or(callee.return_type, |parameter| {
        crate::reference_shell(program, parameter.type_reference)
            .map_or(parameter.type_reference, |shell| shell.referee)
    });
    let operand_types = if subject_on_left {
        [Some(subject_type), operand_type]
    } else {
        [operand_type, Some(subject_type)]
    };
    if !builtin(
        program,
        callee.callable,
        expression,
        binary.operator,
        &operand_types,
    ) {
        return Some(Interval::UNBOUNDED);
    }
    Some(guard_narrowing::comparison_interval(
        binary.operator,
        operand,
        subject_on_left,
    ))
}

fn condition_environment(
    program: &dyn crate::CheckingInput,
    callee: &CalleeContract<'_>,
    environment: &ValueEnvironment,
    condition: ExpressionHandle,
) -> ValueEnvironment {
    let Some(machine) = program.machine_by_symbol(callee.callable) else {
        return environment.clone();
    };
    guard_narrowing::evaluated_condition_environment(
        program,
        machine,
        program.machine_states(machine).first(),
        condition,
        true,
        environment,
    )
}

/// A checked normal-return guarantee over one scalar referent. Arrival
/// requirements are deliberately absent: mutable storage can change during
/// the invocation. The selected formal and builtin comparisons own the bound.
pub(crate) fn normal_return_referent_interval(
    program: &dyn crate::CheckingInput,
    parameter: &typed_trees::signature::StateParameter,
) -> Option<Interval> {
    let shell = crate::reference_shell(program, parameter.type_reference)?;
    let primitive = program.primitive_type_reference(shell.referee)?;
    let owner = program.symbols.get(parameter.symbol).parent;
    let callee = crate::machine_calls::calls::callee_entry_contract(program, owner)?;
    let environment = ValueEnvironment::new();
    let declared = primitive_range(primitive)?.intersect(
        range_constraint_interval(program, shell.referee).unwrap_or(Interval::UNBOUNDED),
    );
    Some(
        program
            .contract_expressions(callee.contracts.iter(), SignatureContractKind::Ensures)
            .fold(declared, |interval, expression| {
                interval.intersect(project(
                    program,
                    &callee,
                    &environment,
                    &[],
                    expression,
                    Some(parameter),
                ))
            }),
    )
}

fn operand_interval(
    program: &dyn crate::CheckingInput,
    callee: &CalleeContract<'_>,
    environment: &ValueEnvironment,
    lengths: &[(SymbolHandle, Interval)],
    expression: ExpressionHandle,
    allow_formals: bool,
) -> Option<(Interval, Option<TypeReferenceHandle>)> {
    if let Some(cast) = program.expression_table.cast_node(expression) {
        // A total Exact widening preserves the actual's value. Other casts
        // may change it or require their own evidence; their result carrier
        // alone cannot establish a relational guarantee about the source.
        if !crate::proof_contracts::arithmetic_domains::cast_ranges::plain_exact_widening_cast(
            program, cast,
        ) {
            return None;
        }
        let (interval, source_type) = operand_interval(
            program,
            callee,
            environment,
            lengths,
            cast.value,
            allow_formals,
        )?;
        let source = program.primitive_type_reference(source_type?)?;
        let target = program.primitive_type_reference(cast.target_type)?;
        return crate::integer_widen_is_total(source, target)
            .then_some((interval, Some(cast.target_type)));
    }
    if let Some(literal) = program.expression_table.integer_node(expression) {
        // Unknown literal typing is a wildcard in candidate lookup, not a
        // guessed same-carrier type that could hide a heterogeneous operator.
        return Some((literal_interval(literal), None));
    }
    if !allow_formals {
        return None;
    }
    if let Some(binary) = program.expression_table.binary_node(expression) {
        // Admit only expressions over the captured immutable operands. The
        // ordinary arithmetic owner then supplies selected-operator and policy
        // semantics, including formation failures, rather than duplicating an
        // arithmetic interpreter in contract projection.
        operand_interval(program, callee, environment, lengths, binary.left, true)?;
        operand_interval(program, callee, environment, lengths, binary.right, true)?;
        let machine = program.machine_by_symbol(callee.callable)?;
        let reference = crate::parameter_expression_result_type_reference(
            program,
            callee.callable,
            callee.parameters,
            expression,
        )?;
        program
            .primitive_type_reference(reference)
            .and_then(primitive_range)?;
        let mut diagnostics = Vec::new();
        let analysis = super::expression_analysis::analyze(
            program,
            machine,
            program.machine_states(machine).first(),
            expression,
            environment,
            None,
            super::ArithmeticDomain::Exact,
            "normal-return contract",
            false,
            &mut diagnostics,
        );
        return diagnostics
            .is_empty()
            .then_some((analysis.value.interval, Some(reference)));
    }
    if let Some(member) = program.expression_table.member_node(expression) {
        use crate::value_custody::places::collection_operations::{
            MeasureReceiver, collection_measure_member,
        };
        let path = program.expression_table.name_path_node(member.receiver)?;
        let parameter = callee.parameters.iter().find(|parameter| {
            parameter.symbol.is_valid()
                && parameter.symbol == path.symbol
                && path.symbol == path.head_symbol
                && !parameter.is_const
        })?;
        if collection_measure_member(
            program,
            member,
            MeasureReceiver::Declared(parameter.type_reference),
        ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
        {
            return None;
        }
        let interval = crate::substitutions::actual_for(lengths, parameter.symbol)?;
        let reference = crate::parameter_expression_result_type_reference(
            program,
            callee.callable,
            callee.parameters,
            expression,
        )?;
        return Some((interval, Some(reference)));
    }
    let path = program.expression_table.name_path_node(expression)?;
    if !path.symbol.is_valid() || path.symbol != path.head_symbol {
        return None;
    }
    let Some(parameter) = callee
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == path.symbol)
    else {
        // `ensures result < Bound` reads the const binder, not a formal.
        // Its standing `requires` ceiling is the operand interval.
        return integer_binder_operand(program, callee, expression);
    };
    if parameter.is_mutable || parameter.is_const {
        return None;
    }
    let primitive = program.primitive_type_reference(parameter.type_reference)?;
    let mut interval = primitive_range(primitive)?;
    if let Some(declared) = range_constraint_interval(program, parameter.type_reference) {
        interval = interval.intersect(declared);
    }
    // Only immutable formal values are read here. Caller expressions and places
    // are never re-evaluated to invent a post-call snapshot.
    if let Some(required) = environment.get(parameter.name.as_str()) {
        interval = interval.intersect(required);
    }
    Some((interval, Some(parameter.type_reference)))
}

fn integer_binder_operand(
    program: &dyn crate::CheckingInput,
    callee: &CalleeContract<'_>,
    expression: ExpressionHandle,
) -> Option<(Interval, Option<TypeReferenceHandle>)> {
    let machine = program.machine_by_symbol(callee.callable)?;
    let state = program.machine_states(machine).first()?;
    let path = program.expression_table.name_path_node(expression)?;
    if !path.symbol.is_valid() || path.symbol != path.head_symbol {
        return None;
    }
    let binder = program
        .machine_type_parameters(machine)
        .iter()
        .find(|parameter| parameter.symbol == path.symbol)?;
    let type_reference = match binder.kind {
        typed_trees::data::TypeParameterKind::Const { type_reference }
        | typed_trees::data::TypeParameterKind::Value { type_reference } => type_reference,
        _ => return None,
    };
    let interval =
        super::invariant_bounds::standing_integer_interval(program, machine, state, expression)?;
    Some((interval, Some(type_reference)))
}

pub(super) fn builtin(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    let Some(spelling) = typed_trees::operator::comparison_operator_spelling(operator) else {
        return false;
    };
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        scope,
        expression,
        spelling,
        operand_types,
    )
}
