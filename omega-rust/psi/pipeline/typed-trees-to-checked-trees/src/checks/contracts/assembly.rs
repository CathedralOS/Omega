use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;
use typed_trees::statement::{AssemblyFactKind, StatementNode};

use super::prover::semantic_contexts_prove_boolean_expression;
use crate::flow::{
    CanonicalPlace, canonical_place_from_expression_in_state, canonical_place_segments_may_overlap,
};
use crate::labels::machine_name;

/// Discharge each first-class `asm where requires`/`ensures` assertion at its
/// exact statement point. The parser places requires nodes before the lowered
/// instructions and ensures nodes after them, so the ordinary invalidation-
/// adjusted flow contexts supply the block entry and exit environments. These
/// assertions consume facts only; they never add a fact to the context.
pub(super) fn check_assembly_fact_contracts(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, state_flow) in facts.flow.control.states.iter() {
        check_state_assembly_facts(program, facts, state_flow, diagnostics);
    }
}

fn check_state_assembly_facts(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == state_flow.machine_symbol)
    else {
        return;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_flow.state_symbol)
    else {
        return;
    };

    let statements = program.statement_table.statements(state.statement_nodes);
    for (statement_index, statement) in statements.iter().enumerate() {
        let StatementNode::AssemblyFact(fact) = statement else {
            continue;
        };
        let entry_constraints = facts
            .flow
            .state_statement(state_flow, statement_index)
            .map(|statement| statement.entry_constraints)
            .unwrap_or(state_flow.entry_constraints);
        let entry_contexts = facts
            .flow
            .semantic_constraint_contexts(entry_constraints)
            .collect::<Vec<_>>();
        let invalidated_inside_block = fact.kind == AssemblyFactKind::Ensures
            && asm_block_writes_fact_place(
                program,
                state.symbol,
                statements,
                statement_index,
                fact.expression,
            );
        if !invalidated_inside_block
            && semantic_contexts_prove_boolean_expression(
                program,
                &facts.semantic,
                &entry_contexts,
                fact.expression,
            )
        {
            continue;
        }

        let (kind, point) = match fact.kind {
            AssemblyFactKind::Requires => ("requires", "block entry"),
            AssemblyFactKind::Ensures => ("ensures", "block exit"),
        };
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove asm `{kind}` fact at {point} in {} state `{}` (statement {statement_index}): {}",
            machine_name(program, state_flow.machine_symbol),
            state.name,
            program.expression_table.display_name(fact.expression),
        )));
    }
}

/// The general flow engine invalidates domain-derived facts across assignments,
/// but direct boolean constraints can still be present in an exit context. An
/// asm postcondition must not reuse such a stale entry constraint after an
/// instruction wrote one of the places it reads.
fn asm_block_writes_fact_place(
    program: &typed_trees::TypedTrees,
    state_symbol: symbols::SymbolHandle,
    statements: &[StatementNode],
    ensures_index: usize,
    fact_expression: ExpressionHandle,
) -> bool {
    let mut instruction_end = ensures_index;
    while instruction_end > 0
        && matches!(
            statements[instruction_end - 1],
            StatementNode::AssemblyFact(ref fact) if fact.kind == AssemblyFactKind::Ensures
        )
    {
        instruction_end -= 1;
    }

    let mut instruction_start = instruction_end;
    while instruction_start > 0 {
        match &statements[instruction_start - 1] {
            StatementNode::AssemblyFact(fact) if fact.kind == AssemblyFactKind::Requires => break,
            StatementNode::AssemblyFact(_) => return false,
            _ => instruction_start -= 1,
        }
    }

    statements[instruction_start..instruction_end]
        .iter()
        .enumerate()
        .filter_map(|(offset, statement)| {
            let StatementNode::Assignment(assignment) = statement else {
                return None;
            };
            canonical_place_from_expression_in_state(
                program,
                state_symbol,
                instruction_start + offset,
                assignment.target,
            )
        })
        .any(|written_place| {
            expression_reads_overlapping_place(
                program,
                state_symbol,
                ensures_index,
                fact_expression,
                &written_place,
            )
        })
}

fn expression_reads_overlapping_place(
    program: &typed_trees::TypedTrees,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    written_place: &CanonicalPlace,
) -> bool {
    if !expression.is_valid() {
        return false;
    }

    let recurse = |expression| {
        expression_reads_overlapping_place(
            program,
            state_symbol,
            statement_index,
            expression,
            written_place,
        )
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => recurse(atomic.value),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => expression_place_may_overlap(
            program,
            state_symbol,
            statement_index,
            expression,
            written_place,
        ),
        ExpressionNode::Indexed(indexed) => {
            expression_place_may_overlap(
                program,
                state_symbol,
                statement_index,
                expression,
                written_place,
            ) || recurse(indexed.index)
        }
        ExpressionNode::Borrow(inner) => recurse(inner.target),
        ExpressionNode::Binary(binary) => recurse(binary.left) || recurse(binary.right),
        ExpressionNode::Unary(unary) => recurse(unary.operand),
        ExpressionNode::Cast(cast) => recurse(cast.value),
        ExpressionNode::Range(range) => {
            (range.start.is_valid() && recurse(range.start))
                || (range.end.is_valid() && recurse(range.end))
        }
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && recurse(call.receiver))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(recurse)
        }
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .copied()
            .any(recurse),
        ExpressionNode::StructLiteral(struct_literal) => program
            .expression_table
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| recurse(field.value)),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

fn expression_place_may_overlap(
    program: &typed_trees::TypedTrees,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    written_place: &CanonicalPlace,
) -> bool {
    canonical_place_from_expression_in_state(program, state_symbol, statement_index, expression)
        .is_some_and(|read_place| {
            read_place.root == written_place.root
                && canonical_place_segments_may_overlap(
                    program,
                    &read_place.segments,
                    &written_place.segments,
                )
        })
}
