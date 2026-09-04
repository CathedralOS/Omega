use crate::TypedTrees;
use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use crate::machine::Machine;
use crate::state::State;

/// Exact typed-expression custody for one machine's private ranking witness.
///
/// The normalized termination plan carries stable semantic text for artifact
/// identity and diagnostics. Checked semantics must not use that text to
/// rediscover source expressions, so lowering retains the exact typed handles
/// separately here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RankingExpressionCustody {
    pub machine: psi_symbols::SymbolHandle,
    pub subjects: Vec<ExpressionHandle>,
    pub view_arguments: Vec<ExpressionHandle>,
    pub rank_range: Option<ExpressionHandle>,
}

/// Resolve the normalized private ranking witness back to the typed
/// expressions retained for its root state. Semantic consumers use this
/// bridge instead of parallel authored spans on `Machine`.
pub fn resolve_machine_witness_subjects(
    program: &TypedTrees,
    machine: &Machine,
) -> Option<Vec<ExpressionHandle>> {
    let witness = machine.termination_plan.implementation_witness.as_ref()?;
    if let Some(custody) = program.ranking_expression_custody_for(machine.symbol) {
        return (custody.subjects.len() == witness.subjects.len())
            .then(|| custody.subjects.clone());
    }
    resolve_machine_witness_expressions(program, machine, &witness.subjects)
}

pub fn resolve_machine_witness_view_arguments(
    program: &TypedTrees,
    machine: &Machine,
) -> Option<Vec<ExpressionHandle>> {
    let witness = machine.termination_plan.implementation_witness.as_ref()?;
    if let Some(custody) = program.ranking_expression_custody_for(machine.symbol) {
        return (custody.view_arguments.len() == witness.view_arguments.len())
            .then(|| custody.view_arguments.clone());
    }
    resolve_machine_witness_expressions(program, machine, &witness.view_arguments)
}

pub fn resolve_machine_witness_expressions(
    program: &TypedTrees,
    machine: &Machine,
    rendered: &[String],
) -> Option<Vec<ExpressionHandle>> {
    let root_state = program.machine_states(machine).first()?;
    resolve_witness_expressions(program, root_state, rendered)
}

pub fn resolve_witness_expressions(
    program: &TypedTrees,
    state: &State,
    rendered: &[String],
) -> Option<Vec<ExpressionHandle>> {
    rendered
        .iter()
        .map(|expected| {
            if expected == "value" {
                return None;
            }
            program
                .expression_table
                .iter_expressions()
                .filter(|(handle, _)| witness_expression_text(program, *handle) == *expected)
                .find_map(|(handle, _)| {
                    expression_belongs_to_state(program, state, handle).then_some(handle)
                })
                .or_else(|| {
                    // Literal view arguments and range bounds have no place root.
                    program
                        .expression_table
                        .iter_expressions()
                        .find_map(|(handle, _)| {
                            (witness_expression_text(program, handle) == *expected)
                                .then_some(handle)
                        })
                })
        })
        .collect()
}

/// Render the source-like subset accepted in a normalized ranking witness.
pub fn witness_expression_text(program: &TypedTrees, expression: ExpressionHandle) -> String {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => literal.text().to_string(),
        ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("."),
        ExpressionNode::Member(member) => format!(
            "{}.{}",
            witness_expression_text(program, member.receiver),
            member.member.as_str()
        ),
        ExpressionNode::Binary(binary) if matches!(binary.operator, BinaryOperator::Subtract) => {
            format!(
                "{} - {}",
                witness_expression_text(program, binary.left),
                witness_expression_text(program, binary.right)
            )
        }
        _ => "value".to_string(),
    }
}

fn expression_belongs_to_state(
    program: &TypedTrees,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    let Some(root) = ranked_expression_root(program, expression) else {
        return false;
    };
    program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.symbol == root)
}

fn ranked_expression_root(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<psi_symbols::SymbolHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => Some(path.symbol),
        ExpressionNode::Member(member) => ranked_expression_root(program, member.receiver),
        ExpressionNode::Cast(cast) => ranked_expression_root(program, cast.value),
        _ => None,
    }
}
