//! Instantiate moved result subtrees as values, not independent possible
//! reference leaves. A missing input case removes its entire payload together.

use super::super::path_instantiation::aggregate_arguments::{AggregateOrigins, reference_leaves};
use super::super::stored_origins::{self, StoredLocalOrigins, StoredWriteOrigin};
use super::super::{FrameInference, Machine, TableCallExpression, TopLevelSymbols, TypedTrees};
use psi_facts::PlaceSegment;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::StatementNode;

pub(super) fn validate_frozen_inputs(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
) -> Option<()> {
    let inputs = program
        .state_parameters(state)
        .iter()
        .filter_map(|parameter| {
            if parameter.is_self
                || super::super::type_reference_is_reference(program, parameter.type_reference)
            {
                return None;
            }
            stored_origins::declared_origins(
                program,
                parameter.symbol,
                parameter.name.as_str(),
                parameter.type_reference,
            )
            .filter(|origins| !origins.cases.is_empty())
        })
        .collect::<Vec<_>>();
    if inputs.is_empty() {
        return Some(());
    }
    for statement in program.statement_table.statements(state.statement_nodes) {
        if stored_origins::statement_exposes_frozen_binding(
            program, machine, state, statement, &inputs,
        ) || matches!(statement, StatementNode::Assignment(assignment)
                if stored_origins::assignment_replaces_case_binding(program, assignment, &inputs))
        {
            return None;
        }
    }
    Some(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn instantiate_moves(
    program: &TypedTrees,
    caller_machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    relative: &mut AggregateOrigins,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<AggregateOrigins> {
    let parameters = program.state_parameters(state);
    let arguments = program.expression_table.expression_handles(call.arguments);
    let mut returned = AggregateOrigins::default();
    for moved in &relative.moves {
        let (index, parameter) = parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == moved.source.root)?;
        if super::super::type_reference_is_reference(program, parameter.type_reference)
            || !crate::type_references::type_references_match(
                program,
                stored_origins::projected_type(
                    program,
                    parameter.type_reference,
                    &moved.source.segments,
                )?,
                moved.type_reference,
            )
        {
            return None;
        }
        let actual = reference_leaves(
            program,
            caller_machine,
            *arguments.get(index)?,
            parameter.type_reference,
            "",
            symbols,
            inference,
        )?;
        let actual = StoredLocalOrigins {
            local_symbol: parameter.symbol,
            references: actual
                .references
                .into_iter()
                .map(|leaf| StoredWriteOrigin {
                    local_symbol: parameter.symbol,
                    local_path: leaf.local_suffix,
                    local_segments: leaf.local_segments,
                    origin: leaf.origin,
                })
                .collect(),
            cases: actual.cases,
            moves: actual.moves,
        };
        // The relation retains possible-element selection, not a callee-local
        // executable index identity in the caller's source namespace.
        let selectors = moved
            .source
            .segments
            .iter()
            .map(|segment| match segment {
                PlaceSegment::Index { .. } => PlaceSegment::Index {
                    expression: Default::default(),
                },
                _ => *segment,
            })
            .collect::<Vec<_>>();
        let projected =
            stored_origins::project_stored_origins(program, &actual, &selectors, false)?;
        for mut leaf in projected.references {
            let mut segments = moved.local_segments.clone();
            segments.extend(leaf.local_segments);
            leaf.local_segments = segments;
            leaf.local_suffix = local_suffix(program, &leaf.local_segments);
            returned.references.push(leaf);
        }
        for case in projected.cases {
            let mut segments = moved.local_segments.clone();
            segments.extend(case);
            returned.cases.push(segments);
        }
        for mut child in projected.moves {
            let mut segments = moved.local_segments.clone();
            segments.extend(child.local_segments);
            child.local_segments = segments;
            returned.moves.push(child);
        }
    }
    // A move owns its whole destination subtree. Leaving declared alternatives
    // alongside the instantiated value would resurrect absent-case references.
    relative.references.retain(|leaf| {
        !relative
            .moves
            .iter()
            .any(|moved| leaf.local_segments.starts_with(&moved.local_segments))
    });
    relative.cases.retain(|case| {
        !relative
            .moves
            .iter()
            .any(|moved| case.starts_with(&moved.local_segments))
    });
    Some(returned)
}

fn local_suffix(program: &TypedTrees, segments: &[PlaceSegment]) -> String {
    let mut suffix = String::new();
    for segment in segments {
        match segment {
            PlaceSegment::Field { symbol } => {
                suffix.push('.');
                suffix.push_str(program.symbols.name(*symbol));
            }
            PlaceSegment::Case { .. } => {}
            _ => break,
        }
    }
    suffix
}
