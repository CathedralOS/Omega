//! Resolve one dispatch subject's value or place after ordinary type inference.
//!
//! Copyable values retain the completed value binding. Noncopy storage keeps
//! its original ownership: resolution has already captured every evaluated
//! address operand through `place_operand_capture`. Guards observe that address
//! through an ordinary shared loan; later sibling writes cannot change the
//! pending observation. Payload receivers retain the prepared original place,
//! so the guard loan can retire before an ordinary ownership transfer. Calls
//! and other produced values remain ordinary owned results.
//!
//! The specialization driver recollects consumers after these types settle;
//! no selected call consumes a provisional dispatch purpose.

use arena::HandleSpan;
use language_core::{LocalBindingRole, Multiplicity, ReferenceAccess};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableBorrowExpression, TableMemberExpression, TableNamePath,
};
use typed_trees::statement::{StatementNode, StatementRootRole};
use typed_trees::types::TypeReferenceNode;

pub(in crate::monomorphization) fn materialize(program: &mut TypedTrees) -> Vec<SymbolHandle> {
    let mut pending = Vec::new();
    let mut aliases = Vec::new();
    let mut guards = Vec::new();
    let mut logical = Vec::new();
    let mut proof_only = None;
    for machine in program.machines() {
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            for (offset, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                let Some(local) = statement.as_local_data() else {
                    continue;
                };
                if local.role != LocalBindingRole::DispatchSubject {
                    continue;
                }
                let storage = validation::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    offset,
                    local.initial_value,
                )
                .is_some_and(|place| place.root.as_symbol().is_some());
                // Preserve the purpose through generic/operator selection
                // until the actual result exists. A provisional carrier is
                // neither a completed value nor authority to classify storage.
                let Some(actual) = completed_subject_type(
                    program,
                    machine,
                    state,
                    local.initial_value,
                    &mut Vec::new(),
                ) else {
                    continue;
                };
                let noncopy_storage =
                    storage && program.type_multiplicity(actual) != Multiplicity::Unrestricted;
                let (mathematical, logical_machine) = if noncopy_storage {
                    let classification = proof_only
                        .get_or_insert_with(|| typed_trees::proof_only::classify(program));
                    let mathematical = classification.proof_only_mention(program, actual).is_some();
                    (
                        mathematical,
                        mathematical && classification.is_proof_machine(program, machine),
                    )
                } else {
                    (false, false)
                };
                // A mathematical place is an exact denotation, not another
                // owned value or address. Preserve it directly when neither
                // the value nor any prepared coordinate can change. This
                // does not erase an authored binding or duplicate linear
                // custody; produced/effectful terms retain ordinary captures.
                if logical_machine
                    && !validation::type_carries_linear_obligation(program, actual)
                    && validation::place_has_immutable_denotation(
                        program,
                        machine,
                        state,
                        offset,
                        local.initial_value,
                    )
                {
                    logical.push((
                        machine.clone(),
                        state_index,
                        state.statement_nodes,
                        offset,
                        local.symbol,
                        local.initial_value,
                    ));
                    continue;
                }
                let observation = if storage {
                    // Mathematical values have no address to observe. Keep
                    // their ordinary value binding and authored relevance;
                    // the proof-computation owner classifies and erases it.
                    // Creating &Nat here would instead make that classifier
                    // correctly reject an impossible storage borrow.
                    (noncopy_storage
                        && !mathematical
                        && validation::place_has_builtin_coordinates(
                            program,
                            machine,
                            Some(state),
                            local.initial_value,
                        ))
                    .then_some(actual)
                } else {
                    None
                };
                if observation.is_some() {
                    aliases.push((local.symbol, local.initial_value));
                }
                pending.push((
                    machine.symbol,
                    state.statement_nodes,
                    offset,
                    local.initial_value,
                    observation,
                ));
            }
        }
    }
    if pending.is_empty() && logical.is_empty() {
        return Vec::new();
    }
    if !aliases.is_empty() {
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                if !pending.iter().any(|(_, body, _, _, observation)| {
                    *body == state.statement_nodes && observation.is_some()
                }) {
                    continue;
                }
                for statement in program.statement_table.statements(state.statement_nodes) {
                    for (role, root) in program.tables.expression_roots(statement) {
                        if role == StatementRootRole::TransitionGuard {
                            let mut observed = Vec::new();
                            validation::expression_traversal::collect_expression_tree(
                                program,
                                root,
                                &mut observed,
                            );
                            guards.extend(observed);
                        }
                    }
                }
            }
        }
    }
    aliases.sort_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
    guards.sort_by_key(|expression| (expression.arena_index(), expression.generation()));
    guards.dedup();
    // Guard occurrences consume the observation loan. Payload receivers and
    // pattern metadata retain the already-captured original storage place,
    // so ordinary loan retirement permits a legitimate selected transfer.
    replace_place_aliases(program, &aliases, &guards);
    let logical_aliases = logical
        .iter()
        .map(|(_, _, _, _, symbol, initial)| (*symbol, *initial))
        .collect::<Vec<_>>();
    let logical_aliases = normalized_place_aliases(program, &logical_aliases);
    // Unlike a runtime place loan, the exact immutable logical denotation
    // serves both the guard and its payload projections.
    replace_place_aliases(program, &logical_aliases, &[]);
    let mut changed = Vec::new();
    for (owner, body, offset, initial, observation) in pending {
        let observed = observation.map(|referee| {
            let expression =
                program
                    .expression_table
                    .insert(ExpressionNode::Borrow(TableBorrowExpression {
                        target: initial,
                        access: ReferenceAccess::Shared,
                    }));
            let span = program.expression_table.source_span(initial);
            program.expression_table.set_source_span(expression, span);
            let reference = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee,
                    access: ReferenceAccess::Shared,
                    lifetime: None,
                });
            (expression, reference)
        });
        let StatementNode::LocalData(local) =
            &mut program.statement_table.statements_mut(body)[offset]
        else {
            unreachable!()
        };
        local.role = LocalBindingRole::Value;
        if let Some((expression, reference)) = observed {
            local.initial_value = expression;
            local.type_reference = reference;
        }
        changed.push(owner);
    }
    for (machine, state_index, old_body, _, _, _) in &logical {
        if program.machine_states(machine)[*state_index].statement_nodes != *old_body {
            continue;
        }
        let statements = program.statement_table.statements(*old_body).to_vec();
        let mut body = HandleSpan::empty();
        for (offset, statement) in statements.into_iter().enumerate() {
            if !logical.iter().any(|(_, _, candidate, position, _, _)| {
                candidate == old_body && *position == offset
            }) {
                program.statement_table.push_statement(&mut body, statement);
            }
        }
        program.machine_states_mut(machine)[*state_index].statement_nodes = body;
        changed.push(machine.symbol);
    }
    changed
}

