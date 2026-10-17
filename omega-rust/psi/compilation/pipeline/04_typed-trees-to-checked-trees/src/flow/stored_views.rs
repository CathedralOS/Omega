//! Exact view addresses come from completed stores. The same query serves
//! operand evaluation and later effect proofs; neither may replay an initializer
//! or replace an unresolved newer descriptor with a historical source address.
use arena::HandleSpan;
use checked_trees::{FlowControlFacts, FlowOperandReferent, FlowStatementFact};
use validation::CanonicalPlace;

pub(crate) fn before_statement(
    program: &dyn validation::CheckingInput,
    semantic: &facts::FactPlan,
    control: &FlowControlFacts,
    state: &typed_trees::state::State,
    statements: HandleSpan<FlowStatementFact>,
    before: usize,
    writes: HandleSpan<FlowOperandReferent>,
    unknown_write: bool,
    source: &CanonicalPlace,
) -> Option<Vec<CanonicalPlace>> {
    let is_view = |reference| super::operand_contexts::operand_binds_live_view(program, reference);
    // Incoming exclusive parameters lend fixed backing, not their outer
    // descriptor slot. An immutable shared parameter is fixed too; a mutable
    // shared binding instead follows its reaching descriptor replacement.
    if source.segments.is_empty()
        && source.root.as_symbol().is_some_and(|symbol| {
            program
                .state_parameter_by_symbol(state, symbol)
                .is_some_and(|parameter| {
                    validation::reference_shell(program, parameter.type_reference).is_some_and(
                        |shell| {
                            shell.access != language_semantics::ReferenceAccess::Shared
                                || !parameter.is_mutable
                        },
                    )
                })
        })
    {
        return None;
    }
    if !super::operand_contexts::binding_origin_is_stable(
        program,
        control,
        writes,
        unknown_write,
        source,
    ) {
        return Some(Vec::new());
    }
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let end = program
        .statement_table
        .statements(state.statement_nodes)
        .len();
    for statement in control
        .statements
        .span_or_empty(statements)
        .iter()
        .rev()
        .filter(|row| {
            order.precedes(row.statement_index, before)
                || before == end && order.position(row.statement_index).is_some()
        })
    {
        if semantic.places.is_valid(statement.stored_destination)
            && control.operands.is_valid(statement.stored_value)
        {
            let destination = super::canonical_place_from_semantic_place(
                program,
                semantic,
                semantic.places.get(statement.stored_destination),
            )?;
            if destination.root == source.root && source.segments.starts_with(&destination.segments)
            {
                let stored = control.operands.get(statement.stored_value);
                let path = &source.segments[destination.segments.len()..];
                let (captured, reference, projection) = if is_view(stored.type_reference) {
                    (stored, stored.type_reference, path)
                } else {
                    let Some((captured, reference)) =
                        super::operand_values::captured_operand_at_path(
                            program,
                            control,
                            stored,
                            stored.type_reference,
                            path,
                        )
                    else {
                        return Some(Vec::new());
                    };
                    (captured, reference, &[][..])
                };
                if !is_view(reference) {
                    return None;
                }
                let places = control
                    .operand_referents
                    .span_or_empty(captured.referents)
                    .iter()
                    .map(|referent| {
                        super::operand_values::project_captured_view_place(
                            CanonicalPlace {
                                root: referent.root,
                                segments: control
                                    .operand_referent_segments
                                    .span_or_empty(referent.segments)
                                    .to_vec(),
                            },
                            projection,
                        )
                    })
                    .collect::<Option<Vec<_>>>();
                return Some(places.unwrap_or_default());
            }
        }
        if !super::operand_contexts::binding_origin_is_stable(
            program,
            control,
            statement.completed_writes,
            statement.completed_unknown_write,
            source,
        ) {
            return Some(Vec::new());
        }
    }
    source
        .root
        .as_symbol()
        .and_then(|root| program.state_local_declaration(state, root))
        .filter(|(_, local)| is_view(local.type_reference))
        .map(|_| Vec::new())
}

/// Resolve only descriptor crossings on an address. Writes to an atomic
/// cell's contents do not replace its containing record's reference binding.
pub(crate) fn address_before_statement(
    program: &dyn validation::CheckingInput,
    semantic: &facts::FactPlan,
    control: &FlowControlFacts,
    state: &typed_trees::state::State,
    statements: HandleSpan<FlowStatementFact>,
    before: usize,
    source: CanonicalPlace,
) -> Option<Vec<CanonicalPlace>> {
    let mut pending = vec![(source, 0usize)];
    let mut output = Vec::new();
    while let Some((source, depth)) = pending.pop() {
        if depth >= 64 {
            return None;
        }
        let mut replaced = false;
        for length in 0..=source.segments.len() {
            let prefix = CanonicalPlace {
                root: source.root,
                segments: source.segments[..length].to_vec(),
            };
            let reference =
                validation::canonical_place_type_reference(program, state.symbol, before, &prefix)?;
            if !super::operand_contexts::operand_binds_live_view(program, reference) {
                continue;
            }
            let Some(places) = before_statement(
                program,
                semantic,
                control,
                state,
                statements,
                before,
                Default::default(),
                false,
                &prefix,
            ) else {
                continue;
            };
            if places.len() == 1 && places[0] == prefix {
                continue;
            }
            for mut place in places {
                place.segments.extend_from_slice(&source.segments[length..]);
                pending.push((place, depth + 1));
            }
            replaced = true;
            break;
        }
        if !replaced && !output.contains(&source) {
            output.push(source);
        }
    }
    Some(output)
}
