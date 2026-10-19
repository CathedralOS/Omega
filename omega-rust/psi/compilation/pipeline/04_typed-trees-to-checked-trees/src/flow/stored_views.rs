//! Exact view addresses come from completed stores. The same query serves
//! operand evaluation and later effect proofs; neither may replay an initializer
//! or replace an unresolved newer descriptor with a historical source address.
use arena::HandleSpan;
use checked_trees::{FlowControlFacts, FlowOperandReferent, FlowStatementFact};
use validation::CanonicalPlace;

use super::stored_values::{StoredObservation, StoredOperand};

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
    let StoredOperand::Value {
        capture,
        reference,
        projection,
    } = capture_before_statement(
        program,
        semantic,
        control,
        state,
        statements,
        before,
        writes,
        unknown_write,
        source,
    )?
    else {
        return Some(Vec::new());
    };
    let capture = control.operands.get(capture);
    // This consumer needs actual descriptor contents. A lifetime-only leaf
    // does not supply an address, even though the reaching store is known.
    let (capture, reference, projection) =
        if super::operand_contexts::operand_binds_live_view(program, reference) {
            (capture, reference, projection.as_slice())
        } else {
            let Some((selected, reference)) = super::operand_values::captured_operand_at_path(
                program,
                control,
                capture,
                reference,
                &projection,
            ) else {
                return Some(Vec::new());
            };
            (selected, reference, &[][..])
        };
    if !super::operand_contexts::operand_binds_live_view(program, reference) {
        return None;
    }
    let places = control
        .operand_referents
        .span_or_empty(capture.referents)
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
                &projection,
            )
        })
        .collect::<Option<Vec<_>>>();
    Some(places.unwrap_or_default())
}

pub(crate) fn capture_before_statement(
    program: &dyn validation::CheckingInput,
    semantic: &facts::FactPlan,
    control: &FlowControlFacts,
    state: &typed_trees::state::State,
    statements: HandleSpan<FlowStatementFact>,
    before: usize,
    writes: HandleSpan<FlowOperandReferent>,
    unknown_write: bool,
    source: &CanonicalPlace,
) -> Option<StoredOperand> {
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
    if let Some(stored) = super::stored_values::before_statement(
        program,
        semantic,
        control,
        state,
        statements,
        before,
        writes,
        unknown_write,
        source,
        StoredObservation::Binding,
    ) {
        return Some(stored);
    }
    source
        .root
        .as_symbol()
        .and_then(|root| program.state_local_declaration(state, root))
        .filter(|(_, local)| is_view(local.type_reference))
        .map(|_| StoredOperand::Unavailable)
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
