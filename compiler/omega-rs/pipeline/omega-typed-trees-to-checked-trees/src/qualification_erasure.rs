use omega_core::semantics::DomainEstablishmentRoute;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::ExpressionNode;
use omega_typed_trees::statement::StatementNode;

/// Erase every validated canonical representation-qualification use.
///
/// Facts are built before this pass, so checked artifacts retain the selected
/// domain and satisfier even though executable consumers see only the original
/// argument expression. Replacing the use root preserves evaluation of that
/// argument while making the witness machine compile-time-only.
pub(crate) fn erase_canonical_qualification_uses(program: &mut TypedTrees) {
    let targets = canonical_satisfier_targets(program);
    if targets.is_empty() {
        return;
    }

    loop {
        let replacements = program
            .expression_table
            .expression_entries()
            .filter_map(|(handle, expression)| {
                let argument = match expression {
                    ExpressionNode::Cast(cast) if cast.qualification_satisfier.is_valid() => {
                        cast.value
                    }
                    ExpressionNode::Call(call)
                        if targets.contains(&call.target_symbol.arena_index()) =>
                    {
                        let [argument] =
                            program.expression_table.expression_handles(call.arguments)
                        else {
                            return None;
                        };
                        *argument
                    }
                    _ => return None,
                };
                Some((
                    handle,
                    program.expression_table.expression(argument).clone(),
                ))
            })
            .collect::<Vec<_>>();
        if replacements.is_empty() {
            break;
        }
        for (handle, replacement) in replacements {
            *program.expression_table.expression_mut(handle) = replacement;
        }
    }

    let state_spans = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .map(|state| state.statement_nodes)
        .collect::<Vec<_>>();
    for span in state_spans {
        let replacements = program
            .statement_table
            .statements(span)
            .iter()
            .enumerate()
            .filter_map(|(index, statement)| {
                let StatementNode::Call(call) = statement else {
                    return None;
                };
                if !targets.contains(&call.target_symbol.arena_index()) {
                    return None;
                }
                let [argument] = program.statement_table.expression_handles(call.arguments) else {
                    return None;
                };
                Some((index, StatementNode::Expression(*argument)))
            })
            .collect::<Vec<_>>();
        let statements = program.statement_table.statements_mut(span);
        for (index, replacement) in replacements {
            statements[index] = replacement;
        }
    }
}

fn canonical_satisfier_targets(program: &TypedTrees) -> Vec<u32> {
    let satisfiers = program
        .domain_definitions()
        .iter()
        .flat_map(|domain| &domain.establishment_routes)
        .filter_map(|route| match route {
            DomainEstablishmentRoute::CanonicalQualification { satisfier } => Some(*satisfier),
            _ => None,
        })
        .collect::<Vec<SymbolHandle>>();

    let mut targets = Vec::new();
    for satisfier in satisfiers {
        targets.push(satisfier.arena_index());
        if let Some(entry) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == satisfier)
            .and_then(|machine| program.machine_states(machine).first())
        {
            targets.push(entry.symbol.arena_index());
        }
    }
    targets.sort_unstable();
    targets.dedup();
    targets
}
