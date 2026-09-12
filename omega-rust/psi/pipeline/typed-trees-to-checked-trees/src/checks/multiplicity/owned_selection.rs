//! Conditional whole-local transfers, keyed by existing source selection nodes.
//! Availability is conservative; residual custody remains in the receipts until
//! the actual death edge. No selected source is disposed at expression join.

use super::*;
use arena::Handle;
use checked_trees::{
    FlowOwnedSelectionReceipt, FlowOwnedSelectionSource, FlowOwnedSelectionTransfer,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern, TableMatchArm};

pub(super) fn record_statement(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    places: &mut [LinearPlace],
) -> Result<bool, Diagnostic> {
    let expressions = statement_expressions(program, statement);
    for (_, receipt) in facts
        .flow
        .ownership
        .owned_selections
        .iter()
        .filter(|(_, receipt)| {
            receipt.state == state.symbol && (receipt.statement_ordinal as usize) < statement_index
        })
    {
        for source in facts
            .flow
            .ownership
            .selection_sources
            .span_or_empty(receipt.sources)
        {
            if expressions
                .iter()
                .any(|expression| names_symbol(program, *expression, source.symbol))
                || matches!(statement, StatementNode::Call(call) if call.receiver_root_symbol == source.symbol)
            {
                return Err(Diagnostic::error(
                    "owned value may have been transferred by an earlier match; it cannot be used here",
                ));
            }
        }
    }
    let selected = expressions
        .iter()
        .copied()
        .filter(|expression| {
            matches!(
                program.expression_table.expression(*expression),
                ExpressionNode::Match(_)
            ) && has_owned_leaf(program, machine, state, *expression)
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Ok(false);
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let (expression, destination, type_reference) = match statement {
        StatementNode::LocalData(local) if !local.is_mutable => {
            (local.initial_value, local.symbol, local.type_reference)
        }
        StatementNode::Expression(expression) if statement_index + 1 == statements.len() => {
            (*expression, SymbolHandle::invalid(), state.return_type)
        }
        _ => return Err(unsupported()),
    };
    if !selected.contains(&expression)
        || !validation::has_plain_owned_contents_with_numeric_constraints(program, type_reference)
        || program.type_multiplicity(type_reference) != Multiplicity::Affine
    {
        return Err(unsupported());
    }
    // The source destination owns one carrier on every selected path. Nested
    // selections recurse into their own authored arms, without expanding paths.
    let mut leaves = Vec::new();
    collect_leaves(
        program,
        machine,
        state,
        expression,
        type_reference,
        Handle::invalid(),
        &mut leaves,
    )?;
    let mut sources = Vec::new();
    for (_, _, symbol) in &leaves {
        if sources
            .iter()
            .any(|source: &FlowOwnedSelectionSource| source.symbol == *symbol)
        {
            continue;
        }
        let (source_ordinal, source_local) = statements[..statement_index]
            .iter()
            .enumerate()
            .find_map(|(ordinal, statement)| match statement {
                StatementNode::LocalData(source) if source.symbol == *symbol => {
                    Some((ordinal, source))
                }
                _ => None,
            })
            .ok_or_else(unsupported)?;
        let place = places
            .iter()
            .find(|place| place.symbol == *symbol && place.path.is_empty())
            .ok_or_else(unsupported)?;
        if source_local.is_mutable
            || !source_local.initial_value.is_valid()
            || !place.live
            || !place.ever_established
            || place.multiplicity != Multiplicity::Affine
            || place.conditional
            || !validation::has_plain_owned_contents_with_numeric_constraints(
                program,
                source_local.type_reference,
            )
            || program.normalized_type_identity(source_local.type_reference)
                != program.normalized_type_identity(type_reference)
        {
            return Err(Diagnostic::error(
                "owned match source must be an available whole immutable plain-affine local of the exact result type",
            ));
        }
        // Loan-bearing selection is outside this receipt's whole-owned scope.
        // Existing borrow checking remains authoritative; do not admit a new
        // conditional move while relying on an unrepresented loan closure.
        if statements[..statement_index].iter().any(|statement| {
            statement_expressions(program, statement).iter().any(|expression| {
                matches!(program.expression_table.expression(*expression), ExpressionNode::Borrow(borrow)
                    if expression_names(program, borrow.target, *symbol))
            })
        }) {
            return Err(Diagnostic::error("owned match source with borrowed custody requires selected loan-closure evidence"));
        }
        let origin_selection = facts
            .flow
            .ownership
            .owned_selection_at(state.symbol, source_ordinal as u32)
            .map(|(handle, _)| handle)
            .unwrap_or_default();
        let provenance = place.provenance.unwrap_or(PermissionProvenance::Unknown);
        if !origin_selection.is_valid() && provenance == PermissionProvenance::Unknown {
            return Err(Diagnostic::error(
                "owned match source has no exact incoming origin",
            ));
        }
        sources.push(FlowOwnedSelectionSource {
            symbol: *symbol,
            statement_ordinal: u32::try_from(source_ordinal).map_err(|_| unsupported())?,
            provenance,
            claim_identity: place
                .claim_identity
                .unwrap_or(PermissionClaimIdentity::Unknown),
            origin_selection,
        });
    }
    sources.sort_by_key(|source| std::cmp::Reverse(source.statement_ordinal));
    let sources = facts.flow.ownership.selection_sources.insert_many(sources);
    let mut transfers = Vec::new();
    for (expression, source_arm, symbol) in leaves {
        let ordinal = facts
            .flow
            .ownership
            .selection_sources
            .span_or_empty(sources)
            .iter()
            .position(|source| source.symbol == symbol)
            .ok_or_else(unsupported)?;
        let source = Handle::from_parts(
            sources.start().arena_index() + ordinal as u32,
            sources.start().generation(),
        );
        transfers.push(FlowOwnedSelectionTransfer {
            expression,
            source_arm,
            source,
        });
    }
    let transfers = facts
        .flow
        .ownership
        .selection_transfers
        .insert_many(transfers);
    let receipt = FlowOwnedSelectionReceipt {
        machine: machine.symbol,
        state: state.symbol,
        statement_ordinal: u32::try_from(statement_index).map_err(|_| unsupported())?,
        expression,
        destination,
        type_reference,
        sources,
        transfers,
        death: PermissionEventSource::StateExit,
    };
    apply_availability(&facts.flow.ownership, &receipt, places);
    facts.flow.ownership.owned_selections.append(receipt);
    Ok(true)
}

pub(super) fn apply_availability(
    ownership: &checked_trees::FlowOwnershipFacts,
    receipt: &FlowOwnedSelectionReceipt,
    places: &mut [LinearPlace],
) {
    // `live` here is the all-path availability summary, not disposition. The
    // conditional complement remains owed in the receipt's source roster.
    for source in ownership.selection_sources.span_or_empty(receipt.sources) {
        if let Some(place) = places
            .iter_mut()
            .find(|place| place.symbol == source.symbol && place.path.is_empty())
        {
            place.live = false;
        }
    }
    if let Some(place) = places
        .iter_mut()
        .find(|place| place.symbol == receipt.destination && place.path.is_empty())
    {
        place.live = true;
        place.ever_established = true;
        place.provenance = Some(PermissionProvenance::Unknown);
        place.claim_identity = Some(PermissionClaimIdentity::Unknown);
    }
}

fn unsupported() -> Diagnostic {
    Diagnostic::error(
        "owned match requires one existing whole plain-affine local per path and an immutable local or return destination; fresh/existing mixtures, projections, calls and borrowed/linear custody require additional ownership evidence",
    )
}

fn has_owned_leaf(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => reachable_arms(program, dispatch.arms)
            .iter()
            .any(|(_, arm)| has_owned_leaf(program, machine, state, arm.value)),
        ExpressionNode::Name(_) => {
            validation::expression_result_type_reference(program, machine, state, expression)
                .is_some_and(|reference| {
                    program.type_multiplicity(reference) != Multiplicity::Unrestricted
                        && validation::scalar_case_value_source(program, expression, reference)
                            .is_some()
                })
        }
        _ => false,
    }
}

fn collect_leaves(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
    source_arm: Handle<TableMatchArm>,
    leaves: &mut Vec<(ExpressionHandle, Handle<TableMatchArm>, SymbolHandle)>,
) -> Result<(), Diagnostic> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) if source_arm.is_valid() => {
            let symbol = validation::scalar_case_value_source(program, expression, type_reference)
                .ok_or_else(unsupported)?;
            leaves.push((expression, source_arm, symbol));
            Ok(())
        }
        ExpressionNode::Match(dispatch) => {
            // Selection predicates cannot introduce ownership effects hidden
            // outside the selected value graph. Scalar calls need their own
            // captured effect/loan sequence before joining this route.
            let mut operands = expression_nodes(program, dispatch.subject);
            for (_, arm) in reachable_arms(program, dispatch.arms) {
                if let MatchPattern::Value(pattern) = arm.pattern {
                    operands.extend(expression_nodes(program, pattern));
                }
            }
            if operands.iter().any(|operand| {
                matches!(
                    program.expression_table.expression(*operand),
                    ExpressionNode::Call(_) | ExpressionNode::Borrow(_) | ExpressionNode::Atomic(_)
                )
            }) {
                return Err(unsupported());
            }
            if operands.iter().any(|operand| {
                matches!(
                    program.expression_table.expression(*operand),
                    ExpressionNode::Name(_)
                ) && validation::expression_result_type_reference(program, machine, state, *operand)
                    .is_none_or(|reference| {
                        program.type_multiplicity(reference) != Multiplicity::Unrestricted
                    })
            }) {
                return Err(Diagnostic::error(
                    "owned match predicates require unrestricted operands without hidden ownership transfers",
                ));
            }
            for (source_arm, arm) in reachable_arms(program, dispatch.arms) {
                collect_leaves(
                    program,
                    machine,
                    state,
                    arm.value,
                    type_reference,
                    source_arm,
                    leaves,
                )?;
            }
            if leaves.is_empty() {
                return Err(unsupported());
            }
            Ok(())
        }
        _ => Err(unsupported()),
    }
}