// A generated local's Unit sentinel is not a completed source type. Follow
// only inferred Unit bindings to their type producer; an authored Unit slot or
// a call whose exact signature returns Unit remains an ordinary Unit value.
fn completed_subject_type(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    seen: &mut Vec<ExpressionHandle>,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if !expression.is_valid() || seen.contains(&expression) {
        return None;
    }
    seen.push(expression);
    let actual = validation::expression_result_type_reference(program, machine, state, expression)?;
    if matches!(
        program.type_reference_table.type_reference(actual),
        typed_trees::types::TypeReferenceNode::Unit
    ) && let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let Some((_, local)) = path
            .resolved_symbol()
            .and_then(|symbol| program.state_local_declaration(state, symbol))
        && local.type_is_inferred
        && matches!(
            program
                .type_reference_table
                .type_reference(local.type_reference),
            typed_trees::types::TypeReferenceNode::Unit
        )
    {
        return completed_subject_type(program, machine, state, local.initial_value, seen);
    }
    Some(actual)
}

fn replace_place_aliases(
    program: &mut TypedTrees,
    aliases: &[(SymbolHandle, ExpressionHandle)],
    guards: &[ExpressionHandle],
) {
    if aliases.is_empty() {
        return;
    }
    let occurrences = program
        .expression_table
        .expression_entries()
        .filter_map(|(expression, _)| {
            if guards
                .binary_search_by_key(
                    &(expression.arena_index(), expression.generation()),
                    |guard| (guard.arena_index(), guard.generation()),
                )
                .is_ok()
            {
                return None;
            }
            let (path, source) = place_alias_replacement(program, aliases, expression)?;
            Some((expression, path, source))
        })
        .collect::<Vec<_>>();
    for (expression, path, source) in occurrences {
        replace_place_alias(program, expression, path, source);
    }
}

