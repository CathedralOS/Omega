use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;
use symbols::JoinedPath;
use typed_trees::statement::{AssemblyFactKind, StatementNode};
use validation::{CanonicalPlace, canonical_place_from_expression_in_state};

use super::prover::{
    live_contexts_prove_boolean_expression, semantic_contexts_prove_boolean_expression,
};
use crate::flow::canonical_place_segments_may_overlap;
use crate::labels::machine_name;

/// Discharge each assembly assertion against ordinary invalidation-adjusted
/// flow contexts. Entry assertions precede instruction evaluation. Exit
/// assertions follow the instructions, including argument evaluation on an
/// enclosing machine edge. Assertions consume facts; they never add premises.
pub(super) fn check_assembly_fact_contracts(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, state_flow) in facts.flow.control.states.iter() {
        check_state_assembly_facts(program, facts, state_flow, diagnostics);
    }
}

fn check_state_assembly_facts(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((_machine, state)) = validation::machine_state_by_symbols(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return;
    };

    let statements = program.statement_table.statements(state.statement_nodes);
    for (statement_index, statement) in statements.iter().enumerate() {
        let Some(fact) = statement.as_assembly_fact() else {
            continue;
        };
        let Some(statement_flow) = facts.flow.state_statement(state_flow, statement_index) else {
            // A label without an incoming executable path owes no edge proof.
            continue;
        };
        let invalidated_inside_block = fact.kind == AssemblyFactKind::Ensures
            && asm_block_writes_fact_place(
                program,
                state.symbol,
                statements,
                statement_index,
                fact.fact,
            );
        // A machine jump evaluates its arguments before taking the edge.
        // Its label exit observes the resulting outgoing frontier, rather
        // than the assertion node's earlier position in the lexical window.
        let transfer = (fact.kind == AssemblyFactKind::Ensures)
            .then(|| enclosing_transfer_for_fact(program, statements, statement_index))
            .flatten();
        let observations: Vec<_> = if let Some(transfer) = transfer {
            facts
                .flow
                .control
                .outgoing
                .span_or_empty(state_flow.outgoing)
                .iter()
                .filter(|edge| edge.statement_index == transfer)
                .map(|edge| (transfer, edge.constraints))
                .collect()
        } else {
            vec![(statement_index, statement_flow.entry_constraints)]
        };
        if !observations.is_empty()
            && observations.iter().all(|(observation, constraints)| {
                let contexts = facts
                    .flow
                    .semantic_constraint_contexts(*constraints)
                    .collect::<Vec<_>>();
                assembly_fact_is_proven(
                    program,
                    facts,
                    state,
                    *observation,
                    &contexts,
                    fact.fact,
                    !invalidated_inside_block,
                )
            })
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
            assembly_fact_label(program, fact.fact),
        )));
    }
}

/// The enclosing machine transfer is retained as the label's final ordinary
/// instruction. Local assembly jumps have no argument evaluation of their own.
fn enclosing_transfer_for_fact(
    program: &dyn ::validation::CheckingInput,
    statements: &[StatementNode],
    fact: usize,
) -> Option<usize> {
    use typed_trees::statement::{AssemblyControlPoint, AssemblyExit};
    let (entry, graph) =
        statements[..fact]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, statement)| {
                let StatementNode::Assembly(AssemblyControlPoint::Entry(graph)) = statement else {
                    return None;
                };
                Some((index, program.statement_table.assembly(*graph)))
            })?;
    let offset = fact.checked_sub(entry + 1)?;
    let block = program
        .statement_table
        .assembly_blocks(graph.blocks)
        .iter()
        .find(|block| {
            (block.start as usize..block.start as usize + block.count as usize).contains(&offset)
        })?;
    matches!(block.exit, AssemblyExit::EnclosingTransition)
        .then_some(entry + block.start as usize + block.count as usize)
}

