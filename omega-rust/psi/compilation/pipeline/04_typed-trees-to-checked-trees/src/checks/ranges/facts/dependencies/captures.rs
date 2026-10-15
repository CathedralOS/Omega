use super::{
    EXPRESSION_WALK_DEPTH_BOUND, ExpressionHandle, ExpressionNode, Machine, State, SymbolHandle,
};
use crate::checks::ranges::RangeFacts;
use crate::checks::ranges::facts::dependencies::ExpressionDependencies;
use crate::checks::ranges::facts::dependencies::collect_reads;
use crate::checks::ranges::facts::dependencies::reads;
use crate::checks::ranges::facts::dependencies::same_reads;

impl<'field> RangeFacts<'field> {
    pub(in crate::checks::ranges) fn alias_integer_value(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        symbol: SymbolHandle,
        name: &str,
    ) {
        if !has_scalar_snapshot_meaning(program, machine, state, expression, 0)
            || !is_integer_value(program, machine, state, expression)
        {
            return;
        }
        let mut reads = Vec::new();
        let bound_lookup = self.bound_lookup();
        if collect_reads(
            program,
            machine,
            state,
            self.statement_index,
            expression,
            self.checked_calls,
            self.checked_operators,
            &mut reads,
            0,
            bound_lookup,
        ) {
            self.alias_index(&program.expression_table.display_name(expression), name);
            self.alias_captured_selector(program, machine, state, expression, symbol);
        }
    }
    /// Rejoin actual typed uses of a selector copied now. Never expand an old
    /// initializer later or substitute names inside rendered source.
    fn alias_captured_selector(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        captured: ExpressionHandle,
        symbol: SymbolHandle,
    ) {
        if !symbol.is_valid() {
            return;
        }
        let mut captured_reads = Vec::new();
        let bound_lookup = self.bound_lookup();
        if !collect_reads(
            program,
            machine,
            state,
            self.statement_index,
            captured,
            self.checked_calls,
            self.checked_operators,
            &mut captured_reads,
            0,
            bound_lookup,
        ) {
            return;
        }
        let transferable = self.preserved_expression_labels(program, machine, state, Some(&[]));
        let originals = self.expression_dependencies.clone();
        // The bound index scans the whole program once; the candidate walk
        // below asks for an identity per expression node per original, so it
        // borrows the worker-owned lookup rather than rebuild per original.
        let bound_lookup = self.bound_lookup();
        let mut matched_sources = Vec::new();
        for original in originals {
            if !transferable.contains(&original.label) {
                continue;
            }
            // A length premise records the metadata observation's reads, but
            // its extent facts belong to the receiver. Retain that premise's
            // preservation judgment while matching the receiver's typed path.
            let source = validation::collection_length_receiver(
                program,
                machine,
                Some(state),
                original.expression,
            )
            .unwrap_or(original.expression);
            if matched_sources.contains(&source) {
                continue;
            }
            matched_sources.push(source);
            if !validation::place_has_builtin_coordinates(program, machine, Some(state), source) {
                continue;
            }
            let Some(source_place) = validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                self.statement_index,
                source,
            ) else {
                continue;
            };
            if !source_place.segments.iter().any(|segment| {
                matches!(segment, facts::PlaceSegment::Index { expression }
                    if program.expression_table.expressions_structurally_equal(*expression, captured))
            }) {
                continue;
            }
            let source_label = program.expression_table.display_name(source);
            for (expression, _) in program.expression_table.iter_expressions() {
                let Some(mut candidate) = validation::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    self.statement_index.saturating_add(1),
                    expression,
                ) else {
                    continue;
                };
                if candidate.root != source_place.root
                    || candidate.segments.len() != source_place.segments.len()
                {
                    continue;
                }
                let mut rebound = false;
                let mut same_snapshot = false;
                for (source_segment, candidate_segment) in
                    source_place.segments.iter().zip(&mut candidate.segments)
                {
                    let (
                        facts::PlaceSegment::Index {
                            expression: source_index,
                        },
                        facts::PlaceSegment::Index {
                            expression: candidate_index,
                        },
                    ) = (source_segment, &*candidate_segment)
                    else {
                        continue;
                    };
                    if !program
                        .expression_table
                        .expressions_structurally_equal(*source_index, captured)
                    {
                        continue;
                    }
                    let Some(selector) = program.expression_table.name_path_node(*candidate_index)
                    else {
                        continue;
                    };
                    let selector_symbol = validation::first_valid_name_path_symbol(
                        selector,
                        &program.expression_table,
                    );
                    let shared_snapshot = selector.symbol.is_valid()
                        && selector.head_symbol == selector.symbol
                        && selector_symbol == Some(selector.symbol)
                        && integer_value_identity(program, bound_lookup, state, *candidate_index)
                            .is_some_and(|value| {
                                value == symbol
                                    || integer_value_identity(
                                        program,
                                        bound_lookup,
                                        state,
                                        captured,
                                    ) == Some(value)
                            });
                    if (selector_symbol != Some(symbol) && !shared_snapshot)
                        || program
                            .expression_table
                            .name_path_members(selector.members)
                            .len()
                            != 1
                    {
                        continue;
                    }
                    let mut selector_reads = Vec::new();
                    if !collect_reads(
                        program,
                        machine,
                        state,
                        self.statement_index,
                        *source_index,
                        self.checked_calls,
                        self.checked_operators,
                        &mut selector_reads,
                        0,
                        bound_lookup,
                    ) || !same_reads(program, Some(&captured_reads), Some(&selector_reads))
                    {
                        continue;
                    }
                    *candidate_segment = source_segment.clone();
                    rebound = true;
                    same_snapshot |= shared_snapshot;
                }
                // Compare the entire typed path after replacing only the
                // selector captured now. Enclosing members and other indices
                // must still agree; a sibling descriptor never inherits bounds.
                if !rebound
                    || !crate::flow::canonical_places_agree(program, &source_place, &candidate)
                    || !validation::place_has_builtin_coordinates(
                        program,
                        machine,
                        Some(state),
                        expression,
                    )
                {
                    continue;
                }
                let mut reads = Vec::new();
                let bound_lookup = self.bound_lookup();
                // The current binding now exists. Later immutable copies may
                // already have typed uses but do not introduce a new value.
                // Read those uses through the shared captured-value identity,
                // then require every resulting dependency to exist now. This
                // does not evaluate a future initializer or admit a new read.
                let prefix = self.statement_index.saturating_add(1);
                let complete = collect_reads(
                    program,
                    machine,
                    state,
                    if same_snapshot {
                        program
                            .statement_table
                            .statements(state.statement_nodes)
                            .len()
                    } else {
                        prefix
                    },
                    expression,
                    self.checked_calls,
                    self.checked_operators,
                    &mut reads,
                    0,
                    bound_lookup,
                );
                if !complete
                    || !reads.iter().all(|read| {
                        reads::root_is_current(program, machine, state, prefix, read.root)
                    })
                {
                    continue;
                }
                let label = program.expression_table.display_name(expression);
                self.alias_index(&source_label, &label);
                self.prove_window_parent(label.clone(), source_label.clone(), None);
                self.alias_collection(&source_label, &label);
                if !self.expression_dependencies.iter().any(|row| {
                    row.expression == expression
                        && row.machine == machine.symbol
                        && row.state == state.symbol
                }) {
                    self.expression_dependencies.push(ExpressionDependencies {
                        expression,
                        label,
                        machine: machine.symbol,
                        state: state.symbol,
                        descriptor: original.descriptor.as_ref().and_then(|_| {
                            validation::canonical_place_from_expression_in_state(
                                program,
                                state.symbol,
                                prefix,
                                expression,
                            )
                        }),
                        reads: Some(reads),
                    });
                }
            }
        }
    }
}

