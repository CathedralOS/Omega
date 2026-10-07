//! Expressions whose exact metadata/field coordinates enter one range judgment.
use super::{
    ExpressionHandle, Machine, RankingRangeMeasure, RankingRangePremises, State, TypedTrees,
};
use typed_trees::expression::{ExpressionNode, TableRangeExpression};

pub(super) fn expressions(
    program: &TypedTrees,
    machine: &Machine,
    root: &State,
    range: &TableRangeExpression,
    measure: RankingRangeMeasure,
    premises: RankingRangePremises,
    arguments: Option<&[ExpressionHandle]>,
    guards: &[(ExpressionHandle, bool)],
    evaluated_prefix: &[ExpressionHandle],
) -> Vec<ExpressionHandle> {
    let mut expressions = vec![range.start, range.end];
    super::push_measure_subjects(measure, &mut expressions);
    expressions.extend(arguments.unwrap_or_default());
    expressions.extend(guards.iter().map(|(expression, _)| *expression));
    expressions.extend(evaluated_prefix);
    if arguments.is_none() || !matches!(premises, RankingRangePremises::RankInvariant) {
        expressions.extend(entry_expressions(program, machine, root));
    }
    expressions
}

/// Requires-fact and constrained-parameter endpoint expressions every range
/// judgment normalizes. The cross-machine call owner installs the same set so
/// a `.len` or other metadata coordinate inside an entry fact binds to the
/// same atom as at a named-state edge.
/// Depth-bounded projection-expression walk shared by the coordinate
/// installers: DFS over `expressions`; the member arm owns each Member
/// node's coordinate work (and steers its receiver recursion), and
/// `extra_arm` walks each site's extra node kinds. `None` ends the walk
/// early on the arm's refusal or a depth/validity breach.
pub(super) fn for_each_projection_expression(
    program: &TypedTrees,
    expressions: &[ExpressionHandle],
    mut member_arm: impl FnMut(
        &typed_trees::expression::TableMemberExpression,
        ExpressionHandle,
        &mut dyn FnMut(ExpressionHandle),
    ) -> Option<()>,
    mut extra_arm: impl FnMut(&ExpressionNode, &mut dyn FnMut(ExpressionHandle)),
) -> Option<()> {
    let mut pending = expressions
        .iter()
        .map(|expression| (*expression, 0))
        .collect::<Vec<_>>();
    while let Some((expression, depth)) = pending.pop() {
        if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        let mut visit = |child: ExpressionHandle| pending.push((child, depth + 1));
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => member_arm(member, expression, &mut visit)?,
            ExpressionNode::Borrow(borrow) => visit(borrow.target),
            ExpressionNode::Binary(binary) => {
                visit(binary.left);
                visit(binary.right);
            }
            ExpressionNode::Unary(unary) => visit(unary.operand),
            ExpressionNode::Atomic(atomic) => visit(atomic.value),
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    visit(field.value);
                }
            }
            other => extra_arm(other, &mut visit),
        }
    }
    Some(())
}

pub(super) fn entry_expressions(
    program: &TypedTrees,
    machine: &Machine,
    root: &State,
) -> Vec<ExpressionHandle> {
    let mut expressions = super::required_contract_expressions(program, machine);
    crate::proof_contracts::contract_entailment::ranking_range::constrained_parameter_bound_expressions(
        program,
        root,
        &mut expressions,
        None,
    );
    expressions
}