/// The general flow engine invalidates domain-derived facts across assignments,
/// but direct boolean constraints can still be present in an exit context. An
/// asm postcondition must not reuse such a stale entry constraint after an
/// instruction wrote one of the places it reads.
fn asm_block_writes_fact_place(
    program: &dyn ::validation::CheckingInput,
    state_symbol: symbols::SymbolHandle,
    statements: &[StatementNode],
    ensures_index: usize,
    proof_fact: arena::Handle<typed_trees::domain::ProofFact>,
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
            StatementNode::Assembly(_) => break,
            StatementNode::AssemblyFact(fact) if fact.kind == AssemblyFactKind::Requires => break,
            StatementNode::AssemblyFact(_) => return false,
            _ => instruction_start -= 1,
        }
    }

    statements[instruction_start..instruction_end]
        .iter()
        .enumerate()
        .filter_map(|(offset, statement)| {
            let assignment = statement.as_assignment()?;
            canonical_place_from_expression_in_state(
                program,
                state_symbol,
                instruction_start + offset,
                assignment.target,
            )
        })
        .any(|written_place| {
            program
                .tables
                .proof_fact_expression_roots(proof_fact)
                .into_iter()
                .any(|expression| {
                    expression_reads_overlapping_place(
                        program,
                        state_symbol,
                        ensures_index,
                        expression,
                        &written_place,
                    )
                })
        })
}

pub(super) fn expression_reads_overlapping_place(
    program: &dyn ::validation::CheckingInput,
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
        ExpressionNode::Match(dispatch) => program
            .expression_table
            .match_deep_any(dispatch, |handle| recurse(handle)),
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
        typed_trees::spelled_literal_expression_pattern!() => false,
    }
}

fn expression_place_may_overlap(
    program: &dyn ::validation::CheckingInput,
    state_symbol: symbols::SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    written_place: &CanonicalPlace,
) -> bool {
    canonical_place_from_expression_in_state(program, state_symbol, statement_index, expression)
        .is_some_and(|read_place| {
            // Compare on STORAGE identity: a callee's mutation summary roots
            // `self` writes at the machine symbol while an authored `self.x`
            // read roots at the state's `&mut self` parameter.
            (read_place.root) == (written_place.root)
                && canonical_place_segments_may_overlap(
                    program,
                    &read_place.segments,
                    &written_place.segments,
                )
        })
}

fn assembly_fact_label(
    program: &dyn validation::CheckingInput,
    handle: arena::Handle<typed_trees::domain::ProofFact>,
) -> String {
    match program.proof_facts.get(handle) {
        typed_trees::domain::ProofFact::Expression(expression) => {
            program.expression_table.display_name(*expression)
        }
        typed_trees::domain::ProofFact::Proposition(application) => program
            .normalize_proposition_application(application, None)
            .map(|formula| formula.identity_label())
            .unwrap_or_else(|| application.name.to_string()),
        typed_trees::domain::ProofFact::Membership(membership) => format!(
            "{} in {}",
            program.expression_table.display_name(membership.value),
            program.domain_path_members(membership.domain).joined_path()
        ),
    }
}
fn assembly_fact_is_proven(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    handle: arena::Handle<typed_trees::domain::ProofFact>,
    entry_preserved: bool,
) -> bool {
    match program.proof_facts.get(handle) {
        typed_trees::domain::ProofFact::Expression(expression) => {
            live_contexts_prove_boolean_expression(
                program,
                facts,
                state,
                statement,
                contexts,
                *expression,
            ) || (entry_preserved
                && semantic_contexts_prove_boolean_expression(
                    program,
                    &facts.semantic,
                    contexts,
                    *expression,
                ))
        }
        typed_trees::domain::ProofFact::Proposition(application) => {
            (entry_preserved
                && program
                    .normalize_proposition_application(application, None)
                    .is_some_and(|formula| {
                        super::prover::contexts_prove_proposition_label(
                            &facts.semantic,
                            program,
                            contexts,
                            &formula.identity_label(),
                        )
                    }))
                || super::prover::live_contexts_prove_proposition_application(
                    program,
                    facts,
                    state,
                    statement,
                    contexts,
                    application,
                )
        }
        typed_trees::domain::ProofFact::Membership(membership) => {
            let Some(place) = canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement,
                membership.value,
            ) else {
                return false;
            };
            if membership.domain_arguments.is_empty() {
                return super::prover::prove_domain_at_place(
                    program,
                    &facts.semantic,
                    contexts,
                    &place,
                    membership.domain_symbol,
                );
            }
            membership.semantic_domain.is_valid() && contexts.iter().any(|context| facts.semantic.context_view(facts.semantic.contexts.get(*context)).facts().any(|candidate| {
                matches!(candidate.payload, facts::domain_membership_symbol_and_instance_pattern!(domain_symbol, semantic_domain) if domain_symbol == membership.domain_symbol && semantic_domain == membership.semantic_domain)
                && candidate.place.as_place().and_then(|handle| crate::flow::canonical_place_from_semantic_place(program, &facts.semantic, facts.semantic.places.get(handle))).is_some_and(|candidate| candidate == place)
            }))
        }
    }
}