fn names_symbol(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    matches!(program.expression_table.expression(expression), ExpressionNode::Name(path) if path.symbol == symbol)
}

fn expression_names(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    expression_nodes(program, expression)
        .iter()
        .any(|expression| names_symbol(program, *expression, symbol))
}

fn statement_expressions(
    program: &typed_trees::TypedTrees,
    statement: &StatementNode,
) -> Vec<ExpressionHandle> {
    let roots = match statement {
        StatementNode::LocalData(local) => vec![local.initial_value],
        StatementNode::Expression(expression) => vec![*expression],
        StatementNode::Assignment(assignment) => vec![assignment.target, assignment.value],
        StatementNode::Call(call) => program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec(),
        StatementNode::AssemblyFact(fact) => vec![fact.expression],
        StatementNode::Transition(transition) => {
            let mut roots = Vec::new();
            if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                roots.push(guard);
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    typed_trees::statement::TransitionTargetNode::Value(value) => {
                        roots.push(*value)
                    }
                    typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => roots
                        .extend_from_slice(program.expression_table.expression_handles(*arguments)),
                    _ => {}
                }
            }
            roots
        }
    };
    roots
        .into_iter()
        .flat_map(|root| expression_nodes(program, root))
        .collect()
}

fn expression_nodes(
    program: &typed_trees::TypedTrees,
    root: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    let mut pending = vec![root];
    let mut nodes = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || nodes.contains(&expression) {
            continue;
        }
        nodes.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                pending.push(dispatch.subject);
                for (_, arm) in reachable_arms(program, dispatch.arms) {
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Atomic(atomic) => pending.extend([atomic.value, atomic.result]),
            ExpressionNode::ArrayLiteral(values) => {
                pending.extend_from_slice(program.expression_table.expression_handles(*values))
            }
            ExpressionNode::Call(call) => {
                pending.push(call.receiver);
                pending
                    .extend_from_slice(program.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Range(range) => pending.extend([range.start, range.end]),
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            _ => {}
        }
    }
    nodes
}

/// Ordered first-match reachability. The source type checker still visits all
/// authored arms. Only literal equality and complete Boolean/wildcard coverage
/// remove ownership alternatives here; no predicate theorem is invented.
fn reachable_arms(
    program: &typed_trees::TypedTrees,
    arms: HandleSpan<TableMatchArm>,
) -> Vec<(Handle<TableMatchArm>, &TableMatchArm)> {
    let mut selected = Vec::new();
    let mut patterns = Vec::new();
    let mut booleans = [false; 2];
    for (ordinal, arm) in program.expression_table.match_arms(arms).iter().enumerate() {
        let mut covered = false;
        match arm.pattern {
            MatchPattern::Wildcard => covered = true,
            MatchPattern::Value(pattern) => {
                let node = program.expression_table.expression(pattern);
                let duplicate = patterns.iter().any(|previous| {
                    match (program.expression_table.expression(*previous), node) {
                        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => {
                            left == right
                        }
                        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left
                            .value_bignum()
                            .zip(right.value_bignum())
                            .is_some_and(|(left, right)| left == right),
                        _ => false,
                    }
                });
                if duplicate {
                    continue;
                }
                patterns.push(pattern);
                if let ExpressionNode::Boolean(value) = node {
                    booleans[usize::from(*value)] = true;
                    covered = booleans == [true, true];
                }
            }
        }
        let handle = Handle::from_parts(
            arms.start().arena_index() + ordinal as u32,
            arms.start().generation(),
        );
        selected.push((handle, arm));
        if covered {
            break;
        }
    }
    selected
}