/// Reuse the shared snapshot identity only through exact, immutable local
/// copies in this state. Its contextual name fallback cannot supply missing
/// typed identities for a dependency-preservation grant.
pub(super) fn integer_value_identity(
    program: &dyn ::validation::CheckingInput,
    bound_lookup: &validation::ImmutableBoundLookup<'_>,
    state: &State,
    mut expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let value = validation::immutable_integer_bound_value_symbol(program, bound_lookup, expression)
        .or_else(|| {
            let normalized = validation::normalize_immutable_integer_bound_expression(
                program,
                bound_lookup,
                expression,
            )?;
            let path = program.expression_table.name_path_node(normalized)?;
            Some(path.symbol)
        })?;
    for _ in 0..EXPRESSION_WALK_DEPTH_BOUND {
        let Some(symbol) = ::validation::single_segment_name_symbol(program, expression) else {
            return None;
        };
        let Some((_, local)) = program.state_local_declaration(state, symbol) else {
            let mut parameters = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| parameter.symbol == symbol);
            let parameter = parameters.next()?;
            return (parameter.symbol == value
                && !parameter.is_mutable
                && parameters.next().is_none())
            .then_some(value);
        };
        if local.is_mutable {
            return None;
        }
        if local.symbol == value {
            return Some(value);
        }
        expression = local.initial_value;
    }
    None
}

/// A copied selector can preserve a pure computation just as it preserves a
/// direct parameter read. Calls and atomic observations need occurrence-specific
/// result evidence; matching their operands does not establish the same value.
/// This does not replay an initializer: the caller captures at this statement
/// and retains the collected reads for subsequent write invalidation.
pub(super) fn has_scalar_snapshot_meaning(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= EXPRESSION_WALK_DEPTH_BOUND
        || !program.expression_table.expression_is_valid(expression)
    {
        return false;
    }
    let child =
        |expression| has_scalar_snapshot_meaning(program, machine, state, expression, depth + 1);
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) | ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
        ExpressionNode::Member(member) => child(member.receiver),
        ExpressionNode::Indexed(indexed) => {
            validation::place_has_builtin_coordinates(program, machine, Some(state), expression)
                && child(indexed.collection)
                && child(indexed.index)
        }
        ExpressionNode::Binary(binary) => {
            validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) && child(binary.left)
                && child(binary.right)
        }
        ExpressionNode::Unary(unary) => child(unary.operand),
        ExpressionNode::Cast(cast) => child(cast.value),
        _ => false,
    }
}

pub(super) fn is_integer_value(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    use typed_trees::types::TypeReferenceNode;
    let Some(mut reference) =
        validation::expression_result_type_reference(program, machine, state, expression)
    else {
        return false;
    };
    for _ in 0..EXPRESSION_WALK_DEPTH_BOUND {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                return program
                    .symbols
                    .builtin_type_atom(*symbol)
                    .is_some_and(|atom| atom.is_fixed_integer());
            }
            // A copied reference still observes storage; it is not a captured
            // integer value. Atomic and aggregate carriers are not snapshots.
            _ => return false,
        }
    }
    false
}
