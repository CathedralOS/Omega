//! Conserved endpoint inputs through members that author no range of their own.
//! Candidate slots name equal transported values, never inferred ranking views.

use super::*;

pub(crate) struct RankingRangeCallEdge<'program> {
    pub source: usize,
    pub destination: usize,
    pub arguments: &'program [ExpressionHandle],
}

/// The caller supplies one strongly connected component and every intra-component
/// occurrence, including parallel calls. Entry/range meaning and each edge's
/// arithmetic obligations remain separate judgments.
pub(crate) fn mixed_call_endpoints_are_pinned(
    program: &TypedTrees,
    members: &[RankingRangeCallMember<'_>],
    edges: &[RankingRangeCallEdge<'_>],
) -> bool {
    let prove = || -> Option<()> {
        let parameters = members
            .iter()
            .map(|member| {
                let (state, _) = scalar_entry(program, member)?;
                Some(
                    program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| !parameter.is_self)
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Option<Vec<_>>>()?;
        for (origin, member) in members.iter().enumerate() {
            if !member.range.is_valid() {
                continue;
            }
            let ExpressionNode::Range(range) = program.expression_table.expression(member.range)
            else {
                return None;
            };
            let mut dependencies = Vec::new();
            for endpoint in [range.start, range.end] {
                endpoint_inputs(program, &parameters[origin], endpoint, &mut dependencies, 0)?;
            }
            for dependency in dependencies {
                let formal = parameters[origin]
                    .iter()
                    .find(|formal| formal.symbol == dependency)?;
                let carrier = exact_integer_parameter(program, formal.type_reference)?;
                let mut candidates = parameters
                    .iter()
                    .enumerate()
                    .map(|(position, formals)| {
                        formals
                            .iter()
                            .map(|formal| {
                                formal.symbol.is_valid()
                                    && !formal.is_const
                                    && exact_integer_parameter(program, formal.type_reference)
                                        == Some(carrier)
                                    && (position != origin || formal.symbol == dependency)
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                // The origin starts with its authored input only. Since the
                // component is strongly connected, every surviving candidate
                // is reached from that singleton. Intersecting every incoming
                // occurrence prevents an unrooted cycle or parallel alternative
                // from manufacturing a second possible endpoint value.
                loop {
                    let mut changed = false;
                    for edge in edges {
                        let source = parameters.get(edge.source)?;
                        let destination = parameters.get(edge.destination)?;
                        if edge.arguments.len() != destination.len() {
                            return None;
                        }
                        for (ordinal, argument) in edge.arguments.iter().enumerate() {
                            if !candidates[edge.destination][ordinal] {
                                continue;
                            }
                            let actual = direct_input(program, *argument, 0);
                            let preserved = source.iter().enumerate().any(|(ordinal, formal)| {
                                candidates[edge.source][ordinal] && actual == Some(formal.symbol)
                            });
                            if !preserved {
                                candidates[edge.destination][ordinal] = false;
                                changed = true;
                            }
                        }
                    }
                    if candidates
                        .iter()
                        .any(|candidates| !candidates.iter().any(|candidate| *candidate))
                    {
                        return None;
                    }
                    if !changed {
                        break;
                    }
                }
            }
        }
        Some(())
    };
    prove().is_some()
}

fn direct_input(
    program: &TypedTrees,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<SymbolHandle> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => direct_input(program, atomic.value, depth + 1),
        ExpressionNode::Name(path)
            if path.symbol.is_valid()
                && path.head_symbol == path.symbol
                && program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 1 =>
        {
            Some(path.symbol)
        }
        _ => None,
    }
}

fn endpoint_inputs(
    program: &TypedTrees,
    parameters: &[&typed_trees::signature::StateParameter],
    expression: ExpressionHandle,
    inputs: &mut Vec<SymbolHandle>,
    depth: usize,
) -> Option<()> {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_) => Some(()),
        ExpressionNode::Atomic(atomic) => {
            endpoint_inputs(program, parameters, atomic.value, inputs, depth + 1)
        }
        ExpressionNode::Name(_) => {
            let symbol = direct_input(program, expression, 0)?;
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.symbol == symbol)?;
            exact_integer_parameter(program, parameter.type_reference)?;
            if !inputs.contains(&symbol) {
                inputs.push(symbol);
            }
            Some(())
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            ) =>
        {
            endpoint_inputs(program, parameters, binary.left, inputs, depth + 1)?;
            endpoint_inputs(program, parameters, binary.right, inputs, depth + 1)
        }
        _ => None,
    }
}