/// Immutable denotation sources can mention earlier dispatch captures. Close
/// those sources before replacing any occurrence: arena order is not a source
/// definition order, and replacing a node does not create a forwarding link.
/// The observation query requires each local coordinate to precede its capture
/// in the same state, so the collected statement order is topological. Every
/// source is copied before rewriting, and every substituted source is already
/// closed. A single occurrence pass then leaves no removed symbol behind.
fn normalized_place_aliases(
    program: &mut TypedTrees,
    aliases: &[(SymbolHandle, ExpressionHandle)],
) -> Vec<(SymbolHandle, ExpressionHandle)> {
    let mut normalized = Vec::with_capacity(aliases.len());
    for &(symbol, source) in aliases {
        let source = program.expression_table.insert_copy(source);
        let mut nodes = Vec::new();
        validation::expression_traversal::collect_expression_tree(program, source, &mut nodes);
        for expression in nodes {
            if let Some((path, source)) = place_alias_replacement(program, &normalized, expression)
            {
                replace_place_alias(program, expression, path, source);
            }
        }
        let position = normalized
            .binary_search_by_key(
                &(symbol.arena_index(), symbol.generation()),
                |(candidate, _)| (candidate.arena_index(), candidate.generation()),
            )
            .unwrap_or_else(|position| position);
        normalized.insert(position, (symbol, source));
    }
    normalized
}

fn place_alias_replacement(
    program: &TypedTrees,
    aliases: &[(SymbolHandle, ExpressionHandle)],
    expression: ExpressionHandle,
) -> Option<(TableNamePath, ExpressionHandle)> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let symbol = path.resolved_symbol()?;
    let index = aliases
        .binary_search_by_key(
            &(symbol.arena_index(), symbol.generation()),
            |(candidate, _)| (candidate.arena_index(), candidate.generation()),
        )
        .ok()?;
    Some((path.clone(), aliases[index].1))
}

fn replace_place_alias(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    path: TableNamePath,
    source: ExpressionHandle,
) {
    // Reuse the ordinary graph copier, then compose projections on that exact
    // captured source. Index operands remain typed occurrences; no index is
    // encoded in a generated name or evaluated again while adding the suffix.
    let mut source = if path.members.count() > 1 {
        program.expression_table.insert_copy(source)
    } else {
        source
    };
    let occurrences = program
        .expression_table
        .authored_selection_occurrences(source)
        .collect::<Vec<_>>();
    for offset in 1..path.members.count() as usize {
        let member = program.expression_table.name_path_members(path.members)[offset].clone();
        let member_symbol = program
            .expression_table
            .name_path_member_symbols(path.member_symbols)
            .get(offset)
            .copied()
            .unwrap_or_default();
        source = program
            .expression_table
            .insert(ExpressionNode::Member(TableMemberExpression {
                receiver: source,
                member_symbol,
                member,
                case_variant: None,
            }));
    }
    program
        .expression_table
        .attach_authored_selection_occurrences(source, occurrences);
    program
        .expression_table
        .replace_with_existing(expression, source);
}

pub(crate) fn require_complete(program: &TypedTrees) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut errors = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(local) = statement.as_local_data() else {
                    continue;
                };
                if local.role == LocalBindingRole::DispatchSubject {
                    errors.push(
                        diagnostics::Diagnostic::error(format!(
                            "cannot determine the dispatch subject storage type in `{}::{}`",
                            machine.name.as_str(),
                            state.name.as_str(),
                        ))
                        .with_source_span(
                            program.expression_table.source_span(local.initial_value),
                        ),
                    );
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
