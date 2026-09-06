use super::*;

impl RangeFacts<'_> {
    pub(in crate::checks::ranges) fn alias_integer_place_value(
        &mut self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        symbol: SymbolHandle,
        name: &str,
    ) {
        if !matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        ) || !is_integer_value(program, machine, state, expression)
        {
            return;
        }
        let mut reads = Vec::new();
        if collect_reads(
            program,
            machine,
            state,
            self.statement_index,
            expression,
            &mut reads,
            0,
        ) {
            self.alias_index(&program.expression_table.display_name(expression), name);
            self.alias_captured_selector(program, machine, state, expression, symbol);
        }
    }
    /// Rejoin actual typed uses of a selector copied now. Never expand an old
    /// initializer later or substitute names inside rendered source.
    fn alias_captured_selector(
        &mut self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        captured: ExpressionHandle,
        symbol: SymbolHandle,
    ) {
        if !symbol.is_valid() {
            return;
        }
        let mut captured_reads = Vec::new();
        if !collect_reads(
            program,
            machine,
            state,
            self.statement_index,
            captured,
            &mut captured_reads,
            0,
        ) {
            return;
        }
        let transferable = self.preserved_expression_labels(program, machine, state, Some(&[]));
        let originals = self.expression_dependencies.clone();
        for original in originals {
            if !transferable.contains(&original.label) {
                continue;
            }
            let ExpressionNode::Indexed(indexed) =
                program.expression_table.expression(original.expression)
            else {
                continue;
            };
            if !program
                .expression_table
                .expressions_structurally_equal(indexed.index, captured)
            {
                continue;
            }
            let mut selector_reads = Vec::new();
            if !collect_reads(
                program,
                machine,
                state,
                self.statement_index,
                indexed.index,
                &mut selector_reads,
                0,
            ) || !same_reads(program, Some(&captured_reads), Some(&selector_reads))
            {
                continue;
            }
            for (expression, node) in program.expression_table.iter_expressions() {
                let ExpressionNode::Indexed(candidate) = node else {
                    continue;
                };
                let ExpressionNode::Name(selector) =
                    program.expression_table.expression(candidate.index)
                else {
                    continue;
                };
                let selector_symbol = crate::lookup::first_valid_name_path_symbol(
                    selector,
                    &program.expression_table,
                );
                let same_snapshot = selector.symbol.is_valid()
                    && selector.head_symbol == selector.symbol
                    && selector_symbol == Some(selector.symbol)
                    && integer_value_identity(program, state, candidate.index).is_some_and(
                        |value| {
                            value == symbol
                                || integer_value_identity(program, state, captured) == Some(value)
                        },
                    );
                if (selector_symbol != Some(symbol) && !same_snapshot)
                    || program
                        .expression_table
                        .name_path_members(selector.members)
                        .len()
                        != 1
                    || !program
                        .expression_table
                        .expressions_structurally_equal(indexed.collection, candidate.collection)
                {
                    continue;
                }
                let mut reads = Vec::new();
                // The current binding now exists. Later immutable copies may
                // already have typed uses but do not introduce a new value.
                // Read those uses through the shared captured-value identity,
                // then require every resulting dependency to exist now. This
                // does not evaluate a future initializer or admit a new read.
                let prefix = self.statement_index.saturating_add(1);
                if !collect_reads(
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
                    &mut reads,
                    0,
                ) || !reads
                    .iter()
                    .all(|read| reads::root_is_current(program, machine, state, prefix, read.root))
                {
                    continue;
                }
                let label = program.expression_table.display_name(expression);
                self.alias_index(&original.label, &label);
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
    program: &TypedTrees,
    state: &State,
    mut expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let value =
        validation::immutable_integer_bound_value_symbol(program, expression).or_else(|| {
            let normalized =
                validation::normalize_immutable_integer_bound_expression(program, expression)?;
            let ExpressionNode::Name(path) = program.expression_table.expression(normalized) else {
                return None;
            };
            Some(path.symbol)
        })?;
    for _ in 0..128 {
        let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
            return None;
        };
        if !path.symbol.is_valid()
            || path.head_symbol != path.symbol
            || program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
        {
            return None;
        }
        let mut locals = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .filter_map(|statement| match statement {
                typed_trees::statement::StatementNode::LocalData(local)
                    if local.symbol == path.symbol =>
                {
                    Some(local)
                }
                _ => None,
            });
        let Some(local) = locals.next() else {
            let mut parameters = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| parameter.symbol == path.symbol);
            let parameter = parameters.next()?;
            return (parameter.symbol == value
                && !parameter.is_mutable
                && parameters.next().is_none())
            .then_some(value);
        };
        if local.is_mutable || locals.next().is_some() {
            return None;
        }
        if local.symbol == value {
            return Some(value);
        }
        expression = local.initial_value;
    }
    None
}

pub(super) fn is_integer_value(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    use symbols::BuiltinTypeAtom;
    use typed_trees::types::TypeReferenceNode;
    let Some(mut reference) = crate::checks::ranges::types::expression_type_reference(
        program, machine, state, expression,
    ) else {
        return false;
    };
    for _ in 0..128 {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                return matches!(
                    program.symbols.builtin_type_atom(*symbol),
                    Some(
                        BuiltinTypeAtom::I8
                            | BuiltinTypeAtom::I16
                            | BuiltinTypeAtom::I32
                            | BuiltinTypeAtom::I64
                            | BuiltinTypeAtom::U8
                            | BuiltinTypeAtom::U16
                            | BuiltinTypeAtom::U32
                            | BuiltinTypeAtom::U64
                    )
                );
            }
            // A copied reference still observes storage; it is not a captured
            // integer value. Atomic and aggregate carriers are not snapshots.
            _ => return false,
        }
    }
    false
}
